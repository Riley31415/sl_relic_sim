//! `relic solve`: the exact solve for one inheritor level, and the tables
//! built from it - what STRATEGY.md is written from.

use std::fmt::Write as _;

use crate::format::{amp_pct, general, pct, pct3, signed_pct};
use crate::rng::Rng;
use crate::solver::{Action, Analysis, Choice, Config, Solver, Strategy, TIERS, Tally};

/// What the command line asked for, gathered in one place.
pub struct SolveArgs {
    pub level: i64,
    pub target: Option<(u8, u8)>,
    pub amp_glory: f64,
    pub amp_despair: f64,
    pub safety_first: bool,
    pub wipe_penalty: f64,
    pub max_level: u32,
    pub seed: u64,
}

impl SolveArgs {
    /// A target strategy when one was given, otherwise the weighted one.
    pub fn strategy(&self) -> Strategy {
        match self.target {
            Some((g, d)) => Strategy::target(g, d).with_weights(self.amp_glory, self.amp_despair),
            None => Strategy::weighted(self.amp_glory, self.amp_despair),
        }
    }

    fn config(&self, level: i64) -> Result<Config, String> {
        Config::for_level(level, self.strategy())
    }
}

fn solve(cfg: Config) -> Analysis {
    Solver::new(cfg).analyse(None)
}

/// The full report for one level: setup, results, distribution, policy,
/// opening move and the safety trade-off.
pub fn report(
    args: &SolveArgs,
    dist: bool,
    tradeoff: bool,
    mc: u64,
    play_count: u64,
) -> Result<String, String> {
    let cfg = Config {
        safety_first: args.safety_first,
        wipe_penalty: args.wipe_penalty,
        ..args.config(args.level)?
    };
    let mut solver = Solver::new(cfg);
    let analysis = solver.analyse(None);
    let mut out = String::new();
    setup(&mut out, &cfg);
    results(&mut out, &analysis, solver.max_amplification());
    if dist {
        distribution(&mut out, &analysis);
    }
    policy(&mut out, &analysis);
    opening(&mut out, &mut solver);
    if tradeoff {
        safety_tradeoff(&mut out, args)?;
    }
    if mc > 0 {
        monte_carlo(&mut out, &mut solver, mc, args.seed);
    }
    if play_count > 0 {
        play(&mut out, &mut solver, play_count, args.seed);
    }
    Ok(out)
}

fn setup(out: &mut String, cfg: &Config) {
    let ladder: Vec<String> = TIERS.iter().map(|&t| pct(t, 0)).collect();
    let _ = writeln!(out, "Inheritor level {}", cfg.level);
    let _ = writeln!(out, "  memory slots per bar : {}", cfg.slots);
    let _ = writeln!(out, "  max spirit power     : {}  (spirit power starts here)", cfg.max_spirit);
    let _ = writeln!(out, "  starting mental str. : {}", cfg.start_mental());
    let _ = writeln!(out, "  glory rate modifier  : {}", signed_pct(cfg.glory_mod));
    let _ = writeln!(out, "  despair rate modifier: {}", signed_pct(cfg.despair_mod));
    let _ = writeln!(
        out,
        "  chance ladder        : {}   (starts at {})",
        ladder.join("  "),
        pct(TIERS[cfg.start_tier as usize], 0)
    );
    let st = &cfg.strategy;
    let _ = writeln!(out, "  strategy             : {}", st.name());
    if st.is_target() {
        let _ = writeln!(
            out,
            "  objective            : ALL OR NOTHING - maximise P({}+ glory successes and {} or fewer despair successes)",
            cfg.want_glory(),
            cfg.allow_despair()
        );
    } else {
        let _ = writeln!(
            out,
            "  objective            : maximise amplification (+{}% per glory success, -{}% per despair success)",
            general(st.w_glory),
            general(st.w_despair)
        );
    }
    let short = 2 * i32::from(cfg.slots) - i32::from(cfg.max_spirit);
    let need = (short.max(0) + 1) / 2;
    let _ = writeln!(
        out,
        "  spirit power needed  : {} to complete both bars, {} on hand -> at least {need} successful mental training{} (worth full value at {} spirit power or below)",
        2 * cfg.slots,
        cfg.max_spirit,
        if need == 1 { "" } else { "s" },
        i32::from(cfg.max_spirit) - 2
    );
}

