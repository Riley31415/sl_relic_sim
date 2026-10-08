//! Tables, charts, plan descriptions and COST.md, built from the model.

use std::fmt::Write as _;

use super::{
    CRIT, DEFAULT_PROFILE, CRIT_TIERS, Game, MAX_LEVEL, Plan, Profile, Quest, RELIC_SHORT, RELICS,
    RelicSet, RunOptions, amplification, damage, lookahead_plans, members, pity_gain, pity_needed, strategy,
};
use crate::economy::{
    CONVERT_FROM, CONVERT_TO, DIAMONDS_PER_SUMMON, RELIC_TYPES, RELICS_PER_ATTEMPT, RELICS_PER_SUMMON,
};
use crate::format::{amp_pct, commas, human, pct, signed_pct};
use crate::solver::{Config, Solver, Strategy, TIERS};

pub const HABITS: &str = "attempt as soon as any relic the quest needs is affordable; use the affordable needed relic that closes the most of the gap per attempt (on a total level; the one you hold the most of otherwise, and of equals); play every attempt to close the gap; keep a result only if it helps";

/// A bar as `glory+ / despair-`, e.g. 4+ / 2-.
pub fn bar_text(glory: u8, despair: Option<u8>) -> String {
    match despair {
        Some(d) => format!("{glory}+ / {d}-"),
        None => format!("{glory}+ / any"),
    }
}

/// A level's demand; `short` uses the two-word relic names.
pub fn requirement_text(game: &Game, level: usize, short: bool) -> String {
    let names = if short { &RELIC_SHORT } else { &RELICS };
    match game.quests.get(level).and_then(Option::as_ref) {
        None => "-".to_string(),
        Some(Quest::Relics { which, bar }) => {
            let listed: Vec<&str> = which.iter().map(|&i| names[i]).collect();
            format!("{}: {}", listed.join(", "), bar_text(bar.glory, bar.despair))
        }
        Some(Quest::Each(bar)) => format!("every relic: {}", bar_text(bar.glory, bar.despair)),
        Some(Quest::Total { glory, despair }) => {
            format!("all twelve together: {glory}+ / {despair}- (totals)")
        }
    }
}

/// What a tier asks for: the crit relic at `mark`% or better.
pub fn tier_text(mark: u8, short: bool) -> String {
    if short {
        format!("{} {mark}%+", RELIC_SHORT[CRIT])
    } else {
        format!("{} at {mark}%+ amplification, every other relic converted into it", RELICS[CRIT])
    }
}

/// One level (or crit relic tier) of a plan's summary.
#[derive(Clone, Debug)]
pub struct Row {
    pub level: usize,
    /// past the top level: the crit relic amplification mark this row reached
    pub tier: Option<u8>,
    pub requirement: String,
    pub short: String,
    pub mean: f64,
    pub median: f64,
    pub p90: f64,
    pub attempts: f64,
    /// share of runs that reached this level by pity
    pub pity: f64,
    pub atk_amp: f64,
    pub crit_amp: f64,
    /// mean damage multiplier, averaged per run (the two amps move together)
    pub damage: f64,
}

impl Row {
    /// "20", or "20 + 42%" for a tier.
    pub fn label(&self) -> String {
        match self.tier {
            Some(mark) => format!("{} + {mark}%", self.level),
            None => self.level.to_string(),
        }
    }

    /// The chart's x-axis label: "L20", or "42%" for a tier.
    fn tick(&self) -> String {
        match self.tier {
            Some(mark) => format!("{mark}%"),
            None => format!("L{}", self.level),
        }
    }

    /// The chart's tooltip name.
    fn name(&self) -> String {
        match self.tier {
            Some(mark) => format!("level {}, crit relic {mark}%", self.level),
            None => format!("level {}", self.level),
        }
    }
}

/// One summary row per level, then per tier: cost, pity share, the two
/// relics, damage.
pub fn level_rows(
    game: &Game,
    plan: &Plan,
    runs: u64,
    seed: u64,
    options: &RunOptions,
) -> Result<Vec<Row>, String> {
    let results = game.simulate(plan, runs, seed, options)?;
    let n = runs as f64;
    let steps = options.max_level + options.tiers.len();
    let mut rows = Vec::with_capacity(steps);
    for index in 0..steps {
        let tier = index.checked_sub(options.max_level).map(|k| options.tiers[k]);
        let level = if tier.is_some() { options.max_level } else { index + 1 };
        let at: Vec<_> = results.iter().map(|run| run.levels[index]).collect();
        let mut costs: Vec<f64> = at.iter().map(|l| l.diamonds as f64).collect();
        costs.sort_by(f64::total_cmp);
        let atk: Vec<f64> = at.iter().map(|l| amplification(l.atk)).collect();
        let crit: Vec<f64> = at.iter().map(|l| amplification(l.crit)).collect();
        let len = costs.len();
        rows.push(Row {
            level,
            tier,
            requirement: tier.map_or_else(|| requirement_text(game, level, false), |m| tier_text(m, false)),
            short: tier.map_or_else(|| requirement_text(game, level, true), |m| tier_text(m, true)),
            mean: costs.iter().sum::<f64>() / n,
            median: costs[len / 2],
            p90: costs[(len - 1).min((0.9 * n) as usize)],
            attempts: at.iter().map(|l| l.attempts as f64).sum::<f64>() / n,
            pity: at.iter().filter(|l| l.by_pity).count() as f64 / n,
            atk_amp: atk.iter().sum::<f64>() / n,
            crit_amp: crit.iter().sum::<f64>() / n,
            damage: atk.iter().zip(&crit).map(|(&a, &c)| damage(level, c, a)).sum::<f64>() / n,
        });
    }
    Ok(rows)
}

// ------------------------------------------------------------------ words

fn names(relics: RelicSet) -> String {
    members(relics).map(|i| RELICS[i]).collect::<Vec<_>>().join(", ")
}

