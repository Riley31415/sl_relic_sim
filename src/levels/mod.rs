//! Raising the inheritor: the game's rules, a player's plan, the Monte Carlo
//! that plays plans out, and the searches that find the cheapest ones.
//!
//! Levelling needs the twelve relics to hit stated glory/despair conditions -
//! or a full pity meter - and a higher inheritor level gives the relics more
//! memory slots and spirit power.  Everything is simulated: the summon stream,
//! every inheritance attempt (drawn from the exact solver's outcome tables),
//! and every keep-or-discard decision.

pub mod report;
mod rules;
mod sim;
mod tables;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use rayon::prelude::*;

pub use rules::{Bar, Quest, Relic, RelicSet, contains, members, set_of};
pub use sim::{Economy, Game, LevelUp, Run, RunOptions};

use crate::economy::{DIAMONDS_PER_SUMMON, RELIC_TYPES, RELICS_PER_ATTEMPT, RELICS_PER_SUMMON};
use crate::rng::Rng;
use rules::LevelRules;
use tables::LocalTables;

pub const RELICS: [&str; 12] = [
    "Giant's Right Hand",
    "Demon Eye of Weakness",
    "Oath of Immortality",
    "Sacred Tree of Rebirth",
    "Ring of Lightning",
    "Golden Star",
    "Seal of the Legendary Archer",
    "Veil of the Night",
    "Spark of Eternity",
    "Mermaid's Tear",
    "Eye of the Sky",
    "Crown of the Great Mountain",
];
/// Two-word names for tables, in the same order.
pub const RELIC_SHORT: [&str; 12] = [
    "Giant Hand",
    "Demon Eye",
    "Immortal Oath",
    "Sacred Tree",
    "Lightning Ring",
    "Golden Star",
    "Archer Seal",
    "Night Veil",
    "Eternal Spark",
    "Mermaid Tear",
    "Sky Eye",
    "Mountain Crown",
];
pub const N_RELICS: usize = RELICS.len();
/// Giant's Right Hand, the attack relic.
pub const ATK: usize = 0;
/// Demon Eye of Weakness, the crit relic.
pub const CRIT: usize = 1;
/// As far as the requirement table is known.
pub const MAX_LEVEL: usize = 10;
pub const SEED: u64 = 20260922;

/// What the inheritor needs in order to BE at each level; level 1 is free.
pub fn requirements() -> Vec<Option<Quest>> {
    let relics = |which: &[usize], glory, despair| {
        Some(Quest::Relics { which: which.to_vec(), bar: Bar::new(glory, despair) })
    };
    let total = |glory, despair| Some(Quest::Total { glory, despair });
    vec![
        None,
        None,
        relics(&[0], 3, None),
        relics(&[1, 2], 3, Some(2)),
        total(25, 21),
        relics(&[3, 4, 5], 5, Some(2)),
        total(44, 18),
        Some(Quest::Each(Bar::new(4, Some(2)))),
        total(59, 17),
        relics(&[6, 7, 8], 6, Some(2)),
        total(68, 19),
    ]
}

/// Pity: every attempt, on any relic, fills a meter; a full meter levels the
/// inheritor up without the quest.  It empties on every level-up.
pub fn pity_needed(goal: usize) -> u32 {
    500 * (goal as u32 - 1)
}

/// Pity one attempt adds at inheritor `level`: 100 at levels 1-3, 120 at 4-7,
/// 140 at 8-11, and +20 every four levels after (180 at 16-19).
pub fn pity_gain(level: usize) -> u32 {
    100 + 20 * (level as u32 / 4)
}