fn results(out: &mut String, a: &Analysis, solver_max: f64) {
    let cfg = &a.cfg;
    let s = f64::from(cfg.slots);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "OPTIMAL PLAY  (exact expectimax, {} states)",
        crate::format::commas(a.states_explored as f64, 0)
    );
    let _ = writeln!(
        out,
        "  expected glory successes : {:.4} / {}   ({} of the bar, higher is better)",
        a.e_glory,
        cfg.slots,
        pct(a.e_glory / s, 2)
    );
    let _ = writeln!(
        out,
        "  expected despair success : {:.4} / {}   ({} of the bar, LOWER is better)",
        a.e_despair_success,
        cfg.slots,
        pct(a.e_despair_success / s, 2)
    );
    let _ = writeln!(
        out,
        "  expected amplification   : {:>8} of a possible {}",
        amp_pct(a.e_amplification, 3),
        amp_pct(solver_max, 0)
    );
    if cfg.strategy.is_target() {
        let _ = writeln!(
            out,
            "  P(hit target {}+ glory, <={} despair) : {}",
            cfg.want_glory(),
            cfg.allow_despair(),
            pct(a.p_target, 4)
        );
    }
    let _ = writeln!(out, "  P(dead-end wipe)         : {}", pct(a.p_dead_end, 4));
    let c = &a.action_counts;
    let _ = writeln!(
        out,
        "  expected actions taken   : {:.2}  (attempt glory {:.2}, attempt despair {:.2}, mental training {:.2} of {})",
        c.total(),
        c.get(Action::Glory),
        c.get(Action::Despair),
        c.get(Action::Train),
        cfg.start_mental()
    );
}

fn distribution(out: &mut String, a: &Analysis) {
    let s = usize::from(a.cfg.slots);
    let mut glory = vec![0.0; s + 1];
    let mut despair = vec![0.0; s + 1];
    for ((g, d), p) in a.dist.iter() {
        glory[usize::from(g)] += p;
        despair[usize::from(d)] += p;
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "OUTCOME DISTRIBUTION  (glory successes: want high. despair successes: want low)");
    let _ = writeln!(out, "  {:>5} | {:>16} | {:>14}", "count", "glory successes", "despair succ.");
    for i in 0..=s {
        let bar = "#".repeat((glory[i] * 40.0).round_ties_even() as usize);
        let _ = writeln!(out, "  {i:>5} | {:>16} | {:>14}  {bar}", pct(glory[i], 2), pct(despair[i], 2));
    }
    let _ = writeln!(out, "  (bars show the glory marginal)");
    let _ = writeln!(out);
    let _ = writeln!(out, "  joint table, rows = glory successes, cols = despair successes");
    let header: String = (0..=s).map(|d| format!("{d:>9}")).collect();
    let _ = writeln!(out, "        {header}");
    for g in 0..=s {
        let cells: String =
            (0..=s).map(|d| format!("{:>9}", pct(a.dist.get((g as u8, d as u8)), 2))).collect();
        let _ = writeln!(out, "  g={g}  {cells}");
    }
}