/// What `plan` does to reach `level`, in words - built from the plan, so it
/// cannot drift from what is simulated.
pub fn describe_step(game: &Game, plan: &Plan, level: usize) -> String {
    if game.quests.get(level).and_then(Option::as_ref).is_none() {
        return "free - every run starts here".to_string();
    }
    describe_quest(game, plan, level) + &describe_filler(game, plan, level)
}

fn describe_quest(game: &Game, plan: &Plan, level: usize) -> String {
    let step = plan.step(level);
    let locked = plan.protected(game, level);
    let (pg, pd) = match step.profile {
        Some(Profile::Bar(g, d)) => (g, d),
        _ => DEFAULT_PROFILE,
    };
    match game.quests[level].as_ref().expect("checked above") {
        Quest::Relics { which, bar } => {
            let then = match (step.aim_at, step.ahead) {
                (Some(l), _) => format!("level {l}'s totals"),
                (None, true) => "the totals ahead".to_string(),
                (None, false) => "the goal's totals".to_string(),
            };
            let mut text =
                format!("roll {} for the best chance of {}, then for {then}", names(super::set_of(which)), bar_text(bar.glory, bar.despair));
            // the levels whose work (or filler) left these relics' stock untouched
            let banked: Vec<String> = (2..level)
                .filter(|&lv| {
                    super::set_of(which) & plan.protected(game, lv) != 0
                        && (game.quests[lv].as_ref().is_some_and(Quest::is_total)
                            || plan.step(lv).filler)
                })
                .map(|lv| lv.to_string())
                .collect();
            if !banked.is_empty() {
                let _ = write!(
                    text,
                    ", paid for partly out of their stock banked during level{} {}",
                    if banked.len() > 1 { "s" } else { "" },
                    banked.join(", ")
                );
            }
            text
        }
        Quest::Each(bar) => {
            let despair = bar.despair.map_or(pd, |d| d.min(pd));
            format!(
                "roll every relic that is short to {}, each for the best chance of its bar, then for {}",
                bar_text(bar.glory.max(pg), Some(despair)),
                match (step.aim_at, step.ahead) {
                    (Some(l), _) => format!("level {l}'s totals"),
                    (None, true) => "the totals ahead".to_string(),
                    (None, false) => "the goal's totals".to_string(),
                }
            )
        }
        Quest::Total { .. } => {
            // which relic is a stock question, not a quality one: preferring the
            // next level's relics (or ones already worked on) never paid
            let mut pool = "whichever relic closes the most of the gap per attempt".to_string();
            if locked != 0 {
                let _ = write!(pool, ", except {}", names(locked));
            }
            let mut text = if step.profile == Some(Profile::Repair) {
                format!(
                    "repair, do not rebuild: using {pool}, shave one despair at a time off relics with 2 or more (the worst first) until the total fits, then add one glory at a time"
                )
            } else {
                format!(
                    "roll {pool} to {} until the totals clear (every relic there{} and still short: the bars go a step further)",
                    bar_text(pg, Some(pd)),
                    if step.trades { " - or traded off its bar for a result the keep rule ranks as high -" } else { "" }
                )
            };
            match (step.aim_at, step.ahead) {
                (Some(l), _) => {
                    let _ = write!(text, "; every attempt closes the gap to level {l}'s totals, not just this level's (9/0 beats a 9/1 that clears it)");
                }
                (None, true) => text.push_str(
                    "; every attempt closes the gap to the totals ahead - the most glory and the least despair any level up to the goal asks - not just this level's (9/0 beats a 9/1 that clears it)",
                ),
                (None, false) => {
                    text.push_str("; every attempt closes this level's gap, and keeps going past it (9/0 beats a 9/1 that clears it)")
                }
            }
            text.push_str(", then goes for more glory and less despair, weighed as those totals still need them; a roll is kept if it brings the board nearer them, or as near with more glory less despair");
            if locked != 0 {
                let next = game.next_named(level).map_or(0, |(lvl, _)| lvl);
                let _ = write!(text, " (their stock is banked for level {next})");
            }
            text
        }
    }
}

/// The pity side of a level: what spare stock is spent on, and the meter.
fn describe_filler(game: &Game, plan: &Plan, level: usize) -> String {
    let (needed, gain) = (pity_needed(level), pity_gain(level - 1));
    let pity = format!(
        "{} pity ({} attempts at +{gain}) levels up without the quest",
        commas(f64::from(needed), 0),
        needed.div_ceil(gain)
    );
    if !plan.step(level).filler {
        return format!(". When nothing the quest needs is affordable, summon. {pity}");
    }
    let mut spare = "the best-stocked spare relic".to_string();
    let locked = plan.protected(game, level);
    if locked != 0 {
        let _ = write!(spare, " (never {})", names(locked));
    }
    let when = match plan.step(level).filler_within {
        Some(n) => format!(
            ", once the spare relics on hand can fill the pity bar in {n} attempts or fewer (summoning until then)"
        ),
        None => String::new(),
    };
    format!(
        ". When nothing the quest needs is affordable, attempt {spare} instead of summoning{when}, played and kept as the quest's own rolls are (the pity is earned whatever it rolls). {pity}"
    )
}

// ------------------------------------------------------------------ console

pub fn print_table(rows: &[Row], name: &str, runs: u64) -> String {
    let mut out = String::new();
    let _ =
        writeln!(out, "Diamond cost to raise the inheritor - plan '{name}', {} runs", commas(runs as f64, 0));
    let _ = writeln!(
        out,
        "  {} diamonds = {RELICS_PER_SUMMON} relics over {RELIC_TYPES} types, {RELICS_PER_ATTEMPT} of a type per attempt",
        commas(DIAMONDS_PER_SUMMON as f64, 0)
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  {:>8} | {:>14} {:>9} {:>9} {:>8} {:>7} | requirement (glory/despair)",
        "lvl", "mean diamonds", "median", "90th", "by pity", "dmg"
    );
    let _ = writeln!(out, "  {}", "-".repeat(100));
    for r in rows {
        let _ = writeln!(
            out,
            "  {:>8} | {:>14} {:>9} {:>9} {:>8} {:>7} | {}",
            r.label(),
            commas(r.mean, 0),
            human(r.median),
            human(r.p90),
            pct(r.pity, 0),
            signed(r.damage - 1.0, 1),
            r.short
        );
    }
    out
}

