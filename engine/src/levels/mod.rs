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

pub use rules::{Bar, Quest, Relic, RelicSet, contains, members, set_of, totals};
pub use sim::{Economy, Game, LevelUp, Move, Run, RunOptions};
pub use tables::{TableKey, expected_closing, outcomes};

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
/// The crit relic tiers a goal can go on to past the top level: the crit
/// relic farmed to this amplification at level 20.
pub const CRIT_TIERS: [u8; 4] = [43, 46, 48, 50];

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

    /// `runs` players under `plan` as they stand on reaching `level` (run i
    /// with the luck it has in `simulate`), to resume under plans that agree
    /// with `plan` on every level up to `level`.
    pub fn prefixes(
        &self,
        plan: &Plan,
        runs: u64,
        seed: u64,
        options: &RunOptions,
        level: usize,
    ) -> Result<Vec<sim::Partial<'_>>, String> {
        let rules = self.rules(plan);
        (0..runs)
            .into_par_iter()
            .map_init(LocalTables::default, |tables, i| {
                let mut partial = sim::start(self, options, (ATK, CRIT), Rng::for_run(seed, i));
                sim::climb(self, &rules, options, (ATK, CRIT), tables, &mut partial, level)?;
                Ok(partial)
            })
            .collect()
    }

    /// Mean diamonds to the end of `options` (the top level, or the tier),
    /// resuming `prefixes` under `plan` - the same as the whole runs would
    /// cost, when `plan` agrees with the prefixes' plan on the levels they
    /// played.
    pub fn mean_cost_from(&self, plan: &Plan, prefixes: &[sim::Partial], options: &RunOptions) -> Result<f64, String> {
        let rules = self.rules(plan);
        let sum = prefixes
            .par_iter()
            .map_init(LocalTables::default, |tables, partial| {
                let mut partial = partial.clone();
                sim::climb(self, &rules, options, (ATK, CRIT), tables, &mut partial, options.max_level)?;
                let run = sim::finish(options, (ATK, CRIT), tables, partial);
                Ok::<u64, String>(run.levels.last().expect("at least level 1").diamonds)
            })
            .try_reduce(|| 0u64, |a, b| Ok(a + b))?;
        Ok(sum as f64 / (prefixes.len().max(1)) as f64)
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
/// When a filler is rolled: always (None), or only when the spare relics on
/// hand can fill the pity bar in at most this many attempts.
pub const FILLER_WINDOWS: [Option<u8>; 4] = [None, Some(5), Some(15), Some(30)];
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
    /// keep the relics the next named-relic level needs out of this level's
    /// work and filler
    pub bank: bool,
    /// when nothing the quest needs is affordable: None to summon, or a bar to
    /// roll the best-stocked spare relic toward, for the pity
    pub filler: bool,
    /// with a filler: roll it only when the spare relics on hand can fill the
    /// pity bar in at most this many attempts - a level-up the stock can pay
    /// for outright - and summon otherwise (None: roll it whenever stuck)
    pub filler_within: Option<u8>,
    /// which relics the quest work goes to first, and whether the others wait
    /// while any of them still needs work ("hard") or not ("soft")
    pub prefer: Option<(RelicSet, bool)>,
    /// every attempt closes the gap to the totals ahead - the most glory and
    /// the least despair any totals level up to the target asks - rather
    /// than this level's own (`Game::horizon_for`)
    pub ahead: bool,
    /// a relic traded off its bar for a result the keep rule ranks as high
    /// counts as having met it, so the bars go further once every relic has
    /// (rather than the work waiting on it)
    pub trades: bool,
}

impl StepChoice {
    /// A total or each level rolled toward `profile`.
    pub fn build(profile: Profile) -> Self {
        StepChoice { profile: Some(profile), ..Self::default() }
    }

    pub fn filler() -> Self {
        StepChoice { filler: true, ..Self::default() }
    }

    pub fn bank(self) -> Self {
        StepChoice { bank: true, ..self }
    }

    pub fn with_filler(self) -> Self {
        StepChoice { filler: true, ..self }
    }