impl Game {
    /// The game as the requirement sheet has it.
    pub fn standard() -> &'static Game {
        static GAME: LazyLock<Game> = LazyLock::new(|| Game::with_quests(requirements()));
        &GAME
    }

    /// The standard economy and pity rules with a different requirement table.
    pub fn with_quests(quests: Vec<Option<Quest>>) -> Game {
        let top = quests.len();
        Game {
            economy: Economy {
                relics: RELIC_TYPES as usize,
                per_summon: RELICS_PER_SUMMON as usize,
                per_attempt: RELICS_PER_ATTEMPT,
                diamonds_per_summon: DIAMONDS_PER_SUMMON,
            },
            quests,
            pity_needed: (0..top).map(|goal| if goal == 0 { 0 } else { pity_needed(goal) }).collect(),
            pity_gain: (0..top).map(pity_gain).collect(),
        }
    }

    /// The last level that names its relics: level 9.  Banking keeps these
    /// relics out of earlier work so their stock is still there when it comes.
    pub fn bank_level(&self) -> usize {
        (0..self.quests.len())
            .rev()
            .find(|&lvl| matches!(self.quests[lvl], Some(Quest::Relics { .. })))
            .expect("some level names its relics")
    }

    pub fn banked_relics(&self) -> RelicSet {
        match &self.quests[self.bank_level()] {
            Some(Quest::Relics { which, .. }) => set_of(which),
            _ => unreachable!("the bank level names its relics"),
        }
    }

    /// Relics that a LATER level names outright.
    pub fn future_named(&self, level: usize) -> RelicSet {
        self.quests
            .iter()
            .skip(level + 1)
            .filter_map(|q| match q {
                Some(Quest::Relics { which, .. }) => Some(set_of(which)),
                _ => None,
            })
            .fold(0, |a, b| a | b)
    }

    pub fn max_level(&self) -> usize {
        self.quests.len() - 1
    }

    fn rules(&self, plan: &Plan) -> Vec<Option<LevelRules>> {
        (0..self.quests.len()).map(|lvl| (lvl >= 2).then(|| plan.rules(self, lvl))).collect()
    }

    /// `runs` players from level 1 to `options.max_level`, spread over every
    /// core.  Run i sees the same luck for a given seed under any plan, so
    /// plans compare run for run.
    pub fn simulate(
        &self,
        plan: &Plan,
        runs: u64,
        seed: u64,
        options: &RunOptions,
    ) -> Result<Vec<Run>, String> {
        if options.start_stock.len() != self.economy.relics {
            return Err(format!("start stock needs {} entries", self.economy.relics));
        }
        let rules = self.rules(plan);
        (0..runs)
            .into_par_iter()
            .map_init(LocalTables::default, |tables, i| {
                sim::run(self, &rules, options, (ATK, CRIT), tables, Rng::for_run(seed, i))
            })
            .collect()
    }

    /// Mean diamonds spent to reach each level from 1 to `options.max_level`
    /// (index 0 is level 1), without keeping every run.  Summed as whole
    /// diamonds, so the answer never depends on how the runs were split.
    pub fn mean_costs(
        &self,
        plan: &Plan,
        runs: u64,
        seed: u64,
        options: &RunOptions,
    ) -> Result<Vec<f64>, String> {
        let rules = self.rules(plan);
        let sums = (0..runs)
            .into_par_iter()
            .map_init(LocalTables::default, |tables, i| {
                sim::run(self, &rules, options, (ATK, CRIT), tables, Rng::for_run(seed, i))
                    .map(|run| run.levels.iter().map(|l| l.diamonds).collect::<Vec<_>>())
            })
            .try_reduce(
                || vec![0; options.max_level],
                |a, b| Ok(a.iter().zip(&b).map(|(x, y)| x + y).collect()),
            )?;
        Ok(sums.into_iter().map(|s| s as f64 / runs.max(1) as f64).collect())
    }
}

impl RunOptions {
    pub fn to(max_level: usize) -> Self {
        RunOptions { max_level, start_stock: vec![0; N_RELICS], pity: true }
    }

    /// The same number of relics of every type on hand at the start.
    pub fn with_stock(mut self, each: u32) -> Self {
        self.start_stock = vec![each; N_RELICS];
        self
    }
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions::to(MAX_LEVEL)
    }
}

// ------------------------------------------------------------------ damage

