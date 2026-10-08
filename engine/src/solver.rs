//! One inheritance attempt, solved exactly.
//!
//! An attempt fills two bars of memory slots, glory (successes good) and
//! despair (successes bad), spending spirit power on each slot and mental
//! strength on mental training, with every roll moving a shared chance ladder.
//! The solver is an exact expectimax dynamic program over every reachable
//! state: each branch is enumerated and weighted by its probability, so the
//! expectations it reports are exact under optimal play.

use std::collections::BTreeMap;
use std::hash::Hash;

use rustc_hash::FxHashMap;

use crate::rng::Rng;

/// The shared success-chance ladder; index 0 is the easiest tier.  A success
/// moves one tier down the list (harder next time), a failure one tier up.
pub const TIERS: [f64; 5] = [0.80, 0.65, 0.50, 0.35, 0.20];
pub const BEST_TIER: u8 = 0;
pub const WORST_TIER: u8 = 4;

const BASE_SLOTS: u8 = 5;
const BASE_MAX_SPIRIT: u8 = 8;

/// The requirement sheet spells the level cycle out as far as level 20;
/// further levels continue the same cycle, an extrapolation.
pub const DOCUMENTED_MAX_LEVEL: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    Glory,
    Despair,
    Train,
}

impl Action {
    /// The order actions are tried in; ties go to the earliest.
    pub const ORDER: [Action; 3] = [Action::Glory, Action::Despair, Action::Train];

    pub fn name(self) -> &'static str {
        match self {
            Action::Glory => "attempt glory",
            Action::Despair => "attempt despair",
            Action::Train => "mental training",
        }
    }

    pub fn letter(self) -> char {
        match self {
            Action::Glory => 'G',
            Action::Despair => 'D',
            Action::Train => 'T',
        }
    }
}

/// What an attempt steers by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Objective {
    /// maximise amplification, +w_glory% a glory success, -w_despair% a
    /// despair success, floored at 0
    Weighted,
    /// all or nothing: maximise P(at least `glory` glory successes and at
    /// most `despair` despair successes)
    Target { glory: u8, despair: u8 },
    /// close the gap to a bar from a memory `gap` steps off it: a result is
    /// worth the steps it closes - a glory success short of `glory` gained, a
    /// despair success over `despair` shed, one each - and nothing if it
    /// closes none (it would not be kept).  The play the levels engine uses.
    Close { glory: u8, despair: u8, gap: u8 },
    /// max amplification above a mark: a result is worth nothing below
    /// `mark`%, something at it and more above it - the best chance of
    /// `mark`% or more first, then the most amplification.  When `mark` is
    /// out of reach part-way, the highest amplification still reachable
    /// takes its place, down to `least`% (anything less would not be kept:
    /// the memory's amplification + 1).  The crit relic farm plays it with
    /// the tier as the mark; a mark just above the memory means "go higher".
    Above { mark: u8, least: u8 },
}

/// What settles moves every objective scores alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Last {
    /// more amplification (the crit relic, and the solve reports)
    Amp,
    /// `glory` a glory success less `despair` a despair success: every relic
    /// but the crit relic.  1:1 unless the levels to come need one more than
    /// the other (on a totals level, weighted by how short of the goal's
    /// glory the board is against how far over its despair)
    Steps { glory: u8, despair: u8 },
}

/// An objective, what breaks its ties, and the amplification weights it is
/// reported in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strategy {
    pub objective: Objective,
    pub w_glory: f64,
    pub w_despair: f64,
    /// a second objective for moves the first scores alike: closing the gap
    /// to (glory, despair) from a memory `gap` steps off - the levels
    /// engine's look-ahead to the goal's totals, after a relic's own bar
    pub then: Option<(u8, u8, u8)>,
    pub last: Last,
}

impl Strategy {
    pub fn weighted(w_glory: f64, w_despair: f64) -> Self {
        Strategy { objective: Objective::Weighted, w_glory, w_despair, then: None, last: Last::Amp }
    }

    /// All or nothing for (glory, despair), reported at +5% / -2%.
    pub fn target(glory: u8, despair: u8) -> Self {
        Strategy { objective: Objective::Target { glory, despair }, w_glory: 5.0, w_despair: 2.0, then: None, last: Last::Amp }
    }

