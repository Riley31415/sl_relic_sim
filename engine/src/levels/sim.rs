//! One player, from level 1 to the top.  Every attempt the plan is re-read
//! against the board as it stands, so a lucky or unlucky roll changes what
//! happens next.

use super::amplification;
use super::rules::{Bar, LevelRules, Quest, Relic, contains, totals};
use super::tables::{LocalTables, Table, TableKey, hit_chance};
use crate::rng::Rng;
use crate::solver::{Config, Strategy};

/// Memory slots per bar on inheritor `level`'s board: no relic rolled there
/// can hold more glory than this.
fn slots_at(level: usize) -> u8 {
    Config::for_level(level as i64, Strategy::default()).map_or(u8::MAX, |c| c.slots)
}

/// The summon economy.
pub struct Economy {
    pub relics: usize,
    pub per_summon: usize,
    pub per_attempt: u32,
    pub diamonds_per_summon: u64,
    /// relics of one type traded in for `convert_to` crit relics, once the
    /// climb is over
    pub convert_from: u32,
    pub convert_to: u32,
}

/// Everything fixed about the game, indexed by level.
pub struct Game {
    pub economy: Economy,
    /// what each level asks for (None below level 2)
    pub quests: Vec<Option<Quest>>,
    /// pity that reaches a level without its quest, by the level reached
    pub pity_needed: Vec<u32>,
    /// pity one attempt adds, by the level it is made at
    pub pity_gain: Vec<u32>,
}

/// What one batch of runs is asked to do.
#[derive(Clone, Debug)]
pub struct RunOptions {
    pub max_level: usize,
    /// relics of each type on hand before the first summon
    pub start_stock: Vec<u32>,
    pub pity: bool,
    /// crit relic amplification marks (percent), each farmed separately from
    /// the player at `max_level`, every other relic converted into crit relics;
    /// on reaching `max_level` the leftover relics are first spent on the crit
    /// relic, played for the first mark
    pub tiers: Vec<u8>,
}

/// The state at one level-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelUp {
    /// diamonds spent so far
    pub diamonds: u64,
    /// attempts made so far
    pub attempts: u64,
    /// reached by a full pity meter rather than the quest
    pub by_pity: bool,
    /// the attack relic's (glory, despair)
    pub atk: Relic,
    /// the crit relic's (glory, despair)
    pub crit: Relic,
}

/// One player's run: a LevelUp for every level from 1, and the final board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub levels: Vec<LevelUp>,
    pub board: Vec<Relic>,
}

/// Stop a malformed requirement table from looping forever.
const MAX_ESCALATIONS: u32 = 400;

#[derive(Clone)]
struct Player<'a> {
    economy: &'a Economy,
    states: Vec<Relic>,
    stock: Vec<u32>,
    summons: u64,
    attempts: u64,
    rng: Rng,
}

impl Player<'_> {
    fn summon(&mut self) {
        for _ in 0..self.economy.per_summon {
            let relic = self.rng.below(self.economy.relics);
            self.stock[relic] += 1;
        }
        self.summons += 1;
    }

    fn affordable(&self, relic: usize) -> bool {
        self.stock[relic] >= self.economy.per_attempt
    }

    /// What the plan does next, given the work `work` set out.
    fn choose(&self, quest: &Quest, rules: &LevelRules, level: usize, profiles: &[(usize, Bar)], todo: &[usize]) -> Move {
        choose(quest, rules, level, profiles, todo, &self.stock, self.economy.per_attempt)
    }

    /// Spend one attempt's worth of `relic` and roll it.
    fn attempt(&mut self, relic: usize, table: &Table) -> Relic {
        self.stock[relic] -= self.economy.per_attempt;
        self.attempts += 1;
        table.sample(self.rng.unit())
    }

    /// Trade every full ten of each other type for crit relics.
    fn convert(&mut self, crit: usize) {
        let (from, to) = (self.economy.convert_from, self.economy.convert_to);
        for relic in (0..self.stock.len()).filter(|&i| i != crit) {
            let lots = self.stock[relic] / from;
            self.stock[relic] -= lots * from;
            self.stock[crit] += lots * to;
        }
    }

    fn level_up(&self, by_pity: bool, atk: usize, crit: usize) -> LevelUp {
        LevelUp {
            diamonds: self.summons * self.economy.diamonds_per_summon,
            attempts: self.attempts,
            by_pity,
            atk: self.states[atk],
            crit: self.states[crit],
        }
    }
}