/// A relic's amplification in percent: +5% a glory, -2% a despair, floored at 0.
pub fn amplification((g, d): Relic) -> f64 {
    (5.0 * f64::from(g) - 2.0 * f64::from(d)).max(0.0)
}

/// The inheritor's own damage multiplier: +5% for each of levels 2-7, then
/// +10% for each level from 8 (50% at level 9, 60% at level 10).
pub fn level_multiplier(level: usize) -> f64 {
    0.05 * (level as f64 - 1.0).min(6.0) + 0.10 * (level as f64 - 7.0).max(0.0)
}

/// Damage relative to a level-1 inheritor with bare relics (1.0 = no gain);
/// the amps are in percent.
pub fn damage(level: usize, crit_amp: f64, atk_amp: f64) -> f64 {
    (1.0 + level_multiplier(level))
        * (1.0 + 4.0 * crit_amp / 100.0 / 5.0)
        * (1.0 + 22.0 * atk_amp / 100.0 / 209.0)
}

// ------------------------------------------------------------------ plans

/// What each relic is rolled toward on an "each" or "total" level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Profile {
    /// a (glory, despair) bar
    Bar(u8, u8),
    /// single-step fixes to the board as it stands
    Repair,
}

pub const DEFAULT_PROFILE: (u8, u8) = (4, 2);
pub const FILLERS: [Option<(u8, u8)>; 5] = [None, Some((4, 2)), Some((4, 1)), Some((5, 2)), Some((5, 1))];
pub const TOTAL_PROFILES: [Profile; 7] = [
    Profile::Bar(3, 2),
    Profile::Bar(3, 1),
    Profile::Bar(4, 2),
    Profile::Bar(4, 1),
    Profile::Bar(5, 2),
    Profile::Bar(5, 1),
    Profile::Repair,
];
pub const EACH_PROFILES: [Profile; 4] =
    [Profile::Bar(4, 2), Profile::Bar(4, 1), Profile::Bar(5, 2), Profile::Bar(5, 1)];

/// Everything a player decides for one level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StepChoice {
    /// None: the default 4/2 (a level that names its relics always rolls them
    /// to its own bar)
    pub profile: Option<Profile>,
    /// on a total level, also keep rolls that close the board's gap
    pub closer: bool,
    /// play attempts for max glory + min despair, not all-or-nothing
    pub score: bool,
    /// keep the level-9 relics out of this level's work and filler
    pub bank: bool,
    /// when nothing the quest needs is affordable: None to summon, or a bar to
    /// roll the best-stocked spare relic toward, for the pity
    pub filler: Option<(u8, u8)>,
    /// which relics the quest work goes to first, and whether the others wait
    /// while any of them still needs work ("hard") or not ("soft")
    pub prefer: Option<(RelicSet, bool)>,
}

impl StepChoice {
    /// A total or each level rolled toward `profile`.
    pub fn build(profile: Profile) -> Self {
        StepChoice { profile: Some(profile), ..Self::default() }
    }

    pub fn filler(bar: (u8, u8)) -> Self {
        StepChoice { filler: Some(bar), ..Self::default() }
    }

    pub fn closer(self) -> Self {
        StepChoice { closer: true, ..self }
    }

    pub fn score(self) -> Self {
        StepChoice { score: true, ..self }
    }

    pub fn bank(self) -> Self {
        StepChoice { bank: true, ..self }
    }

    pub fn with_filler(self, bar: (u8, u8)) -> Self {
        StepChoice { filler: Some(bar), ..self }
    }

    pub fn label(&self) -> String {
        let bar = |(g, d): (u8, u8)| report::bar_text(g, Some(d));
        let mut parts = match self.profile {
            None => vec!["roll the named relics to the bar".to_string()],
            Some(profile) => {
                let mut parts = vec![match profile {
                    Profile::Repair => "repair".to_string(),
                    Profile::Bar(g, d) => bar((g, d)),
                }];
                parts.push(if self.score { "max glory + min despair" } else { "all-or-nothing" }.to_string());
                if self.closer {
                    parts.push("keep gap-closers".to_string());
                }
                parts
            }
        };
        parts.push(match self.filler {
            Some(f) => format!("filler to {}", bar(f)),
            None => "no filler".to_string(),
        });
        if self.bank {
            parts.push("bank level-9 relics".to_string());
        }
        parts.join(", ")
    }
}