    /// Then, for moves the objective scores alike, close the gap to (glory,
    /// despair) from a memory `gap` steps off; then more glory, less despair.
    pub fn then_close(self, glory: u8, despair: u8, gap: u8) -> Self {
        Strategy { then: Some((glory, despair, gap)), last: Last::Steps { glory: 1, despair: 1 }, ..self }
    }

    /// Maximize glory, minimize despair until a limit: every glory gained
    /// up to `glory` (max useful glory) and every despair shed down to
    /// `despair` (min useful despair) from the memory `from` counts, reported
    /// at +5% / -2%.  Only how far off `from` is matters.
    pub fn close(glory: u8, despair: u8, from: (u8, u8)) -> Self {
        let gap = glory.saturating_sub(from.0) + from.1.saturating_sub(despair);
        Strategy {
            objective: Objective::Close { glory, despair, gap },
            w_glory: 5.0,
            w_despair: 2.0,
            then: None,
            last: Last::Steps { glory: 1, despair: 1 },
        }
    }

    /// What settles the rest: `glory` a glory success less `despair` a
    /// despair success.
    pub fn tie(self, glory: u8, despair: u8) -> Self {
        Strategy { last: Last::Steps { glory, despair }, ..self }
    }

    /// Maximize amplification above a target: the minimum useful amplification `mark`%,
    /// at +5% / -2%, over the memory `from`
    /// if one is in place (a result must beat it to be kept, so the mark is
    /// at least one above it).  No mark (0) over no memory, or one at +0%,
    /// is no target at all: the most amplification, expected
    /// (`Strategy::default`) - scored as the chance of +0% or more it would
    /// play for the chance of finishing alone.
    pub fn above(mark: u8, from: Option<(u8, u8)>) -> Self {
        let least = from.map_or(0, |(g, d)| (5 * i32::from(g) - 2 * i32::from(d)).clamp(0, 254) as u8 + 1);
        if mark == 0 && least <= 1 {
            return Strategy::default();
        }
        Strategy { objective: Objective::Above { mark: mark.max(least), least }, w_glory: 5.0, w_despair: 2.0, then: None, last: Last::Amp }
    }

    pub fn with_weights(self, w_glory: f64, w_despair: f64) -> Self {
        Strategy { w_glory, w_despair, ..self }
    }

    pub fn is_target(&self) -> bool {
        matches!(self.objective, Objective::Target { .. })
    }

    pub fn name(&self) -> String {
        use crate::format::general;
        match self.objective {
            Objective::Target { glory, despair } => format!("target ({glory}, {despair})"),
            Objective::Close { glory, despair, gap } => format!("close to ({glory}, {despair}) from {gap} off"),
            Objective::Above { mark, least } => format!("max amplification above {mark}% (kept from {least}%)"),
            Objective::Weighted => {
                format!("weighted (+{}%/-{}%)", general(self.w_glory), general(self.w_despair))
            }
        }
    }
}

impl Default for Strategy {
    fn default() -> Self {
        Strategy::weighted(5.0, 2.0)
    }
}

/// Everything one attempt needs, derived from the inheritor level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub level: u32,
    /// memory slots per bar
    pub slots: u8,
    /// spirit power cap, and the starting spirit power
    pub max_spirit: u8,
    pub glory_mod: f64,
    pub despair_mod: f64,
    pub train_mod: f64,
    /// index into TIERS at the start of the attempt
    pub start_tier: u8,
    pub strategy: Strategy,
    /// minimise the chance of a dead-end wipe before anything else
    pub safety_first: bool,
    /// extra score charged for a wipe (ignored when safety_first)
    pub wipe_penalty: f64,
    /// glory slots that start the attempt already filled with a success
    pub start_glory: u8,
    /// despair slots that start the attempt already filled with a failure
    pub start_despair_fail: u8,
}

impl Config {
    /// The board at inheritor `level`, every bonus from level 2 up to it added
    /// in (see `level_bonus`).
    pub fn for_level(level: i64, strategy: Strategy) -> Result<Config, String> {
        if level < 1 {
            return Err(format!("inheritor level {level} is not a real level (1 and up)"));
        }
        let (mut slots, mut max_spirit, mut glory_mod, mut despair_mod) =
            (BASE_SLOTS, BASE_MAX_SPIRIT, 0.0, 0.0);
        let (mut start_glory, mut start_despair_fail) = (0, 0);
        for lv in 2..=level {
            match level_bonus(lv) {
                Bonus::Glory => glory_mod += 0.02,
                Bonus::Despair => despair_mod += -0.02,
                Bonus::Slot => slots += 1,
                Bonus::Spirit => max_spirit += 1,
                Bonus::StartGlory => start_glory += 1,
                Bonus::StartDespairFail => start_despair_fail += 1,
            }
        }
        Ok(Config {
            level: level as u32,
            slots,
            max_spirit,
            glory_mod,
            despair_mod,
            train_mod: 0.0,
            // the head starts move the chance ladder like real results: a
            // success one tier harder, a failure one tier easier
            start_tier: (BEST_TIER + start_glory).min(WORST_TIER).saturating_sub(start_despair_fail),
            strategy,
            safety_first: false,
            wipe_penalty: 0.0,
            start_glory,
            start_despair_fail,
        })
    }

