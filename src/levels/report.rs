//! Tables, charts, plan descriptions and LEVELS.md, built from the model.

use std::fmt::Write as _;

use super::{
    DEFAULT_PROFILE, Game, MAX_LEVEL, Plan, Profile, Quest, RELIC_SHORT, RELICS, RelicSet, RunOptions,
    amplification, damage, members, pity_gain, pity_needed, strategy,
};
use crate::economy::{DIAMONDS_PER_SUMMON, RELIC_TYPES, RELICS_PER_ATTEMPT, RELICS_PER_SUMMON};
use crate::format::{commas, human, pct};

pub const HABITS: &str = "attempt as soon as any relic the quest needs is affordable; use whichever needed relic you hold the most of; keep a result only if it helps";

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

/// One level of a plan's summary.
#[derive(Clone, Debug)]
pub struct Row {
    pub level: usize,
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

/// One summary row per level: cost, pity share, the two relics, damage.
pub fn level_rows(
    game: &Game,
    plan: &Plan,
    runs: u64,
    seed: u64,
    options: &RunOptions,
) -> Result<Vec<Row>, String> {
    let results = game.simulate(plan, runs, seed, options)?;
    let n = runs as f64;
    let mut rows = Vec::with_capacity(options.max_level);
    for index in 0..options.max_level {
        let level = index + 1;
        let at: Vec<_> = results.iter().map(|run| run.levels[index]).collect();
        let mut costs: Vec<f64> = at.iter().map(|l| l.diamonds as f64).collect();
        costs.sort_by(f64::total_cmp);
        let atk: Vec<f64> = at.iter().map(|l| amplification(l.atk)).collect();
        let crit: Vec<f64> = at.iter().map(|l| amplification(l.crit)).collect();
        let len = costs.len();
        rows.push(Row {
            level,
            requirement: requirement_text(game, level, false),
            short: requirement_text(game, level, true),
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
    let score = if step.score {
        "; play each attempt for max glory and min despair, weighted equally, not all-or-nothing for the bar"
    } else {
        ""
    };
    let (pg, pd) = match step.profile {
        Some(Profile::Bar(g, d)) => (g, d),
        _ => DEFAULT_PROFILE,
    };
    match game.quests[level].as_ref().expect("checked above") {
        Quest::Relics { which, bar } => {
            let mut text =
                format!("roll {} to {}", names(super::set_of(which)), bar_text(bar.glory, bar.despair));
            // the levels whose work (or filler) left these relics' stock untouched
            let banked: Vec<String> = (2..level)
                .filter(|&lv| {
                    super::set_of(which) & plan.protected(game, lv) != 0
                        && (game.quests[lv].as_ref().is_some_and(Quest::is_total)
                            || plan.step(lv).filler.is_some())
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
            format!("roll every relic that is short to {}{score}", bar_text(bar.glory.max(pg), Some(despair)))
        }
        Quest::Total { .. } => {
            // which relic is a stock question, not a quality one: preferring the
            // next level's relics (or ones already worked on) never paid
            let mut pool = "whichever relics you hold the most of".to_string();
            if locked != 0 {
                let _ = write!(pool, ", except {}", names(locked));
            }
            let mut text = if step.profile == Some(Profile::Repair) {
                format!(
                    "repair, do not rebuild: using {pool}, shave one despair off the worst relics until the total fits, then add one glory at a time"
                )
            } else {
                format!("roll {pool} to {} until the totals clear", bar_text(pg, Some(pd)))
            };
            text.push_str(score);
            if step.closer {
                text.push_str("; take any roll that closes the gap, even if it is not better on both bars");
            }
            if locked != 0 {
                let _ = write!(text, " (their stock is banked for level {})", game.bank_level());
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
    let Some((g, d)) = plan.step(level).filler else {
        return format!(". When nothing the quest needs is affordable, summon. {pity}");
    };
    let mut spare = "the best-stocked spare relic".to_string();
    let locked = plan.protected(game, level);
    if locked != 0 {
        let _ = write!(spare, " (never {})", names(locked));
    }
    format!(
        ". When nothing the quest needs is affordable, attempt {spare} instead of summoning, rolled for max glory + min despair and kept if it moves toward {}. {pity}",
        bar_text(g, Some(d))
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
        "  {:>3} | {:>14} {:>9} {:>9} {:>8} {:>7} | requirement (glory/despair)",
        "lvl", "mean diamonds", "median", "90th", "by pity", "dmg"
    );
    let _ = writeln!(out, "  {}", "-".repeat(100));
    for r in rows {
        let _ = writeln!(
            out,
            "  {:>3} | {:>14} {:>9} {:>9} {:>8} {:>7} | {}",
            r.level,
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

/// Right-axis labels: +0%, +10%, +20% ...
pub const DMG_TICK_STEP: u32 = 10;

/// Evenly stepped dmg-increase labels up to the first one at or above
/// `top_pct`.  On a log axis they are plotted at log(1 + x), so they bunch up
/// as they rise; on a linear axis they are evenly spaced.
pub fn dmg_ticks(top_pct: f64) -> Vec<u32> {
    let last = DMG_TICK_STEP * ((top_pct / f64::from(DMG_TICK_STEP) - 1e-9).ceil().max(1.0) as u32);
    (0..=last).step_by(DMG_TICK_STEP as usize).collect()
}

/// A float the way the charts have always printed one: 183.2, 184.0.
fn float(x: f64) -> String {
    if x.fract() == 0.0 { format!("{x:.1}") } else { format!("{x}") }
}

/// Columns on a linear diamond axis (left) with a red line on the right axis.
/// `line` holds multipliers (1.25 = +25%), labelled as percent increase and
/// plotted as log(multiplier), or linearly if not `log_scale`.
#[allow(clippy::too_many_arguments)]
fn dual_chart(
    levels: &[usize],
    bars: &[f64],
    line: &[f64],
    title: &str,
    desc: &str,
    bar_name: &str,
    line_name: &str,
    log_scale: bool,
) -> String {
    let top = bars.iter().copied().fold(f64::MIN, f64::max);
    let step = 10f64.powi(top.log10().floor() as i32);
    // headroom so the tallest bar's value label never hits the edge
    let y_top = step * (top * 1.12 / step).ceil();
    // the right axis tops out at the first +10% label above the line
    let line_ticks = dmg_ticks((line.iter().copied().fold(f64::MIN, f64::max) - 1.0) * 100.0);
    let scale = |m: f64| if log_scale { m.ln() } else { m - 1.0 };
    let line_top = scale(1.0 + f64::from(*line_ticks.last().expect("at least +0%")) / 100.0);

    let (ml, mr, mt, mb) = (58usize, 50usize, 40usize, 42usize);
    let (pitch, bar_w) = (47usize, 33usize);
    let (plot_w, plot_h) = (pitch * bars.len(), 210usize);
    let (width, height) = (ml + plot_w + mr, mt + plot_h + mb);
    let base_y = mt + plot_h;
    let x_mid = |i: usize| (ml + i * pitch) as f64 + pitch as f64 / 2.0;
    let y_bar = |v: f64| mt as f64 + plot_h as f64 * (1.0 - v / y_top);
    let y_line = |m: f64| mt as f64 + plot_h as f64 * (1.0 - scale(m) / line_top);

    let mut out = vec![
        format!(
            "<svg viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" role=\"img\" xmlns=\"http://www.w3.org/2000/svg\" aria-label=\"{title}\">"
        ),
        format!("<title>{title}</title>"),
        format!("<desc>{desc}</desc>"),
        concat!(
            "<style>",
            ".lv-bar{fill:#2a78d6}.lv-grid{stroke:#e1e0d9;stroke-width:1}",
            ".lv-axis{stroke:#c3c2b7;stroke-width:1}.lv-ink{fill:#52514e}",
            ".lv-muted{fill:#898781}",
            ".lv-line{stroke:#d03b32;stroke-width:2;fill:none}.lv-dot{fill:#d03b32}",
            ".lv-halo{paint-order:stroke;stroke:#ffffff;stroke-width:3px;stroke-linejoin:round}",
            ".lv-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}",
            ".lv-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}",
            "@media(prefers-color-scheme:dark){",
            ".lv-bar{fill:#3987e5}.lv-grid{stroke:#2c2c2a}.lv-axis{stroke:#383835}",
            ".lv-ink{fill:#c3c2b7}.lv-line{stroke:#ff6a5f}.lv-dot{fill:#ff6a5f}",
            ".lv-halo{stroke:#1f1f1e}}",
            "</style>"
        )
        .to_string(),
    ];
    // left axis: diamonds, with gridlines
    let ticks = 4;
    for i in 0..=ticks {
        let v = y_top * f64::from(i) / f64::from(ticks);
        let y = y_bar(v);
        out.push(format!(
            "<line class=\"lv-grid\" x1=\"{ml}\" y1=\"{y:.1}\" x2=\"{}\" y2=\"{y:.1}\"/>",
            ml + plot_w
        ));
        out.push(format!(
            "<text class=\"lv-t lv-muted\" x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            ml - 8,
            y + 4.0,
            human(v)
        ));
    }
    // right axis: dmg increase in percent, on a log or a linear scale
    for &p in &line_ticks {
        let y = y_line(1.0 + f64::from(p) / 100.0);
        out.push(format!(
            "<line class=\"lv-axis\" x1=\"{}\" y1=\"{y:.1}\" x2=\"{}\" y2=\"{y:.1}\"/>",
            ml + plot_w,
            ml + plot_w + 4
        ));
        out.push(format!(
            "<text class=\"lv-t lv-muted\" x=\"{}\" y=\"{:.1}\">+{p}%</text>",
            ml + plot_w + 7,
            y + 4.0
        ));
    }
    // axis titles and legend, above the plot
    out.push(format!(
        "<text class=\"lv-t lv-muted\" x=\"{}\" y=\"{}\" text-anchor=\"end\">diamonds</text>",
        ml - 8,
        mt - 12
    ));
    out.push(format!(
        "<text class=\"lv-t lv-muted\" x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text>",
        ml + plot_w + 46,
        mt - 12,
        if log_scale { "log dmg" } else { "dmg" }
    ));
    let lx = ml + 4;
    out.push(format!(
        "<rect class=\"lv-bar\" x=\"{lx}\" y=\"{}\" width=\"10\" height=\"10\" rx=\"2\"/>",
        mt - 30
    ));
    out.push(format!("<text class=\"lv-t lv-ink\" x=\"{}\" y=\"{}\">{bar_name}</text>", lx + 14, mt - 21));
    let lx2 = (lx + 22) as f64 + 6.2 * bar_name.len() as f64;
    out.push(format!(
        "<line class=\"lv-line\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
        float(lx2),
        mt - 25,
        float(lx2 + 16.0),
        mt - 25
    ));
    out.push(format!(
        "<text class=\"lv-t lv-ink\" x=\"{}\" y=\"{}\">{line_name}</text>",
        float(lx2 + 20.0),
        mt - 21
    ));

    // the columns, each labelled with its value
    for (i, (&lvl, &v)) in levels.iter().zip(bars).enumerate() {
        let (x, y) = (x_mid(i) - bar_w as f64 / 2.0, y_bar(v));
        let rad = 3f64.min(base_y as f64 - y);
        let w = bar_w as f64;
        out.push(format!(
            "<path class=\"lv-bar\" d=\"M{x:.1},{base_y} V{:.1} Q{x:.1},{y:.1} {:.1},{y:.1} H{:.1} Q{:.1},{y:.1} {:.1},{:.1} V{base_y} Z\"><title>level {lvl}: {} diamonds</title></path>",
            y + rad,
            x + rad,
            x + w - rad,
            x + w,
            x + w,
            y + rad,
            commas(v, 0)
        ));
        out.push(format!(
            "<text class=\"lv-t lv-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">L{lvl}</text>",
            x_mid(i),
            base_y + 15
        ));
        out.push(format!(
            "<text class=\"lv-b lv-ink lv-halo\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            x_mid(i),
            y - 6.0,
            human(v)
        ));
    }
    out.push(format!(
        "<line class=\"lv-axis\" x1=\"{ml}\" y1=\"{base_y}\" x2=\"{}\" y2=\"{base_y}\"/>",
        ml + plot_w
    ));
    out.push(format!(
        "<line class=\"lv-axis\" x1=\"{}\" y1=\"{mt}\" x2=\"{}\" y2=\"{base_y}\"/>",
        ml + plot_w,
        ml + plot_w
    ));

    // the red line on top, with a dot per level
    let points: Vec<String> =
        line.iter().enumerate().map(|(i, &m)| format!("{:.1},{:.1}", x_mid(i), y_line(m))).collect();
    out.push(format!("<polyline class=\"lv-line\" points=\"{}\"/>", points.join(" ")));
    for (i, (&lvl, &m)) in levels.iter().zip(line).enumerate() {
        out.push(format!(
            "<circle class=\"lv-dot\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"3\"><title>level {lvl}: +{} {line_name}</title></circle>",
            x_mid(i),
            y_line(m),
            pct(m - 1.0, 1)
        ));
    }
    out.push(format!(
        "<text class=\"lv-t lv-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">inheritor level reached</text>",
        ml as f64 + plot_w as f64 / 2.0,
        height - 6
    ));
    out.push("</svg>".to_string());
    out.join("\n")
}

/// Cumulative diamonds to reach each level (columns) and the total damage
/// increase so far (red line).
pub fn chart_svg(rows: &[Row], title: &str, log_scale: bool) -> String {
    let data = &rows[1..];
    let (first, last) = (&data[0], &data[data.len() - 1]);
    let desc = format!(
        "Cumulative mean diamonds by inheritor level, {} at level {} rising to {} at level {}; damage increase +{} by then.",
        human(first.mean),
        first.level,
        human(last.mean),
        last.level,
        pct(last.damage - 1.0, 0)
    );
    dual_chart(
        &data.iter().map(|r| r.level).collect::<Vec<_>>(),
        &data.iter().map(|r| r.mean).collect::<Vec<_>>(),
        &data.iter().map(|r| r.damage).collect::<Vec<_>>(),
        title,
        &desc,
        "average diamonds",
        "dmg increase",
        log_scale,
    )
}

/// Diamonds for each single level-up (columns) and the damage that level-up
/// adds on top of the previous level (red line).
pub fn marginal_chart_svg(rows: &[Row], title: &str, log_scale: bool) -> String {
    let pairs: Vec<(&Row, &Row)> = rows[1..].iter().zip(rows).collect();
    dual_chart(
        &pairs.iter().map(|(r, _)| r.level).collect::<Vec<_>>(),
        &pairs.iter().map(|(r, p)| r.mean - p.mean).collect::<Vec<_>>(),
        &pairs.iter().map(|(r, p)| r.damage / p.damage).collect::<Vec<_>>(),
        title,
        "Mean diamonds spent on each level-up, and the damage increase that level-up adds over the one before.",
        "marginal diamonds",
        "marginal dmg increase",
        log_scale,
    )
}

// ------------------------------------------------------------------ LEVELS.md

/// (plan, heading, blurb) for each mode LEVELS.md covers.
fn doc_modes() -> [(&'static str, &'static str, String); 2] {
    [
        (
            "greedy",
            "Greedy mode",
            "Every level-up made as cheap as it can be on its own, ignoring later levels.".to_string(),
        ),
        (
            "lookahead",
            "Look-ahead mode",
            format!(
                "The cheapest route to level {MAX_LEVEL} overall: some levels cost more so later ones cost less."
            ),
        ),
    ]
}

/// LEVELS.md, generated end to end from the simulation.
pub fn markdown(runs: u64, seed: u64) -> Result<String, String> {
    let game = Game::standard();
    let mut out = vec![
        "# Raising the inheritor".to_string(),
        String::new(),
        "<!-- generated by `relic levels --markdown LEVELS.md` - do not edit -->".to_string(),
        String::new(),
        format!(
            "Diamonds from inheritor level 1 to {MAX_LEVEL}, starting with no relics. {} diamonds buys {RELICS_PER_SUMMON} random relics of {RELIC_TYPES} types; an attempt uses {RELICS_PER_ATTEMPT} of one type. A level is reached by its quest or by a full pity meter, whichever comes first.",
            commas(DIAMONDS_PER_SUMMON as f64, 0)
        ),
        String::new(),
        format!(
            "dmg increase = (1 + level multiplier) x (1 + 4/5 x crit amp) x (1 + 22/209 x atk amp) - 1, where the level multiplier is +5% a level for levels 2-7 and +10% a level from 8 to {MAX_LEVEL}. Marginal columns are relative to the level before."
        ),
    ];
    for (name, title, blurb) in doc_modes() {
        let plan = strategy(name).expect("a named plan");
        let rows = level_rows(game, &plan, runs, seed, &RunOptions::default())?;
        out.extend([
            String::new(),
            format!("## {title}"),
            String::new(),
            format!("{blurb} {} simulations.", commas(runs as f64, 0)),
            String::new(),
            "| level | requirement (glory/despair) | average diamonds | marginal diamonds | median | reached by pity | crit relic amp | atk relic amp | dmg increase | marginal dmg increase |".to_string(),
            "|---|---|---|---|---|---|---|---|---|---|".to_string(),
        ]);
        let mut prev: Option<&Row> = None;
        for r in &rows {
            let (step, pity, more) = match prev {
                None => ("-".to_string(), "-".to_string(), "-".to_string()),
                Some(p) => (
                    human(r.mean - p.mean),
                    pct(r.pity, 0),
                    format!("+{}", pct(r.damage / p.damage - 1.0, 1)),
                ),
            };
            out.push(format!(
                "| **{}** | {} | {} | {step} | {} | {pity} | {:.2}% | {:.2}% | +{} | {more} |",
                r.level,
                r.short,
                commas(r.mean, 0),
                human(r.median),
                r.crit_amp,
                r.atk_amp,
                pct(r.damage - 1.0, 1)
            ));
            prev = Some(r);
        }
        // log damage axis, then linear; side by side where there is room
        for (log_scale, axis) in [(true, "log"), (false, "linear")] {
            out.extend([
                String::new(),
                "<div style=\"display:flex;flex-wrap:wrap;gap:12px\">".to_string(),
                chart_svg(&rows, &format!("{title}: total cost and damage ({axis})"), log_scale),
                marginal_chart_svg(&rows, &format!("{title}: each level-up ({axis})"), log_scale),
                "</div>".to_string(),
            ]);
        }
    }
    out.extend([
        String::new(),
        "## Strategy at each level".to_string(),
        String::new(),
        format!("On every step, both modes: {HABITS}."),
    ]);
    for level in 2..=MAX_LEVEL {
        out.extend([
            String::new(),
            format!("### Level {level} - {}", requirement_text(game, level, false)),
            String::new(),
        ]);
        for (name, title, _) in doc_modes() {
            let plan = strategy(name).expect("a named plan");
            let mode = title.split(' ').next().expect("a word");
            out.push(format!("- **{mode}:** {}.", describe_step(game, &plan, level)));
        }
    }
    Ok(out.join("\n") + "\n")
}
