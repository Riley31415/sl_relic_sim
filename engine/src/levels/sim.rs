//! One player, from level 1 to the top.  Every attempt the plan is re-read
//! against the board as it stands, so a lucky or unlucky roll changes what
//! happens next.

use super::amplification;
use super::rules::{Bar, LevelRules, Quest, Relic, contains, totals};
use super::tables::{LocalTables, Table, TableKey, expected_closing};
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

    /// What the plan does next, given the work `work` set out, the
    /// attempts still needed to fill the pity bar and the goal's totals.
    #[allow(clippy::too_many_arguments)]
    fn choose(
        &self,
        quest: &Quest,
        rules: &LevelRules,
        level: usize,
        profiles: &[(usize, Bar)],
        todo: &[usize],
        pity_left: Option<u32>,
        horizon: Horizon,
    ) -> Move {
        let ctx = Context { quest, rules, level, horizon };
        choose(&ctx, profiles, todo, &self.states, &self.stock, self.economy.per_attempt, pity_left)
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
    let mut partial = start(game, options, tracked, rng);
    climb(game, rules, options, tracked, tables, &mut partial, options.max_level)?;
    Ok(finish(options, tracked, tables, partial))
}

/// A run part-way through: the player as it stands on reaching level
/// `levels.len()`, and a LevelUp for every level so far.  Resuming it plays
/// on exactly as the run would have (its luck is in the player), so runs that
/// share every level's rules below some level can share that much of the
/// work.
#[derive(Clone)]
pub struct Partial<'a> {
    player: Player<'a>,
    levels: Vec<LevelUp>,
}

impl Partial<'_> {
    /// The level reached so far.
    pub fn level(&self) -> usize {
        self.levels.len()
    }
}

/// A run at level 1, nothing spent.
pub fn start<'a>(game: &'a Game, options: &RunOptions, tracked: (usize, usize), rng: Rng) -> Partial<'a> {
    let player = Player {
        economy: &game.economy,
        states: vec![(0, 0); game.economy.relics],
        stock: options.start_stock.clone(),
        summons: 0,
        attempts: 0,
        rng,
    };
    let levels = vec![player.level_up(false, tracked.0, tracked.1)];
    Partial { player, levels }
}

/// Play `partial` on under `rules` until it reaches level `until` (no
/// further than `options.max_level`).
pub fn climb(
    game: &Game,
    rules: &[Option<LevelRules>],
    options: &RunOptions,
    tracked: (usize, usize),
    tables: &mut LocalTables,
    partial: &mut Partial,
    until: usize,
) -> Result<(), String> {
    let Partial { player, levels } = partial;
    let n = game.economy.relics;
    let mut profiles: Vec<(usize, Bar)> = Vec::with_capacity(n);
    let mut todo: Vec<usize> = Vec::with_capacity(n);

    for level in levels.len()..until.min(options.max_level) {
        let goal = level + 1;
        let quest = game.quests[goal].as_ref().ok_or(format!("no requirement for level {goal}"))?;
        let rules = rules[goal].as_ref().ok_or(format!("no plan for level {goal}"))?;
        let horizon = game.horizon_for(goal, options.max_level, rules.ahead);
        let full = if options.pity { game.pity_needed[goal] } else { u32::MAX };
        let gain = game.pity_gain[level];
        let mut meter = 0u32;

        while meter < full && !quest.satisfied(&player.states) {
            if !work(quest, rules, &player.states, slots_at(level), horizon, &mut profiles, &mut todo) {
                return Err(format!("stuck trying to reach level {goal}"));
            }
            // summon until something is worth attempting
            let pity_left = options.pity.then(|| (full - meter).div_ceil(gain));
            let next = loop {
                match player.choose(quest, rules, level, &profiles, &todo, pity_left, horizon) {
                    Move::Summon => player.summon(),
                    roll => break roll,
                }
            };
            meter += gain;
            match next {
                Move::Quest { relic, bar, key } | Move::Filler { relic, bar, key } => {
                    let rolled = player.attempt(relic, tables.get(key));
                    if keeps(quest, horizon, &player.states, relic, rolled, bar) {
                        player.states[relic] = rolled;
                    }
                }
                Move::Summon => unreachable!("the loop above summons"),
            }
        }
        levels.push(player.level_up(!quest.satisfied(&player.states), tracked.0, tracked.1));
    }
    Ok(())
}