    pub fn start_mental(&self) -> u8 {
        self.slots
    }

    /// The glory target, clamped to what the bar can hold.
    pub fn want_glory(&self) -> u8 {
        match self.strategy.objective {
            Objective::Target { glory, .. } => glory.min(self.slots),
            _ => 0,
        }
    }

    pub fn allow_despair(&self) -> u8 {
        match self.strategy.objective {
            Objective::Target { despair, .. } => despair.min(self.slots),
            _ => 0,
        }
    }
}

/// What reaching a level adds to the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bonus {
    /// +2% glory rate
    Glory,
    /// -2% despair rate
    Despair,
    /// one more memory slot per bar
    Slot,
    /// one more spirit power
    Spirit,
    /// every attempt starts with one more glory slot filled with a success
    StartGlory,
    /// every attempt starts with one more despair slot filled with a failure
    StartDespairFail,
}

/// The bonus for reaching `level` (2 and up).  Levels 2-12 cycle glory,
/// despair, slot, spirit power.  From 13: glory, despair, a glory success to
/// start with (15), the 9th slot (16), glory, despair, a despair failure to
/// start with (19), the 10th slot (20).  Past 20 that run of eight is assumed
/// to repeat.
pub fn level_bonus(level: i64) -> Bonus {
    use Bonus::{Despair, Glory, Slot, Spirit, StartDespairFail, StartGlory};
    if level <= 12 {
        [Slot, Spirit, Glory, Despair][(level % 4) as usize]
    } else {
        [Glory, Despair, StartGlory, Slot, Glory, Despair, StartDespairFail, Slot]
            [((level - 13) % 8) as usize]
    }
}

/// (glory filled, glory successes, despair filled, despair successes,
/// mental strength, spirit power, tier)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct State {
    pub gf: u8,
    pub gs: u8,
    pub df: u8,
    pub ds: u8,
    pub ms: u8,
    pub sp: u8,
    pub tier: u8,
}

impl State {
    fn key(self) -> u64 {
        u64::from_le_bytes([self.gf, self.gs, self.df, self.ds, self.ms, self.sp, self.tier, 0])
    }
}

/// How a state is left under optimal play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// both bars full: the attempt is over
    Done,
    Act(Action),
    /// no spirit power, no mental strength, bars unfinished: a wipe
    DeadEnd,
}

#[derive(Clone, Copy, Debug)]
pub struct Value {
    pub p_finish: f64,
    pub score: f64,
    /// the second objective (`Strategy::then`), expected: what breaks a tie
    /// in the first
    pub then: f64,
    /// the chance of ending on the nearest result still reachable from here
    /// (a target's box, or a mark; the objective itself while it can be
    /// hit) - what breaks a tie after that
    pub near: f64,
    /// what settles the rest (`Strategy::last`): expected amplification %,
    /// or glory successes less despair successes
    pub last: f64,
    pub choice: Choice,
}

/// A running total per key that remembers the order keys first appeared in,
/// so sums over it come out the same every time.
#[derive(Clone, Debug)]
pub struct Tally<K> {
    index: FxHashMap<K, usize>,
    items: Vec<(K, f64)>,
}

impl<K: Copy + Eq + Hash> Default for Tally<K> {
    fn default() -> Self {
        Tally { index: FxHashMap::default(), items: Vec::new() }
    }
}

impl<K: Copy + Eq + Hash> Tally<K> {
    pub fn add(&mut self, key: K, amount: f64) {
        match self.index.get(&key) {
            Some(&i) => self.items[i].1 += amount,
            None => {
                self.index.insert(key, self.items.len());
                self.items.push((key, amount));
            }
        }
    }

