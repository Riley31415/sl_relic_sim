//! The exact solver behind a flat C ABI, for the Chrome extension.
//!
//! Every export takes and returns plain numbers, so the module needs no
//! bindings glue: JavaScript instantiates it and calls the functions directly.
//! One solver is kept at a time; `configure` replaces it, and its memo fills
//! in lazily as states are asked about.
//!
//! Action codes: 0 attempt glory, 1 attempt despair, 2 mental training,
//! 3 done (both bars full), 4 dead end (a wipe), -1 a state or config that
//! is not valid.

use std::cell::RefCell;
use std::collections::BTreeMap;

use relic::levels::{self, Advice, Game, Move, N_RELICS, Next, Quest, TableKey, Target};
use relic::solver::{Action, Choice, Config, Objective, Solver, State, Strategy, TIERS};

thread_local! {
    static SOLVER: RefCell<Option<Solver>> = const { RefCell::new(None) };
}

fn board(level: u32) -> Option<Config> {
    Config::for_level(i64::from(level), Strategy::default()).ok()
}

/// Memory slots per bar at `level`, or 0 for a level that does not exist.
#[unsafe(no_mangle)]
pub extern "C" fn cfg_slots(level: u32) -> u32 {
    board(level).map_or(0, |c| u32::from(c.slots))
}

/// Spirit power cap (and starting spirit power) at `level`.
#[unsafe(no_mangle)]
pub extern "C" fn cfg_max_spirit(level: u32) -> u32 {
    board(level).map_or(0, |c| u32::from(c.max_spirit))
}

/// Glory slots an attempt starts with already a success.
#[unsafe(no_mangle)]
pub extern "C" fn cfg_start_glory(level: u32) -> u32 {
    board(level).map_or(0, |c| u32::from(c.start_glory))
}

/// Despair slots an attempt starts with already a failure.
#[unsafe(no_mangle)]
pub extern "C" fn cfg_start_despair_fail(level: u32) -> u32 {
    board(level).map_or(0, |c| u32::from(c.start_despair_fail))
}

/// What `level`'s bonuses add to the glory success chance (0.10 for +10%).
#[unsafe(no_mangle)]
pub extern "C" fn cfg_glory_mod(level: u32) -> f64 {
    board(level).map_or(0.0, |c| c.glory_mod)
}

/// What `level`'s bonuses add to the despair success chance (-0.10 for -10%).
#[unsafe(no_mangle)]
pub extern "C" fn cfg_despair_mod(level: u32) -> f64 {
    board(level).map_or(0.0, |c| c.despair_mod)
}

/// Index into the 80/65/50/35/20% ladder an attempt opens on.
#[unsafe(no_mangle)]
pub extern "C" fn cfg_start_tier(level: u32) -> u32 {
    board(level).map_or(0, |c| u32::from(c.start_tier))
}

/// The base chance of ladder tier `tier`, in percent.
#[unsafe(no_mangle)]
pub extern "C" fn tier_percent(tier: u32) -> u32 {
    TIERS.get(tier as usize).map_or(0, |p| (p * 100.0).round() as u32)
}

/// A tie weight from JavaScript, at most 10.
fn on_board_tie(v: u32) -> u8 {
    u8::try_from(v.min(10)).expect("at most 10")
}

/// Set the board and objective every later query is answered for.
///
/// `kind` 0: max amplification above a mark (+5% a glory success, -2% a
/// despair success): the best chance of `a`% or more, then the most
/// amplification, a result kept only above the memory (`fg`, `fd`) - `a` 0
/// for "just above the memory";
/// 1: close the gap to >= `a` glory and <= `b` despair successes (`b` past
/// the slots: any despair) from the memory (`fg`, `fd`) - every glory gained
/// or despair shed counts, a result closing none counts nothing - then more
/// glory, less despair, weighed `c` : `d` (0 : 0 for 1 : 1);
/// 2: the best chance of `a`+ glory / `b`- despair, then closing the gap to
/// (`c`, `d`) from the memory (255, 0: every slot a glory, no despair), then
/// more glory, less despair.
/// Kind 0 with `a` 0 and no memory known is no mark at all: the most
/// amplification, expected (a wipe paying 0%).
/// A memory not known is given as (0, 255): as far off as can be (for kind
/// 0: none, any finished result counts).
/// `wipe_penalty` charges a wipe extra (0 is the plain optimum).
/// Returns 0, or -1 for a level or kind that does not exist.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn configure(level: u32, kind: u32, a: u32, b: u32, c: u32, d: u32, fg: u32, fd: u32, wipe_penalty: f64) -> i32 {
    let Ok(slots) = Config::for_level(i64::from(level), Strategy::default()).map(|c| c.slots) else { return -1 };
    let on_board = |v: u32| u8::try_from(v.min(u32::from(slots))).expect("within the slots");
    let from = (on_board(fg), on_board(fd));
    let known = fd != 255;
    let strategy = match kind {
        0 => Strategy::above(u8::try_from(a.min(255)).expect("narrowed"), known.then_some(from)),
        1 if (c, d) == (0, 0) => Strategy::close(on_board(a), on_board(b), from),
        1 => Strategy::close(on_board(a), on_board(b), from).tie(on_board_tie(c), on_board_tie(d)),
        2 => {
            let Objective::Close { glory, despair, gap } = Strategy::close(on_board(c), on_board(d), from).objective else {
                unreachable!("close makes Close")
            };
            Strategy::target(on_board(a), on_board(b)).then_close(glory, despair, gap)
        }
        _ => return -1,
    };
    let Ok(mut cfg) = Config::for_level(i64::from(level), strategy) else { return -1 };
    cfg.wipe_penalty = wipe_penalty.max(0.0);
    SOLVER.with(|s| *s.borrow_mut() = Some(Solver::new(cfg)));
    0
}

