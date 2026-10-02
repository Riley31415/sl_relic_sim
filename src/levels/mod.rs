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

use crate::economy::{
    CONVERT_FROM, CONVERT_TO, DIAMONDS_PER_SUMMON, RELIC_TYPES, RELICS_PER_ATTEMPT, RELICS_PER_SUMMON,
};
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
pub const MAX_LEVEL: usize = 20;
pub const SEED: u64 = 20260922;
/// Greedy mode keeps going past the top level: the crit relic farmed to each
/// of these amplifications, each tier a separate farm from level 20 played
/// for its own mark.
pub const GREEDY_TIERS: [u8; 4] = [43, 46, 48, 50];

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
        total(71, 18),
        total(73, 17),
        // Abyss's Water Drop, Eye of Typhoon, Emperor Ring on the sheet: the
        // three relics no earlier level names
        relics(&[9, 10, 11], 7, Some(2)),
        total(80, 20),
        total(83, 20),
        total(85, 20),
        total(89, 21),
        total(91, 20),
        total(94, 19),
        total(96, 16),
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
                convert_from: CONVERT_FROM,
                convert_to: CONVERT_TO,
            },
            quests,
            pity_needed: (0..top).map(|goal| if goal == 0 { 0 } else { pity_needed(goal) }).collect(),
            pity_gain: (0..top).map(pity_gain).collect(),
        }
    }

    /// The next level after `level` that names its relics, and those relics.
    /// Banking keeps them out of the work before it, so their stock is still
    /// there when that level comes.
    pub fn next_named(&self, level: usize) -> Option<(usize, RelicSet)> {
        (level + 1..self.quests.len()).find_map(|lvl| match &self.quests[lvl] {
            Some(Quest::Relics { which, .. }) => Some((lvl, set_of(which))),
            _ => None,
        })
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
                || vec![0; options.max_level + options.tiers.len()],
                |a, b| Ok(a.iter().zip(&b).map(|(x, y)| x + y).collect()),
            )?;
        Ok(sums.into_iter().map(|s| s as f64 / runs.max(1) as f64).collect())
    }
}

impl RunOptions {
    pub fn to(max_level: usize) -> Self {
        RunOptions { max_level, start_stock: vec![0; N_RELICS], pity: true, tiers: Vec::new() }
    }

