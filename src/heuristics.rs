//! `relic heuristics`: rules a player can hold in their head, scored against
//! the exact optimum on the same exhaustive probability sweep (not sampling),
//! so the gap to optimal play is an exact number.

use std::fmt::Write as _;

use crate::format::pct;
use crate::solver::{Action, Config, Solver, State, Strategy, TIERS};

/// First preference that is actually legal.
fn pick(legal: &[Action], preferences: [Action; 3]) -> Action {
    preferences.into_iter().find(|a| legal.contains(a)).unwrap_or(legal[0])
}

use Action::{Despair, Glory, Train};

/// Baseline: run the glory bar out, then despair; mental training only when dry.
pub fn always_glory_first() -> impl Fn(State, &[Action]) -> Action {
    |s, legal| if s.sp == 0 { Train } else { pick(legal, [Glory, Despair, Train]) }
}

/// Glory on the good rates, despair on the bad ones, train when dry.
pub fn tier_split(split: u8) -> impl Fn(State, &[Action]) -> Action {
    move |s, legal| {
        if s.sp == 0 {
            Train
        } else if s.tier < split {
            pick(legal, [Glory, Despair, Train])
        } else if s.tier > split {
            pick(legal, [Despair, Glory, Train])
        } else {
            pick(legal, [Train, Glory, Despair])
        }
    }
}

/// Tier split, plus mental training early enough that the fuel never runs
/// out: only while missing at least 2 spirit power (so the +2 is never wasted
/// against the cap) and while the bars still need more fuel than is on hand.
pub fn tier_split_with_fuel(cfg: Config, split: u8, reserve: i32) -> impl Fn(State, &[Action]) -> Action {
    move |s, legal| {
        if s.sp == 0 {
            return Train;
        }
        let fills_left = i32::from(cfg.slots - s.gf) + i32::from(cfg.slots - s.df);
        if legal.contains(&Train)
            && i32::from(s.sp) <= i32::from(cfg.max_spirit) - 2
            && fills_left - i32::from(s.sp) > reserve
        {
            return Train;
        }
        tier_split(split)(s, legal)
    }
}

/// The full rule of thumb, tuned at inheritor level 7 (level 1 cannot
/// exercise it: one mental training always closes its fuel gap).
///
/// 1. fuel - mental training when missing 2+ spirit power, the slots left
///    exceed spirit power, and the rate is at `fuel_max_tier` or better
///    (a training at 20% fails four times in five).
/// 2. tier split - with both bars open, glory on the good rates, despair on
///    the rest.
/// 3. stall - with one bar left, attempt it on the rates that suit it and
///    train on the rest: mental strength has no other job by then.
/// 4. never strand yourself - out of spirit power, train.
///
/// `fuel_max_tier` -1 disables the fuel clause; 4 lets it fire at any rate.
#[derive(Clone, Copy)]
pub struct FullRule {
    pub glory_max: u8,
    pub despair_min: u8,
    pub reserve: i32,
    pub glory_stall: u8,
    pub despair_stall: u8,
    pub fuel_max_tier: i32,
}

impl Default for FullRule {
    fn default() -> Self {
        FullRule {
            glory_max: 1,
            despair_min: 2,
            reserve: 0,
            glory_stall: 2,
            despair_stall: 2,
            fuel_max_tier: 2,
        }
    }
}

impl FullRule {
    /// The previous rule, tuned at level 1: its rate-blind fuel clause is inert
    /// at level 1 and costly above it.
    pub fn level1_tuned() -> Self {
        FullRule {
            glory_max: 1,
            despair_min: 3,
            reserve: 2,
            glory_stall: 1,
            despair_stall: 2,
            fuel_max_tier: 4,
        }
    }