/// A change as a signed percent: +7.6%.
fn signed(x: f64, places: usize) -> String {
    let body = pct(x.abs(), places);
    if x < 0.0 && body.chars().any(|c| c.is_ascii_digit() && c != '0') {
        format!("-{body}")
    } else {
        format!("+{body}")
    }
}

pub fn compare(game: &Game, runs: u64, seed: u64, max_level: usize) -> Result<String, String> {
    let mut out = String::new();
    let _ = writeln!(out, "Plan comparison to level {max_level}, {} runs each", commas(runs as f64, 0));
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  {:>10} | {:>14} {:>9} {:>9} {:>7}",
        "plan", "mean diamonds", "median", "90th", "dmg"
    );
    let _ = writeln!(out, "  {}", "-".repeat(58));
    let mut best: Option<(&str, f64)> = None;
    for (name, plan) in super::strategies() {
        let rows = level_rows(game, &plan, runs, seed, &RunOptions::to(max_level))?;
        let top = rows.last().expect("at least level 1");
        let _ = writeln!(
            out,
            "  {name:>10} | {:>14} {:>9} {:>9} {:>7}",
            commas(top.mean, 0),
            human(top.median),
            human(top.p90),
            signed(top.damage - 1.0, 1)
        );
        if best.is_none_or(|(_, cost)| top.mean < cost) {
            best = Some((name, top.mean));
        }
    }
    let (name, cost) = best.expect("at least one plan");
    let _ = writeln!(out);
    let _ = writeln!(out, "  cheapest to level {max_level}: '{name}' at {} diamonds", commas(cost, 0));
    Ok(out)
}

// ------------------------------------------------------------------ charts

/// A dotted line across the efficiency chart: damage bought some other way,
/// to measure the level-ups against.
pub struct Reference {
    pub label: &'static str,
    pub multiplier: f64,
    pub diamonds: f64,
    /// the colour class its line (`lv-{colour}`) and label (`lv-{colour}t`) wear
    pub colour: &'static str,
    /// whether its label starts from the line's left end rather than its right
    pub label_left: bool,
}

/// The references drawn across the efficiency chart.
pub const EFFICIENCY_REFERENCES: [Reference; 2] = [
    Reference { label: "19* Orr: 16% dmg for 156k dia", multiplier: 1.1664, diamonds: 156_700.0, colour: "gold", label_left: false },
    Reference { label: "25* Orr: 16% dmg for 251k dia", multiplier: 1.1598, diamonds: 250_509.8659, colour: "cyan", label_left: true },
];

/// The diamonds between labels on the cost chart's x axis.
pub const COST_TICK: f64 = 250_000.0;

/// How far above the reference the efficiency chart goes; anything higher
/// (the odd level-up that comes almost free) is pinned to the top edge with
/// its value.
pub const EFFICIENCY_CAP: f64 = 2.0;

/// The efficiency chart starts here; the level-ups before it are so cheap
/// they dwarf everything after them.
pub const EFFICIENCY_FROM: usize = 5;

/// Damage bought per diamond: ln(dmg multiplier) per million diamonds.
pub fn efficiency(multiplier: f64, diamonds: f64) -> f64 {
    multiplier.ln() / diamonds * 1e6
}

/// Damage-axis labels every `step` percent, up to the first one at or above
/// `top_pct`.
pub fn dmg_ticks(top_pct: f64, step: u32) -> Vec<u32> {
    let last = step * ((top_pct / f64::from(step) - 1e-9).ceil().max(1.0) as u32);
    (0..=last).step_by(step as usize).collect()
}

/// A round step that splits `span` into about `parts`: 1, 2, 2.5 or 5 x 10^n.
fn nice_step(span: f64, parts: f64) -> f64 {
    let raw = span / parts;
    let unit = 10f64.powi(raw.log10().floor() as i32);
    [1.0, 2.0, 2.5, 5.0, 10.0].into_iter().map(|k| k * unit).find(|&s| s >= raw).unwrap_or(10.0 * unit)
}

/// A value on a linear axis, with as many decimals as its step needs.
fn tick_text(v: f64, step: f64) -> String {
    let places = (-step.log10().floor()).max(0.0) as usize;
    format!("{v:.places$}")
}

/// The colours and type every levels chart shares, light and dark.
const CHART_STYLE: &str = concat!(
    "<style>",
    ".lv-bar{fill:#2a78d6;stroke:#ffffff;stroke-width:0.5}.lv-grid{stroke:#e1e0d9;stroke-width:1}",
    ".lv-axis{stroke:#c3c2b7;stroke-width:1;fill:none}.lv-ink{fill:#52514e}",
    ".lv-muted{fill:#898781}",
    ".lv-eff{stroke:#7b3fb8;stroke-width:2;fill:none}.lv-effdot{fill:#7b3fb8}.lv-efft{fill:#7b3fb8}",
    ".lv-ref{stroke-width:2;stroke-dasharray:5 4}",
    ".lv-gold{stroke:#e0b000}.lv-goldt{fill:#9a7700}.lv-cyan{stroke:#0e9fb3}.lv-cyant{fill:#087686}",
    ".lv-halo{paint-order:stroke;stroke:#ffffff;stroke-width:3px;stroke-linejoin:round}",
    ".lv-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}",
    ".lv-s{font:10px system-ui,-apple-system,'Segoe UI',sans-serif}",
    ".lv-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}",
    ".lv-h{font:600 13px system-ui,-apple-system,'Segoe UI',sans-serif}",
    "@media(prefers-color-scheme:dark){",
    ".lv-bar{fill:#3987e5;stroke:#1f1f1e}.lv-grid{stroke:#2c2c2a}.lv-axis{stroke:#383835}",
    ".lv-ink{fill:#c3c2b7}.lv-eff{stroke:#b287e8}.lv-effdot{fill:#b287e8}.lv-efft{fill:#b287e8}",
    ".lv-gold{stroke:#f2c94c}.lv-goldt{fill:#f2c94c}.lv-cyan{stroke:#3fc6d8}.lv-cyant{fill:#3fc6d8}",
    ".lv-halo{stroke:#1f1f1e}}",
    "</style>"
);