/// A player's decisions: one StepChoice per level (the default for any level
/// left out).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub steps: BTreeMap<usize, StepChoice>,
}

impl Plan {
    pub fn new(steps: impl IntoIterator<Item = (usize, StepChoice)>) -> Self {
        Plan { steps: steps.into_iter().collect() }
    }

    pub fn step(&self, level: usize) -> StepChoice {
        self.steps.get(&level).copied().unwrap_or_default()
    }

    /// This plan with one level's decisions replaced.
    pub fn with(&self, level: usize, choice: StepChoice) -> Plan {
        let mut steps = self.steps.clone();
        steps.insert(level, choice);
        Plan { steps }
    }

    /// Relics banked while working on `level`.
    pub fn protected(&self, game: &Game, level: usize) -> RelicSet {
        if self.step(level).bank { game.banked_relics() & game.future_named(level) } else { 0 }
    }

    /// This level's decisions as the simulation takes them.
    fn rules(&self, game: &Game, level: usize) -> LevelRules {
        let s = self.step(level);
        let (g, d) = match s.profile {
            Some(Profile::Bar(g, d)) => (g, d),
            _ => DEFAULT_PROFILE,
        };
        LevelRules {
            base: Bar::new(g, Some(d)),
            surgical: s.profile == Some(Profile::Repair),
            closer: s.closer,
            score: s.score.then_some((1.0, 1.0)),
            filler: s.filler.map(|(g, d)| Bar::new(g, Some(d))),
            locked: self.protected(game, level),
            prefer: s.prefer,
        }
    }
}

/// The two plans LEVELS.md is about, found by the searches below with the
/// pity rule on, and a do-the-minimum baseline.
pub fn strategies() -> Vec<(&'static str, Plan)> {
    use Profile::{Bar as B, Repair};
    let total = |p: Profile| StepChoice::build(p).closer().score();
    vec![
        // the cheapest route to the top level overall (`--lookahead`)
        (
            "lookahead",
            Plan::new([
                (4, total(B(5, 1))),
                (5, StepChoice::filler((4, 2))),
                (6, total(Repair)),
                (7, StepChoice::build(B(4, 2)).score()),
                (8, total(Repair).with_filler((4, 1))),
                (9, StepChoice::filler((5, 2))),
                (10, total(Repair).with_filler((4, 1))),
            ]),
        ),
        // every level-up as cheap as it can be on its own (`--greedy`)
        (
            "greedy",
            Plan::new([
                (2, StepChoice::filler((4, 2))),
                (3, StepChoice::filler((4, 2))),
                (4, total(B(5, 1))),
                (5, StepChoice::filler((4, 2))),
                (6, total(B(5, 1)).with_filler((4, 2))),
                (7, StepChoice::build(B(4, 2)).score().with_filler((4, 2))),
                (8, total(Repair).with_filler((4, 1))),
                (9, StepChoice::filler((4, 2))),
                (10, total(Repair).with_filler((4, 1))),
            ]),
        ),
        // 4/2 everywhere, all-or-nothing, summon whenever stuck
        ("minimal", Plan::default()),
    ]
}

pub fn strategy(name: &str) -> Option<Plan> {
    strategies().into_iter().find(|(n, _)| *n == name).map(|(_, plan)| plan)
}