    /// The filler only when the spare stock fills the pity bar within `attempts`.
    pub fn within(self, attempts: u8) -> Self {
        StepChoice { filler_within: Some(attempts), ..self }
    }

    /// A compact code for the saved plan files: "R,a,b,t,f" for repair, the
    /// totals ahead, bank, trades as good as the bar, filler ("fw15": only
    /// when the spare stock fills the pity bar within 15 attempts); "B52"
    /// for a 5/2 bar; "-" for the default.
    /// `prefer` is not a search knob and has no code.
    pub fn code(&self) -> String {
        let mut parts = Vec::new();
        match self.profile {
            Some(Profile::Repair) => parts.push("R".to_string()),
            Some(Profile::Bar(g, d)) => parts.push(format!("B{g}{d}")),
            None => {}
        }
        for (on, flag) in [(self.ahead, "a"), (self.bank, "b"), (self.trades, "t")] {
            if on {
                parts.push(flag.to_string());
            }
        }
        if self.filler {
            parts.push(match self.filler_within {
                Some(n) => format!("fw{n}"),
                None => "f".to_string(),
            });
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
                // an older plan's "keep gap-closers": every roll that narrows the gap is kept now
                "c" => {}
                // an older plan's "s" (max glory + min despair): every attempt looks ahead now
                "s" => {}
                "a" => choice.ahead = true,
                "t" => choice.trades = true,
                "b" => choice.bank = true,
                _ if part.starts_with('B') => {
                    let (g, d) = digits(&part[1..])?;
                    choice.profile = Some(Profile::Bar(g, d));
                }
                _ if part.starts_with('f') => {
                    // an older plan's filler bar ("f42") no longer matters: a
                    // filler is kept as a quest roll is
                    let (bar, within) = part[1..].split_once('w').map_or((&part[1..], None), |(b, w)| (b, Some(w)));
                    if !bar.is_empty() {
                        digits(bar)?;
                    }
                    choice.filler = true;
                    choice.filler_within = match within {
                        Some(w) => Some(w.parse().map_err(|_| format!("bad window '{w}' in step code '{code}'"))?),
                        None => None,
                    };
                }
                _ => return Err(format!("unknown part '{part}' in step code '{code}'")),
            }
        }
        Ok(choice)
    }

    pub fn label(&self) -> String {
        let bar = |(g, d): (u8, u8)| report::bar_text(g, Some(d));
        let mut parts = match self.profile {
            None => vec!["roll the named relics to the bar".to_string()],
            Some(profile) => vec![match profile {
                Profile::Repair => "repair".to_string(),
                Profile::Bar(g, d) => bar((g, d)),
            }],
        };
        parts.push(match (self.filler, self.filler_within) {
            (true, Some(n)) => format!("filler when it fills the pity bar within {n}"),
            (true, None) => "filler".to_string(),
            (false, _) => "no filler".to_string(),
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
            filler: s.filler,
            filler_within: if s.filler { s.filler_within.map(u32::from) } else { None },
            locked: self.protected(game, level),
            prefer: s.prefer,
            ahead: s.ahead,
            trades: s.trades,
        }
    }
}