    pub fn get(&self, key: K) -> f64 {
        self.index.get(&key).map_or(0.0, |&i| self.items[i].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, f64)> + '_ {
        self.items.iter().copied()
    }

    pub fn total(&self) -> f64 {
        self.items.iter().map(|&(_, v)| v).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The largest entry; the first one seen wins a tie.
    pub fn most_common(&self) -> Option<(K, f64)> {
        self.items.iter().copied().fold(None, |best, (k, v)| match best {
            Some((_, bv)) if bv >= v => best,
            _ => Some((k, v)),
        })
    }
}

/// Exact outcome statistics for a policy.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub cfg: Config,
    pub states_explored: usize,
    /// mean amplification %, floored per outcome
    pub e_amplification: f64,
    pub e_glory: f64,
    /// despair successes - lower is better
    pub e_despair_success: f64,
    pub p_dead_end: f64,
    /// target strategies: share landing inside the box
    pub p_target: f64,
    /// (glory successes, despair successes) -> probability
    pub dist: Tally<(u8, u8)>,
    /// action -> expected number of times taken
    pub action_counts: Tally<Action>,
    pub action_by_tier: BTreeMap<u8, Tally<Action>>,
    pub action_by_sp: FxHashMap<(u8, u8), Tally<Action>>,
}

impl Analysis {
    /// (every outcome, cumulative probability), outcomes in (glory, despair)
    /// order: what a Monte Carlo draws an attempt's result from.
    pub fn outcome_table(&self) -> (Vec<(u8, u8)>, Vec<f64>) {
        let mut items: Vec<_> = self.dist.iter().collect();
        items.sort_by_key(|&(gd, _)| gd);
        let mut running = 0.0;
        let mut cum: Vec<f64> = items
            .iter()
            .map(|&(_, p)| {
                running += p;
                running
            })
            .collect();
        if let Some(last) = cum.last_mut() {
            *last = 1.0;
        }
        (items.into_iter().map(|(gd, _)| gd).collect(), cum)
    }
}

/// One step of a played-out attempt.
pub struct Step {
    pub state: State,
    pub action: Action,
    pub succeeded: bool,
    pub next: State,
}

pub struct AttemptResult {
    pub dead_end: bool,
    pub glory_success: u8,
    pub despair_success: u8,
    pub mental_left: u8,
    pub spirit_left: u8,
}

/// A hand-written policy: (state, legal actions) -> the action to take.
pub type Policy<'a> = &'a dyn Fn(State, &[Action]) -> Action;

/// The optimal policy and exact expectations for one Config.
pub struct Solver {
    pub cfg: Config,
    chance: [[f64; 5]; 3],
    memo: FxHashMap<u64, Value>,
}

impl Solver {
    pub fn new(cfg: Config) -> Self {
        let mut chance = [[0.0; 5]; 3];
        for (a, action) in Action::ORDER.into_iter().enumerate() {
            for (t, &base) in TIERS.iter().enumerate() {
                let modifier = match action {
                    Action::Glory => cfg.glory_mod,
                    Action::Despair => cfg.despair_mod,
                    Action::Train => cfg.train_mod,
                };
                chance[a][t] = (base + modifier).clamp(0.0, 1.0);
            }
        }
        Solver { cfg, chance, memo: FxHashMap::default() }
    }

    // ------------------------------------------------------------------ rules

    pub fn start_state(&self) -> State {
        let c = &self.cfg;
        let (glory, fails) = (c.start_glory, c.start_despair_fail);
        State {
            gf: glory,
            gs: glory,
            df: fails,
            ds: 0,
            ms: c.start_mental(),
            sp: c.max_spirit,
            tier: c.start_tier,
        }
    }

    /// Success chance of an action at a tier, after modifiers.
    pub fn chance(&self, action: Action, tier: u8) -> f64 {
        self.chance[action as usize][tier as usize]
    }

    pub fn legal_actions(&self, s: State) -> Vec<Action> {
        let slots = self.cfg.slots;
        let mut out = Vec::with_capacity(3);
        if s.sp > 0 && s.gf < slots {
            out.push(Action::Glory);
        }
        if s.sp > 0 && s.df < slots {
            out.push(Action::Despair);
        }
        if s.ms > 0 {
            out.push(Action::Train);
        }
        out
    }

    pub fn is_terminal(&self, s: State) -> bool {
        s.gf == self.cfg.slots && s.df == self.cfg.slots
    }