fn policy(out: &mut String, a: &Analysis) {
    let _ = writeln!(out);
    let _ = writeln!(out, "POLICY: what optimal play actually does");
    let _ = writeln!(out);
    let _ = writeln!(out, "  share of decisions by chance tier (probability weighted)");
    let _ = writeln!(
        out,
        "  {:>6} | {:>14} {:>16} {:>16} | {:>8}",
        "tier", "attempt glory", "attempt despair", "mental training", "weight"
    );
    for (t, &rate) in TIERS.iter().enumerate() {
        let Some(counts) = a.action_by_tier.get(&(t as u8)).filter(|c| !c.is_empty()) else { continue };
        let total = counts.total();
        let _ = writeln!(
            out,
            "  {:>6} | {:>14} {:>16} {:>16} | {total:>8.3}",
            pct(rate, 0),
            pct(counts.get(Action::Glory) / total, 1),
            pct(counts.get(Action::Despair) / total, 1),
            pct(counts.get(Action::Train) / total, 1)
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  dominant choice by tier x spirit power  (G attempt glory / D attempt despair / T mental training)"
    );
    let _ = writeln!(out, "  lowercase = the choice is mixed, it depends on how full the bars are");
    let max_sp = a.cfg.max_spirit;
    let header: String = (0..=max_sp).map(|sp| format!("{sp:>4}")).collect();
    let _ = writeln!(out, "  {:>6}   spirit power", "");
    let _ = writeln!(out, "         {header}");
    for (t, &rate) in TIERS.iter().enumerate() {
        let row: String = (0..=max_sp)
            .map(|sp| match a.action_by_sp.get(&(t as u8, sp)).filter(|c| !c.is_empty()) {
                None => "   .".to_string(),
                Some(counts) => {
                    let (action, mass) = counts.most_common().expect("non-empty");
                    let letter = action.letter();
                    let shown =
                        if mass / counts.total() > 0.999 { letter } else { letter.to_ascii_lowercase() };
                    format!("{shown:>4}")
                }
            })
            .collect();
        let _ = writeln!(out, "  {:>6} {row}", pct(rate, 0));
    }
}

fn opening(out: &mut String, solver: &mut Solver) {
    let start = solver.start_state();
    let _ = writeln!(out);
    let _ = writeln!(out, "  opening move, expected score of each option from the start state");
    let best = solver.best_action(start);
    for action in solver.legal_actions(start) {
        let mut ev = 0.0;
        for (p, next, _ok) in solver.transitions(start, action) {
            if p != 0.0 {
                ev += p * solver.value(next).score;
            }
        }
        let mark = if best == Choice::Act(action) { "  <== optimal" } else { "" };
        let _ = writeln!(out, "    {:<16} {ev:7.4}{mark}", action.name());
    }
}

/// How much score dodging the dead-end wipe costs: a wipe penalty is an extra
/// charge on a wiped attempt; "min wipe first" drives the wipe chance as low
/// as the rules allow before looking at score at all.
fn safety_tradeoff(out: &mut String, args: &SolveArgs) -> Result<(), String> {
    let _ = writeln!(out);
    let _ = writeln!(out, "SAFETY TRADE-OFF: cost of dodging the dead-end wipe");
    let _ = writeln!(
        out,
        "  {:>16} | {:>9} {:>13} {:>9} {:>9}",
        "objective", "E[glory]", "E[desp succ]", "E[amp]", "P(wipe)"
    );
    let base = args.config(args.level)?;
    let mut rows: Vec<(String, Config)> = [0.0, 3.0, 10.0, 30.0]
        .into_iter()
        .map(|p| (format!("wipe penalty {}", general(p)), Config { wipe_penalty: p, ..base }))
        .collect();
    rows.push(("min wipe first".to_string(), Config { safety_first: true, ..base }));
    for (label, cfg) in rows {
        let a = solve(cfg);
        let _ = writeln!(
            out,
            "  {label:>16} | {:>9.4} {:>13.4} {:>9} {:>9}",
            a.e_glory,
            a.e_despair_success,
            amp_pct(a.e_amplification, 3),
            pct(a.p_dead_end, 4)
        );
    }
    Ok(())
}

fn monte_carlo(out: &mut String, solver: &mut Solver, trials: u64, seed: u64) {
    let mut rng = Rng::for_run(seed, 0);
    let (mut g, mut d, mut dead) = (0.0, 0.0, 0u64);
    for _ in 0..trials {
        let res = solver.attempt(&mut rng, None);
        g += f64::from(res.glory_success);
        d += f64::from(res.despair_success);
        dead += u64::from(res.dead_end);
    }
    let n = trials as f64;
    let _ = writeln!(out);
    let _ = writeln!(out, "MONTE CARLO CROSS-CHECK ({} attempts, seed {seed})", crate::format::commas(n, 0));
    let _ = writeln!(out, "  mean glory successes : {:.4}", g / n);
    let _ = writeln!(out, "  mean despair success : {:.4}", d / n);
    let _ = writeln!(out, "  dead-end rate        : {}", pct(dead as f64 / n, 4));
}

fn play(out: &mut String, solver: &mut Solver, count: u64, seed: u64) {
    let mut rng = Rng::for_run(seed, 0);
    for i in 0..count {
        let mut log = Vec::new();
        let res = solver.attempt(&mut rng, Some(&mut log));
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "SAMPLE ATTEMPT {}   (SUCCESS = the roll succeeded: good on glory, bad on despair; bars read successes/attempted)",
            i + 1
        );
        for step in &log {
            let s = step.state;
            let chance = solver.chance(step.action, s.tier);
            let _ = writeln!(
                out,
                "  sp={} ms={} tier={:>3} glory={}/{} despair={}/{}  ->  {:<16} @ {:>3}  {:<7} -> next tier {}",
                s.sp,
                s.ms,
                pct(TIERS[s.tier as usize], 0),
                s.gs,
                s.gf,
                s.ds,
                s.df,
                step.action.name(),
                pct(chance, 0),
                if step.succeeded { "SUCCESS" } else { "fail" },
                pct(TIERS[step.next.tier as usize], 0)
            );
        }
        if res.dead_end {
            let _ = writeln!(
                out,
                "  DEAD END - attempt wiped (0 glory successes, {} despair successes)",
                res.despair_success
            );
        } else {
            let _ = writeln!(
                out,
                "  result: {} glory successes, {} despair successes (sp left {}, ms left {})",
                res.glory_success, res.despair_success, res.spirit_left, res.mental_left
            );
        }
    }
}