/// A state as read off the screen, if it is one the configured board allows.
fn state(solver: &Solver, gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32) -> Option<State> {
    let c = &solver.cfg;
    let n = |v: u32| u8::try_from(v).ok();
    let s = State { gf: n(gf)?, gs: n(gs)?, df: n(df)?, ds: n(ds)?, ms: n(ms)?, sp: n(sp)?, tier: n(tier)? };
    let fits = s.gf <= c.slots
        && s.df <= c.slots
        && s.gs <= s.gf
        && s.ds <= s.df
        && s.ms <= c.start_mental()
        && s.sp <= c.max_spirit
        && usize::from(s.tier) < TIERS.len();
    fits.then_some(s)
}

/// Solve one state with the configured solver; None without one or for a
/// state the board cannot hold.
fn solve(gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32) -> Option<relic::solver::Value> {
    with_state(gf, gs, df, ds, ms, sp, tier, |solver, s| Some(solver.value(s)))
}

/// The optimal action in a state (codes in the module docs).
#[unsafe(no_mangle)]
pub extern "C" fn best(gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32) -> i32 {
    match solve(gf, gs, df, ds, ms, sp, tier).map(|v| v.choice) {
        Some(Choice::Act(Action::Glory)) => 0,
        Some(Choice::Act(Action::Despair)) => 1,
        Some(Choice::Act(Action::Train)) => 2,
        Some(Choice::Done) => 3,
        Some(Choice::DeadEnd) => 4,
        None => -1,
    }
}

/// Expected objective from a state under optimal play: amplification % for
/// kind 0, P(hit) for kinds 1 and 2.  NaN for an invalid state.
#[unsafe(no_mangle)]
pub extern "C" fn expected_score(gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32) -> f64 {
    solve(gf, gs, df, ds, ms, sp, tier).map_or(f64::NAN, |v| v.score)
}

/// P(the attempt finishes without a wipe) from a state under optimal play.
#[unsafe(no_mangle)]
pub extern "C" fn finish_chance(gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32) -> f64 {
    solve(gf, gs, df, ds, ms, sp, tier).map_or(f64::NAN, |v| v.p_finish)
}

/// 1 if some play from a state can still fill both bars, 0 if it can only
/// wipe, -1 for an invalid state.
#[unsafe(no_mangle)]
pub extern "C" fn can_finish(gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32) -> i32 {
    with_state(gf, gs, df, ds, ms, sp, tier, |solver, s| Some(i32::from(solver.can_finish(s)))).unwrap_or(-1)
}

// ------------------------------------------------------------------ moves

/// Numbers out, for `move_value` and `outcomes`: every result's three
/// numbers (at most 11 x 11 of them) after the wipe chance.
static mut OUT: [f64; 1 + 3 * 121] = [0.0; 1 + 3 * 121];

/// Where `OUT` lives in the module's memory.
#[unsafe(no_mangle)]
pub extern "C" fn out() -> *mut f64 {
    (&raw mut OUT).cast::<f64>()
}

fn action_of(code: i32) -> Option<Action> {
    match code {
        0 => Some(Action::Glory),
        1 => Some(Action::Despair),
        2 => Some(Action::Train),
        _ => None,
    }
}

/// With the configured solver and a state it allows, `f`'s answer.
fn with_state<T>(gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32, f: impl FnOnce(&mut Solver, State) -> Option<T>) -> Option<T> {
    SOLVER.with(|cell| {
        let mut guard = cell.borrow_mut();
        let solver = guard.as_mut()?;
        let s = state(solver, gf, gs, df, ds, ms, sp, tier)?;
        f(solver, s)
    })
}

/// The move `code` names, if `s` allows it.
fn legal(solver: &Solver, s: State, code: i32) -> Option<Action> {
    action_of(code).filter(|a| solver.legal_actions(s).contains(a))
}