    /// (probability, next state, roll succeeded) for both ways a roll can go.
    pub fn transitions(&self, s: State, action: Action) -> [(f64, State, bool); 2] {
        let p = self.chance(action, s.tier);
        let harder = (s.tier + 1).min(WORST_TIER);
        let easier = s.tier.saturating_sub(1); // BEST_TIER is 0
        let (win, lose) = match action {
            Action::Glory => (
                State { gf: s.gf + 1, gs: s.gs + 1, sp: s.sp - 1, tier: harder, ..s },
                State { gf: s.gf + 1, sp: s.sp - 1, tier: easier, ..s },
            ),
            Action::Despair => (
                State { df: s.df + 1, ds: s.ds + 1, sp: s.sp - 1, tier: harder, ..s },
                State { df: s.df + 1, sp: s.sp - 1, tier: easier, ..s },
            ),
            Action::Train => (
                State { ms: s.ms - 1, sp: (s.sp + 2).min(self.cfg.max_spirit), tier: harder, ..s },
                State { ms: s.ms - 1, tier: easier, ..s },
            ),
        };
        [(p, win, true), (1.0 - p, lose, false)]
    }

    /// The relic's amplification in percent: w_glory a glory success minus
    /// w_despair a despair success, floored at 0 - a relic never comes out
    /// worse than no relic, so a wipe and an awful result both pay 0%.
    pub fn amplification(&self, glory: u8, despair: u8) -> f64 {
        let s = &self.cfg.strategy;
        (s.w_glory * f64::from(glory) - s.w_despair * f64::from(despair)).max(0.0)
    }

    /// A full glory bar and no despair.
    pub fn max_amplification(&self) -> f64 {
        self.cfg.strategy.w_glory * f64::from(self.cfg.slots)
    }

    pub fn min_amplification(&self) -> f64 {
        0.0
    }

    /// Did an outcome land inside the target box?  Target strategies only.
    pub fn hits_target(&self, glory: u8, despair: u8) -> bool {
        self.cfg.strategy.is_target() && glory >= self.cfg.want_glory() && despair <= self.cfg.allow_despair()
    }

    /// Target strategies: how many steps a result is from the target box -
    /// a glory success short or a despair success over is one step each
    /// (so 7/2 is one step from 7/1, as is 6/1).
    pub fn shortfall(&self, glory: u8, despair: u8) -> u8 {
        self.cfg.want_glory().saturating_sub(glory) + despair.saturating_sub(self.cfg.allow_despair())
    }

    /// The nearest result still reachable from `s`, as a key that only
    /// changes when it moves - None where the objective has no such notion,
    /// or nothing worth anything can be reached.  A target: the fewest steps
    /// from its box (every glory slot left a success, no more despair).  A
    /// mark: the highest amplification reachable, as far as the mark, if it
    /// is one a result would be kept at.
    fn nearest(&self, s: State) -> Option<u8> {
        let (best_glory, despair) = (s.gs + (self.cfg.slots - s.gf), s.ds);
        match self.cfg.strategy.objective {
            Objective::Target { .. } => Some(self.shortfall(best_glory, despair)),
            Objective::Above { mark, least } => {
                let best = self.amplification(best_glory, despair).min(f64::from(mark)) as u8;
                (best >= least).then_some(best)
            }
            _ => None,
        }
    }

    /// The second objective at a finished attempt: the steps closed toward
    /// `Strategy::then`'s (glory, despair), nothing if none (0 with none).
    pub fn then(&self, glory: u8, despair: u8) -> f64 {
        match self.cfg.strategy.then {
            Some((want, allow, gap)) => {
                let now = f64::from(want.saturating_sub(glory)) + f64::from(despair.saturating_sub(allow));
                (f64::from(gap) - now).max(0.0)
            }
            None => 0.0,
        }
    }

    /// What settles the rest at a finished attempt (`Strategy::last`).
    pub fn last(&self, glory: u8, despair: u8) -> f64 {
        match self.cfg.strategy.last {
            Last::Amp => self.amplification(glory, despair),
            Last::Steps { glory: wg, despair: wd } => {
                f64::from(wg) * f64::from(glory) - f64::from(wd) * f64::from(despair)
            }
        }
    }

