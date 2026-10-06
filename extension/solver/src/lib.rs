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

use relic::levels::{self, Advice, Game, Move, N_RELICS, Next, TableKey, Target};
use relic::solver::{Action, Choice, Config, Solver, State, Strategy, TIERS};

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

/// Set the board and objective every later query is answered for.
///
/// `kind` 0: most amplification (+5% a glory success, -2% a despair success);
/// 1: all or nothing for >= `a` glory and <= `b` despair successes (`b` past
/// the slots: any despair);
/// 2: all or nothing for an amplification of at least `a`%;
/// 3: a score of `wa` a glory success plus `wb` a despair slot left clean.
/// `wipe_penalty` charges a wipe extra (0 is the plain optimum).
/// Returns 0, or -1 for a level or kind that does not exist.
#[unsafe(no_mangle)]
pub extern "C" fn configure(level: u32, kind: u32, a: u32, b: u32, wa: f64, wb: f64, wipe_penalty: f64) -> i32 {
    let narrow = |v: u32| u8::try_from(v.min(255)).ok();
    let strategy = match (kind, narrow(a), narrow(b)) {
        (0, _, _) => Strategy::default(),
        (1, Some(glory), Some(despair)) => Strategy::target(glory, despair),
        (2, Some(mark), _) => Strategy::reach(mark),
        (3, _, _) if wa.is_finite() && wb.is_finite() => Strategy::score(wa, wb),
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
    SOLVER.with(|cell| {
        let mut guard = cell.borrow_mut();
        let solver = guard.as_mut()?;
        let s = state(solver, gf, gs, df, ds, ms, sp, tier)?;
        Some(solver.value(s))
    })
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

// ---------------------------------------------------------------- advisor

/// Exchange area for the advisor, shared with JavaScript (see `io`):
/// in  [0..24) each relic's glory then despair, [24..36) each relic's stock;
/// out [40..49) the advice (see `advise`).
static mut IO: [i32; 64] = [0; 64];
/// out: a score key's weights (glory, clean despair)
static mut IO_F: [f64; 2] = [0.0; 2];

/// Where the exchange areas live in the module's memory.
#[unsafe(no_mangle)]
pub extern "C" fn io() -> *mut i32 {
    (&raw mut IO).cast::<i32>()
}

#[unsafe(no_mangle)]
pub extern "C" fn io_f() -> *mut f64 {
    (&raw mut IO_F).cast::<f64>()
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
    Game::standard().advise(&plan, target, level as usize, &states, &stock).ok()
}

/// The look-ahead plan's next step toward `target_level` (with `tier` the
/// crit relic's mark past the top, 0 for none) from the board in `io`, seen
/// at inheritor `level`.  Returns the kind of step, the rest in io[40..49]:
///
/// 0 level up (the next level's requirement is met), 1 roll a relic for the
/// requirement, 2 roll a spare relic for the pity, 3 summon, 4 convert other
/// relics into the crit relic, 5 farm the crit relic, 6 done, -1 no advice.
///
/// io[40] the level worked toward.  For a roll: io[41] the relic; io[42] how
/// it is played - 1 target (io[44] glory, io[45] despair, -1 any), 2 score
/// (weights in io_f), 3 reach (io[44] the mark) - solved at level io[43];
/// io[46] / io[47] the bar's glory / despair (-1 any; -1 both for a farm);
/// io[48] 1 if any roll closing the gap is kept too.
#[unsafe(no_mangle)]
pub extern "C" fn advise(target_level: u32, tier: u32, level: u32) -> i32 {
    let Some(a) = advice(target_level, tier, level) else { return -1 };
    // SAFETY: as in `board_in`
    let (io, io_f) = unsafe { (&mut *(&raw mut IO), &mut *(&raw mut IO_F)) };
    io[40..].fill(-1);
    io[40] = a.goal as i32;
    if let Some((relic, key)) = a.roll() {
        io[41] = relic as i32;
        match key {
            TableKey::Target { level, glory, despair } => {
                io[42] = 1;
                io[43] = i32::from(level);
                io[44] = i32::from(glory);
                io[45] = despair.map_or(-1, i32::from);
            }
            TableKey::Score { level, w_glory, w_despair } => {
                io[42] = 2;
                io[43] = i32::from(level);
                io_f[0] = f64::from_bits(w_glory);
                io_f[1] = f64::from_bits(w_despair);
            }
            TableKey::Reach { level, mark } => {
                io[42] = 3;
                io[43] = i32::from(level);
                io[44] = i32::from(mark);
            }
        }
    }
    match &a.next {
        Next::Move(Move::Quest { bar, closer, .. }) => {
            io[46] = i32::from(bar.glory);
            io[47] = bar.despair.map_or(-1, i32::from);
            io[48] = i32::from(*closer);
        }
        Next::Move(Move::Filler { bar, .. }) => {
            io[46] = i32::from(bar.glory);
            io[47] = bar.despair.map_or(-1, i32::from);
            io[48] = 0;
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

/// Whether the step `advise` gives for the same inputs keeps a fresh
/// (glory, despair) on the relic it rolls: 1 keep, 0 discard, -1 not a roll.
#[unsafe(no_mangle)]
pub extern "C" fn keeps(target_level: u32, tier: u32, level: u32, glory: u32, despair: u32) -> i32 {
    let (Some(a), Some((states, _))) = (advice(target_level, tier, level), board_in()) else { return -1 };
    let (Ok(g), Ok(d)) = (u8::try_from(glory), u8::try_from(despair)) else { return -1 };
    Game::standard().keeps(&a, &states, (g, d)).map_or(-1, i32::from)
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
    }

    #[test]
    fn matches_the_native_solver() {
        assert_eq!(configure(19, 0, 0, 0, 0.0, 0.0, 0.0), 0);
        let mut native = Solver::new(Config::for_level(19, Strategy::default()).unwrap());
        let start = native.start_state();
        let code = best(1, 1, 1, 0, 9, 10, 0);
        let want = match native.best_action(start) {
            Choice::Act(Action::Glory) => 0,
            Choice::Act(Action::Despair) => 1,
            Choice::Act(Action::Train) => 2,
            other => panic!("start state solved to {other:?}"),
        };
        assert_eq!(code, want);
        let analysis = native.analyse(None);
        let score = expected_score(1, 1, 1, 0, 9, 10, 0);
        assert!((score - analysis.e_amplification).abs() < 1e-9, "{score} vs {}", analysis.e_amplification);
    }

    #[test]
    fn rejects_impossible_states() {
        assert_eq!(configure(19, 0, 0, 0, 0.0, 0.0, 0.0), 0);
        assert_eq!(best(10, 0, 0, 0, 9, 10, 0), -1); // 10 glory slots on a 9-slot board
        assert_eq!(best(2, 3, 0, 0, 9, 10, 0), -1); // more successes than fills
        assert_eq!(best(1, 1, 1, 0, 9, 11, 0), -1); // over the spirit cap
        assert_eq!(best(1, 1, 1, 0, 9, 10, 5), -1); // no such tier
        assert_eq!(best(9, 5, 9, 2, 0, 3, 2), 3);
        assert_eq!(best(5, 2, 5, 2, 0, 0, 2), 4);
        assert_eq!(configure(19, 7, 0, 0, 0.0, 0.0, 0.0), -1);
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
    }

    #[test]
    fn advises_like_the_native_advisor() {
        load(&BOARD, &STOCK);
        // Mountain Crown, all or nothing for 8+ / 2- at level 18, gap-closers kept
        assert_eq!(advise(20, 46, 18), 1);
        let io = unsafe { &*(&raw const IO) };
        assert_eq!(&io[40..49], &[19, 11, 1, 18, 8, 2, 8, 2, 1]);
        assert_eq!(keeps(20, 46, 18, 8, 2), 1);
        assert_eq!(keeps(20, 46, 18, 7, 1), 1); // gap 5 -> 4
        assert_eq!(keeps(20, 46, 18, 7, 2), 0);
        assert_eq!(keeps(20, 46, 18, 0, 0), 0); // a wipe
        // too few Mountain Crowns: the next best-stocked 3-despair relic
        let mut stock = STOCK;
        stock[11] = 9;
        load(&BOARD, &stock);
        assert_eq!(advise(20, 46, 18), 1);
        assert_eq!(unsafe { (*(&raw const IO))[41] }, 2);
        // nothing affordable: summon, and no roll to keep
        load(&BOARD, &[0; 12]);
        assert_eq!(advise(20, 46, 18), 3);
        assert_eq!(keeps(20, 46, 18, 9, 0), -1);
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
        assert_eq!(configure(18, 3, 0, 0, 1.0, 1.0, 0.0), 0); // score
        assert!(matches!(best(1, 1, 0, 0, 9, 10, 1), 0..=2));
        assert_eq!(configure(18, 1, 8, 255, 0.0, 0.0, 0.0), 0); // any despair
        assert!(matches!(best(1, 1, 0, 0, 9, 10, 1), 0..=2));
    }
}