/// How far below the plot the x-axis title sits.
const AXIS_TITLE_DROP: f64 = 36.0;

/// Opens a chart with its title written across the top.
fn svg_open(width: usize, height: usize, title: &str, desc: &str) -> Vec<String> {
    vec![
        format!(
            "<svg viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" role=\"img\" xmlns=\"http://www.w3.org/2000/svg\" aria-label=\"{title}\">"
        ),
        format!("<title>{title}</title>"),
        format!("<desc>{desc}</desc>"),
        CHART_STYLE.to_string(),
        format!("<text class=\"lv-h lv-ink\" x=\"8\" y=\"18\">{title}</text>"),
    ]
}

/// The x axis of the cost chart: linear in diamonds, broken before the last
/// bar when that one is far out on its own, so the rest are not squashed.
struct CostAxis {
    /// the unbroken stretch runs 0..a_max over a_width pixels
    a_max: f64,
    a_width: f64,
    /// (the lone far bar's diamonds, where the break's gap starts and ends)
    far: Option<(f64, f64, f64)>,
    left: f64,
}

impl CostAxis {
    const GAP: f64 = 18.0;
    const FAR_WIDTH: f64 = 56.0;

    fn new(costs: &[f64], left: f64, width: f64) -> (Self, f64) {
        let mut sorted = costs.to_vec();
        sorted.sort_by(f64::total_cmp);
        let top = sorted[sorted.len() - 1];
        let second = if sorted.len() > 1 { sorted[sorted.len() - 2] } else { 0.0 };
        let broken = second > 0.0 && top > 1.5 * second;
        let span = if broken { second } else { top };
        let step = COST_TICK;
        // broken, the axis stops just past the last bar before the break;
        // whole, it runs on to the next tick
        let a_max = if broken { span * 1.02 } else { step * (span * 1.04 / step).ceil() };
        if broken {
            let a_width = width - Self::GAP - Self::FAR_WIDTH;
            let gap_start = left + a_width;
            let axis = CostAxis { a_max, a_width, far: Some((top, gap_start, gap_start + Self::GAP)), left };
            (axis, step)
        } else {
            (CostAxis { a_max, a_width: width, far: None, left }, step)
        }
    }

    fn x(&self, v: f64) -> f64 {
        match self.far {
            Some((top, _, gap_end)) if v > self.a_max => {
                gap_end + Self::FAR_WIDTH / 2.0 + (v - top) / self.a_max * self.a_width
            }
            _ => self.left + v / self.a_max * self.a_width,
        }
    }

    /// The pixel spans the axis is drawn over, either side of the break.
    fn spans(&self) -> Vec<(f64, f64)> {
        match self.far {
            Some((_, gap_start, gap_end)) => {
                vec![(self.left, gap_start), (gap_end, gap_end + Self::FAR_WIDTH)]
            }
            None => vec![(self.left, self.left + self.a_width)],
        }
    }
}

/// Chart 1: every level (and tier) as a bar standing at its average total
/// diamonds, as tall as its total damage increase on a log axis.
pub fn cost_chart_svg(rows: &[Row], title: &str) -> String {
    let data = &rows[1..];
    let (ml, mr, mt, mb) = (58.0, 20.0, 56.0, 44.0);
    let (plot_w, plot_h) = (620.0, 250.0);
    let (width, height) = (ml + plot_w + mr, mt + plot_h + mb);
    let base_y = mt + plot_h;
    let costs: Vec<f64> = data.iter().map(|r| r.mean).collect();
    let (axis, step) = CostAxis::new(&costs, ml, plot_w);
    let top_dmg = data.iter().map(|r| r.damage).fold(1.0, f64::max);
    let ticks = dmg_ticks((top_dmg - 1.0) * 100.0, 20);
    let y_top = (1.0 + f64::from(*ticks.last().expect("at least +0%")) / 100.0).ln();
    let y_of = |m: f64| mt + plot_h * (1.0 - m.ln() / y_top);
    let last = &data[data.len() - 1];

    let desc = format!(
        "Each level as a bar at its average total diamonds, as tall as its damage increase on a log axis: {} at {} for +{}.",
        last.name(),
        human(last.mean),
        pct(last.damage - 1.0, 0)
    );
    let mut out = svg_open(width as usize, height as usize, title, &desc);

    // damage: gridlines (split at the break) and labels, thinned where the
    // log scale crowds them
    let mut last_label = f64::MAX;
    for &p in &ticks {
        let y = y_of(1.0 + f64::from(p) / 100.0);
        if last_label - y >= 12.0 {
            for (x1, x2) in axis.spans() {
                out.push(format!(
                    "<line class=\"lv-grid\" x1=\"{x1:.1}\" y1=\"{y:.1}\" x2=\"{x2:.1}\" y2=\"{y:.1}\"/>"
                ));
            }
            out.push(format!(
                "<text class=\"lv-t lv-muted\" x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">+{p}%</text>",
                ml - 8.0,
                y + 4.0
            ));
            last_label = y;
        }
    }
    out.push(format!(
        "<text class=\"lv-t lv-muted\" x=\"{}\" y=\"{}\" text-anchor=\"end\">dmg</text>",
        ml - 8.0,
        mt - 12.0
    ));

    // diamonds: ticks along the unbroken stretch, the far bar labelled alone
    let mut v = 0.0;
    while v <= axis.a_max + 1e-6 {
        out.push(format!(
            "<text class=\"lv-t lv-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            axis.x(v),
            base_y + 15.0,
            human(v)
        ));
        v += step;
    }
    for (x1, x2) in axis.spans() {
        out.push(format!(
            "<line class=\"lv-axis\" x1=\"{x1:.1}\" y1=\"{base_y}\" x2=\"{x2:.1}\" y2=\"{base_y}\"/>"
        ));
    }
    if let Some((top, gap_start, gap_end)) = axis.far {
        // the zig-zag across the gap marks the axis as broken
        let zig: Vec<String> = (0..=6)
            .map(|k| {
                let x = gap_start + (gap_end - gap_start) * f64::from(k) / 6.0;
                let y = base_y
                    + if k % 2 == 1 {
                        -4.0
                    } else if k == 0 || k == 6 {
                        0.0
                    } else {
                        4.0
                    };
                format!("{x:.1},{y:.1}")
            })
            .collect();
        out.push(format!("<polyline class=\"lv-axis\" points=\"{}\"/>", zig.join(" ")));
        out.push(format!(
            "<text class=\"lv-t lv-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            axis.x(top),
            base_y + 15.0,
            human(top)
        ));
    }
    out.push(format!(
        "<text class=\"lv-t lv-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">total diamonds spent{}</text>",
        ml + plot_w / 2.0,
        base_y + AXIS_TITLE_DROP,
        if axis.far.is_some() { " (axis broken before the last bar)" } else { "" }
    ));

    // the bars, then their names wherever they do not collide
    const BAR_W: f64 = 6.0;
    for r in data {
        let (x, y) = (axis.x(r.mean), y_of(r.damage));
        out.push(format!(
            "<rect class=\"lv-bar\" x=\"{:.1}\" y=\"{y:.1}\" width=\"{BAR_W}\" height=\"{:.1}\"><title>{}: {} diamonds, +{} dmg</title></rect>",
            x - BAR_W / 2.0,
            base_y - y,
            r.name(),
            commas(r.mean, 0),
            pct(r.damage - 1.0, 1)
        ));
    }
    // every bar's name sits just above it, overlapping or not
    for r in data {
        out.push(format!(
            "<text class=\"lv-s lv-ink lv-halo\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            axis.x(r.mean),
            y_of(r.damage) - 5.0,
            r.tick()
        ));
    }
    out.push("</svg>".to_string());
    out.join("\n")
}