/// One summary line per level.
pub fn all_levels(args: &SolveArgs) -> Result<String, String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:>4} {:>6} {:>6} {:>7} {:>7} {:>9} {:>13} {:>9} {:>9} {:>9}",
        "lvl",
        "slots",
        "maxSP",
        "glory%",
        "desp%",
        "E[glory]",
        "E[desp succ]",
        "E[amp]",
        "per slot",
        "P(wipe)"
    );
    for level in 1..=args.max_level {
        let cfg = args.config(i64::from(level))?;
        let a = solve(cfg);
        let per_slot = a.e_amplification / (cfg.strategy.w_glory * f64::from(cfg.slots));
        let _ = writeln!(
            out,
            "{level:>4} {:>6} {:>6} {:>7} {:>7} {:>9.4} {:>13.4} {:>9} {:>8} {:>9}",
            cfg.slots,
            cfg.max_spirit,
            signed_pct(cfg.glory_mod),
            signed_pct(cfg.despair_mod),
            a.e_glory,
            a.e_despair_success,
            amp_pct(a.e_amplification, 2),
            pct(per_slot, 1),
            pct(a.p_dead_end, 4)
        );
    }
    Ok(out)
}

/// Every level under the weighted objective and the all-or-nothing target.
pub fn compare_strategies(args: &SolveArgs) -> Result<String, String> {
    let (tg, td) = args.target.unwrap_or((4, 2));
    let strategies = [
        Strategy::weighted(args.amp_glory, args.amp_despair),
        Strategy::target(tg, td).with_weights(args.amp_glory, args.amp_despair),
    ];
    let mut solved = Vec::new();
    for level in 1..=args.max_level {
        let mut pair = Vec::new();
        for strat in strategies {
            pair.push(solve(Config::for_level(i64::from(level), strat)?));
        }
        solved.push(pair);
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "Strategy comparison: weighted amplification vs the all-or-nothing target ({tg}, {td})"
    );
    let _ = writeln!(out, "  E[desp] is despair SUCCESSES (lower is better); failure = dead-end wipe");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  {:>3} {:>5} | {:>22} | {:>8} {:>8} {:>8} {:>8} | {:>9}",
        "lvl", "slots", "strategy", "E[glory]", "E[desp]", "E[amp]", "failure", "hit rate"
    );
    for (level, pair) in (1..).zip(&solved) {
        for (i, a) in pair.iter().enumerate() {
            let lvl = if i == 0 { format!("{level:>3} {:>5}", a.cfg.slots) } else { " ".repeat(9) };
            let hit = if a.cfg.strategy.is_target() { pct(a.p_target, 3) } else { "-".to_string() };
            let _ = writeln!(
                out,
                "  {lvl} | {:>22} | {:>8.4} {:>8.4} {:>8} {:>8} | {hit:>9}",
                a.cfg.strategy.name(),
                a.e_glory,
                a.e_despair_success,
                amp_pct(a.e_amplification, 2),
                pct(a.p_dead_end, 3)
            );
        }
        let _ = writeln!(out);
    }

    let _ = writeln!(
        out,
        "  Is chasing the box worth it?  P(>={tg} glory and <={td} despair), measured the same way for both"
    );
    let _ = writeln!(
        out,
        "  {:>3} | {:>10} {:>15} {:>8} | {:>13}",
        "lvl", "weighted", "all-or-nothing", "gain", "amp given up"
    );
    for (level, pair) in (1..).zip(&solved) {
        let rate = |a: &Analysis| -> f64 {
            a.dist.iter().filter(|&((g, d), _)| g >= tg && d <= td).map(|(_, p)| p).sum()
        };
        let (r0, r1) = (rate(&pair[0]), rate(&pair[1]));
        let _ = writeln!(
            out,
            "  {level:>3} | {:>10} {:>15} {:>8} | {:>13}",
            pct(r0, 3),
            pct(r1, 3),
            pct(r1 - r0, 3),
            amp_pct(pair[1].e_amplification - pair[0].e_amplification, 2)
        );
    }
    Ok(out)
}