    /// Farm the crit relic to each of `tiers` once `max_level` is reached.
    pub fn with_tiers(mut self, tiers: &[u8]) -> Self {
        self.tiers = tiers.to_vec();
        self
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

/// The inheritor's own damage multiplier: +5% for each of levels 2-7, +10%
/// for 8-15, +15% for 16-19 and +20% for 20 (30% at level 7, 110% at 15,
/// 170% at 19, 190% at 20).
pub fn level_multiplier(level: usize) -> f64 {
    (2..=level)
        .map(|lvl| match lvl {
            ..=7 => 0.05,
            8..=15 => 0.10,
            16..=19 => 0.15,
            _ => 0.20,
        })
        .sum()
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
    /// keep the relics the next named-relic level needs out of this level's
    /// work and filler
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

    /// A compact code for the saved plan files: "R,c,s,b,f41" for repair,
    /// keep gap-closers, score, bank, filler 4/1; "B52" for a 5/2 bar; "-" for
    /// the default.  `prefer` is not a search knob and has no code.
    pub fn code(&self) -> String {
        let mut parts = Vec::new();
        match self.profile {
            Some(Profile::Repair) => parts.push("R".to_string()),
            Some(Profile::Bar(g, d)) => parts.push(format!("B{g}{d}")),
            None => {}
        }
        for (on, flag) in [(self.closer, "c"), (self.score, "s"), (self.bank, "b")] {
            if on {
                parts.push(flag.to_string());
            }
        }
        if let Some((g, d)) = self.filler {
            parts.push(format!("f{g}{d}"));
        }
        if parts.is_empty() { "-".to_string() } else { parts.join(",") }
    }

    pub fn parse(code: &str) -> Result<StepChoice, String> {
        let digits = |s: &str| -> Result<(u8, u8), String> {
            let b = s.as_bytes();
            match b {
                [g, d] if g.is_ascii_digit() && d.is_ascii_digit() => Ok((g - b'0', d - b'0')),
                _ => Err(format!("bad bar '{s}' in step code '{code}'")),
            }
        };
        let mut choice = StepChoice::default();
        for part in code.split(',').map(str::trim).filter(|p| !p.is_empty() && *p != "-") {
            match part {
                "R" => choice.profile = Some(Profile::Repair),
                "c" => choice.closer = true,
                "s" => choice.score = true,
                "b" => choice.bank = true,
                _ if part.starts_with('B') => {
                    let (g, d) = digits(&part[1..])?;
                    choice.profile = Some(Profile::Bar(g, d));
                }
                _ if part.starts_with('f') => choice.filler = Some(digits(&part[1..])?),
                _ => return Err(format!("unknown part '{part}' in step code '{code}'")),
            }
        }
        Ok(choice)
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
            parts.push("bank the next named relics".to_string());
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

    /// "4=B51,c,s; 6=R,c,s": every level that is not the default.
    pub fn code(&self) -> String {
        self.steps
            .iter()
            .filter(|(_, c)| **c != StepChoice::default())
            .map(|(level, c)| format!("{level}={}", c.code()))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn parse(code: &str) -> Result<Plan, String> {
        let mut steps = BTreeMap::new();
        for step in code.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let (level, choice) = step.split_once('=').ok_or(format!("no '=' in plan step '{step}'"))?;
            let level: usize =
                level.trim().parse().map_err(|_| format!("bad level in plan step '{step}'"))?;
            steps.insert(level, StepChoice::parse(choice)?);
        }
        Ok(Plan { steps })
    }

    /// The same plan with nothing past `level`.
    pub fn up_to(&self, level: usize) -> Plan {
        Plan { steps: self.steps.range(..=level).map(|(&l, &c)| (l, c)).collect() }
    }

    /// This plan with one level's decisions replaced.
    pub fn with(&self, level: usize, choice: StepChoice) -> Plan {
        let mut steps = self.steps.clone();
        steps.insert(level, choice);
        Plan { steps }
    }

    /// Relics banked while working on `level`.
    pub fn protected(&self, game: &Game, level: usize) -> RelicSet {
        match game.next_named(level) {
            Some((_, relics)) if self.step(level).bank => relics,
            _ => 0,
        }
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
                (6, total(Repair)),
                (7, StepChoice::build(B(4, 2)).score()),
                (8, total(Repair).with_filler((4, 1))),
                (10, total(B(5, 2))),
                (11, total(Repair).with_filler((5, 2))),
                (12, total(Repair).with_filler((5, 2))),
                (14, total(Repair).with_filler((4, 1))),
                (15, total(Repair).with_filler((4, 2))),
                (16, total(Repair).with_filler((4, 2))),
                (17, total(Repair).with_filler((4, 1))),
                (18, total(Repair).with_filler((4, 2))),
                (19, StepChoice::build(Repair).closer().with_filler((4, 2))),
                (20, StepChoice::build(Repair).closer().with_filler((4, 2))),
            ]),
        ),
        // every level-up as cheap as it can be on its own (`--greedy`)
        (
            "greedy",
            Plan::new([
                (2, StepChoice::filler((4, 2))),
                (3, StepChoice::filler((4, 2))),
                (4, total(B(5, 2))),
                (5, StepChoice::filler((4, 2))),
                (6, total(B(4, 1)).with_filler((5, 2))),
                (7, StepChoice::build(B(4, 2)).with_filler((4, 2))),
                (8, total(Repair).with_filler((4, 1))),
                (9, StepChoice::filler((4, 2))),
                (10, total(Repair).with_filler((4, 1))),
                (11, total(Repair).with_filler((5, 2))),
                (12, total(Repair).with_filler((5, 2))),
                (13, StepChoice::filler((4, 2))),
                (14, total(Repair).with_filler((4, 1))),
                (15, total(Repair).with_filler((4, 2))),
                (16, total(Repair).with_filler((4, 2))),
                (17, total(Repair).with_filler((4, 1))),
                (18, total(Repair).with_filler((4, 2))),
                (19, StepChoice::build(Repair).closer().with_filler((4, 2))),
                (20, total(Repair).with_filler((4, 2))),
            ]),
        ),
        // 4/2 everywhere, all-or-nothing, summon whenever stuck
        ("minimal", Plan::default()),
    ]
}

/// What a look-ahead plan is searched for: reaching `level`, or, with a
/// tier, farming the crit relic to that mark once the top level is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Target {
    pub level: usize,
    pub tier: Option<u8>,
}

impl Target {
    pub const fn level(level: usize) -> Self {
        Target { level, tier: None }
    }

    pub const fn tier(mark: u8) -> Self {
        Target { level: MAX_LEVEL, tier: Some(mark) }
    }

    /// "12", or "20+41" for a tier.
    pub fn code(&self) -> String {
        match self.tier {
            Some(mark) => format!("{}+{mark}", self.level),
            None => self.level.to_string(),
        }
    }

    pub fn parse(code: &str) -> Result<Target, String> {
        let number = |s: &str| s.trim().parse().map_err(|_| format!("bad target '{code}'"));
        match code.split_once('+') {
            Some((level, mark)) => Ok(Target { level: number(level)?, tier: Some(number(mark)? as u8) }),
            None => Ok(Target::level(number(code)?)),
        }
    }

    /// The runs this target is simulated with; its result is the last row.
    pub fn options(&self) -> RunOptions {
        let options = RunOptions::to(self.level);
        match self.tier {
            Some(mark) => options.with_tiers(&[mark]),
            None => options,
        }
    }
}

/// Every look-ahead target: each level from 2 to the top, then greedy's tiers.
pub fn targets() -> Vec<Target> {
    (2..=MAX_LEVEL).map(Target::level).chain(GREEDY_TIERS.iter().map(|&m| Target::tier(m))).collect()
}