/// Play one run under `rules` (one entry per level reached).  `tracked` is
/// (attack relic, crit relic).
pub fn run(
    game: &Game,
    rules: &[Option<LevelRules>],
    options: &RunOptions,
    tracked: (usize, usize),
    tables: &mut LocalTables,
    rng: Rng,
) -> Result<Run, String> {
    let n = game.economy.relics;
    let mut player = Player {
        economy: &game.economy,
        states: vec![(0, 0); n],
        stock: options.start_stock.clone(),
        summons: 0,
        attempts: 0,
        rng,
    };
    let mut levels = vec![player.level_up(false, tracked.0, tracked.1)];
    let mut profiles: Vec<(usize, Bar)> = Vec::with_capacity(n);
    let mut todo: Vec<usize> = Vec::with_capacity(n);

    for level in 1..options.max_level {
        let goal = level + 1;
        let quest = game.quests[goal].as_ref().ok_or(format!("no requirement for level {goal}"))?;
        let rules = rules[goal].as_ref().ok_or(format!("no plan for level {goal}"))?;
        let full = if options.pity { game.pity_needed[goal] } else { u32::MAX };
        let gain = game.pity_gain[level];
        let (mut meter, mut escalation) = (0u32, 0u32);

        while meter < full && !quest.satisfied(&player.states) {
            if !work(quest, rules, &player.states, slots_at(level), &mut escalation, &mut profiles, &mut todo) {
                return Err(format!("stuck trying to reach level {goal}"));
            }
            // summon until something is worth attempting
            let next = loop {
                match player.choose(quest, rules, level, &profiles, &todo) {
                    Move::Summon => player.summon(),
                    roll => break roll,
                }
            };
            meter += gain;
            match next {
                Move::Filler { relic, bar, key } => {
                    let rolled = player.attempt(relic, tables.get(key));
                    if filler_keeps(quest, &player.states, relic, rolled, bar) {
                        player.states[relic] = rolled;
                    }
                }
                Move::Quest { relic, bar, key, closer } => {
                    let rolled = player.attempt(relic, tables.get(key));
                    if keeps(quest, closer, &player.states, relic, rolled, bar) {
                        player.states[relic] = rolled;
                    }
                }
                Move::Summon => unreachable!("the loop above summons"),
            }
        }
        levels.push(player.level_up(!quest.satisfied(&player.states), tracked.0, tracked.1));
    }

    // a run that goes on to the tiers first spends what the climb left over:
    // everything converted into crit relics and attempted for the first tier,
    // without summoning more
    let crit = tracked.1;
    if let (Some(&first), Some(top)) = (options.tiers.first(), levels.last_mut()) {
        let key = TableKey::Reach { level: options.max_level as u8, mark: first };
        player.convert(crit);
        while player.affordable(crit) {
            let rolled = player.attempt(crit, tables.get(key));
            if amplification(rolled) > amplification(player.states[crit]) {
                player.states[crit] = rolled;
            }
        }
        *top = player.level_up(top.by_pity, tracked.0, tracked.1);
    }

    // the tiers: each its own farm from the same top-level player, everything
    // converted into the crit relic and every attempt played for that tier's
    // mark, the best result kept
    for &mark in &options.tiers {
        let key = TableKey::Reach { level: options.max_level as u8, mark };
        let mut farm = player.clone();
        while amplification(farm.states[crit]) < f64::from(mark) {
            farm.convert(crit);
            if !farm.affordable(crit) {
                farm.summon();
                continue;
            }
            let rolled = farm.attempt(crit, tables.get(key));
            if amplification(rolled) > amplification(farm.states[crit]) {
                farm.states[crit] = rolled;
            }
        }
        levels.push(farm.level_up(false, tracked.0, tracked.1));
    }
    Ok(Run { levels, board: player.states })
}

/// What a plan does next at one level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// nothing the quest needs, and no spare relic for a filler, is affordable
    Summon,
    /// nothing the quest needs is affordable: a spare relic rolled for the
    /// pity, kept only if it moves toward `bar` without setting the quest back
    Filler { relic: usize, bar: Bar, key: TableKey },
    /// a relic the quest needs, rolled toward `bar` and played per `key`; with
    /// `closer`, any roll that narrows a total's gap is kept as well
    Quest { relic: usize, bar: Bar, key: TableKey, closer: bool },
}