// -------------------------------------------------------- amplification grid

fn by_amplification(solver: &Solver, a: &Analysis) -> Tally<u64> {
    let mut by_amp = Tally::default();
    for ((g, d), p) in a.dist.iter() {
        by_amp.add(solver.amplification(g, d).to_bits(), p);
    }
    by_amp
}

/// P(exactly each glory/despair combination), with the amplification each
/// one pays - as text, or (`html`) as a shaded table plus the curve SVG.
pub fn amplification_table(args: &SolveArgs, html: bool) -> Result<String, String> {
    let cfg = Config::for_level(args.level, Strategy::weighted(args.amp_glory, args.amp_despair))?;
    let mut solver = Solver::new(cfg);
    let a = solver.analyse(None);
    let s = cfg.slots;
    let peak = a.dist.iter().map(|(_, p)| p).fold(f64::MIN, f64::max);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "Inheritor level {}: amplification = +{}% per glory success, -{}% per despair success",
        cfg.level,
        general(args.amp_glory),
        general(args.amp_despair)
    );
    let _ = writeln!(
        out,
        "  {} slots per bar, {} max spirit power, glory {}, despair {}",
        cfg.slots,
        cfg.max_spirit,
        signed_pct(cfg.glory_mod),
        signed_pct(cfg.despair_mod)
    );
    let _ = writeln!(
        out,
        "  expected amplification : {}   (range {} to {})",
        amp_pct(a.e_amplification, 3),
        amp_pct(solver.min_amplification(), 0),
        amp_pct(solver.max_amplification(), 0)
    );
    let _ = writeln!(out, "  expected glory         : {:.4} / {s}", a.e_glory);
    let _ = writeln!(out, "  expected despair       : {:.4} / {s}", a.e_despair_success);
    let _ = writeln!(
        out,
        "  dead-end wipe          : {}  (scores {}, the worst possible)",
        pct(a.p_dead_end, 3),
        amp_pct(solver.amplification(0, s), 0)
    );
    let _ = writeln!(
        out,
        "  most likely single cell: {} of attempts, and the grid below sums to 100%",
        pct3(peak)
    );

    if html {
        let _ = writeln!(out);
        out.push_str(&amplification_html(&solver, &a));
        let _ = writeln!(out);
        out.push_str(&amplification_curve_svg(&solver, &a));
        out.push('\n');
        return Ok(out);
    }

    let header = format!("  desp |{}   <- glory", (0..=s).map(|g| format!("{g:>8}")).collect::<String>());
    let rule = format!("  {}", "-".repeat(header.len() - 2));
    let _ = writeln!(out);
    let _ = writeln!(out, "  P(exactly this combination), percent.  The grid sums to 100%.");
    let _ = writeln!(out, "  glory increases left to right; despair decreases top to bottom,");
    let _ = writeln!(out, "  so the best outcome is the bottom-right corner.");
    let _ = writeln!(out);
    let _ = writeln!(out, "{header}");
    let _ = writeln!(out, "{rule}");
    for d in (0..=s).rev() {
        let cells: String = (0..=s).map(|g| format!("{:>8}", pct3(a.dist.get((g, d))))).collect();
        let _ = writeln!(out, "  {d:>4} |{cells}");
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "  the same cells as amplification values:");
    let _ = writeln!(out, "{header}");
    let _ = writeln!(out, "{rule}");
    for d in (0..=s).rev() {
        let cells: String =
            (0..=s).map(|g| format!("{:>8}", amp_pct(solver.amplification(g, d), 0))).collect();
        let _ = writeln!(out, "  {d:>4} |{cells}");
    }
    Ok(out)
}