/// The level-20 look-ahead plan, written out (a start for the searches),
/// and a do-the-minimum baseline.
pub fn strategies() -> Vec<(&'static str, Plan)> {
    use Profile::{Bar as B, Repair};
    let total = StepChoice::build;
    vec![
        // the cheapest route to the top level overall (`--lookahead`)
        (
            "lookahead",
            Plan::new([
                (4, total(B(5, 1))),
                (6, total(Repair)),
                (7, StepChoice::build(B(4, 2))),
                (8, total(Repair).with_filler()),
                (10, total(B(5, 2))),
                (11, total(Repair).with_filler()),
                (12, total(Repair).with_filler()),
                (14, total(Repair).with_filler()),
                (15, total(Repair).with_filler()),
                (16, total(Repair).with_filler()),
                (17, total(Repair).with_filler()),
                (18, total(Repair).with_filler()),
                (19, StepChoice::build(Repair).with_filler()),
                (20, StepChoice::build(Repair).with_filler()),
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

/// Every look-ahead target: each level from 2 to the top, then the crit relic tiers.
pub fn targets() -> Vec<Target> {
    (2..=MAX_LEVEL).map(Target::level).chain(CRIT_TIERS.iter().map(|&m| Target::tier(m))).collect()
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


/// Every StepChoice worth trying for `level` (banking aside).  A level that
/// names its relics only decides its filler: closing each relic's gap to the
/// bar is the way to clear it, and no other relic is part of its quest.
pub fn step_choices(game: &Game, level: usize) -> Vec<StepChoice> {
    // no filler, or a filler with each window
    let fillers: Vec<(bool, Option<u8>)> =
        std::iter::once((false, None)).chain(FILLER_WINDOWS.iter().map(|&w| (true, w))).collect();
    let with = |profile: Option<Profile>, (filler, filler_within): (bool, Option<u8>)| StepChoice {
        profile,
        filler,
        filler_within,
        ..StepChoice::default()
    };
    let mut out = Vec::new();
    match game.quests[level].as_ref().expect("a real level") {
        Quest::Relics { .. } => out.extend(fillers.iter().map(|&f| with(None, f))),
        Quest::Each(_) | Quest::Total { .. } => {
            let profiles: &[Profile] = if matches!(game.quests[level], Some(Quest::Each(_))) { &EACH_PROFILES } else { &TOTAL_PROFILES };
            for &p in profiles {
                out.extend(fillers.iter().map(|&f| with(Some(p), f)));
            }
        }
    }
    out
}

/// The knobs a level has on the way to `target`, for a descent that turns
/// one knob at a time.
pub fn knobs(game: &Game, level: usize, target: usize) -> Vec<&'static str> {
    let mut knobs = vec!["filler", "window"];
    if game.next_named(level).is_some() {
        knobs.push("bank");
    }
    match game.quests[level] {
        Some(Quest::Each(_)) => knobs.push("profile"),
        Some(Quest::Total { .. }) => knobs.extend(["profile", "trades"]),
        _ => {}
    }
    if game.horizon_for(level, target, true) != game.horizon_for(level, target, false) {
        knobs.push("ahead");
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
        "filler" => both.iter().map(|&f| StepChoice { filler: f, ..current }).collect(),
        // a window only matters with a filler to roll
        "window" if !current.filler => Vec::new(),
        "window" => FILLER_WINDOWS.iter().map(|&w| StepChoice { filler_within: w, ..current }).collect(),
        "bank" => both.iter().map(|&b| StepChoice { bank: b, ..current }).collect(),
        "ahead" => both.iter().map(|&a| StepChoice { ahead: a, ..current }).collect(),
        // a repair asks every relic for a step past where it is: nothing to trade
        "trades" if current.profile == Some(Profile::Repair) => Vec::new(),
        "trades" => both.iter().map(|&t| StepChoice { trades: t, ..current }).collect(),
        "profile" => profiles.iter().map(|&p| StepChoice { profile: Some(p), ..current }).collect(),
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

fn ranked(scores: Vec<(f64, usize)>) -> Vec<(f64, usize)> {
    let mut scores = scores;
    scores.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    scores
}

/// The cheapest plan to `max_level` (and its tier, if any) overall, by
/// coordinate descent from `start`: every knob of every level is turned in
/// turn, each value scored on the total cost, and a change is kept if it
/// beats the plan on the screening runs and still beats it - by any margin -
/// on a larger, fresh set of paired runs.  Stops when a sweep changes nothing
/// (or after `SWEEPS`).
///
/// Every option for a level plays the levels below it exactly as the plan
/// does, so those are simulated once per level (`Game::prefixes`) and each
/// option is resumed from there - the same costs as whole runs, for less.
pub fn lookahead_search(
    game: &Game,
    start: &Plan,
    o: SearchOptions,
    log: &dyn Fn(&str),
) -> Result<(Plan, f64), String> {
    const SWEEPS: u64 = 4;
    let options = o.target().options();
    let mut plan = start.clone();
    for sweep in 0..SWEEPS {
        let mut changed = false;
        let check = o.seed + 1000 + sweep;
        // the plan's cost on the confirming runs, while it stays the plan
        let mut confirmed: Option<f64> = None;
        for level in 2..=o.max_level {
            let screen = game.prefixes(&plan, o.runs, o.seed + sweep, &options, level - 1)?;
            let mut current = game.mean_cost_from(&plan, &screen, &options)?;
            let mut confirming: Option<Vec<sim::Partial>> = None;
            for knob in knobs(game, level, o.max_level) {
                let trials = turn(game, level, knob, plan.step(level));
                if trials.is_empty() {
                    continue; // nothing to turn it to (a window, with no filler)
                }
                let scored = ranked(
                    trials
                        .iter()
                        .enumerate()
                        .map(|(i, &t)| Ok((game.mean_cost_from(&plan.with(level, t), &screen, &options)?, i)))
                        .collect::<Result<_, String>>()?,
                );
                let (screened, best) = (scored[0].0, trials[scored[0].1]);
                if screened >= current {
                    continue; // nothing beats the plan on the screening runs
                }
                if confirming.is_none() {
                    confirming = Some(game.prefixes(&plan, o.final_runs, check, &options, level - 1)?);
                }
                let fresh = confirming.as_deref().expect("just made");
                let now = match confirmed {
                    Some(cost) => cost,
                    None => game.mean_cost_from(&plan, fresh, &options)?,
                };
                let new = game.mean_cost_from(&plan.with(level, best), fresh, &options)?;
                confirmed = Some(now);
                if new < now {
                    log(&format!(
                        "sweep {sweep} L{level} {knob}: {} -> {}  {} -> {}",
                        plan.step(level).label(),
                        best.label(),
                        crate::format::commas(now, 0),
                        crate::format::commas(new, 0)
                    ));
                    plan = plan.with(level, best);
                    (current, confirmed) = (screened, Some(new));
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
/// plan reaches that target cheapest - the named look-ahead plan, the plan
/// saved for it, the plan just found for the target before it (a
/// tier's neighbour shares nearly all of its plan), and `extra` - and trimmed
/// to the levels it plays.  A start close to the answer leaves the descent
/// little to do.  `log` hears each target as it finishes.
pub fn lookahead_all(
    game: &Game,
    only: Option<&[Target]>,
    extra: &[Plan],
    log: &dyn Fn(&str),
) -> Result<Vec<(Target, Plan, f64)>, String> {
    let named = [strategy("lookahead").expect("a named plan")];
    let saved = lookahead_plans().unwrap_or_default();
    let mut out: Vec<(Target, Plan, f64)> = Vec::new();
    for target in targets().into_iter().filter(|t| only.is_none_or(|o| o.contains(t))) {
        let o = SearchOptions::per_target(target);
        let mut starts: Vec<&Plan> = named.iter().chain(extra).collect();
        starts.extend(saved.iter().filter(|(t, _)| *t == target).map(|(_, p)| p));
        starts.extend(out.last().map(|(_, p, _)| p));
        // and each of those looking to the totals ahead wherever that differs
        let ahead: Vec<Plan> = starts
            .iter()
            .map(|p| {
                (2..=target.level)
                    .filter(|&l| game.horizon_for(l, target.level, true) != game.horizon_for(l, target.level, false))
                    .fold((*p).clone(), |plan, l| plan.with(l, StepChoice { ahead: true, ..p.step(l) }))
            })
            .collect();
        starts.extend(ahead.iter());
        // and each of those with trades counting as their bar on every bar level
        let traded: Vec<Plan> = starts
            .iter()
            .map(|p| {
                (2..=target.level)
                    .filter(|&l| matches!(game.quests[l], Some(Quest::Total { .. })) && p.step(l).profile != Some(Profile::Repair))
                    .fold((*p).clone(), |plan, l| plan.with(l, StepChoice { trades: true, ..p.step(l) }))
            })
            .collect();
        starts.extend(traded.iter());
        let mut best: Option<(f64, &Plan)> = None;
        for start in starts {
            let cost = target_cost(game, start, o.final_runs, o.seed + 3000, target)?;
            if best.is_none_or(|(c, _)| cost < c) {
                best = Some((cost, start));
            }
        }
        let start = best.expect("the named starts").1.up_to(target.level);
        let (plan, cost) = lookahead_search(game, &start, o, &|_| {})?;
        let plan = plan.up_to(target.level);
        log(&format!("{}: {}  {}", target.code(), crate::format::commas(cost, 0), plan.code()));
        out.push((target, plan, cost));
    }
    Ok(out)
}

// ------------------------------------------------------------------ advice

/// What to do next, from a board seen part-way through a climb.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    /// the board meets the next level's quest already
    LevelUp,
    /// a roll for the quest or a filler, or a summon
    Move(Move),
    /// at the top, short of the tier: trade every full lot of each other
    /// relic, as (relic, lots), into the crit relic
    Convert(Vec<(usize, u32)>),
    /// at the top, short of the tier: roll the crit relic played per `key`,
    /// keeping any higher amplification
    Farm { key: TableKey },
    /// the target is reached
    Done,
}

/// A plan's next step from one board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advice {
    /// the inheritor level the board was seen at
    pub level: usize,
    /// the level being worked toward (for a tier, the top itself)
    pub goal: usize,
    pub next: Next,
    /// the relics the level's quest work is on, each with the bar it is
    /// rolled toward (empty past the top, or once the quest is met)
    pub work: Vec<(usize, Bar)>,
    /// the totals every attempt at this level closes the gap to
    /// (`Game::horizon_for`, as the plan has it for the level)
    pub horizon: sim::Horizon,
}

impl Advice {
    /// The relic rolled next and how that attempt is played, if it is a roll.
    pub fn roll(&self) -> Option<(usize, TableKey)> {
        match self.next {
            Next::Move(Move::Quest { relic, key, .. } | Move::Filler { relic, key, .. }) => {
                Some((relic, key))
            }
            Next::Farm { key } => Some((CRIT, key)),
            _ => None,
        }
    }
}

impl Game {
    /// What `plan` does next toward `target` from a board seen at inheritor
    /// `level`: `states` is every relic's (glory, despair), `stock` how many
    /// of each are on hand and `pity` the pity points earned toward the next
    /// level (None if not known: a filler that waits for the bar to be in
    /// reach is then not rolled).  The same decisions the Monte Carlo makes:
    /// it too works out each attempt from the board alone.
    pub fn advise(
        &self,
        plan: &Plan,
        target: Target,
        level: usize,
        states: &[Relic],
        stock: &[u32],
        pity: Option<u32>,
    ) -> Result<Advice, String> {
        let n = self.economy.relics;
        if states.len() != n || stock.len() != n {
            return Err(format!("a board needs all {n} relics"));
        }
        if level < 1 || level > target.level {
            return Err(format!("level {level} is not on the way to {}", target.code()));
        }
        if level < target.level {
            let goal = level + 1;
            let quest = self.quests[goal].as_ref().ok_or(format!("no requirement for level {goal}"))?;
            let rules = plan.rules(self, goal);
            let horizon = self.horizon_for(goal, target.level, rules.ahead);
            if quest.satisfied(states) {
                return Ok(Advice { level, goal, next: Next::LevelUp, work: Vec::new(), horizon });
            }
            let left = pity.map(|p| self.pity_needed[goal].saturating_sub(p).div_ceil(self.pity_gain[level]));
            let (next, work) = sim::next_move(quest, &rules, level, states, stock, self.economy.per_attempt, left, horizon)
                .ok_or(format!("stuck trying to reach level {goal}"))?;
            return Ok(Advice { level, goal, next: Next::Move(next), work, horizon });
        }
        // at the top: the crit relic farmed to the tier, as the runs do it
        let next = match target.tier {
            Some(mark) if amplification(states[CRIT]) < f64::from(mark) => {
                let lots: Vec<(usize, u32)> = (0..n)
                    .filter(|&i| i != CRIT)
                    .map(|i| (i, stock[i] / self.economy.convert_from))
                    .filter(|&(_, lots)| lots > 0)
                    .collect();
                if !lots.is_empty() {
                    Next::Convert(lots)
                } else if stock[CRIT] >= self.economy.per_attempt {
                    Next::Farm { key: TableKey::above(level as u8, mark, states[CRIT]) }
                } else {
                    Next::Move(Move::Summon)
                }
            }
            _ => Next::Done,
        };
        Ok(Advice { level, goal: level, next, work: Vec::new(), horizon: self.horizon(target.level) })
    }

    /// Whether the roll `advice` makes keeps `new` on the relic it rolls;
    /// None if it is not a roll.
    pub fn keeps(&self, advice: &Advice, states: &[Relic], new: Relic) -> Option<bool> {
        match advice.next {
            Next::Move(Move::Quest { relic, bar, .. } | Move::Filler { relic, bar, .. }) => {
                let quest = self.quests[advice.goal].as_ref().expect("advice is for a real level");
                Some(sim::keeps(quest, advice.horizon, states, relic, new, bar))
            }
            Next::Farm { .. } => Some(self.keeps_farmed(states, new)),
            _ => None,
        }
    }

    /// Whether a roll on `relic` toward `bar`, for the quest of level `goal`
    /// and closing the gap to the totals `horizon` (a quest roll or a filler
    /// alike, as the advice had them), keeps `new`: it ranks above the
    /// memory in place (its bar newly met, the board nearer those totals,
    /// more glory less despair).  What the roll was is all it takes - not the
    /// plan, nor the pity.
    pub fn keeps_roll(&self, horizon: sim::Horizon, goal: usize, relic: usize, bar: Bar, states: &[Relic], new: Relic) -> bool {
        let quest = self.quests[goal].as_ref().expect("a real level");
        sim::keeps(quest, horizon, states, relic, new, bar)
    }

    /// The totals of level `level`, or of the last level before it that asks
    /// for totals.
    pub fn horizon(&self, level: usize) -> sim::Horizon {
        (2..=level.min(self.max_level())).rev().find_map(|l| match self.quests[l] {
            Some(Quest::Total { glory, despair }) => Some((glory, despair)),
            _ => None,
        })
    }

    /// The totals the work for level `goal` closes the gap to, on the way to
    /// `target`: a totals level's own (a level naming its relics: the
    /// target's totals), or, `ahead`, the totals ahead - the most glory and
    /// the least despair any totals level from `goal` to `target` asks, so
    /// a tighter despair cap on the way counts.  However little the level
    /// asks itself: 9/0 beats a 9/1 that clears it, and the tie-break weighs
    /// glory against despair as these totals still need them.  Which is
    /// cheaper is the plan's call, level by level ("a").
    pub fn horizon_for(&self, goal: usize, target: usize, ahead: bool) -> sim::Horizon {
        if ahead {
            let totals = (goal..=target.min(self.max_level())).filter_map(|l| match self.quests[l] {
                Some(Quest::Total { glory, despair }) => Some((glory, despair)),
                _ => None,
            });
            return totals.reduce(|(g, d), (g2, d2)| (g.max(g2), d.min(d2))).or_else(|| self.horizon(target));
        }
        match self.quests.get(goal) {
            Some(Some(Quest::Total { glory, despair })) => Some((*glory, *despair)),
            _ => self.horizon(target),
        }
    }

    /// Whether a crit relic farm roll keeps `new`: more amplification.
    pub fn keeps_farmed(&self, states: &[Relic], new: Relic) -> bool {
        amplification(new) > amplification(states[CRIT])
    }
}

/// The saved look-ahead plan for `target`.
pub fn lookahead_plan(target: Target) -> Result<Plan, String> {
    lookahead_plans()?
        .into_iter()
        .find(|(t, _)| *t == target)
        .map(|(_, plan)| plan)
        .ok_or(format!("no look-ahead plan for {}", target.code()))
}