/// A run at the top (see `climb`) to its end: the crit relic farmed to each
/// of `options.tiers`.
pub fn finish(options: &RunOptions, tracked: (usize, usize), tables: &mut LocalTables, partial: Partial) -> Run {
    let Partial { player, mut levels } = partial;
    let crit = tracked.1;
    let mut board = player.states.clone();
    // the top level's own row: what the climb left over converted into crit
    // relics and spent on the crit relic, without summoning more, every
    // attempt for the most amplification above what it has
    if options.max_level == super::MAX_LEVEL
        && let Some(top) = levels.last_mut()
    {
        let best = 5.0 * f64::from(slots_at(options.max_level));
        let mut spent = player.clone();
        spent.convert(crit);
        while spent.affordable(crit) && amplification(spent.states[crit]) < best {
            let key = TableKey::above(options.max_level as u8, 0, spent.states[crit]);
            let rolled = spent.attempt(crit, tables.get(key));
            if amplification(rolled) > amplification(spent.states[crit]) {
                spent.states[crit] = rolled;
            }
        }
        *top = spent.level_up(top.by_pity, tracked.0, tracked.1);
        board = spent.states;
    }

    // the tiers: each its own farm from the moment the top level is reached,
    // the leftovers and everything after converted into the crit relic and
    // every attempt played for that tier's mark, the best result kept
    for &mark in &options.tiers {
        let mut farm = player.clone();
        while amplification(farm.states[crit]) < f64::from(mark) {
            farm.convert(crit);
            if !farm.affordable(crit) {
                farm.summon();
                continue;
            }
            let key = TableKey::above(options.max_level as u8, mark, farm.states[crit]);
            let rolled = farm.attempt(crit, tables.get(key));
            if amplification(rolled) > amplification(farm.states[crit]) {
                farm.states[crit] = rolled;
            }
        }
        levels.push(farm.level_up(false, tracked.0, tracked.1));
    }
    Run { levels, board }
}

/// What a plan does next at one level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// nothing the quest needs, and no spare relic for a filler, is affordable
    Summon,
    /// nothing the quest needs is affordable: a spare relic rolled for the
    /// pity (earned whatever the roll), played and kept as a quest roll -
    /// `bar` its own bar if the quest has one for it (none: kept only if
    /// better on both bars, or on a total if it narrows the gap to its totals)
    Filler { relic: usize, bar: Bar, key: TableKey },
    /// a relic the quest needs, rolled toward `bar` and played per `key`,
    /// closing the quest's gap; kept if it narrows it (see `keeps`)
    Quest { relic: usize, bar: Bar, key: TableKey },
}