/// RdYlGn with a LIGHT red at the bottom: most of a joint-probability grid
/// sits near zero, and a saturated red there reads as a heavy block rather
/// than as "rare".
const HEAT_STOPS: [(f64, [f64; 3]); 5] = [
    (0.00, [247.0, 190.0, 185.0]),
    (0.25, [252.0, 174.0, 132.0]),
    (0.50, [255.0, 255.0, 191.0]),
    (0.75, [166.0, 217.0, 106.0]),
    (1.00, [26.0, 152.0, 80.0]),
];
pub const GLORY_GOLD: [f64; 3] = [212.0, 175.0, 55.0];
pub const DESPAIR_RED: [f64; 3] = [139.0, 0.0, 0.0];
/// Table ink is always black: flipping to white over the dark ends would hold
/// contrast better but make the grid read as two different tables.
const INK: &str = "#000000";

fn hex(rgb: [f64; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0] as u8, rgb[1] as u8, rgb[2] as u8)
}

/// The red-to-green scale at `fraction`: (background, text).
pub fn heat_colour(fraction: f64) -> (String, &'static str) {
    let f = fraction.clamp(0.0, 1.0);
    let mut rgb = HEAT_STOPS[4].1;
    for pair in HEAT_STOPS.windows(2) {
        let ((lo, c_lo), (hi, c_hi)) = (pair[0], pair[1]);
        if f <= hi {
            let span = hi - lo;
            let k = if span == 0.0 { 0.0 } else { (f - lo) / span };
            rgb = std::array::from_fn(|i| (c_lo[i] + (c_hi[i] - c_lo[i]) * k).round_ties_even());
            break;
        }
    }
    (hex(rgb), INK)
}

/// White at 0 through to `rgb` at 1: (background, text).
pub fn ramp_colour(fraction: f64, rgb: [f64; 3]) -> (String, &'static str) {
    let f = fraction.clamp(0.0, 1.0);
    (hex(std::array::from_fn(|i| (255.0 + (rgb[i] - 255.0) * f).round_ties_even())), INK)
}