/// What the configured solver scores, as the engine settled it: 0 the most
/// amplification, expected; 1 a mark - out[0] the mark (%), out[1] the least
/// amplification a result is kept at; 2 a target; 3 closing a gap.  -1
/// without a solver.  So what is shown of a score is what was played for.
#[unsafe(no_mangle)]
pub extern "C" fn objective() -> i32 {
    SOLVER.with(|cell| {
        let guard = cell.borrow();
        let Some(solver) = guard.as_ref() else { return -1 };
        match solver.cfg.strategy.objective {
            Objective::Weighted => 0,
            Objective::Above { mark, least } => {
                // SAFETY: as in `board_in`
                let out = unsafe { &mut *(&raw mut OUT) };
                out[0] = f64::from(mark);
                out[1] = f64::from(least);
                1
            }
            Objective::Target { .. } => 2,
            Objective::Close { .. } => 3,
        }
    })
}

/// The chance move `action` (codes as `best`) succeeds at ladder tier
/// `tier` on the configured board, its level's bonuses in.  NaN without a
/// solver, or for no such move or tier.
#[unsafe(no_mangle)]
pub extern "C" fn chance(action: i32, tier: u32) -> f64 {
    SOLVER.with(|cell| {
        let guard = cell.borrow();
        let (Some(solver), Some(action)) = (guard.as_ref(), action_of(action)) else { return f64::NAN };
        u8::try_from(tier).ok().filter(|&t| usize::from(t) < TIERS.len()).map_or(f64::NAN, |t| solver.chance(action, t))
    })
}

/// What a move is worth: `action` (codes as `best`) taken in the state, then
/// optimal play; `action` -1, optimal play from the state itself.  out[0..5]:
/// P(no wipe), the objective's score (as `expected_score`), the second
/// objective's (`Strategy::then`), the chance of the nearest result still
/// reachable, and what settles the rest (`Strategy::last`) - the order
/// `best` weighs them in.  Returns 0, or -1 for a state not valid or a move
/// it does not allow.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn move_value(action: i32, gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32) -> i32 {
    let v = with_state(gf, gs, df, ds, ms, sp, tier, |solver, s| {
        if action < 0 { Some(solver.value(s)) } else { legal(solver, s, action).map(|a| solver.action_value(s, a)) }
    });
    let Some(v) = v else { return -1 };
    // SAFETY: as in `board_in`
    let out = unsafe { &mut *(&raw mut OUT) };
    out[..5].copy_from_slice(&[v.p_finish, v.score, v.then, v.near, v.last]);
    0
}

/// Every result an attempt can end on from the state: `action` (codes as
/// `best`) taken first - -1 the optimal move - and optimal play after.
/// out[0] the chance of a wipe, then per result its glory successes,
/// despair successes and chance, in (glory, despair) order.  Returns how
/// many results, or -1 for a state not valid or a move it does not allow.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn outcomes(action: i32, gf: u32, gs: u32, df: u32, ds: u32, ms: u32, sp: u32, tier: u32) -> i32 {
    let swept = with_state(gf, gs, df, ds, ms, sp, tier, |solver, s| {
        let first = if action < 0 { None } else { Some(legal(solver, s, action)?) };
        Some(sweep(solver, s, first))
    });
    let Some((wipe, results)) = swept else { return -1 };
    // SAFETY: as in `board_in`
    let out = unsafe { &mut *(&raw mut OUT) };
    out[0] = wipe;
    for (i, (&(g, d), &p)) in results.iter().enumerate() {
        out[1 + 3 * i..4 + 3 * i].copy_from_slice(&[f64::from(g), f64::from(d), p]);
    }
    i32::try_from(results.len()).expect("at most 121 results")
}

/// Probability pushed forward from `start` through `first` and then the
/// optimal policy, exactly: (P(wipe), each finished result's chance).
fn sweep(solver: &mut Solver, start: State, first: Option<Action>) -> (f64, BTreeMap<(u8, u8), f64>) {
    // every move fills a slot or spends mental strength: this falls by one a move
    let slots = usize::from(solver.cfg.slots);
    let potential = |s: State| usize::from(s.ms) + 2 * slots - usize::from(s.gf) - usize::from(s.df);
    let key = |s: State| [s.gf, s.gs, s.df, s.ds, s.ms, s.sp, s.tier];
    let top = potential(start);
    let mut layers: Vec<BTreeMap<[u8; 7], (State, f64)>> = vec![BTreeMap::new(); top + 1];
    layers[top].insert(key(start), (start, 1.0));
    let (mut wipe, mut results) = (0.0, BTreeMap::new());
    for level in (0..=top).rev() {
        for (_, (s, mass)) in std::mem::take(&mut layers[level]) {
            if solver.is_terminal(s) {
                *results.entry((s.gs, s.ds)).or_insert(0.0) += mass;
                continue;
            }
            let action = match first.filter(|_| s == start) {
                Some(action) => action,
                None => match solver.best_action(s) {
                    Choice::Act(action) => action,
                    _ => {
                        wipe += mass;
                        continue;
                    }
                },
            };
            for (p, next, _ok) in solver.transitions(s, action) {
                if p != 0.0 {
                    layers[potential(next)].entry(key(next)).or_insert((next, 0.0)).1 += mass * p;
                }
            }
        }
    }
    (wipe, results)
}