    pub fn policy(self, cfg: Config) -> impl Fn(State, &[Action]) -> Action {
        move |s, legal| {
            if s.sp == 0 {
                return Train;
            }
            let glory_left = cfg.slots - s.gf;
            let despair_left = cfg.slots - s.df;
            let fills_left = i32::from(glory_left) + i32::from(despair_left);
            if legal.contains(&Train)
                && i32::from(s.sp) <= i32::from(cfg.max_spirit) - 2
                && fills_left - i32::from(s.sp) > self.reserve
                && i32::from(s.tier) <= self.fuel_max_tier
            {
                return Train;
            }
            if glory_left > 0 && despair_left > 0 {
                if s.tier <= self.glory_max {
                    return Glory;
                }
                if s.tier >= self.despair_min {
                    return Despair;
                }
                return pick(legal, [Train, Glory, Despair]);
            }
            if glory_left > 0 {
                return if s.tier > self.glory_stall && legal.contains(&Train) { Train } else { Glory };
            }
            if s.tier < self.despair_stall && legal.contains(&Train) { Train } else { Despair }
        }
    }
}

/// The comparison table, and where the fuel clause and the glory/despair
/// split are best set.
pub fn report(level: i64, amp_glory: f64, amp_despair: f64) -> Result<String, String> {
    let cfg = Config::for_level(level, Strategy::weighted(amp_glory, amp_despair))?;
    let mut solver = Solver::new(cfg);
    let best = solver.analyse(None);
    let mut out = String::new();

    let _ = writeln!(out, "Inheritor level {}: hand-written rules vs the exact optimum", cfg.level);
    let _ = writeln!(
        out,
        "  {:>27} | {:>9} {:>13} {:>9} {:>9} | {:>10}",
        "policy", "E[glory]", "E[desp succ]", "E[amp]", "P(wipe)", "vs optimal"
    );
    type Named = (&'static str, Box<dyn Fn(State, &[Action]) -> Action>);
    let policies: Vec<Named> = vec![
        ("glory bar first (baseline)", Box::new(always_glory_first())),
        ("tier split", Box::new(tier_split(2))),
        ("tier split + fuel", Box::new(tier_split_with_fuel(cfg, 2, -1))),
        ("old rule (tuned @ level 1)", Box::new(FullRule::level1_tuned().policy(cfg))),
        ("FULL RULE (tuned @ level 7)", Box::new(FullRule::default().policy(cfg))),
    ];
    let mut rows = vec![("OPTIMAL (lookup table)", best.clone())];
    for (name, policy) in &policies {
        rows.push((name, solver.analyse(Some(policy.as_ref()))));
    }
    for (name, a) in &rows {
        let gap = a.e_amplification - best.e_amplification;
        let gap_s = if gap == 0.0 { "-".to_string() } else { format!("{gap:+.3}%") };
        let _ = writeln!(
            out,
            "  {name:>27} | {:>9.4} {:>13.4} {:>8.3}% {:>8.4}% | {gap_s:>10}",
            a.e_glory,
            a.e_despair_success,
            a.e_amplification,
            100.0 * a.p_dead_end
        );
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "  where the fuel clause is gated (the rest of the rule held fixed)");
    let _ = writeln!(out, "  {:>19} | {:>9} {:>9}", "mental training at", "E[amp]", "P(wipe)");
    for f in -1..TIERS.len() as i32 {
        let rule = FullRule { fuel_max_tier: f, ..FullRule::default() };
        let a = solver.analyse(Some(&rule.policy(cfg)));
        let name = match f {
            -1 => "never".to_string(),
            4 => "any rate".to_string(),
            _ => format!("{} or better", pct(TIERS[f as usize], 0)),
        };
        let star = if f == 2 { "  <== default" } else { "" };
        let _ =
            writeln!(out, "  {name:>19} | {:>8.3}% {:>8.4}%{star}", a.e_amplification, 100.0 * a.p_dead_end);
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "  where the glory/despair split sits (despair takes everything below)");
    let _ = writeln!(out, "  {:>19} | {:>9} {:>9}", "attempt glory up to", "E[amp]", "P(wipe)");
    for split in 0..TIERS.len() as u8 {
        let rule = FullRule { glory_max: split, despair_min: split + 1, ..FullRule::default() };
        let a = solver.analyse(Some(&rule.policy(cfg)));
        let star = if split == 1 { "  <== default" } else { "" };
        let _ = writeln!(
            out,
            "  {:>18} | {:>8.3}% {:>8.4}%{star}",
            pct(TIERS[split as usize], 0),
            a.e_amplification,
            100.0 * a.p_dead_end
        );
    }
    Ok(out)
}