/// The work this level asks for: the bar each relic is rolled toward, in
/// `profiles`, and the relics still short of theirs, in `todo`.  When
/// nothing is short the bars are pushed further, and `escalation` (how far)
/// stays raised for the rest of the level.  False if that never ends.
fn work(
    quest: &Quest,
    rules: &LevelRules,
    states: &[Relic],
    slots: u8,
    escalation: &mut u32,
    profiles: &mut Vec<(usize, Bar)>,
    todo: &mut Vec<usize>,
) -> bool {
    loop {
        plan(quest, rules, states, slots, profiles);
        for _ in 0..*escalation {
            escalate(quest, states, slots, profiles);
        }
        todo.clear();
        todo.extend(profiles.iter().filter(|(i, bar)| !bar.met_by(states[*i])).map(|p| p.0));
        if !todo.is_empty() {
            break;
        }
        *escalation += 1;
        if *escalation > MAX_ESCALATIONS {
            return false;
        }
    }
    if let Some((preferred, true)) = rules.prefer
        && todo.iter().any(|&i| contains(preferred, i))
    {
        todo.retain(|&i| contains(preferred, i));
    }
    true
}

/// Attempt with what is on hand, or summon?  A needed relic if any is
/// affordable, else a filler, else a summon.  Of the needed relics: on a
/// total, the one most likely to reach its bar in one attempt (a relic with
/// less glory and more despair takes a step more easily), sparing the
/// relics `rules.spare` keeps for later; otherwise the best-stocked.
fn choose(
    quest: &Quest,
    rules: &LevelRules,
    level: usize,
    profiles: &[(usize, Bar)],
    todo: &[usize],
    stock: &[u32],
    per_attempt: u32,
) -> Move {
    let affordable = |i: usize| stock[i] >= per_attempt;
    if !todo.iter().any(|&i| affordable(i)) {
        if let Some(bar) = rules.filler {
            let spare = (0..stock.len())
                .filter(|&i| affordable(i) && !contains(rules.locked, i) && !todo.contains(&i));
            if spare.clone().next().is_some() {
                let key = TableKey::score(level as u8, (1.0, 1.0));
                return Move::Filler { relic: best_stocked(stock, spare), bar, key };
            }
        }
        return Move::Summon;
    }
    let bar_of = |relic: usize| profiles.iter().find(|p| p.0 == relic).expect("a relic to do has a profile").1;
    let key_for = |bar: Bar| match rules.score {
        Some(weights) => TableKey::score(level as u8, weights),
        None => TableKey::Target { level: level as u8, glory: bar.glory, despair: bar.despair },
    };
    let mut ready: Vec<usize> = todo.iter().copied().filter(|&i| affordable(i)).collect();
    if let Some((preferred, _)) = rules.prefer
        && ready.iter().any(|&i| contains(preferred, i))
    {
        ready.retain(|&i| contains(preferred, i));
    }
    let relic = if quest.is_total() {
        if ready.iter().any(|&i| !contains(rules.spare, i)) {
            ready.retain(|&i| !contains(rules.spare, i));
        }
        // most likely to reach its bar; the best-stocked of equals
        let mut best: Option<(usize, f64)> = None;
        for &i in &ready {
            let bar = bar_of(i);
            let p = hit_chance(key_for(bar), bar);
            if best.is_none_or(|(b, bp)| p > bp + 1e-12 || ((p - bp).abs() <= 1e-12 && stock[i] > stock[b])) {
                best = Some((i, p));
            }
        }
        best.expect("an affordable relic to do").0
    } else {
        best_stocked(stock, ready.into_iter())
    };
    let bar = bar_of(relic);
    Move::Quest { relic, bar, key: key_for(bar), closer: rules.closer }
}

/// The plan's next move from a board seen at `level`, with no memory of the
/// attempts before it: the bars are pushed past the plan only as far as it
/// takes to leave something to roll.  Also the relics the quest work is on,
/// with their bars.  None if nothing would ever be left to roll.
pub(super) fn next_move(
    quest: &Quest,
    rules: &LevelRules,
    level: usize,
    states: &[Relic],
    stock: &[u32],
    per_attempt: u32,
) -> Option<(Move, Vec<(usize, Bar)>)> {
    let (mut escalation, mut profiles, mut todo) = (0, Vec::new(), Vec::new());
    if !work(quest, rules, states, slots_at(level), &mut escalation, &mut profiles, &mut todo) {
        return None;
    }
    let bars = profiles.iter().filter(|(i, _)| todo.contains(i)).copied().collect();
    Some((choose(quest, rules, level, &profiles, &todo, stock, per_attempt), bars))
}