/// What row `i`'s marginal columns are measured from: the row before it (a
/// tier's, the tier before it, or level 20 for the first).
fn before(rows: &[Row], i: usize) -> Option<&Row> {
    i.checked_sub(1).map(|p| &rows[p])
}

/// Chart 2: each level-up's (or tier's) efficiency - ln of the damage it adds
/// per million diamonds it costs - against the reference.
pub fn efficiency_chart_svg(rows: &[Row], title: &str) -> String {
    // (the row, what it is measured from, its efficiency)
    let points: Vec<(&Row, &Row, f64)> = (1..rows.len())
        .filter(|&i| rows[i].level >= EFFICIENCY_FROM)
        .filter_map(|i| {
            let p = before(rows, i)?;
            let cost = rows[i].mean - p.mean;
            (cost > 0.0).then(|| (&rows[i], p, efficiency(rows[i].damage / p.damage, cost)))
        })
        .collect();
    let references: Vec<(&Reference, f64)> =
        EFFICIENCY_REFERENCES.iter().map(|r| (r, efficiency(r.multiplier, r.diamonds))).collect();
    let reference = references.iter().map(|r| r.1).fold(0.0, f64::max);

    let (ml, mr, mt, mb) = (58.0, 20.0, 56.0, 44.0);
    let pitch = 34.0;
    let (plot_w, plot_h) = (pitch * points.len() as f64, 230.0);
    let (width, height) = (ml + plot_w + mr, mt + plot_h + mb);
    let base_y = mt + plot_h;
    // a level-up paid for out of banked stock can be a hundred times the rest;
    // past EFFICIENCY_CAP x the highest reference a point is pinned to the top
    let top = points.iter().map(|p| p.2).fold(reference, f64::max).min(EFFICIENCY_CAP * reference) * 1.08;
    let step = nice_step(top, 4.0);
    let y_top = step * (top / step).ceil();
    let x_mid = |i: usize| ml + i as f64 * pitch + pitch / 2.0;
    let y_of = |e: f64| mt + plot_h * (1.0 - e.min(y_top) / y_top);

    let best = points.iter().max_by(|a, b| a.2.total_cmp(&b.2)).expect("at least one level-up");
    let refs_text: Vec<String> = references.iter().map(|(r, e)| format!("{} at {e:.2}", r.label)).collect();
    let desc = format!(
        "Efficiency of each level-up: ln of the damage it adds per million diamonds. Best: {} at {:.2}; the reference lines: {}.",
        best.0.name(),
        best.2,
        refs_text.join(", ")
    );
    let mut out = svg_open(width as usize, height as usize, title, &desc);

    let mut v = 0.0;
    while v <= y_top + 1e-9 {
        let y = y_of(v);
        out.push(format!(
            "<line class=\"lv-grid\" x1=\"{ml}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\"/>",
            ml + plot_w
        ));
        out.push(format!(
            "<text class=\"lv-t lv-muted\" x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            ml - 8.0,
            y + 4.0,
            tick_text(v, step)
        ));
        v += step;
    }
    out.push(format!(
        "<text class=\"lv-t lv-muted\" x=\"{}\" y=\"{}\">ln(marginal dmg multiplier) per 1M marginal diamonds</text>",
        ml - 50.0,
        mt - 22.0
    ));
    out.push(format!(
        "<line class=\"lv-axis\" x1=\"{ml}\" y1=\"{base_y}\" x2=\"{:.1}\" y2=\"{base_y}\"/>",
        ml + plot_w
    ));

    // the references, dotted, each labelled above or below its line wherever
    // the label covers the least of the data; the labels go on top of the
    // line, so they stay legible where nowhere is clear
    let dots: Vec<(f64, f64)> = points.iter().enumerate().map(|(i, p)| (x_mid(i), y_of(p.2))).collect();
    let mut taken: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut labels = Vec::new();
    for (r, e) in &references {
        let ry = y_of(*e);
        out.push(format!(
            "<line class=\"lv-ref lv-{}\" x1=\"{ml}\" y1=\"{ry:.1}\" x2=\"{:.1}\" y2=\"{ry:.1}\"/>",
            r.colour,
            ml + plot_w
        ));
        let text_w = label_width(r.label);
        let (x0, y) = place_label(
            &dots,
            &taken,
            text_w,
            &[ry - 6.0, ry + 15.0],
            (ml + 6.0, ml + plot_w - 6.0),
            (mt, base_y),
            r.label_left,
        );
        taken.push((x0, y - 11.0, x0 + text_w, y + 2.0));
        let (x, anchor) = if r.label_left { (x0, "start") } else { (x0 + text_w, "end") };
        labels.push(format!(
            "<text class=\"lv-b lv-{}t lv-halo\" x=\"{x:.1}\" y=\"{y:.1}\" text-anchor=\"{anchor}\">{}</text>",
            r.colour, r.label
        ));
    }

    let line: Vec<String> = dots.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect();
    out.push(format!("<polyline class=\"lv-eff\" points=\"{}\"/>", line.join(" ")));
    for (i, (r, p, e)) in points.iter().enumerate() {
        out.push(format!(
            "<circle class=\"lv-effdot\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.5\"><title>{}: +{} dmg for {} diamonds = {e:.3}</title></circle>",
            x_mid(i),
            y_of(*e),
            r.name(),
            pct(r.damage / p.damage - 1.0, 1),
            human(r.mean - p.mean)
        ));
        out.push(format!(
            "<text class=\"lv-t lv-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            x_mid(i),
            base_y + 15.0,
            r.tick()
        ));
        if *e > y_top {
            out.push(format!(
                "<text class=\"lv-s lv-ink lv-halo\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">↑{}</text>",
                x_mid(i),
                mt - 5.0,
                if *e < 10.0 { format!("{e:.1}") } else { format!("{e:.0}") }
            ));
        }
    }
    out.extend(labels);
    // the line's own name, along the bottom, as far right as it can go
    // while covering the least of the line
    let text_w = label_width(SERIES_LABEL);
    let (x0, y) =
        place_label(&dots, &taken, text_w, &[base_y - 8.0], (ml + 6.0, ml + plot_w - 6.0), (mt, base_y), false);
    let x = x0 + text_w;
    out.push(format!(
        "<text class=\"lv-b lv-efft lv-halo\" x=\"{x:.1}\" y=\"{y:.1}\" text-anchor=\"end\">{SERIES_LABEL}</text>"
    ));
    let x_title = match rows.iter().find(|r| r.tier.is_some()) {
        Some(r) => format!(
            "inheritor level reached, then crit relic amplification at level {} (all relics converted to crit)",
            r.level
        ),
        None => "inheritor level reached".to_string(),
    };
    out.push(format!(
        "<text class=\"lv-t lv-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">{x_title}</text>",
        ml + plot_w / 2.0,
        base_y + AXIS_TITLE_DROP
    ));
    out.push("</svg>".to_string());
    out.join("\n")
}