/// The joint outcome grid as an HTML table, shaded relative to the most
/// likely cell - no single combination gets near 100% on its own.
fn amplification_html(solver: &Solver, a: &Analysis) -> String {
    let s = solver.cfg.slots;
    let peak = a.dist.iter().map(|(_, p)| p).fold(f64::MIN, f64::max);
    let cell = "padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);";
    let head = "padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;";
    let mut out = String::new();
    let _ = writeln!(out, "<table style=\"border-collapse:collapse;font-size:13px;\">");
    let glory_heads: String = (0..=s)
        .map(|g| {
            let (bg, fg) = ramp_colour(f64::from(g) / f64::from(s), GLORY_GOLD);
            format!("<th style=\"{head}background:{bg};color:{fg};\">{g}</th>")
        })
        .collect();
    let _ = writeln!(
        out,
        "<thead><tr><th style=\"{head}color:#000000;background:#ffffff;\">despair &darr; / glory &rarr;</th>{glory_heads}</tr></thead>"
    );
    let _ = writeln!(out, "<tbody>");
    for d in (0..=s).rev() {
        let (d_bg, d_fg) = ramp_colour(f64::from(d) / f64::from(s), DESPAIR_RED);
        let cells: String = (0..=s)
            .map(|g| {
                let p = a.dist.get((g, d));
                let (bg, fg) = heat_colour(if peak > 0.0 { p / peak } else { 0.0 });
                format!(
                    "<td style=\"{cell}background:{bg};color:{fg};\" title=\"{g} glory, {d} despair - amplifies {}%\">{}</td>",
                    general(solver.amplification(g, d)),
                    pct3(p)
                )
            })
            .collect();
        let _ = writeln!(out, "<tr><th style=\"{head}background:{d_bg};color:{d_fg};\">{d}</th>{cells}</tr>");
    }
    let _ = writeln!(out, "</tbody></table>");
    out
}