/// Of `candidates`, the relic we hold the most of (first on a tie).
fn best_stocked(stock: &[u32], candidates: impl Iterator<Item = usize>) -> usize {
    let mut best: Option<usize> = None;
    for relic in candidates {
        if best.is_none_or(|b| stock[relic] > stock[b]) {
            best = Some(relic);
        }
    }
    best.expect("best_stocked needs at least one candidate")
}

/// The bar each relic is rolled toward for this level, in relic order, on a
/// board of `slots` slots per bar.
fn plan(quest: &Quest, rules: &LevelRules, states: &[Relic], slots: u8, out: &mut Vec<(usize, Bar)>) {
    out.clear();
    match quest {
        Quest::Relics { which, bar } => out.extend(which.iter().map(|&i| (i, *bar))),
        Quest::Each(bar) => {
            // every relic has to clear this one, banked or not
            let despair = match (bar.despair, rules.base.despair) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            let merged = Bar::new(bar.glory.max(rules.base.glory), despair);
            out.extend((0..states.len()).map(|i| (i, merged)));
        }
        Quest::Total { glory, despair } => {
            let pool = (0..states.len()).filter(|&i| !contains(rules.locked, i));
            if rules.surgical {
                repair(states, *glory, *despair, slots, pool, out);
            } else {
                out.extend(pool.map(|i| (i, rules.base)));
            }
        }
    }
}

/// Single-step fixes for a total: shave a despair off the worst relics while
/// the budget is blown, otherwise ask each relic for one more glory - each
/// that has room for it: a relic with every slot a glory success already
/// cannot be asked for more (it would be rolled for nothing).
fn repair(
    states: &[Relic],
    need_glory: u32,
    need_despair: u32,
    slots: u8,
    pool: impl Iterator<Item = usize> + Clone,
    out: &mut Vec<(usize, Bar)>,
) {
    let (glory, despair) = totals(states);
    if despair > need_despair {
        // 2 -> 1 is far cheaper than 1 -> 0, so never ask a relic below 1
        let worst = pool.clone().map(|i| states[i].1).max().unwrap_or(0).max(2);
        out.extend(
            pool.clone()
                .filter(|&i| states[i].1 >= worst)
                .map(|i| (i, Bar::new(states[i].0, Some(states[i].1 - 1)))),
        );
        if !out.is_empty() {
            return;
        }
    }
    let step = u8::from(glory < need_glory);
    out.extend(
        pool.filter(|&i| step == 0 || states[i].0 < slots).map(|i| (i, Bar::new(states[i].0 + step, Some(states[i].1)))),
    );
}

/// Nothing left to roll but a total is still short: ask one relic for more
/// (more glory only of a relic with room for it on `slots` slots).
fn escalate(quest: &Quest, states: &[Relic], slots: u8, profiles: &mut [(usize, Bar)]) {
    let Quest::Total { despair: need_despair, .. } = *quest else { return };
    if profiles.is_empty() {
        return;
    }
    let first_max = |key: &dyn Fn(Relic) -> (i32, i32)| {
        let mut best = 0;
        for k in 1..profiles.len() {
            if key(states[profiles[k].0]) > key(states[profiles[best].0]) {
                best = k;
            }
        }
        best
    };
    if totals(states).1 > need_despair {
        // the relic with the most despair loses one
        let k = first_max(&|(g, d)| (i32::from(d), -i32::from(g)));
        let (g, d) = states[profiles[k].0];
        let bar = &mut profiles[k].1;
        *bar = Bar::new(bar.glory.max(g), Some(d.saturating_sub(1)));
    } else {
        // the relic with the least glory gains one, if any has room
        let k = first_max(&|(g, d)| (-i32::from(g), i32::from(d)));
        let g = states[profiles[k].0].0;
        if g < slots {
            profiles[k].1.glory = g + 1;
        }
    }
}

/// Keep a roll over what the relic already has?  Anything that dominates, or
/// the first result to clear the bar.  A dead end lands as (0, 0) and so is
/// refused by the same rule.
fn accept(old: Relic, new: Relic, bar: Bar) -> bool {
    let dominates = new.0 >= old.0 && new.1 <= old.1 && new != old;
    dominates || (bar.met_by(new) && !bar.met_by(old))
}

/// Keep a quest roll?  As `accept` has it, or, with `closer` on a total
/// level, if it narrows the board's gap.
pub(super) fn keeps(
    quest: &Quest,
    closer: bool,
    states: &[Relic],
    pick: usize,
    new: Relic,
    bar: Bar,
) -> bool {
    accept(states[pick], new, bar)
        || (closer && quest.is_total() && quest.gap_after(states, pick, new) < quest.gap(states))
}