/// What the efficiency chart's line is.
const SERIES_LABEL: &str = "Relic Inheritance by level";

/// About how wide a bold 11px label is.
fn label_width(text: &str) -> f64 {
    6.3 * text.chars().count() as f64
}

/// Where a label `w` wide goes, as its left edge and baseline: on one of the
/// baselines `ys`, slid in from the `left` end (or the `right`, unless
/// `from_left`), wherever it covers the least of the line through `dots`
/// (ties go to the spot nearest that end, then the first baseline), inside
/// the plot (top, bottom) and off the labels already `taken`.
fn place_label(
    dots: &[(f64, f64)],
    taken: &[(f64, f64, f64, f64)],
    w: f64,
    ys: &[f64],
    (left, right): (f64, f64),
    (top, bottom): (f64, f64),
    from_left: bool,
) -> (f64, f64) {
    let start = if from_left { left } else { right - w };
    let mut best = ((start, ys[0]), usize::MAX);
    let ys: Vec<f64> = ys.iter().copied().filter(|&y| y - 10.0 >= top && y + 2.0 <= bottom).collect();
    let mut k = 0.0;
    while best.1 > 0 {
        let x0 = if from_left { left + 6.0 * k } else { right - w - 6.0 * k };
        if x0 < left || x0 + w > right {
            break;
        }
        for &y in &ys {
            let b = (x0 - 4.0, y - 13.0, x0 + w + 4.0, y + 6.0);
            if !taken.iter().any(|t| t.0 < b.2 && b.0 < t.2 && t.1 < b.3 && b.1 < t.3) {
                let hits = line_hits(dots, b);
                if hits < best.1 {
                    best = ((x0, y), hits);
                }
            }
        }
        k += 1.0;
    }
    best.0
}

/// How much of the polyline through `dots` falls in the box (x0, y0, x1, y1):
/// its dots, and points every couple of pixels along each segment.
fn line_hits(dots: &[(f64, f64)], (x0, y0, x1, y1): (f64, f64, f64, f64)) -> usize {
    let inside = |(x, y): (f64, f64)| x0 <= x && x <= x1 && y0 <= y && y <= y1;
    let along = dots.windows(2).map(|w| {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        let steps = ((bx - ax).hypot(by - ay) / 2.0).ceil().max(1.0) as usize;
        (0..=steps).filter(|&k| {
            let t = k as f64 / steps as f64;
            inside((ax + (bx - ax) * t, ay + (by - ay) * t))
        })
        .count()
    });
    dots.iter().filter(|&&d| inside(d)).count() + along.sum::<usize>()
}

// ------------------------------------------------------------------ COST.md