// ---------------------------------------------------------------- advisor

/// Exchange area for the advisor, shared with JavaScript (see `io`):
/// in  [0..24) each relic's glory then despair, [24..36) each relic's stock,
/// [36] how full the pity bar is (per mille, -1 not known);
/// out [40..52) the advice (see `advise`), [52..55) a level's requirement
/// (see `quest`).
static mut IO: [i32; 64] = [0; 64];

/// Where the exchange area lives in the module's memory.
#[unsafe(no_mangle)]
pub extern "C" fn io() -> *mut i32 {
    (&raw mut IO).cast::<i32>()
}

/// How full the pity bar is, as JavaScript wrote it in io[36]: per mille,
/// -1 if not known.
fn pity_in() -> Option<u32> {
    // SAFETY: as in `board_in`
    let io = unsafe { &*(&raw const IO) };
    u32::try_from(io[36]).ok().map(|p| p.min(1000))
}

/// The board JavaScript wrote: every relic's (glory, despair), and stock.
fn board_in() -> Option<(Vec<(u8, u8)>, Vec<u32>)> {
    // SAFETY: wasm is single-threaded; JavaScript writes the area between calls
    let io = unsafe { &*(&raw const IO) };
    let n = |v: i32| u8::try_from(v).ok();
    let states = (0..N_RELICS).map(|i| Some((n(io[2 * i])?, n(io[2 * i + 1])?))).collect::<Option<Vec<_>>>()?;
    let stock = (0..N_RELICS).map(|i| u32::try_from(io[24 + i]).ok()).collect::<Option<Vec<_>>>()?;
    Some((states, stock))
}

fn target(level: u32, tier: u32) -> Target {
    let level = level as usize;
    if tier == 0 { Target::level(level) } else { Target { level, tier: u8::try_from(tier).ok() } }
}

fn advice(target_level: u32, tier: u32, level: u32) -> Option<Advice> {
    let target = target(target_level, tier);
    let (states, stock) = board_in()?;
    let plan = levels::lookahead_plan(target).ok()?;
    let goal = (level as usize + 1).min(levels::MAX_LEVEL);
    let pity = pity_in().map(|permille| levels::pity_needed(goal) * permille / 1000);
    Game::standard().advise(&plan, target, level as usize, &states, &stock, pity).ok()
}

/// The Look Ahead plan's next step toward `target_level` (with `tier` the
/// crit relic's mark past the top, 0 for none) from the board in `io`, seen
/// at inheritor `level`.  Returns the kind of step, the rest in io[40..52]:
///
/// 0 level up (the next level's requirement is met), 1 roll a relic for the
/// requirement, 2 roll a spare relic for the pity, 3 summon, 4 convert other
/// relics into the crit relic, 5 farm the crit relic, 6 done, -1 no advice.
///
/// io[40] the level worked toward.  For a roll: io[41] the relic; io[42] how
/// it is played, from the relic's own memory, solved at level io[43] - 1
/// closing the gap to (io[44] glory, io[45] despair) (the totals it plays to),
/// ties weighed io[48] : io[49], 2
/// the best chance of the bar (io[44] glory, io[45] despair) then closing
/// the gap to (io[48] glory, io[49] despair), 3 a farm roll: max
/// amplification above the mark io[44]; io[46] / io[47] the work's bar
/// glory / despair (-1 any; -1 both for a farm).  io[50] / io[51]: the
/// totals every roll at this level closes the gap to, glory / despair (-1
/// both for none) - for `keeps_roll`.
#[unsafe(no_mangle)]
pub extern "C" fn advise(target_level: u32, tier: u32, level: u32) -> i32 {
    let Some(a) = advice(target_level, tier, level) else { return -1 };
    // SAFETY: as in `board_in`
    let io = unsafe { &mut *(&raw mut IO) };
    io[40..52].fill(-1);
    io[40] = a.goal as i32;
    if let Some((glory, despair)) = a.horizon {
        io[50] = glory as i32;
        io[51] = despair as i32;
    }
    if let Some((relic, key)) = a.roll() {
        io[41] = relic as i32;
        match key {
            TableKey::Close { level, glory, despair, tie: (wg, wd), .. } => {
                io[42] = 1;
                io[43] = i32::from(level);
                io[44] = i32::from(glory);
                io[45] = i32::from(despair);
                io[48] = i32::from(wg);
                io[49] = i32::from(wd);
            }
            TableKey::Target { level, glory, despair, then: (tg, td, _) } => {
                io[42] = 2;
                io[43] = i32::from(level);
                io[44] = i32::from(glory);
                io[45] = i32::from(despair);
                io[48] = i32::from(tg);
                io[49] = i32::from(td);
            }
            TableKey::Above { level, mark, .. } => {
                io[42] = 3;
                io[43] = i32::from(level);
                io[44] = i32::from(mark);
            }
        }
    }
    match &a.next {
        Next::Move(Move::Quest { bar, .. } | Move::Filler { bar, .. }) => {
            io[46] = i32::from(bar.glory);
            io[47] = bar.despair.map_or(-1, i32::from);
        }
        _ => {}
    }
    match a.next {
        Next::LevelUp => 0,
        Next::Move(Move::Quest { .. }) => 1,
        Next::Move(Move::Filler { .. }) => 2,
        Next::Move(Move::Summon) => 3,
        Next::Convert(_) => 4,
        Next::Farm { .. } => 5,
        Next::Done => 6,
    }
}

