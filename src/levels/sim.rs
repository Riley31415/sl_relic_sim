//! One player, from level 1 to the top.  Every attempt the plan is re-read
//! against the board as it stands, so a lucky or unlucky roll changes what
//! happens next.

use super::rules::{Bar, LevelRules, Quest, Relic, contains, totals};
use super::tables::{LocalTables, Table, TableKey};
use crate::rng::Rng;

/// The summon economy.
pub struct Economy {
    pub relics: usize,
    pub per_summon: usize,
    pub per_attempt: u32,
    pub diamonds_per_summon: u64,
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

    /// Spend one attempt's worth of `relic` and roll it.
    fn attempt(&mut self, relic: usize, table: &Table) -> Relic {
        self.stock[relic] -= self.economy.per_attempt;
        self.attempts += 1;
        table.sample(self.rng.unit())
    }

    /// Of `candidates`, the relic we hold the most of (first on a tie).
    fn best_stocked(&self, candidates: impl Iterator<Item = usize>) -> usize {
        let mut best: Option<usize> = None;
        for relic in candidates {
            if best.is_none_or(|b| self.stock[relic] > self.stock[b]) {
                best = Some(relic);
            }
        }
        best.expect("best_stocked needs at least one candidate")
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
            plan(quest, rules, &player.states, &mut profiles);
            for _ in 0..escalation {
                escalate(quest, &player.states, &mut profiles);
            }
            todo.clear();
            todo.extend(profiles.iter().filter(|(i, bar)| !bar.met_by(player.states[*i])).map(|p| p.0));
            if todo.is_empty() {
                escalation += 1;
                if escalation > MAX_ESCALATIONS {
                    return Err(format!("stuck trying to reach level {goal}"));
                }
                continue;
            }
            if let Some((preferred, true)) = rules.prefer
                && todo.iter().any(|&i| contains(preferred, i))
            {
                todo.retain(|&i| contains(preferred, i));
            }

            // summon, or attempt with what is already on hand?
            let mut filler_pick = None;
            while !todo.iter().any(|&i| player.affordable(i)) {
                if rules.filler.is_some() {
                    let spare = (0..n).filter(|&i| {
                        player.affordable(i) && !contains(rules.locked, i) && !todo.contains(&i)
                    });
                    if spare.clone().next().is_some() {
                        filler_pick = Some(player.best_stocked(spare));
                        break;
                    }
                }
                player.summon();
            }
            meter += gain;

            if let (Some(pick), Some(filler)) = (filler_pick, rules.filler) {
                // nothing the quest needs is ready: roll a spare relic for the pity
                let rolled = player.attempt(pick, tables.get(TableKey::score(level as u8, (1.0, 1.0))));
                if filler_keeps(quest, &player.states, pick, rolled, filler) {
                    player.states[pick] = rolled;
                }
                continue;
            }

            let affordable = todo.iter().copied().filter(|&i| player.affordable(i));
            let pick = match rules.prefer {
                Some((preferred, _)) if affordable.clone().any(|i| contains(preferred, i)) => {
                    player.best_stocked(affordable.filter(|&i| contains(preferred, i)))
                }
                _ => player.best_stocked(affordable),
            };
            let bar = profiles.iter().find(|p| p.0 == pick).expect("pick has a profile").1;
            let key = match rules.score {
                Some(weights) => TableKey::score(level as u8, weights),
                None => TableKey::Target { level: level as u8, glory: bar.glory, despair: bar.despair },
            };
            let rolled = player.attempt(pick, tables.get(key));
            let old = player.states[pick];
            let mut take = accept(old, rolled, bar);
            if !take && rules.closer && quest.is_total() {
                let before = quest.gap(&player.states);
                player.states[pick] = rolled;
                take = quest.gap(&player.states) < before;
            }
            player.states[pick] = if take { rolled } else { old };
        }
        levels.push(player.level_up(!quest.satisfied(&player.states), tracked.0, tracked.1));
    }
    Ok(Run { levels, board: player.states })
}

/// The bar each relic is rolled toward for this level, in relic order.
fn plan(quest: &Quest, rules: &LevelRules, states: &[Relic], out: &mut Vec<(usize, Bar)>) {
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
                repair(states, *glory, *despair, pool, out);
            } else {
                out.extend(pool.map(|i| (i, rules.base)));
            }
        }
    }
}