    /// What the solver steers by at a finished attempt.
    pub fn objective(&self, glory: u8, despair: u8) -> f64 {
        let s = &self.cfg.strategy;
        match s.objective {
            Objective::Target { .. } => f64::from(u8::from(self.hits_target(glory, despair))),
            Objective::Weighted => self.amplification(glory, despair),
            Objective::Close { glory: want, despair: allow, gap } => {
                let now = f64::from(want.saturating_sub(glory)) + f64::from(despair.saturating_sub(allow));
                (f64::from(gap) - now).max(0.0)
            }
            Objective::Above { mark, .. } => {
                f64::from(u8::from(self.amplification(glory, despair) >= f64::from(mark)))
            }
        }
    }

    // ---------------------------------------------------------------- solving

    /// Compare (p_finish, score, near, amplification).  safety_first dodges
    /// the wipe before looking at score; otherwise the two trade linearly,
    /// wipe_penalty 0 being the plain expected-score maximiser.  Ties - an
    /// all-or-nothing objective scores every action alike once its target is
    /// met or out of reach - go to the better chance of the nearest result
    /// still reachable (out of reach of 7/1 with 2 despair already: 7/2; of
    /// 46% part-way, the highest amplification left), then to the more
    /// amplification; never at the objective's expense, since they only
    /// decide ties.
    fn better(&self, cand: (f64, f64, f64, f64, f64), best: (f64, f64, f64, f64, f64)) -> bool {
        const EPS: f64 = 1e-12;
        let (c, b) = if self.cfg.safety_first {
            if (cand.0 - best.0).abs() > EPS {
                return cand.0 > best.0;
            }
            (cand.1, best.1)
        } else {
            let pen = self.cfg.wipe_penalty;
            (cand.1 + pen * cand.0, best.1 + pen * best.0)
        };
        if (c - b).abs() > EPS {
            return c > b;
        }
        for (x, y) in [(cand.2, best.2), (cand.3, best.3)] {
            if (x - y).abs() > EPS {
                return x > y;
            }
        }
        cand.4 > best.4 + EPS
    }

    /// Solve a state exactly: P(finish), expected score, the optimal choice.
    /// The objective only steers the choice; both numbers are honest
    /// expectations under the policy.  Every action fills a slot or spends
    /// mental strength, so the graph is acyclic.
    pub fn value(&mut self, s: State) -> Value {
        if let Some(&hit) = self.memo.get(&s.key()) {
            return hit;
        }
        let result = if self.is_terminal(s) {
            let (score, then, last) = (self.objective(s.gs, s.ds), self.then(s.gs, s.ds), self.last(s.gs, s.ds));
            // a finished attempt is its own nearest result (if it is one worth anything)
            let near = f64::from(u8::from(self.nearest(s).is_some()));
            Value { p_finish: 1.0, score, then, near, last, choice: Choice::Done }
        } else {
            let k = self.nearest(s);
            let mut best: Option<Value> = None;
            for action in self.legal_actions(s) {
                let v = self.act(s, action, k);
                if best.is_none_or(|b| {
                    self.better((v.p_finish, v.score, v.then, v.near, v.last), (b.p_finish, b.score, b.then, b.near, b.last))
                }) {
                    best = Some(v);
                }
            }
            // a dead end inherits nothing at all: 0 glory, 0 despair, the floor
            // (and below any finished result on glory less despair)
            let last = match self.cfg.strategy.last {
                Last::Amp => 0.0,
                Last::Steps { despair: wd, .. } => -f64::from(wd) * f64::from(self.cfg.slots) - 1.0,
            };
            best.unwrap_or(Value { p_finish: 0.0, score: 0.0, then: 0.0, near: 0.0, last, choice: Choice::DeadEnd })
        };
        self.memo.insert(s.key(), result);
        result
    }

    /// Taking `action` in `s` (a legal one), then playing optimally: what
    /// `value` weighs it by against the other actions.
    pub fn action_value(&mut self, s: State, action: Action) -> Value {
        let k = self.nearest(s);
        self.act(s, action, k)
    }

    /// `action_value`, given the nearest result still reachable from `s`.
    fn act(&mut self, s: State, action: Action, k: Option<u8>) -> Value {
        let (mut p_finish, mut score, mut then, mut near, mut last) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (prob, next, _ok) in self.transitions(s, action) {
            if prob != 0.0 {
                let sub = self.value(next);
                p_finish += prob * sub.p_finish;
                score += prob * sub.score;
                then += prob * sub.then;
                last += prob * sub.last;
                // a step that puts the nearest result out of reach ends nearer to nothing
                if k.is_some() && self.nearest(next) == k {
                    near += prob * sub.near;
                }
            }
        }
        Value { p_finish, score, then, near, last, choice: Choice::Act(action) }
    }