/// Whether a roll keeps a fresh (glory, despair), judged on the board in io
/// as it stood when the attempt began: a roll on `relic` toward
/// (`bar_glory`, `bar_despair`; -1 any) for the quest of level `goal`,
/// closing the gap to the totals (`h_glory`, `h_despair`; -1 none) - a
/// quest roll or a filler alike, as `advise` described it - or, `farm` 1, a
/// crit relic farm roll.  1 keep, 0 discard, -1 for a roll that cannot be.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn keeps_roll(
    h_glory: i32,
    h_despair: i32,
    goal: u32,
    relic: u32,
    bar_glory: i32,
    bar_despair: i32,
    farm: u32,
    glory: u32,
    despair: u32,
) -> i32 {
    let Some((states, _)) = board_in() else { return -1 };
    let (Ok(g), Ok(d)) = (u8::try_from(glory), u8::try_from(despair)) else { return -1 };
    let game = Game::standard();
    if farm == 1 {
        return i32::from(game.keeps_farmed(&states, (g, d)));
    }
    let goal = goal as usize;
    let (Ok(bar_glory), relic) = (u8::try_from(bar_glory), relic as usize) else { return -1 };
    if !(2..=levels::MAX_LEVEL).contains(&goal) || relic >= N_RELICS {
        return -1;
    }
    let bar = levels::Bar::new(bar_glory, u8::try_from(bar_despair).ok());
    let horizon = u32::try_from(h_glory).ok().zip(u32::try_from(h_despair).ok());
    i32::from(game.keeps_roll(horizon, goal, relic, bar, &states, (g, d)))
}

/// What being at inheritor `level` asks for: 0 nothing; 1 the relics in the
/// bitmask io[54] (bit i relic i) each at a bar, or 2 every relic at it -
/// io[52] glory or more, io[53] despair or less (-1 any); 3 the totals over
/// every relic, io[52] glory or more and io[53] despair or less.  -1 past
/// the requirement table.
#[unsafe(no_mangle)]
pub extern "C" fn quest(level: u32) -> i32 {
    let Some(quest) = levels::requirements().into_iter().nth(level as usize) else { return -1 };
    // SAFETY: as in `board_in`
    let io = unsafe { &mut *(&raw mut IO) };
    io[52..55].fill(-1);
    let bar = |io: &mut [i32; 64], bar: levels::Bar| {
        io[52] = i32::from(bar.glory);
        io[53] = bar.despair.map_or(-1, i32::from);
    };
    match quest {
        None => 0,
        Some(Quest::Relics { which, bar: b }) => {
            bar(io, b);
            io[54] = i32::from(levels::set_of(&which));
            1
        }
        Some(Quest::Each(b)) => {
            bar(io, b);
            2
        }
        Some(Quest::Total { glory, despair }) => {
            io[52] = glory as i32;
            io[53] = despair as i32;
            3
        }
    }
}

/// The top inheritor level: no pity bar there, nothing above it.
#[unsafe(no_mangle)]
pub extern "C" fn max_level() -> u32 {
    levels::MAX_LEVEL as u32
}

/// Pity points that level the inheritor up to `goal` without its requirement.
#[unsafe(no_mangle)]
pub extern "C" fn pity_needed(goal: u32) -> u32 {
    if goal < 2 { 0 } else { levels::pity_needed(goal as usize) }
}