/// The look-ahead plan for each target, in `targets()` order: the one saved
/// by `relic levels --lookahead-all` (lookahead_plans.txt), or for a target
/// not searched yet the level-20 look-ahead plan cut down to it.
pub fn lookahead_plans() -> Result<Vec<(Target, Plan)>, String> {
    let saved = parse_plans(include_str!("lookahead_plans.txt"))?;
    let fallback = strategy("lookahead").expect("a named plan");
    Ok(targets()
        .into_iter()
        .map(|target| {
            let plan = saved.iter().find(|(t, _)| *t == target).map(|(_, p)| p.clone());
            (target, plan.unwrap_or_else(|| fallback.up_to(target.level)))
        })
        .collect())
}

/// Lines of "target: plan code"; blank lines and # comments skipped.
pub fn parse_plans(text: &str) -> Result<Vec<(Target, Plan)>, String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let (target, plan) = line.split_once(':').ok_or(format!("no ':' in '{line}'"))?;
            Ok((Target::parse(target)?, Plan::parse(plan)?))
        })
        .collect()
}

pub fn format_plans(plans: &[(Target, Plan)]) -> String {
    let mut out = String::from(
        "# The look-ahead plan for each target: the cheapest route to that level, or\n# to level 20 plus a crit relic tier.  Generated by\n# `relic levels --lookahead-all src/levels/lookahead_plans.txt`.\n",
    );
    for (target, plan) in plans {
        out.push_str(&format!("{}: {}\n", target.code(), plan.code()));
    }
    out
}

pub fn strategy(name: &str) -> Option<Plan> {
    strategies().into_iter().find(|(n, _)| *n == name).map(|(_, plan)| plan)
}

/// The crit relic tiers a named plan goes on to farm at the top level.
pub fn tiers(name: &str) -> &'static [u8] {
    if name == "greedy" { &GREEDY_TIERS } else { &[] }
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
    if game.next_named(level).is_some() {
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

/// Mean diamonds to reach `target` from level 1.
pub fn target_cost(game: &Game, plan: &Plan, runs: u64, seed: u64, target: Target) -> Result<f64, String> {
    Ok(*game.mean_costs(plan, runs, seed, &target.options())?.last().expect("at least level 1"))
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
    /// for a look-ahead: also farm the crit relic to this mark at `max_level`,
    /// and score plans on the whole cost
    pub tier: Option<u8>,
}

impl SearchOptions {
    pub fn greedy() -> Self {
        SearchOptions { runs: 10_000, final_runs: 50_000, seed: 777, max_level: MAX_LEVEL, tier: None }
    }

    pub fn lookahead() -> Self {
        SearchOptions { runs: 10_000, final_runs: 50_000, seed: 4242, max_level: MAX_LEVEL, tier: None }
    }

    /// The look-ahead settings, aimed at one target.
    pub fn per_target(target: Target) -> Self {
        SearchOptions { max_level: target.level, tier: target.tier, ..SearchOptions::lookahead() }
    }

    fn target(&self) -> Target {
        Target { level: self.max_level, tier: self.tier }
    }
}

/// One level of a greedy search.
#[derive(Clone, Debug)]
pub struct GreedyRow {
    pub level: usize,
    pub choice: StepChoice,
    pub cost: f64,
    pub runner_up: Option<(StepChoice, f64)>,
    /// what banking the next named relics would have cost this step
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

/// The cheapest plan to `max_level` (and its tier, if any) overall, by
/// coordinate descent from
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
                                target_cost(game, &plan.with(level, t), o.runs, o.seed + sweep, o.target())?,
                                i,
                            ))
                        })
                        .collect::<Result<_, String>>()?,
                );
                let best = trials[scored[0].1];
                let check = o.seed + 1000 + sweep;
                let now = target_cost(game, &plan, o.final_runs, check, o.target())?;
                let new = target_cost(game, &plan.with(level, best), o.final_runs, check, o.target())?;
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
    let cost = target_cost(game, &plan, o.final_runs, o.seed + 2000, o.target())?;
    Ok((plan, cost))
}

/// A look-ahead search for every target in turn, each starting from whichever
/// of the saved greedy and look-ahead plans reaches that target cheaper, and
/// trimmed to the levels it plays.  `log` hears each target as it finishes.
pub fn lookahead_all(
    game: &Game,
    only: Option<&[Target]>,
    log: &dyn Fn(&str),
) -> Result<Vec<(Target, Plan, f64)>, String> {
    let starts = [strategy("greedy").expect("a named plan"), strategy("lookahead").expect("a named plan")];
    let mut out = Vec::new();
    for target in targets().into_iter().filter(|t| only.is_none_or(|o| o.contains(t))) {
        let o = SearchOptions::per_target(target);
        let mut best: Option<(f64, &Plan)> = None;
        for start in &starts {
            let cost = target_cost(game, start, o.final_runs, o.seed + 3000, target)?;
            if best.is_none_or(|(c, _)| cost < c) {
                best = Some((cost, start));
            }
        }
        let start = best.expect("two starts").1.up_to(target.level);
        let (plan, cost) = lookahead_search(game, &start, o, &|_| {})?;
        let plan = plan.up_to(target.level);
        log(&format!("{}: {}  {}", target.code(), crate::format::commas(cost, 0), plan.code()));
        out.push((target, plan, cost));
    }
    Ok(out)
}