    pub fn best_action(&mut self, s: State) -> Choice {
        self.value(s).choice
    }

    /// Strictly decreases by one per action; orders the forward sweep.
    fn potential(&self, s: State) -> usize {
        usize::from(s.ms) + 2 * usize::from(self.cfg.slots) - usize::from(s.gf) - usize::from(s.df)
    }

    // --------------------------------------------------------------- analysis

    /// Push probability mass forward through a policy, exactly: the optimal
    /// one by default, or a hand-written one scored on the same footing.
    pub fn analyse(&mut self, policy: Option<Policy>) -> Analysis {
        let start = self.start_state();
        let top = self.potential(start);
        let mut layers: Vec<Tally<State>> = (0..=top).map(|_| Tally::default()).collect();
        layers[top].add(start, 1.0);

        let mut dist = Tally::default();
        let mut action_counts = Tally::default();
        let mut action_by_tier: BTreeMap<u8, Tally<Action>> = BTreeMap::new();
        let mut action_by_sp: FxHashMap<(u8, u8), Tally<Action>> = FxHashMap::default();
        let mut p_dead = 0.0;

        for level in (0..=top).rev() {
            let layer = std::mem::take(&mut layers[level]);
            for (s, mass) in layer.iter() {
                if self.is_terminal(s) {
                    dist.add((s.gs, s.ds), mass);
                    continue;
                }
                let legal = self.legal_actions(s);
                if legal.is_empty() {
                    p_dead += mass;
                    dist.add((0, 0), mass); // incomplete: nothing inherited
                    continue;
                }
                let action = match policy {
                    Some(policy) => policy(s, &legal),
                    None => match self.best_action(s) {
                        Choice::Act(action) => action,
                        other => unreachable!("a live state solved to {other:?}"),
                    },
                };
                assert!(legal.contains(&action), "policy chose illegal action {action:?} in {s:?}");
                action_counts.add(action, mass);
                action_by_tier.entry(s.tier).or_default().add(action, mass);
                action_by_sp.entry((s.tier, s.sp)).or_default().add(action, mass);
                for (prob, next, _ok) in self.transitions(s, action) {
                    if prob != 0.0 {
                        layers[self.potential(next)].add(next, mass * prob);
                    }
                }
            }
        }

        let mut p_target: f64 =
            dist.iter().filter(|&((g, d), _)| self.hits_target(g, d)).map(|(_, p)| p).sum();
        if self.hits_target(0, 0) {
            // incomplete attempts share the (0, 0) cell but never count as a hit
            p_target -= p_dead;
        }
        let e_glory = dist.iter().map(|((g, _), p)| f64::from(g) * p).sum();
        let e_despair = dist.iter().map(|((_, d), p)| f64::from(d) * p).sum();
        let e_amp = dist.iter().map(|((g, d), p)| self.amplification(g, d) * p).sum();
        Analysis {
            cfg: self.cfg,
            states_explored: self.memo.len(),
            e_amplification: e_amp,
            e_glory,
            e_despair_success: e_despair,
            p_dead_end: p_dead,
            p_target,
            dist,
            action_counts,
            action_by_tier,
            action_by_sp,
        }
    }

    // ------------------------------------------------------------- simulating

    /// Play out one attempt under the optimal policy.
    pub fn attempt(&mut self, rng: &mut Rng, mut log: Option<&mut Vec<Step>>) -> AttemptResult {
        let mut s = self.start_state();
        loop {
            if self.is_terminal(s) {
                return AttemptResult {
                    dead_end: false,
                    glory_success: s.gs,
                    despair_success: s.ds,
                    mental_left: s.ms,
                    spirit_left: s.sp,
                };
            }
            let action = match self.best_action(s) {
                Choice::Act(action) => action,
                _ => {
                    return AttemptResult {
                        dead_end: true,
                        glory_success: 0,
                        despair_success: 0,
                        mental_left: 0,
                        spirit_left: 0,
                    };
                }
            };
            let succeeded = rng.unit() < self.chance(action, s.tier);
            let [(_, win, _), (_, lose, _)] = self.transitions(s, action);
            let next = if succeeded { win } else { lose };
            if let Some(log) = log.as_deref_mut() {
                log.push(Step { state: s, action, succeeded, next });
            }
            s = next;
        }
    }
}