/// Pity points one attempt adds at inheritor `level`.
#[unsafe(no_mangle)]
pub extern "C" fn pity_gain(level: u32) -> u32 {
    levels::pity_gain(level as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_19_board() {
        assert_eq!(cfg_slots(19), 9);
        assert_eq!(cfg_max_spirit(19), 10);
        assert_eq!(cfg_start_glory(19), 1);
        assert_eq!(cfg_start_despair_fail(19), 1);
        assert_eq!(cfg_start_tier(19), 0);
        assert_eq!(cfg_slots(0), 0);
        // the +10% / -10% its board prints under the Attempt buttons
        assert!((cfg_glory_mod(19) - 0.10).abs() < 1e-9 && (cfg_despair_mod(19) + 0.10).abs() < 1e-9);
        assert!((cfg_glory_mod(16) - 0.08).abs() < 1e-9 && (cfg_despair_mod(17) + 0.08).abs() < 1e-9);
    }

    #[test]
    fn matches_the_native_solver() {
        // max amplification above 40%, over a memory of 7/1
        assert_eq!(configure(19, 0, 40, 0, 0, 0, 7, 1, 0.0), 0);
        let mut native = Solver::new(Config::for_level(19, Strategy::above(40, Some((7, 1)))).unwrap());
        let start = native.start_state();
        let code = best(1, 1, 1, 0, 9, 10, 0);
        let want = match native.best_action(start) {
            Choice::Act(Action::Glory) => 0,
            Choice::Act(Action::Despair) => 1,
            Choice::Act(Action::Train) => 2,
            other => panic!("start state solved to {other:?}"),
        };
        assert_eq!(code, want);
        // the score is the chance of 40% or more
        let score = expected_score(1, 1, 1, 0, 9, 10, 0);
        let want = native.value(start).score;
        assert!((score - want).abs() < 1e-12 && score > 0.0, "{score} vs {want}");
    }

    /// Held by the tests that use OUT: they would read each other's numbers.
    static OUT_USERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn out_now() -> Vec<f64> {
        // SAFETY: the tests that write OUT hold OUT_USERS
        unsafe { (&*(&raw const OUT)).to_vec() }
    }

    #[test]
    fn values_each_move_as_best_weighs_it() {
        let _out = OUT_USERS.lock().unwrap();
        // a target at 7/1, then on toward 9/0, from a memory of 7/3
        assert_eq!(configure(19, 2, 7, 1, 9, 0, 7, 3, 0.0), 0);
        let s = [5, 3, 4, 1, 6, 4, 2];
        let [gf, gs, df, ds, ms, sp, tier] = s;
        assert_eq!(move_value(-1, gf, gs, df, ds, ms, sp, tier), 0);
        let whole = out_now()[..5].to_vec();
        assert_eq!(whole[0], finish_chance(gf, gs, df, ds, ms, sp, tier));
        assert_eq!(whole[1], expected_score(gf, gs, df, ds, ms, sp, tier));
        // the move it picks is worth exactly the state; none scores more
        let chosen = best(gf, gs, df, ds, ms, sp, tier);
        assert_eq!(move_value(chosen, gf, gs, df, ds, ms, sp, tier), 0);
        assert_eq!(out_now()[..5], whole[..]);
        for action in 0..3 {
            assert_eq!(move_value(action, gf, gs, df, ds, ms, sp, tier), 0);
            assert!(out_now()[1] <= whole[1] + 1e-12);
        }
        assert_eq!(move_value(3, gf, gs, df, ds, ms, sp, tier), -1); // no such move
        assert_eq!(move_value(2, gf, gs, df, ds, 0, sp, tier), -1); // no mental strength to train with
        // a target here: so it says
        assert_eq!(objective(), 2);
        // a mark of 0 over no memory is the most amplification; over a memory, raised to beat it
        assert_eq!(configure(19, 0, 0, 0, 0, 0, 0, 255, 0.0), 0);
        assert_eq!(objective(), 0);
        assert_eq!(configure(19, 0, 0, 0, 0, 0, 7, 1, 0.0), 0);
        assert_eq!(objective(), 1);
        assert_eq!(out_now()[..2], [34.0, 34.0]);
        assert_eq!(configure(19, 2, 7, 1, 9, 0, 7, 3, 0.0), 0);
        // the ladder at level 19: +10% glory, -10% despair, as its board's buttons say
        assert!((chance(0, 2) - 0.60).abs() < 1e-9 && (chance(1, 2) - 0.40).abs() < 1e-9 && chance(2, 2) == 0.5);
        assert!(chance(0, 5).is_nan() && chance(3, 0).is_nan());
    }

    #[test]
    fn spreads_every_result_a_move_can_end_on() {
        let _out = OUT_USERS.lock().unwrap();
        assert_eq!(configure(19, 0, 46, 0, 0, 0, 8, 2, 0.0), 0);
        let analysis = Solver::new(Config::for_level(19, Strategy::above(46, Some((8, 2)))).unwrap()).analyse(None);
        // from the start, the optimal move first: the whole attempt's spread
        let n = outcomes(-1, 1, 1, 1, 0, 9, 10, 0);
        assert_eq!(n as usize, analysis.dist.iter().filter(|&((g, d), _)| (g, d) != (0, 0) || analysis.p_dead_end == 0.0).count());
        let out = out_now();
        assert!((out[0] - analysis.p_dead_end).abs() < 1e-12);
        let mut total = out[0];
        for i in 0..n as usize {
            let (g, d, p) = (out[1 + 3 * i] as u8, out[2 + 3 * i] as u8, out[3 + 3 * i]);
            let want = analysis.dist.get((g, d)) - if (g, d) == (0, 0) { analysis.p_dead_end } else { 0.0 };
            assert!((p - want).abs() < 1e-12, "{g}/{d}: {p} vs {want}");
            total += p;
        }
        assert!((total - 1.0).abs() < 1e-9);
        // each move first: a distribution too, and the chosen one is the whole attempt's
        let chosen = best(1, 1, 1, 0, 9, 10, 0);
        for action in 0..3 {
            let m = outcomes(action, 1, 1, 1, 0, 9, 10, 0);
            let out = out_now();
            let sum: f64 = out[0] + (0..m as usize).map(|i| out[3 + 3 * i]).sum::<f64>();
            assert!((sum - 1.0).abs() < 1e-9, "move {action}");
            // the chance of 46% or more, summed off the spread, is the move's score
            let hit: f64 = (0..m as usize).filter(|&i| 5.0 * out[1 + 3 * i] - 2.0 * out[2 + 3 * i] >= 46.0).map(|i| out[3 + 3 * i]).sum();
            assert_eq!(move_value(action, 1, 1, 1, 0, 9, 10, 0), 0);
            assert!((hit - out_now()[1]).abs() < 1e-9, "move {action}: {hit}");
            if action == chosen {
                assert_eq!(m, n);
            }
        }
        // a finished board is its own result
        assert_eq!(outcomes(-1, 9, 7, 9, 2, 0, 3, 2), 1);
        assert_eq!(out_now()[..4], [0.0, 7.0, 2.0, 1.0]);
    }

    #[test]
    fn tells_each_level_requirement() {
        let io = || unsafe { (&*(&raw const IO))[52..55].to_vec() };
        assert_eq!(quest(19), 3);
        assert_eq!(io()[..2], [94, 19]); // the totals test/images/main-counts.png asks for
        assert_eq!(quest(13), 1);
        assert_eq!(io(), [7, 2, (1 << 9) | (1 << 10) | (1 << 11)]);
        assert_eq!(quest(7), 2);
        assert_eq!(io()[..2], [4, 2]);
        assert_eq!(quest(2), 1);
        assert_eq!(io(), [3, -1, 1]); // Giant's Right Hand at 3 glory, any despair
        assert_eq!(quest(1), 0);
        assert_eq!(quest(21), -1);
    }

    #[test]
    fn rejects_impossible_states() {
        assert_eq!(configure(19, 0, 0, 0, 0, 0, 0, 255, 0.0), 0);
        assert_eq!(best(10, 0, 0, 0, 9, 10, 0), -1); // 10 glory slots on a 9-slot board
        assert_eq!(best(2, 3, 0, 0, 9, 10, 0), -1); // more successes than fills
        assert_eq!(best(1, 1, 1, 0, 9, 11, 0), -1); // over the spirit cap
        assert_eq!(best(1, 1, 1, 0, 9, 10, 5), -1); // no such tier
        assert_eq!(best(9, 5, 9, 2, 0, 3, 2), 3);
        assert_eq!(best(5, 2, 5, 2, 0, 0, 2), 4);
        assert_eq!(configure(19, 7, 0, 0, 0, 0, 0, 255, 0.0), -1);
    }

    /// The level-18 board of the repo's advice tests (tests/levels.rs), the
    /// one in test/fixtures/main-counts.png bar Mountain Crown's stock.
    const BOARD: [(u8, u8); 12] =
        [(7, 1), (7, 1), (8, 3), (8, 2), (8, 2), (7, 1), (8, 2), (8, 3), (7, 1), (9, 1), (7, 2), (8, 3)];
    const STOCK: [u32; 12] = [207, 115, 135, 92, 109, 28, 156, 105, 36, 180, 4, 137];

    fn load(board: &[(u8, u8)], stock: &[u32]) {
        // SAFETY: tests touching IO run one at a time (they share the advisor test)
        let io = unsafe { &mut *(&raw mut IO) };
        for (i, &(g, d)) in board.iter().enumerate() {
            io[2 * i] = i32::from(g);
            io[2 * i + 1] = i32::from(d);
        }
        for (i, &n) in stock.iter().enumerate() {
            io[24 + i] = n as i32;
        }
        io[36] = -1; // the pity not known
    }

    #[test]
    fn advises_like_the_native_advisor() {
        load(&BOARD, &STOCK);
        // Mountain Crown (8/3) toward 8/2 at level 18, played to close the
        // gap to the totals ahead, level 20's (4 glory short, 6 despair over): up to 9 glory, 0 despair
        assert_eq!(advise(20, 46, 18), 1);
        let io = unsafe { &*(&raw const IO) };
        assert_eq!(&io[40..48], &[19, 11, 1, 18, 9, 0, 8, 2]);
        // the rest weighed 2 : 3: 4 glory short, 6 despair over; those totals for the keep rule
        assert_eq!(&io[48..50], &[2, 3]);
        assert_eq!(&io[50..52], &[96, 16]);
        // kept as that roll keeps (relic 11 toward 8/2, for level 19, to the totals it played to)
        let h = (io[50], io[51]);
        assert_eq!(keeps_roll(h.0, h.1, 19, 11, 8, 2, 0, 8, 2), 1);
        assert_eq!(keeps_roll(h.0, h.1, 19, 11, 8, 2, 0, 7, 1), 1); // gap 10 -> 9
        // the same gap, but a despair shed weighs more than the glory lost (2 : 3)
        assert_eq!(keeps_roll(h.0, h.1, 19, 11, 8, 2, 0, 7, 2), 1);
        assert_eq!(keeps_roll(h.0, h.1, 19, 11, 8, 2, 0, 0, 0), 0); // a wipe
        assert_eq!(keeps_roll(h.0, h.1, 25, 11, 8, 2, 0, 9, 0), -1); // no such level
        // a farm roll keeps more amplification on the crit relic (7/1: 33%)
        assert_eq!(keeps_roll(h.0, h.1, 20, 1, -1, -1, 1, 8, 1), 1);
        assert_eq!(keeps_roll(h.0, h.1, 20, 1, -1, -1, 1, 7, 1), 0);
        // too few Mountain Crowns: the next best-stocked 3-despair relic
        let mut stock = STOCK;
        stock[11] = 9;
        load(&BOARD, &stock);
        assert_eq!(advise(20, 46, 18), 1);
        assert_eq!(unsafe { (*(&raw const IO))[41] }, 2);
        // nothing affordable: summon
        load(&BOARD, &[0; 12]);
        assert_eq!(advise(20, 46, 18), 3);
        // a goal below the top: the same quest work toward it, and done on reaching it
        load(&BOARD, &STOCK);
        assert_eq!(advise(19, 0, 18), 1);
        assert_eq!(advise(18, 0, 18), 6);
        // requirement met: level up
        let mut board = BOARD;
        board[9] = (9, 0);
        board[10] = (9, 0);
        board[2] = (8, 1);
        load(&board, &STOCK);
        assert_eq!(advise(20, 46, 18), 0);
        assert_eq!(pity_needed(19), 9000);
        assert_eq!(pity_gain(18), 180);
    }

    #[test]
    fn solves_every_kind_of_key() {
        assert_eq!(configure(18, 1, 8, 255, 0, 0, 7, 3, 0.0), 0); // close, any despair, from 7/3
        assert!(matches!(best(1, 1, 0, 0, 9, 10, 1), 0..=2));
        assert_eq!(configure(18, 1, 8, 1, 0, 0, 0, 255, 0.0), 0); // close, memory not known
        assert!(matches!(best(1, 1, 0, 0, 9, 10, 1), 0..=2));
        assert_eq!(configure(18, 2, 8, 1, 9, 0, 7, 3, 0.0), 0); // the bar 8/1, then to 9/0, from 7/3
        assert!(matches!(best(1, 1, 0, 0, 9, 10, 1), 0..=2));
        assert_eq!(configure(18, 2, 8, 1, 255, 0, 0, 255, 0.0), 0); // the bar, then more glory less despair
        assert!(matches!(best(1, 1, 0, 0, 9, 10, 1), 0..=2));
        assert_eq!(configure(20, 0, 46, 0, 0, 0, 8, 2, 0.0), 0); // above 46%, over 8/2
        assert!(matches!(best(1, 1, 1, 0, 9, 10, 0), 0..=2));
        assert_eq!(configure(20, 0, 0, 0, 0, 0, 0, 255, 0.0), 0); // any finished result, no memory
        assert_eq!(configure(20, 0, 0, 0, 0, 0, 0, 255, 0.0), 0); // no mark, no memory: the most amplification
        assert!(matches!(best(1, 1, 1, 0, 9, 10, 0), 0..=2));
        assert!(expected_score(1, 1, 1, 0, 9, 10, 0) > 20.0); // a score in amplification %, not a chance
        assert_eq!(configure(20, 3, 46, 0, 0, 0, 8, 2, 0.0), -1); // no kind 3
    }
}