/// Single-step fixes for a total: shave a despair off the worst relics while
/// the budget is blown, otherwise ask each relic for one more glory.
fn repair(
    states: &[Relic],
    need_glory: u32,
    need_despair: u32,
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
    out.extend(pool.map(|i| (i, Bar::new(states[i].0 + step, Some(states[i].1)))));
}

/// Nothing left to roll but a total is still short: ask one relic for more.
fn escalate(quest: &Quest, states: &[Relic], profiles: &mut [(usize, Bar)]) {
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
        // the relic with the least glory gains one
        let k = first_max(&|(g, d)| (-i32::from(g), i32::from(d)));
        let g = states[profiles[k].0].0;
        profiles[k].1.glory = g + 1;
    }
}

/// Keep a roll over what the relic already has?  Anything that dominates, or
/// the first result to clear the bar.  A dead end lands as (0, 0) and so is
/// refused by the same rule.
fn accept(old: Relic, new: Relic, bar: Bar) -> bool {
    let dominates = new.0 >= old.0 && new.1 <= old.1 && new != old;
    dominates || (bar.met_by(new) && !bar.met_by(old))
}

/// Keep a filler roll?  Only if it helps toward the filler bar and cannot set
/// back the quest in progress.
fn filler_keeps(quest: &Quest, states: &[Relic], pick: usize, new: Relic, filler: Bar) -> bool {
    let old = states[pick];
    if !accept(old, new, filler) {
        return false;
    }
    match quest {
        Quest::Total { .. } => {
            let mut trial = states.to_vec();
            trial[pick] = new;
            quest.gap(&trial) <= quest.gap(states)
        }
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
    fn repair_shaves_despair_from_the_worst_relics_first() {
        let states = [(4, 3), (4, 2), (5, 3), (4, 1)];
        let mut out = Vec::new();
        repair(&states, 10, 5, 0..4, &mut out);
        assert_eq!(out, vec![(0, Bar::new(4, Some(2))), (2, Bar::new(5, Some(2)))]);
    }

    #[test]
    fn repair_adds_glory_once_despair_fits() {
        let states = [(4, 1), (3, 0)];
        let mut out = Vec::new();
        repair(&states, 9, 5, 0..2, &mut out);
        assert_eq!(out, vec![(0, Bar::new(5, Some(1))), (1, Bar::new(4, Some(0)))]);
        // nothing short: hold every relic where it is
        out.clear();
        repair(&states, 7, 5, 0..2, &mut out);
        assert_eq!(out, vec![(0, Bar::new(4, Some(1))), (1, Bar::new(3, Some(0)))]);
    }

    #[test]
    fn repair_never_asks_a_relic_below_one_despair() {
        let states = [(4, 1), (4, 1)];
        let mut out = Vec::new();
        repair(&states, 0, 1, 0..2, &mut out);
        // over budget, but no relic sits at 2+: fall through to holding
        assert_eq!(out, vec![(0, Bar::new(4, Some(1))), (1, Bar::new(4, Some(1)))]);
    }

    #[test]
    fn escalation_targets_the_relic_holding_the_board_back() {
        let states = [(5, 1), (3, 2), (4, 3)];
        let mut profiles: Vec<_> = (0..3).map(|i| (i, Bar::new(4, Some(2)))).collect();
        escalate(&total(12, 5), &states, &mut profiles); // despair 6 > 5
        assert_eq!(profiles[2].1, Bar::new(4, Some(2)));
        let states = [(5, 1), (3, 1), (4, 1)];
        escalate(&total(13, 5), &states, &mut profiles); // glory short
        assert_eq!(profiles[1].1, Bar::new(4, Some(2)));
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
            prefer: None,
        };
        let mut out = Vec::new();
        plan(&Quest::Each(Bar::new(4, Some(2))), &rules, &[(0, 0), (0, 0)], &mut out);
        // the stricter of each, and banked relics are not exempt
        assert_eq!(out, vec![(0, Bar::new(5, Some(1))), (1, Bar::new(5, Some(1)))]);
        plan(&total(10, 10), &rules, &[(0, 0), (0, 0)], &mut out);
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