/// The work this level asks for: the bar each relic is rolled toward, in
/// `profiles`, and the relics still short of theirs, in `todo` - from the
/// board alone.  When nothing is short the bars are pushed further, as far
/// as it takes to leave something to roll.  With `rules.trades`, a relic
/// short of its bar that the keep rule ranks at least as high as the bar (a
/// trade it took: 6/3 for 5/2, the gap as near and more glory) has as good
/// as met it, and once every relic has the bars are pushed too (the traded
/// relics stay in the work).  False if that never ends.
fn work(
    quest: &Quest,
    rules: &LevelRules,
    states: &[Relic],
    slots: u8,
    horizon: Horizon,
    profiles: &mut Vec<(usize, Bar)>,
    todo: &mut Vec<usize>,
) -> bool {
    let traded = |i: usize, bar: Bar| {
        let point = (bar.glory, bar.despair.unwrap_or(states[i].1));
        !bar.met_by(states[i]) && rank(quest, horizon, states, i, states[i], bar) >= rank(quest, horizon, states, i, point, bar)
    };
    let (mut escalation, mut as_good) = (0, Vec::new());
    loop {
        plan(quest, rules, states, slots, profiles);
        if escalation == 0 && rules.trades {
            as_good.extend(profiles.iter().filter(|&&(i, bar)| traded(i, bar)).map(|p| p.0));
        }
        let mut pushed = true;
        for _ in 0..escalation {
            pushed = escalate(quest, states, slots, profiles, &as_good);
        }
        todo.clear();
        todo.extend(profiles.iter().filter(|(i, bar)| !bar.met_by(states[*i])).map(|p| p.0));
        // done once there is more than traded relics to work - or nothing
        // more to push, and they are all there is
        if todo.iter().any(|i| !as_good.contains(i)) || (!pushed && !todo.is_empty()) {
            break;
        }
        escalation += 1;
        if escalation > MAX_ESCALATIONS {
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

/// The totals the play closes the gap to (glory needed, despair allowed),
/// if there are any (`Game::horizon_for`).
pub type Horizon = Option<(u32, u32)>;

/// What one decision is made for: the quest, the plan's rules for it, the
/// level it is made at and the totals it closes the gap to.
struct Context<'a> {
    quest: &'a Quest,
    rules: &'a LevelRules,
    level: usize,
    horizon: Horizon,
}

/// Attempt with what is on hand, or summon?  A needed relic if any is
/// affordable, else a filler (with `rules.filler_within`, only when the spare
/// relics on hand can fill the pity bar - `pity_left` attempts off, None if
/// not known - in that many attempts or fewer), else a summon.  Of the
/// needed relics: on a total, the one expected to close the most of the gap
/// in one attempt (a relic with less glory and more
/// despair takes a step more easily) - a relic a later level names too: it
/// is no better kept back; otherwise the best-stocked.  Every attempt is played as
/// `play` has it.
fn choose(
    ctx: &Context,
    profiles: &[(usize, Bar)],
    todo: &[usize],
    states: &[Relic],
    stock: &[u32],
    per_attempt: u32,
    pity_left: Option<u32>,
) -> Move {
    let (quest, rules) = (ctx.quest, ctx.rules);
    let affordable = |i: usize| stock[i] >= per_attempt;
    // a relic's bar for this level's work, if it has one
    let bar_of = |relic: usize| profiles.iter().find(|p| p.0 == relic).map_or(Bar::new(0, None), |p| p.1);
    let key_for = |relic: usize| play(ctx, states, relic, bar_of(relic));
    if !todo.iter().any(|&i| affordable(i)) {
        if rules.filler {
            let spare = (0..stock.len())
                .filter(|&i| affordable(i) && !contains(rules.locked, i) && !todo.contains(&i));
            let on_hand: u32 = spare.clone().map(|i| stock[i] / per_attempt).sum();
            let in_reach = match rules.filler_within {
                None => true,
                Some(limit) => pity_left.is_some_and(|left| left <= limit && left <= on_hand),
            };
            if in_reach && spare.clone().next().is_some() {
                let relic = best_stocked(stock, spare);
                return Move::Filler { relic, bar: bar_of(relic), key: key_for(relic) };
            }
        }
        return Move::Summon;
    }
    let mut ready: Vec<usize> = todo.iter().copied().filter(|&i| affordable(i)).collect();
    if let Some((preferred, _)) = rules.prefer
        && ready.iter().any(|&i| contains(preferred, i))
    {
        ready.retain(|&i| contains(preferred, i));
    }
    let relic = if quest.is_total() {
        // the most of the gap closed; the best-stocked of equals
        let mut best: Option<(usize, f64)> = None;
        for &i in &ready {
            let key = key_for(i);
            let TableKey::Close { glory, despair, .. } = key else { unreachable!("a total plays to close the gap") };
            let p = expected_closing(key, (glory, despair), states[i]);
            if best.is_none_or(|(b, bp)| p > bp + 1e-12 || ((p - bp).abs() <= 1e-12 && stock[i] > stock[b])) {
                best = Some((i, p));
            }
        }
        best.expect("an affordable relic to do").0
    } else {
        best_stocked(stock, ready.into_iter())
    };
    Move::Quest { relic, bar: bar_of(relic), key: key_for(relic) }
}

/// How an attempt on relic `pick` is played: a relic the quest holds to a
/// bar (`named_bar`) for the best chance of it, then to close the gap to the
/// totals it plays to; any other relic to close that gap - however little
/// this level asks (9/0 beats a 9/1 that clears it); then, either way, more
/// glory and less despair, weighed as those totals need them.
fn play(ctx: &Context, states: &[Relic], pick: usize, bar: Bar) -> TableKey {
    let level = ctx.level as u8;
    let (glory, despair) = aim(ctx.horizon, states, pick);
    match named_bar(ctx.quest, pick, bar) {
        Some(bar) => TableKey::target(level, bar, (glory, despair), states[pick]),
        None => TableKey::close(level, glory, despair, states[pick]).tie(tie(ctx.quest, ctx.horizon, states)),
    }
}

/// The bar the quest holds relic `pick` to, if any: a named relic's on a
/// named level, every relic's on an every-relic level (`bar`, the plan's
/// bar for it).  A total holds no relic to a bar.
pub(super) fn named_bar(quest: &Quest, pick: usize, bar: Bar) -> Option<Bar> {
    match quest {
        Quest::Relics { which, .. } if which.contains(&pick) => Some(bar),
        Quest::Each(_) => Some(bar),
        _ => None,
    }
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
    pity_left: Option<u32>,
    horizon: Horizon,
) -> Option<(Move, Vec<(usize, Bar)>)> {
    let (mut profiles, mut todo) = (Vec::new(), Vec::new());
    if !work(quest, rules, states, slots_at(level), horizon, &mut profiles, &mut todo) {
        return None;
    }
    let bars = profiles.iter().filter(|(i, _)| todo.contains(i)).copied().collect();
    let ctx = Context { quest, rules, level, horizon };
    Some((choose(&ctx, &profiles, &todo, states, stock, per_attempt, pity_left), bars))
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

/// Single-step fixes for a total: shave a despair off any relic with 2 or
/// more while the budget is blown (which one is `choose`'s call: the likeliest
/// step, so the worst relic first - an 8/3 drops to 8/2 more easily than an
/// 8/2 to 8/1 - but an 8/2 rather than nothing when the worst cannot be
/// afforded), otherwise ask each relic for one more glory - each
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
        out.extend(
            pool.clone().filter(|&i| states[i].1 >= 2).map(|i| (i, Bar::new(states[i].0, Some(states[i].1 - 1)))),
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
/// - less despair while the total has too much, the relic with the most
/// first, otherwise more glory, the relic with the least first (and only of
/// a relic with room for it on `slots` slots) - never one in `skip`.  False
/// if no relic can be asked for more.
fn escalate(quest: &Quest, states: &[Relic], slots: u8, profiles: &mut [(usize, Bar)], skip: &[usize]) -> bool {
    let Quest::Total { despair: need_despair, .. } = *quest else { return false };
    let over = totals(states).1 > need_despair;
    let room = |(g, d): Relic| if over { d > 0 } else { g < slots };
    // the relic most worth asking: the most despair (then least glory), or the least glory (then most despair)
    let key = |(g, d): Relic| if over { (i32::from(d), -i32::from(g)) } else { (-i32::from(g), i32::from(d)) };
    let mut best: Option<usize> = None;
    for k in (0..profiles.len()).filter(|&k| !skip.contains(&profiles[k].0) && room(states[profiles[k].0])) {
        if best.is_none_or(|b| key(states[profiles[k].0]) > key(states[profiles[b].0])) {
            best = Some(k);
        }
    }
    let Some(k) = best else { return false };
    let (g, d) = states[profiles[k].0];
    let bar = &mut profiles[k].1;
    *bar = if over { Bar::new(bar.glory.max(g), Some(d - 1)) } else { Bar { glory: g + 1, ..*bar } };
    true
}

/// What an attempt on relic `pick` closes the gap to, as (glory, despair):
/// what the relic would have to become to bring the board to the totals
/// `horizon` on its own - every glory short and despair over on the board
/// counts, as far as one relic can take it.  With no totals ahead, every
/// slot a glory and no despair.  Off the
/// board's range where the gap is more than a relic holds (`TableKey::close`
/// brings it in).
pub(super) fn aim(horizon: Horizon, states: &[Relic], pick: usize) -> (i64, i64) {
    match horizon {
        Some((glory, despair)) => {
            let (g, d) = totals(states);
            let (g0, d0) = states[pick];
            (i64::from(g0) + i64::from(glory) - i64::from(g), i64::from(d0) + i64::from(despair) - i64::from(d))
        }
        None => (i64::from(u8::MAX), 0),
    }
}

/// How a relic's memory ranks for the play (`play`): its bar met (if the
/// quest holds it to one), then the board nearer the totals `horizon`, then
/// glory less despair (weighed by `tie`).
fn rank(quest: &Quest, horizon: Horizon, states: &[Relic], pick: usize, r: Relic, bar: Bar) -> (bool, i64, i64) {
    let named = named_bar(quest, pick, bar);
    let met = named.is_some_and(|bar| bar.met_by(r));
    let (wg, wd) = if named.is_some() { (1, 1) } else { tie(quest, horizon, states) };
    let off = horizon.map_or(0, |(glory, despair)| {
        let (g, d) = totals(states);
        let (g, d) = (g - u32::from(states[pick].0) + u32::from(r.0), d - u32::from(states[pick].1) + u32::from(r.1));
        i64::from(glory.saturating_sub(g) + d.saturating_sub(despair))
    });
    (met, -off, i64::from(wg) * i64::from(r.0) - i64::from(wd) * i64::from(r.1))
}

/// How a glory success weighs against a despair success once a totals
/// level's gap is settled, as (glory, despair): as the goal's totals `goal`
/// still need them - how short of its glory the board is against how far
/// over its despair.  In five classes, by the share s of the shortfall that
/// is glory: under a third 1 : 3, under a half 2 : 3, a half 1 : 1, up to two
/// thirds 3 : 2, more 3 : 1 (4 short, 6 over: 2 : 3).  A few classes decide
/// as the exact ratio does wherever a glory meets a despair or two, and keep
/// the attempt tables few.  1 : 1 where the goal needs neither, and on a
/// level that holds relics to bars.
pub(super) fn tie(quest: &Quest, goal: Horizon, states: &[Relic]) -> (u8, u8) {
    let Some((glory, despair)) = goal.filter(|_| quest.is_total()) else { return (1, 1) };
    let (g, d) = totals(states);
    let (short, over) = (glory.saturating_sub(g), d.saturating_sub(despair));
    let all = short + over;
    match () {
        _ if all == 0 || 2 * short == all => (1, 1),
        _ if 3 * short < all => (1, 3),
        _ if 2 * short < all => (2, 3),
        _ if 3 * short <= 2 * all => (3, 2),
        _ => (3, 1),
    }
}

/// Keep a roll (a quest roll or a filler alike)?  If it ranks above the
/// memory in place (`rank`): its bar newly met, or as met and the board
/// nearer the totals `horizon`, or as near and more glory less despair
/// (weighed as those totals need them, `tie`).  A dead end lands as (0, 0)
/// and is never kept.
pub(super) fn keeps(quest: &Quest, horizon: Horizon, states: &[Relic], pick: usize, new: Relic, bar: Bar) -> bool {
    new != (0, 0) && rank(quest, horizon, states, pick, new, bar) > rank(quest, horizon, states, pick, states[pick], bar)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(glory: u32, despair: u32) -> Quest {
        Quest::Total { glory, despair }
    }

    #[test]
    fn at_the_top_the_leftovers_go_into_the_crit_relic_for_the_most_amplification() {
        use super::super::{ATK, CRIT, MAX_LEVEL, Target, lookahead_plan};
        let game = Game::standard();
        let rules = game.rules(&lookahead_plan(Target::level(MAX_LEVEL)).unwrap());
        let options = RunOptions::to(MAX_LEVEL);
        let mut tables = LocalTables::default();
        let mut extra = 0;
        for i in 0..30 {
            let mut partial = start(game, &options, (ATK, CRIT), Rng::for_run(5, i));
            climb(game, &rules, &options, (ATK, CRIT), &mut tables, &mut partial, MAX_LEVEL).unwrap();
            let before = *partial.levels.last().unwrap();
            let after = finish(&options, (ATK, CRIT), &mut tables, partial).levels[MAX_LEVEL - 1];
            // more attempts, no more diamonds, and a crit relic at least as good
            assert_eq!((after.diamonds, after.by_pity, after.atk), (before.diamonds, before.by_pity, before.atk));
            assert!(after.attempts >= before.attempts);
            assert!(amplification(after.crit) >= amplification(before.crit));
            extra += after.attempts - before.attempts;
        }
        assert!(extra > 0);
    }

    #[test]
    fn a_bar_comes_first_then_the_board_nearer_the_goal() {
        // a 7/3 relic named for 7/1; the goal's totals 30 glory / 3 despair,
        // the board at 15 / 5 (15 short, 2 over: 17 off)
        let bar = Bar::new(7, Some(1));
        let named = Quest::Relics { which: vec![0], bar };
        let goal = Some((30, 3));
        let states = [(7, 3), (8, 2)];
        assert!(keeps(&named, goal, &states, 0, (7, 1), bar)); // the bar, newly met
        assert!(keeps(&named, goal, &states, 0, (7, 2), bar)); // short of it, but the board 16 off
        assert!(!keeps(&named, goal, &states, 0, (6, 2), bar)); // still 17 off, and no more glory less despair
        assert!(keeps(&named, goal, &states, 0, (6, 1), bar)); // a glory for two despair: 16 off
        assert!(!keeps(&named, goal, &states, 0, (0, 0), bar)); // a wipe
        // the bar met: never given up, however much nearer the board would be
        let met = [(7, 1), (8, 2)];
        assert!(keeps(&named, goal, &met, 0, (9, 1), bar));
        assert!(!keeps(&named, goal, &met, 0, (6, 0), bar));
    }

    #[test]
    fn a_total_looks_past_its_own_quest_to_the_goal() {
        // this level asks 15 / 3, the goal 30 / 0: 9/0 beats a 9/1 that clears this level
        let quest = total(15, 3);
        let goal = Some((30, 0));
        let states = [(9, 1), (6, 0)];
        assert!(keeps(&quest, goal, &states, 0, (9, 0), Bar::new(0, None)));
        // what an attempt aims at: the relic fixing the goal's totals alone
        let states = [(8, 2), (8, 1)];
        assert_eq!(aim(Some((20, 0)), &states, 0), (12, -1));
        let key = TableKey::close(19, 12, -1, (8, 2));
        assert_eq!(key, TableKey::Close { level: 19, glory: 9, despair: 0, gap: 3, tie: (1, 1) });
        assert_eq!(TableKey::close(19, 9, 0, (7, 1)), key); // as far off: the same table
        // no totals ahead: every slot a glory, no despair
        assert_eq!(aim(None, &states, 0), (255, 0));
        // a named relic: its bar first, then the goal's totals
        let named = Quest::Relics { which: vec![0], bar: Bar::new(8, Some(1)) };
        let ctx = Context { quest: &named, rules: &rules_for_tests(), level: 19, horizon: Some((20, 0)) };
        assert_eq!(
            play(&ctx, &states, 0, Bar::new(8, Some(1))),
            TableKey::Target { level: 19, glory: 8, despair: 1, then: (9, 0, 3) }
        );
        assert_eq!(play(&ctx, &states, 1, Bar::new(0, None)), TableKey::close(19, 12, -2, (8, 1)));
    }

    fn rules_for_tests() -> LevelRules {
        LevelRules {
            base: Bar::new(4, Some(2)),
            surgical: true,
            filler: false,
            filler_within: None,
            locked: 0,
            prefer: None,
            ahead: false,
            trades: false,
        }
    }

    #[test]
    fn repair_shaves_despair_off_any_relic_above_1() {
        let states = [(4, 3), (4, 2), (5, 3), (4, 1)];
        let mut out = Vec::new();
        repair(&states, 10, 5, 9, 0..4, &mut out);
        assert_eq!(out, vec![(0, Bar::new(4, Some(2))), (1, Bar::new(4, Some(1))), (2, Bar::new(5, Some(2)))]);
    }

    #[test]
    fn a_shave_goes_to_the_worst_relic_when_affordable_and_an_8_2_rather_than_a_filler() {
        // level 19 for 20 (96+ / 16- totals), at 92 / 19: Mountain Crown 8/3
        // is the easiest despair to shave, but with 7 on hand the Archer
        // Seal's 8/2 -> 8/1 is rolled, not a pity filler
        let rules = LevelRules {
            base: Bar::new(4, Some(2)),
            surgical: true,
            filler: true,
            filler_within: None,
            locked: 0,
            prefer: None,
            ahead: false,
            trades: false,
        };
        let states = [(7, 1), (7, 1), (8, 2), (8, 2), (8, 2), (7, 1), (8, 2), (8, 1), (7, 1), (9, 1), (7, 2), (8, 3)];
        let quest = total(96, 16);
        let pick = |stock: &[u32]| {
            let mut profiles = Vec::new();
            plan(&quest, &rules, &states, 9, &mut profiles);
            let todo: Vec<usize> = profiles.iter().map(|p| p.0).collect();
            let ctx = Context { quest: &quest, rules: &rules, level: 19, horizon: Some((96, 16)) };
            match choose(&ctx, &profiles, &todo, &states, stock, 10, None) {
                Move::Quest { relic, bar, .. } => (relic, bar),
                _ => panic!("a quest roll"),
            }
        };
        let mut stock = [137, 115, 125, 92, 109, 28, 136, 5, 36, 160, 4, 7];
        assert_eq!(pick(&stock), (6, Bar::new(8, Some(1))));
        stock[11] = 50;
        assert_eq!(pick(&stock), (11, Bar::new(8, Some(2))));
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
        escalate(&total(12, 5), &states, 9, &mut profiles, &[]); // despair 6 > 5
        assert_eq!(profiles[2].1, Bar::new(4, Some(2)));
        let states = [(5, 1), (3, 1), (4, 1)];
        escalate(&total(13, 5), &states, 9, &mut profiles, &[]); // glory short
        assert_eq!(profiles[1].1, Bar::new(4, Some(2)));
    }

    #[test]
    fn a_total_takes_the_relic_closing_the_most_of_the_gap() {
        let rules = LevelRules {
            base: Bar::new(5, Some(2)),
            surgical: true,
            filler: false,
            filler_within: None,
            locked: 0,
            prefer: None,
            ahead: false,
            trades: false,
        };
        // glory short: 8/1 -> 9/1, or 7/2 -> 8/2 (despair room to spare)
        let states = [(8, 1), (7, 2)];
        let profiles = vec![(0, Bar::new(9, Some(1))), (1, Bar::new(8, Some(2)))];
        let quest = total(30, 10);
        let pick = |rules: &LevelRules, stock: &[u32]| match choose(
            &Context { quest: &quest, rules, level: 18, horizon: Some((30, 10)) },
            &profiles,
            &[0, 1],
            &states,
            stock,
            10,
            None,
        ) {
            Move::Quest { relic, .. } => relic,
            other => panic!("expected a quest roll, got {other:?}"),
        };
        // the relic with more room to close, though the other is far better stocked
        assert_eq!(pick(&rules, &[300, 20]), 1);
        // and the other when it is all that is affordable
        assert_eq!(pick(&rules, &[300, 5]), 0);
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
        escalate(&total(30, 5), &full, 9, &mut profiles, &[]);
        assert!(profiles.iter().all(|(_, bar)| bar.glory <= 9));
    }

    #[test]
    fn a_totals_level_weighs_glory_against_despair_as_the_goal_needs() {
        // the goal 96 / 16, the board 92 / 22: 4 glory short, 6 despair over
        let mut board = vec![(8, 2); 11];
        board.push((4, 0));
        assert_eq!(totals(&board), (92, 22));
        let level = total(94, 19);
        assert_eq!(tie(&level, Some((96, 16)), &board), (2, 3));
        assert_eq!(tie(&level, Some((100, 22)), &board), (3, 1)); // 8 short, none over
        assert_eq!(tie(&level, Some((95, 19)), &board), (1, 1)); // 3 short, 3 over
        assert_eq!(tie(&level, Some((80, 30)), &board), (1, 1)); // the goal needs neither
        assert_eq!(tie(&Quest::Each(Bar::new(4, Some(2))), Some((96, 16)), &board), (1, 1));
        // as near the goal's totals either way (+1 glory or -1 despair): the
        // despair shed weighs more, the goal being further over on despair
        // than short on glory
        let goal = Some((96, 16));
        let quest = total(80, 30);
        assert!(rank(&quest, goal, &board, 0, (8, 1), Bar::new(0, None)) > rank(&quest, goal, &board, 0, (9, 2), Bar::new(0, None)));
        assert!(keeps(&quest, goal, &board, 0, (8, 1), Bar::new(0, None)));
    }

    #[test]
    fn a_filler_is_kept_as_a_quest_roll_is() {
        let named = Quest::Relics { which: vec![0], bar: Bar::new(5, Some(2)) };
        let states = [(5, 2), (3, 0)];
        // a named relic that clears its bar: a roll off it is refused
        assert!(!keeps(&named, None, &states, 0, (6, 3), Bar::new(5, Some(2))));
        // a relic the quest does not name, no totals ahead: more glory less despair
        assert!(keeps(&named, None, &states, 1, (4, 0), Bar::new(0, None)));
        assert!(!keeps(&named, None, &states, 1, (4, 2), Bar::new(0, None)));
        // with totals ahead: anything bringing the board nearer them
        let mut board = vec![(5, 3)];
        board.extend([(5, 1); 10]);
        board.push((4, 1)); // 59 glory, 14 despair
        assert!(keeps(&named, Some((60, 17)), &board, 11, (5, 2), Bar::new(0, None))); // a glory short: +1
        assert!(!keeps(&named, Some((59, 14)), &board, 11, (5, 2), Bar::new(0, None))); // despair now over
    }

    #[test]
    fn an_each_level_merges_its_bar_with_the_plan() {
        let rules = LevelRules {
            base: Bar::new(5, Some(1)),
            surgical: false,
            filler: false,
            filler_within: None,
            locked: 0b01,
            prefer: None,
            ahead: false,
            trades: false,
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