/// The amplification distribution as an inline SVG column chart: one column
/// per achievable amplification.  The wipe column carries the critical colour.
fn amplification_curve_svg(solver: &Solver, a: &Analysis) -> String {
    let cfg = &solver.cfg;
    let by_amp = by_amplification(solver, a);
    let value = |bits: u64| f64::from_bits(bits);
    let lo = solver.min_amplification() as i64;
    let hi = solver.max_amplification() as i64;
    let span = (hi - lo + 1) as f64;
    let (mode_bits, peak) = by_amp.most_common().expect("a distribution is never empty");
    let y_top = 0.05 * (peak / 0.05).ceil();
    let mean: f64 = by_amp.iter().map(|(v, p)| value(v) * p).sum();

    let (pitch, bar_w) = (9.0, 7.0);
    let (ml, mr, mt, mb) = (46.0, 14.0, 26.0, 36.0);
    let (plot_w, plot_h) = (span * pitch, 210.0);
    let (width, height) = (ml + plot_w + mr, mt + plot_h + mb);
    let x_of = |v: f64| ml + (v - lo as f64) * pitch + (pitch - bar_w) / 2.0;
    let x_mid = |v: f64| ml + (v - lo as f64) * pitch + pitch / 2.0;
    let y_of = |p: f64| mt + plot_h * (1.0 - p / y_top);
    let n = crate::format::general;

    let mut out = vec![
        format!(
            "<svg viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" xmlns=\"http://www.w3.org/2000/svg\" aria-label=\"Distribution of relic amplification at inheritor level {lvl}\">",
            w = n(width),
            h = n(height),
            lvl = cfg.level
        ),
        format!("<title>Amplification distribution, inheritor level {}</title>", cfg.level),
        format!(
            "<desc>Chance of each exact amplification under optimal weighted play. Mean {mean:.2} percent. Peak {:.1} percent of attempts at an amplification of {} percent.</desc>",
            100.0 * peak,
            n(value(mode_bits))
        ),
        concat!(
            "<style>",
            ".ac-bar{fill:#2a78d6}.ac-wipe{fill:#d03b3b}",
            ".ac-grid{stroke:#e1e0d9;stroke-width:1}.ac-axis{stroke:#c3c2b7;stroke-width:1}",
            ".ac-ink{fill:#52514e}.ac-muted{fill:#898781}.ac-mean{stroke:#52514e}",
            ".ac-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}",
            ".ac-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}",
            "@media(prefers-color-scheme:dark){",
            ".ac-bar{fill:#3987e5}.ac-grid{stroke:#2c2c2a}.ac-axis{stroke:#383835}",
            ".ac-ink{fill:#c3c2b7}.ac-mean{stroke:#c3c2b7}}",
            "</style>"
        )
        .to_string(),
    ];

    // recessive horizontal grid and y labels
    let step = 0.05;
    let ticks = (y_top / step).round_ties_even() as i64;
    for i in 0..=ticks {
        let p = i as f64 * step;
        let y = y_of(p);
        out.push(format!(
            "<line class=\"ac-grid\" x1=\"{}\" y1=\"{y:.1}\" x2=\"{}\" y2=\"{y:.1}\"/>",
            n(ml),
            n(ml + plot_w)
        ));
        out.push(format!(
            "<text class=\"ac-t ac-muted\" x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">{:.0}%</text>",
            n(ml - 8.0),
            y + 4.0,
            100.0 * p
        ));
    }
    out.push(format!(
        "<text class=\"ac-t ac-muted\" x=\"{}\" y=\"{}\" text-anchor=\"end\">chance</text>",
        n(ml - 8.0),
        n(mt - 9.0)
    ));

    // columns, rounded at the data end
    let radius = 3f64.min(bar_w / 2.0);
    let base_y = mt + plot_h;
    let mut sorted: Vec<(u64, f64)> = by_amp.iter().collect();
    sorted.sort_by(|x, y| value(x.0).total_cmp(&value(y.0)));
    for (bits, p) in sorted {
        if p <= 0.0 {
            continue;
        }
        let v = value(bits);
        let (x, y) = (x_of(v), y_of(p));
        let r = radius.min(base_y - y);
        let class = if v == lo as f64 { "ac-wipe" } else { "ac-bar" };
        out.push(format!(
            "<path class=\"{class}\" d=\"M{x:.1},{} V{:.1} Q{x:.1},{y:.1} {:.1},{y:.1} H{:.1} Q{:.1},{y:.1} {:.1},{:.1} V{} Z\"><title>amplification {}: {}</title></path>",
            n(base_y),
            y + r,
            x + r,
            x + bar_w - r,
            x + bar_w,
            x + bar_w,
            y + r,
            n(base_y),
            n(v),
            pct3(p)
        ));
    }

    // baseline and x labels every 10
    out.push(format!(
        "<line class=\"ac-axis\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
        n(ml),
        n(base_y),
        n(ml + plot_w),
        n(base_y)
    ));
    for v in (lo..=hi).filter(|v| v % 10 == 0) {
        out.push(format!(
            "<text class=\"ac-t ac-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">{v}%</text>",
            x_mid(v as f64),
            n(base_y + 15.0)
        ));
    }
    out.push(format!(
        "<text class=\"ac-t ac-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">amplification (+{}% per glory success, -{}% per despair success)</text>",
        ml + plot_w / 2.0,
        n(height - 6.0),
        n(cfg.strategy.w_glory),
        n(cfg.strategy.w_despair)
    ));

    // selective direct labels: the mean, the mode, and the wipe
    let mx = x_mid(mean);
    out.push(format!(
        "<line class=\"ac-mean\" x1=\"{mx:.1}\" y1=\"{}\" x2=\"{mx:.1}\" y2=\"{}\" stroke-dasharray=\"3 3\" stroke-width=\"1.5\"/>",
        n(mt - 6.0),
        n(base_y)
    ));
    out.push(format!(
        "<text class=\"ac-b ac-ink\" x=\"{:.1}\" y=\"{}\">mean {mean:.1}%</text>",
        mx + 5.0,
        n(mt - 12.0)
    ));
    out.push(format!(
        "<text class=\"ac-b ac-ink\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
        x_mid(value(mode_bits)),
        y_of(peak) - 6.0,
        pct3(peak)
    ));
    // the floored column holds the wipe, plus any run washed out to zero
    let wipe_p = by_amp.get((lo as f64).to_bits());
    if wipe_p > 0.0 {
        let washouts = wipe_p - a.p_dead_end;
        let name = if washouts < 5e-5 { "wipe" } else { "wipe + washouts" };
        out.push(format!(
            "<text class=\"ac-b ac-wipe\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"start\">{name} {}</text>",
            x_mid(lo as f64) + 6.0,
            y_of(wipe_p) - 8.0,
            pct3(wipe_p)
        ));
    }
    out.push("</svg>".to_string());
    out.join("\n")
}