/// What one inheritance attempt is worth at each inheritor level, played
/// optimally for amplification: the board each level gives, and the exact
/// average amplification of a single attempt on it.
fn attempt_table() -> Vec<String> {
    let mut out = vec![
        String::new(),
        "## One attempt at each level".to_string(),
        String::new(),
        "The average amplification of a single inheritance attempt made at each inheritor level, played for the most amplification (+5% a glory success, -2% a despair success, floored at 0%). Exact, from the solver, not simulated. Level 1 is the starting board; a level-20 attempt uses the 10th slot, which no level-up ever gets to. The first rate is the chance the attempt opens on: the level-15 glory success moves it down a step, the level-19 despair failure back up.".to_string(),
        String::new(),
        SMALL_TABLE.to_string(),
        String::new(),
        "| level | memory slots | spirit power | glory rate | despair rate | starts with | first rate | average amplification | best possible | wipe chance |".to_string(),
        "|---|---|---|---|---|---|---|---|---|---|".to_string(),
    ];
    for level in 1..=MAX_LEVEL {
        let cfg = Config::for_level(level as i64, Strategy::default()).expect("level >= 1");
        let mut solver = Solver::new(cfg);
        let a = solver.analyse(None);
        let head_start = [(cfg.start_glory, "glory success"), (cfg.start_despair_fail, "despair fail")]
            .into_iter()
            .filter(|&(n, _)| n > 0)
            .map(|(n, what)| format!("{n} {what}"))
            .collect::<Vec<_>>();
        out.push(format!(
            "| **{level}** | {} | {} | {} | {} | {} | {} | **{:.2}%** | {} | {} |",
            cfg.slots,
            cfg.max_spirit,
            signed_pct(cfg.glory_mod),
            signed_pct(cfg.despair_mod),
            if head_start.is_empty() { "-".to_string() } else { head_start.join(", ") },
            pct(TIERS[usize::from(cfg.start_tier)], 0),
            a.e_amplification,
            amp_pct(solver.max_amplification(), 0),
            pct(a.p_dead_end, 2)
        ));
    }
    out.extend([String::new(), "</div>".to_string()]);
    out
}

/// One row per Look Ahead target, each from a simulation of that target's
/// own plan; level 1 comes first, as in every table.
pub fn target_rows(game: &Game, runs: u64, seed: u64) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for (target, plan) in lookahead_plans()? {
        let result = level_rows(game, &plan, runs, seed, &target.options())?;
        if rows.is_empty() {
            rows.push(result[0].clone());
        }
        let index = if target.tier.is_some() { target.level } else { target.level - 1 };
        rows.push(result[index].clone());
    }
    Ok(rows)
}

/// Wraps a markdown table so it renders in smaller type and fits the screen;
/// the blank line after it lets the table inside still parse as markdown.
const SMALL_TABLE: &str = "<div style=\"font-size:0.72em\">";

/// COST.md, generated end to end from the simulation.
pub fn markdown(runs: u64, seed: u64) -> Result<String, String> {
    let game = Game::standard();
    let mut out = vec![
        "# Raising the inheritor".to_string(),
        String::new(),
        "<!-- generated by `relic levels --markdown COST.md` - do not edit -->".to_string(),
        String::new(),
        format!(
            "Diamonds from inheritor level 1 to {MAX_LEVEL}, starting with no relics. {} diamonds buys {RELICS_PER_SUMMON} random relics of {RELIC_TYPES} types; an attempt uses {RELICS_PER_ATTEMPT} of one type. A level is reached by its quest or by a full pity meter, whichever comes first.",
            commas(DIAMONDS_PER_SUMMON as f64, 0)
        ),
        String::new(),
        "dmg increase = (1 + level multiplier) x (1 + 4/5 x crit amp) x (1 + 22/209 x atk amp) - 1, where the level multiplier is +5% a level for levels 2-7, +10% for 8-15, +15% for 16-19 and +20% for 20. Marginal columns are relative to the row before: marginal dmg increase is (1 + this row's dmg increase) / (1 + the row before's) - 1.".to_string(),
    ];
    let rows = target_rows(game, runs, seed)?;
    let title = "Look Ahead plans";
    let blurb = format!(
        "Every row is its own Look Ahead: a plan searched for the cheapest route to that row alone - that level, or level {MAX_LEVEL} plus that crit relic tier ({}) - and every attempt on the way closes the gap to its level's totals - or, where the plan says so (a), to the totals ahead: the most glory and the least despair any level up to the goal asks - then goes for more glory and less despair, weighed as those totals still need them (a relic held to a bar plays for the bar first, then the goal's totals or the totals ahead). Each row is simulated with its own plan and only its own result is shown, so neighbouring rows can come from different plans. A tier's plan is searched on the whole cost, climb and farm together: on reaching level {MAX_LEVEL} the relics left over are converted into the {} crit relic ({CONVERT_FROM} of one type for {CONVERT_TO}) and spent on it, then it is farmed, every attempt played for the best chance of the tier's mark.",
        tier_list(&CRIT_TIERS),
        RELICS[CRIT]
    );
    out.extend([
        String::new(),
        format!("## {title}"),
        String::new(),
        format!("{blurb} {} simulations.", commas(runs as f64, 0)),
        String::new(),
        SMALL_TABLE.to_string(),
        String::new(),
        "| level | requirement (glory/despair) | total diamonds | marginal diamonds | reached by pity | crit relic amp | atk relic amp | dmg increase | marginal dmg increase |".to_string(),
        "|---|---|---|---|---|---|---|---|---|".to_string(),
    ]);
    for (i, r) in rows.iter().enumerate() {
        let (step, pity, more) = match before(&rows, i) {
            None => ("-".to_string(), "-".to_string(), "-".to_string()),
            Some(p) => (
                commas(r.mean - p.mean, 0),
                if r.tier.is_some() { "-".to_string() } else { pct(r.pity, 0) },
                format!("+{}", pct(r.damage / p.damage - 1.0, 1)),
            ),
        };
        out.push(format!(
            "| **{}** | {} | {} | {step} | {pity} | {:.2}% | {:.2}% | +{} | {more} |",
            r.label(),
            r.short,
            commas(r.mean, 0),
            r.crit_amp,
            r.atk_amp,
            pct(r.damage - 1.0, 1)
        ));
    }
    out.extend([String::new(), "</div>".to_string()]);
    // side by side where there is room, stacked where there is not
    out.extend([
        String::new(),
        "<div style=\"display:flex;flex-wrap:wrap;gap:12px\">".to_string(),
        cost_chart_svg(&rows, "Damage vs Cost"),
        efficiency_chart_svg(&rows, "Level Efficiency"),
        "</div>".to_string(),
        String::new(),
        format!(
            "<sub>Left: each bar stands at a level's total diamonds and is as tall as its damage increase (log scale). Right: efficiency, ln(1 + marginal dmg increase) per million marginal diamonds; the dotted lines are {}.</sub>",
            EFFICIENCY_REFERENCES
                .iter()
                .map(|r| format!("{} (ln({}) per {} diamonds)", r.label, r.multiplier, commas(r.diamonds, 0)))
                .collect::<Vec<_>>()
                .join(" and ")
        ),
    ]);
    out.extend(attempt_table());
    out.extend(strategy_sections(game));
    Ok(out.join("\n") + "\n")
}