/// Keep a filler roll?  Only if it helps toward the filler bar and cannot set
/// back the quest in progress.
pub(super) fn filler_keeps(quest: &Quest, states: &[Relic], pick: usize, new: Relic, filler: Bar) -> bool {
    let old = states[pick];
    if !accept(old, new, filler) {
        return false;
    }
    match quest {
        Quest::Total { .. } => quest.gap_after(states, pick, new) <= quest.gap(states),
        // a relic the level needs must not drop back below the bar it clears
        Quest::Relics { which, bar } if which.contains(&pick) => bar.met_by(new) || !bar.met_by(old),
        Quest::Relics { .. } => true,
        Quest::Each(bar) => bar.met_by(new) || !bar.met_by(old),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(glory: u32, despair: u32) -> Quest {
        Quest::Total { glory, despair }
    }

    #[test]
    fn a_roll_is_kept_when_it_dominates_or_first_clears_the_bar() {
        let bar = Bar::new(4, Some(2));
        assert!(accept((3, 2), (4, 2), bar)); // better glory
        assert!(accept((4, 2), (4, 1), bar)); // less despair
        assert!(!accept((4, 2), (4, 2), bar)); // no change
        assert!(accept((3, 0), (4, 2), bar)); // worse despair, but clears the bar
        assert!(!accept((5, 1), (4, 2), bar)); // already clear: only a dominating roll
        assert!(!accept((2, 1), (0, 0), bar)); // a dead end is never taken
    }

    #[test]
    fn a_roll_short_of_the_bar_but_nearer_is_kept() {
        // a 7/3 relic asked for 7/1: a 7/2 misses the bar but is a step
        // nearer - kept, for a named quest or a total, closer rule or not
        let bar = Bar::new(7, Some(1));
        let states = [(7, 3), (8, 2)];
        let named = Quest::Relics { which: vec![0], bar };
        for closer in [false, true] {
            assert!(keeps(&named, closer, &states, 0, (7, 2), bar));
            assert!(keeps(&total(15, 3), closer, &states, 0, (7, 2), bar));
            assert!(!keeps(&named, closer, &states, 0, (6, 2), bar)); // a glory given up for it: not nearer
        }
    }

    #[test]
    fn repair_shaves_despair_from_the_worst_relics_first() {
        let states = [(4, 3), (4, 2), (5, 3), (4, 1)];
        let mut out = Vec::new();
        repair(&states, 10, 5, 9, 0..4, &mut out);
        assert_eq!(out, vec![(0, Bar::new(4, Some(2))), (2, Bar::new(5, Some(2)))]);
    }

    #[test]
    fn repair_adds_glory_once_despair_fits() {
        let states = [(4, 1), (3, 0)];
        let mut out = Vec::new();
        repair(&states, 9, 5, 9, 0..2, &mut out);
        assert_eq!(out, vec![(0, Bar::new(5, Some(1))), (1, Bar::new(4, Some(0)))]);
        // nothing short: hold every relic where it is
        out.clear();
        repair(&states, 7, 5, 9, 0..2, &mut out);
        assert_eq!(out, vec![(0, Bar::new(4, Some(1))), (1, Bar::new(3, Some(0)))]);
    }

    #[test]
    fn repair_never_asks_a_relic_below_one_despair() {
        let states = [(4, 1), (4, 1)];
        let mut out = Vec::new();
        repair(&states, 0, 1, 9, 0..2, &mut out);
        // over budget, but no relic sits at 2+: fall through to holding
        assert_eq!(out, vec![(0, Bar::new(4, Some(1))), (1, Bar::new(4, Some(1)))]);
    }

    #[test]
    fn escalation_targets_the_relic_holding_the_board_back() {
        let states = [(5, 1), (3, 2), (4, 3)];
        let mut profiles: Vec<_> = (0..3).map(|i| (i, Bar::new(4, Some(2)))).collect();
        escalate(&total(12, 5), &states, 9, &mut profiles); // despair 6 > 5
        assert_eq!(profiles[2].1, Bar::new(4, Some(2)));
        let states = [(5, 1), (3, 1), (4, 1)];
        escalate(&total(13, 5), &states, 9, &mut profiles); // glory short
        assert_eq!(profiles[1].1, Bar::new(4, Some(2)));
    }

    #[test]
    fn a_total_takes_the_step_most_likely_to_land() {
        let rules = LevelRules {
            base: Bar::new(5, Some(2)),
            surgical: true,
            closer: true,
            score: None,
            filler: None,
            locked: 0,
            spare: 0,
            prefer: None,
        };
        // glory short: 8/1 -> 9/1, or 7/2 -> 8/2 (despair room to spare)
        let profiles = vec![(0, Bar::new(9, Some(1))), (1, Bar::new(8, Some(2)))];
        let pick = |rules: &LevelRules, stock: &[u32]| match choose(&total(30, 10), rules, 18, &profiles, &[0, 1], stock, 10) {
            Move::Quest { relic, .. } => relic,
            other => panic!("expected a quest roll, got {other:?}"),
        };
        // the easier step, though the other relic is far better stocked
        assert_eq!(pick(&rules, &[300, 20]), 1);
        // unless it is spared for later and the other is affordable
        let spare = LevelRules { spare: 0b10, ..rules.clone() };
        assert_eq!(pick(&spare, &[300, 20]), 0);
        // a spared relic is still used when it is all there is
        assert_eq!(pick(&spare, &[5, 20]), 1);
    }

    #[test]
    fn a_relic_with_every_slot_a_glory_is_never_asked_for_more() {
        // 9 slots: the 9/1 relic has no room for a 10th glory success
        let states = [(9, 1), (7, 1), (8, 2)];
        let mut out = Vec::new();
        repair(&states, 30, 5, 9, 0..3, &mut out); // despair fits, glory short
        assert_eq!(out, vec![(1, Bar::new(8, Some(1))), (2, Bar::new(9, Some(2)))]);
        // and escalation asks no more glory of a full relic either
        let full = [(9, 1), (9, 0)];
        let mut profiles: Vec<_> = (0..2).map(|i| (i, Bar::new(9, Some(1)))).collect();
        escalate(&total(30, 5), &full, 9, &mut profiles);
        assert!(profiles.iter().all(|(_, bar)| bar.glory <= 9));
    }

    #[test]
    fn filler_never_sets_the_quest_back() {
        let named = Quest::Relics { which: vec![0], bar: Bar::new(5, Some(2)) };
        let states = [(5, 2), (0, 0)];
        // the named relic already clears its bar: a roll off it is refused
        assert!(!filler_keeps(&named, &states, 0, (6, 3), Bar::new(4, Some(2))));
        // any other relic is free to move toward the filler bar
        assert!(filler_keeps(&named, &states, 1, (4, 2), Bar::new(4, Some(2))));
        // on a total, clearing the filler bar must not widen the gap
        let mut board = vec![(5, 3)];
        board.extend([(5, 1); 10]);
        board.push((4, 1)); // exactly 59 glory
        assert!(!filler_keeps(&total(59, 17), &board, 0, (4, 2), Bar::new(4, Some(2))));
    }

    #[test]
    fn an_each_level_merges_its_bar_with_the_plan() {
        let rules = LevelRules {
            base: Bar::new(5, Some(1)),
            surgical: false,
            closer: false,
            score: None,
            filler: None,
            locked: 0b01,
            spare: 0,
            prefer: None,
        };
        let mut out = Vec::new();
        plan(&Quest::Each(Bar::new(4, Some(2))), &rules, &[(0, 0), (0, 0)], 9, &mut out);
        // the stricter of each, and banked relics are not exempt
        assert_eq!(out, vec![(0, Bar::new(5, Some(1))), (1, Bar::new(5, Some(1)))]);
        plan(&total(10, 10), &rules, &[(0, 0), (0, 0)], 9, &mut out);
        assert_eq!(out, vec![(1, Bar::new(5, Some(1)))]); // total work skips banked relics
    }

    #[test]
    fn totals_and_bars() {
        assert!(Bar::new(3, None).met_by((3, 5)));
        assert!(!Bar::new(3, Some(2)).met_by((3, 3)));
        let q = total(10, 3);
        assert_eq!(q.gap(&[(4, 2), (4, 2)]), 2 + 1);
        assert!(q.satisfied(&[(5, 1), (5, 2)]));
    }

    #[test]
    fn a_table_samples_by_cumulative_probability() {
        let table = Table::new(vec![(0, 0), (3, 1), (5, 0)], vec![0.25, 0.75, 1.0]);
        assert_eq!(table.sample(0.0), (0, 0));
        assert_eq!(table.sample(0.25), (3, 1));
        assert_eq!(table.sample(0.9999), (5, 0));
    }
}