/// Every StepChoice worth trying for `level` (banking aside).  A level that
/// names its relics only decides its filler: all-or-nothing is the best way
/// to clear a bar, and no other relic is part of its quest.
pub fn step_choices(game: &Game, level: usize) -> Vec<StepChoice> {
    let with = |profile: Option<Profile>, closer, score, filler| StepChoice {
        profile,
        closer,
        score,
        filler,
        ..StepChoice::default()
    };
    let mut out = Vec::new();
    match game.quests[level].as_ref().expect("a real level") {
        Quest::Relics { .. } => out.extend(FILLERS.map(|f| with(None, false, false, f))),
        Quest::Each(_) => {
            for p in EACH_PROFILES {
                for s in [false, true] {
                    out.extend(FILLERS.map(|f| with(Some(p), false, s, f)));
                }
            }
        }
        Quest::Total { .. } => {
            for p in TOTAL_PROFILES {
                for c in [false, true] {
                    for s in [false, true] {
                        out.extend(FILLERS.map(|f| with(Some(p), c, s, f)));
                    }
                }
            }
        }
    }
    out
}

/// The knobs a level has, for a descent that turns one knob at a time.
pub fn knobs(game: &Game, level: usize) -> Vec<&'static str> {
    let mut knobs = vec!["filler"];
    if game.future_named(level) & game.banked_relics() != 0 {
        knobs.push("bank");
    }
    match game.quests[level] {
        Some(Quest::Each(_)) => knobs.extend(["profile", "score"]),
        Some(Quest::Total { .. }) => knobs.extend(["profile", "closer", "score"]),
        _ => {}
    }
    knobs
}

/// `current` with one knob turned to each other value it can take.
pub fn turn(game: &Game, level: usize, knob: &str, current: StepChoice) -> Vec<StepChoice> {
    let profiles: &[Profile] = match game.quests[level] {
        Some(Quest::Each(_)) => &EACH_PROFILES,
        _ => &TOTAL_PROFILES,
    };
    let both = [false, true];
    let values: Vec<StepChoice> = match knob {
        "filler" => FILLERS.iter().map(|&f| StepChoice { filler: f, ..current }).collect(),
        "bank" => both.iter().map(|&b| StepChoice { bank: b, ..current }).collect(),
        "profile" => profiles.iter().map(|&p| StepChoice { profile: Some(p), ..current }).collect(),
        "closer" => both.iter().map(|&c| StepChoice { closer: c, ..current }).collect(),
        "score" => both.iter().map(|&s| StepChoice { score: s, ..current }).collect(),
        other => unreachable!("no knob called {other}"),
    };
    values.into_iter().filter(|&v| v != current).collect()
}

// ------------------------------------------------------------------ searches

/// Mean diamonds from level 1 to `max_level`.
pub fn total_cost(game: &Game, plan: &Plan, runs: u64, seed: u64, max_level: usize) -> Result<f64, String> {
    Ok(*game.mean_costs(plan, runs, seed, &RunOptions::to(max_level))?.last().expect("at least level 1"))
}

/// Mean diamonds spent on the step from level - 1 to `level` alone.
pub fn step_cost(game: &Game, plan: &Plan, level: usize, runs: u64, seed: u64) -> Result<f64, String> {
    let costs = game.mean_costs(plan, runs, seed, &RunOptions::to(level))?;
    Ok(costs[level - 1] - costs[level - 2])
}

/// How hard a search looks.
#[derive(Clone, Copy, Debug)]
pub struct SearchOptions {
    /// runs to screen every option
    pub runs: u64,
    /// fresh runs to confirm the best before it is kept
    pub final_runs: u64,
    pub seed: u64,
    pub max_level: usize,
}

impl SearchOptions {
    pub fn greedy() -> Self {
        SearchOptions { runs: 10_000, final_runs: 50_000, seed: 777, max_level: MAX_LEVEL }
    }

    pub fn lookahead() -> Self {
        SearchOptions { runs: 10_000, final_runs: 50_000, seed: 4242, max_level: MAX_LEVEL }
    }
}

/// One level of a greedy search.
#[derive(Clone, Debug)]
pub struct GreedyRow {
    pub level: usize,
    pub choice: StepChoice,
    pub cost: f64,
    pub runner_up: Option<(StepChoice, f64)>,
    /// what banking the level-9 relics would have cost this step
    pub bank_cost: Option<f64>,
}