/// One level's decisions in a few words: how its quest is played, and what
/// happens when nothing the quest needs is affordable.
fn step_parts(game: &Game, plan: &Plan, level: usize) -> (String, String) {
    let s = plan.step(level);
    let (pg, pd) = match s.profile {
        Some(Profile::Bar(g, d)) => (g, d),
        _ => DEFAULT_PROFILE,
    };
    let mut play = match game.quests[level].as_ref().expect("a real level") {
        Quest::Relics { .. } if s.ahead => "roll the named relics for the best chance of their bar, then the totals ahead".to_string(),
        Quest::Relics { .. } => "roll the named relics for the best chance of their bar, then the goal's totals".to_string(),
        Quest::Each(bar) => {
            let despair = bar.despair.map_or(pd, |d| d.min(pd));
            format!("roll every short relic to {}", bar_text(bar.glory.max(pg), Some(despair)))
        }
        Quest::Total { .. } if s.profile == Some(Profile::Repair) => "repair the board".to_string(),
        Quest::Total { .. } => format!("build relics to {}", bar_text(pg, Some(pd))),
    };
    if !matches!(game.quests[level], Some(Quest::Relics { .. })) {
        let total = matches!(game.quests[level], Some(Quest::Total { .. }));
        play += &match (total, s.aim_at, s.ahead) {
            (true, Some(l), _) => format!(", closing the gap to level {l}'s totals"),
            (false, Some(l), _) => format!(", then level {l}'s totals"),
            (true, None, true) => ", closing the gap to the totals ahead".to_string(),
            (true, None, false) => ", closing the gap".to_string(),
            (false, None, true) => ", then the totals ahead".to_string(),
            (false, None, false) => ", then the goal's totals".to_string(),
        };
    }
    if let Some((next, _)) = game.next_named(level).filter(|_| plan.protected(game, level) != 0) {
        play += &format!(", with {} kept back for level {next}", names(plan.protected(game, level)));
    }
    let stuck = match (s.filler, s.filler_within) {
        (true, Some(n)) => {
            format!("summon, unless the spare relics can fill the pity bar within {n} attempts: then they go to pity")
        }
        (true, None) => "spare relics go to pity".to_string(),
        (false, _) => "summon, keeping spare relics".to_string(),
    };
    (play, stuck)
}

/// "42%, 43% ... and 50%".
fn tier_list(marks: &[u8]) -> String {
    let all: Vec<String> = marks.iter().map(|m| format!("{m}%")).collect();
    match all.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => all.join(""),
    }
}

/// How the crit relic tiers past the top level are played.
fn tier_steps(marks: &[u8]) -> String {
    format!(
        "- **Past level {MAX_LEVEL}, tiers {}:** the level {MAX_LEVEL} row converts every leftover relic into crit relics ({CONVERT_FROM} of one type for {CONVERT_TO}) the moment level {MAX_LEVEL} is reached and spends them all on the {}, without summoning more, every attempt played to maximize amplification above the relic's current. Each tier is its own farm from the moment level {MAX_LEVEL} is reached: convert the leftovers, and keep converting as soon as a type reaches {CONVERT_FROM}, attempt the crit relic, summoning when short, and play every attempt to maximize amplification above a target - the tier's own mark as the Minimum Useful Amplification. A result is kept only if it raises the relic's amplification.",
        tier_list(marks),
        RELICS[CRIT],
    )
}

/// A plan's levels, grouped where they play the same way.
fn grouped_steps(game: &Game, plan: &Plan) -> Vec<String> {
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for level in 2..=MAX_LEVEL {
        let (play, stuck) = step_parts(game, plan, level);
        let text = format!("{play}; when stuck, {stuck}");
        match groups.iter_mut().find(|(t, _)| *t == text) {
            Some((_, levels)) => levels.push(level),
            None => groups.push((text, vec![level])),
        }
    }
    groups
        .into_iter()
        .map(|(text, levels)| {
            let list = levels.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
            let noun = if levels.len() > 1 { "Levels" } else { "Level" };
            format!("- **{noun} {list}:** {text}.")
        })
        .collect()
}

/// How the level-20 plan plays the climb, level by level but grouped, and
/// the tier farm past it.
fn strategy_sections(game: &Game) -> Vec<String> {
    let look = strategy("lookahead").expect("a named plan");
    let mut out = vec![
        String::new(),
        "## Strategy".to_string(),
        String::new(),
        format!("On every step: {HABITS}."),
        String::new(),
        format!(
            "Pity: a level also comes free once its meter fills - 500 for level 2, 500 more each level ({} for level {MAX_LEVEL}) - at +100 an attempt on levels 1-3, +120 on 4-7, +140 on 8-11, +160 on 12-15 and +180 on 16-19. The meter empties at every level-up, and an attempt earns its pity whatever it rolls.",
            commas(f64::from(pity_needed(MAX_LEVEL)), 0)
        ),
        String::new(),
        format!("Every row of the Look Ahead table has its own plan (all saved in `engine/src/levels/lookahead_plans.txt`); this is the one for level {MAX_LEVEL}, level by level:"),
        String::new(),
    ];
    out.extend(grouped_steps(game, &look));
    out.push(tier_steps(&CRIT_TIERS));
    out
}