fn ranked(scores: Vec<(f64, usize)>) -> Vec<(f64, usize)> {
    let mut scores = scores;
    scores.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    scores
}

/// Make each level as cheap as possible on its own, in order.
///
/// Level by level, every StepChoice is scored on the cost of that step alone,
/// from the board the greedy choices so far leave behind, and the cheapest is
/// locked in; what a choice does to later levels is ignored.  The closest
/// three are re-run on more, fresh runs before one is picked, and every
/// candidate sees the same luck, so the ranking is not a coin toss.
pub fn greedy_search(
    game: &Game,
    o: SearchOptions,
    log: &dyn Fn(&str),
) -> Result<(Plan, Vec<GreedyRow>), String> {
    let mut plan = Plan::default();
    let mut report = Vec::new();
    for level in 2..=o.max_level {
        let choices = step_choices(game, level);
        let score = |i: usize, runs, seed| step_cost(game, &plan.with(level, choices[i]), level, runs, seed);
        let mut scored = ranked(
            (0..choices.len()).map(|i| Ok((score(i, o.runs, o.seed)?, i))).collect::<Result<_, String>>()?,
        );
        if scored.len() > 1 {
            scored = ranked(
                scored
                    .iter()
                    .take(3)
                    .map(|&(_, i)| Ok((score(i, o.final_runs, o.seed + 1)?, i)))
                    .collect::<Result<_, String>>()?,
            );
        }
        let (cost, best) = scored[0];
        plan = plan.with(level, choices[best]);
        let bank_cost = match game.quests[level] {
            Some(Quest::Total { .. }) if game.future_named(level) != 0 => Some(step_cost(
                game,
                &plan.with(level, choices[best].bank()),
                level,
                o.final_runs,
                o.seed + 1,
            )?),
            _ => None,
        };
        log(&format!("L{level}: {}  {}", choices[best].label(), crate::format::commas(cost, 0)));
        report.push(GreedyRow {
            level,
            choice: choices[best],
            cost,
            runner_up: scored.get(1).map(|&(c, i)| (choices[i], c)),
            bank_cost,
        });
    }
    Ok((plan, report))
}

/// The cheapest plan to `max_level` overall, by coordinate descent from
/// `start`: every knob of every level is turned in turn, each value scored on
/// the total cost, and a change is kept only if it still wins by 0.5% on a
/// larger, fresh set of paired runs.  Stops when a sweep changes nothing.
pub fn lookahead_search(
    game: &Game,
    start: &Plan,
    o: SearchOptions,
    log: &dyn Fn(&str),
) -> Result<(Plan, f64), String> {
    const SWEEPS: u64 = 4;
    let mut plan = start.clone();
    for sweep in 0..SWEEPS {
        let mut changed = false;
        for level in 2..=o.max_level {
            for knob in knobs(game, level) {
                let trials = turn(game, level, knob, plan.step(level));
                let scored = ranked(
                    trials
                        .iter()
                        .enumerate()
                        .map(|(i, &t)| {
                            Ok((
                                total_cost(game, &plan.with(level, t), o.runs, o.seed + sweep, o.max_level)?,
                                i,
                            ))
                        })
                        .collect::<Result<_, String>>()?,
                );
                let best = trials[scored[0].1];
                let check = o.seed + 1000 + sweep;
                let now = total_cost(game, &plan, o.final_runs, check, o.max_level)?;
                let new = total_cost(game, &plan.with(level, best), o.final_runs, check, o.max_level)?;
                if new < now * 0.995 {
                    log(&format!(
                        "sweep {sweep} L{level} {knob}: {} -> {}  {} -> {}",
                        plan.step(level).label(),
                        best.label(),
                        crate::format::commas(now, 0),
                        crate::format::commas(new, 0)
                    ));
                    plan = plan.with(level, best);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let cost = total_cost(game, &plan, o.final_runs, o.seed + 2000, o.max_level)?;
    Ok((plan, cost))
}
