//! The exact solver: the board at each level, the rules of an attempt, and
//! the expectations the solve reports.

use relic::rng::Rng;
use relic::solver::{Action, BEST_TIER, Choice, Config, Solver, State, Strategy, TIERS, WORST_TIER};

fn level(n: i64) -> Config {
    Config::for_level(n, Strategy::default()).unwrap()
}

fn st(gf: u8, gs: u8, df: u8, ds: u8, ms: u8, sp: u8, tier: u8) -> State {
    State { gf, gs, df, ds, ms, sp, tier }
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// An independent recursive evaluation of the optimal policy, not sharing the
/// forward sweep, so the two can be checked against each other: (E[glory],
/// E[despair successes], P(wipe)).
fn forward_expectations(solver: &mut Solver, s: State) -> (f64, f64, f64) {
    if solver.is_terminal(s) {
        return (f64::from(s.gs), f64::from(s.ds), 0.0);
    }
    let Choice::Act(action) = solver.best_action(s) else {
        return (0.0, 0.0, 1.0); // an incomplete attempt inherits nothing
    };
    let (mut g, mut d, mut w) = (0.0, 0.0, 0.0);
    for (p, next, _ok) in solver.transitions(s, action) {
        if p != 0.0 {
            let sub = forward_expectations(solver, next);
            g += p * sub.0;
            d += p * sub.1;
            w += p * sub.2;
        }
    }
    (g, d, w)
}

// ------------------------------------------------------------------ levels

#[test]
fn unknown_levels_are_refused() {
    for bad in [0, -1, -100] {
        assert!(Config::for_level(bad, Strategy::default()).is_err(), "level {bad}");
    }
}

#[test]
fn cumulative_bonuses() {
    //  level, slots, max_spirit, glory_mod, despair_mod
    let expected = [
        (1, 5, 8, 0.00, 0.00),
        (2, 5, 8, 0.02, 0.00),
        (3, 5, 8, 0.02, -0.02),
        (4, 6, 8, 0.02, -0.02),
        (5, 6, 9, 0.02, -0.02),
        (6, 6, 9, 0.04, -0.02),
        (7, 6, 9, 0.04, -0.04),
        (8, 7, 9, 0.04, -0.04),
        (9, 7, 10, 0.04, -0.04),
        (10, 7, 10, 0.06, -0.04),
        (11, 7, 10, 0.06, -0.06),
        (12, 8, 10, 0.06, -0.06),
        (13, 8, 10, 0.08, -0.06),
        (14, 8, 10, 0.08, -0.08),
        (15, 8, 10, 0.08, -0.08),
        (16, 9, 10, 0.08, -0.08),
        (17, 9, 10, 0.10, -0.08),
        (18, 9, 10, 0.10, -0.10),
        (19, 9, 10, 0.10, -0.10),
        (20, 10, 10, 0.10, -0.10),
    ];
    for (lvl, slots, sp, gm, dm) in expected {
        let cfg = level(lvl);
        assert_eq!((cfg.slots, cfg.max_spirit), (slots, sp), "level {lvl}");
        assert!(close(cfg.glory_mod, gm, 1e-12), "level {lvl}");
        assert!(close(cfg.despair_mod, dm, 1e-12), "level {lvl}");
    }
}

#[test]
fn level_15_and_19_start_the_bars_partly_filled() {
    assert_eq!((level(14).start_glory, level(14).start_despair_fail), (0, 0));
    assert_eq!((level(15).start_glory, level(15).start_despair_fail), (1, 0));
    assert_eq!((level(18).start_glory, level(18).start_despair_fail), (1, 0));
    assert_eq!((level(19).start_glory, level(19).start_despair_fail), (1, 1));
    assert_eq!((level(20).start_glory, level(20).start_despair_fail), (1, 1));
    let start = Solver::new(level(19)).start_state();
    assert_eq!((start.gf, start.gs, start.df, start.ds), (1, 1, 1, 0));
    // the glory success moves the ladder down a tier, the despair failure back up
    for (lv, tier) in [(14, BEST_TIER), (15, 1), (18, 1), (19, BEST_TIER), (20, BEST_TIER)] {
        assert_eq!(level(lv).start_tier, tier, "level {lv}");
    }
    assert_eq!(TIERS[usize::from(level(15).start_tier)], 0.65);
    // a head start only ever helps
    let (before, after) = (Solver::new(level(14)).analyse(None), Solver::new(level(15)).analyse(None));
    assert!(after.e_amplification > before.e_amplification);
    assert!(after.e_glory >= 1.0);
    let (before, after) = (Solver::new(level(18)).analyse(None), Solver::new(level(19)).analyse(None));
    assert!(after.e_despair_success < before.e_despair_success);
    assert!(after.p_dead_end < before.p_dead_end);
}

#[test]
fn the_bonuses_cycle_every_four_levels() {
    // to level 12, four levels add one slot, one spirit power, +2% and -2%
    for lvl in [2, 5, 8] {
        let (low, high) = (level(lvl), level(lvl + 4));
        assert_eq!(high.slots, low.slots + 1, "level {lvl}+4");
        assert_eq!(high.max_spirit, low.max_spirit + 1, "level {lvl}+4");
        assert!(close(high.glory_mod, low.glory_mod + 0.02, 1e-12));
        assert!(close(high.despair_mod, low.despair_mod - 0.02, 1e-12));
    }
    // from 13 the spirit power step is gone: one slot, +2% and -2%
    for lvl in [12, 13, 16, 17, 20] {
        let (low, high) = (level(lvl), level(lvl + 4));
        assert_eq!(high.slots, low.slots + 1, "level {lvl}+4");
        assert_eq!(high.max_spirit, low.max_spirit, "level {lvl}+4");
        assert!(close(high.glory_mod, low.glory_mod + 0.02, 1e-12));
        assert!(close(high.despair_mod, low.despair_mod - 0.02, 1e-12));
    }
}

#[test]
fn mental_strength_equals_slots() {
    for lvl in 1..25 {
        let cfg = level(lvl);
        assert_eq!(cfg.start_mental(), cfg.slots);
    }
}

// ------------------------------------------------------------------ rules

#[test]
fn the_tier_ladder_clamps() {
    let solver = Solver::new(level(1));
    // success at the easiest tier moves down; failure there stays put
    let [(_, win, _), (_, lose, _)] = solver.transitions(st(0, 0, 0, 0, 5, 7, BEST_TIER), Action::Glory);
    assert_eq!((win.tier, lose.tier), (BEST_TIER + 1, BEST_TIER));
    let [(_, win, _), (_, lose, _)] = solver.transitions(st(0, 0, 0, 0, 5, 7, WORST_TIER), Action::Glory);
    assert_eq!((win.tier, lose.tier), (WORST_TIER, WORST_TIER - 1));
}

#[test]
fn training_cannot_overcap_spirit_power() {
    let solver = Solver::new(level(1));
    let [(_, win, _), (_, lose, _)] = solver.transitions(st(0, 0, 0, 0, 5, 7, BEST_TIER), Action::Train);
    assert_eq!(win.sp, 8); // +2 would be 9, capped at 8
    assert_eq!(lose.sp, 7); // a failed training gains nothing
    assert_eq!((win.ms, lose.ms), (4, 4)); // mental strength spent either way
}

#[test]
fn legality() {
    use Action::{Despair, Glory, Train};
    let solver = Solver::new(level(1));
    assert_eq!(solver.legal_actions(st(0, 0, 0, 0, 3, 0, 0)), vec![Train]); // no spirit power
    assert_eq!(solver.legal_actions(st(0, 0, 0, 0, 0, 3, 0)), vec![Glory, Despair]); // no mental strength
    assert_eq!(solver.legal_actions(st(5, 3, 0, 0, 0, 3, 0)), vec![Despair]); // glory bar full
    assert!(solver.legal_actions(st(2, 1, 2, 1, 0, 0, 0)).is_empty());
}

#[test]
fn a_dead_end_is_a_total_wipe() {
    let mut solver = Solver::new(level(1));
    let v = solver.value(st(2, 2, 2, 0, 0, 0, 0)); // bars unfinished, no resources
    assert_eq!(v.choice, Choice::DeadEnd);
    assert_eq!(v.p_finish, 0.0);
    // amplification is floored, so a wipe pays 0 rather than going negative
    assert_eq!(v.score, solver.min_amplification());
}

#[test]
fn terminal_scoring() {
    let mut solver = Solver::new(level(1));
    let v = solver.value(st(5, 4, 5, 1, 2, 3, 0));
    assert_eq!(v.choice, Choice::Done);
    assert_eq!(v.p_finish, 1.0);
    assert_eq!(v.score, 5.0 * 4.0 - 2.0 * 1.0);
}

#[test]
fn chance_modifiers_apply_per_action() {
    let solver = Solver::new(level(8)); // +4% glory, -4% despair
    assert!(close(solver.chance(Action::Glory, 0), 0.84, 1e-12));
    assert!(close(solver.chance(Action::Despair, 0), 0.76, 1e-12));
    assert!(close(solver.chance(Action::Train, 0), 0.80, 1e-12));
}

#[test]
fn chance_stays_in_range() {
    let solver = Solver::new(Config { train_mod: 0.5, ..level(1) });
    assert!(solver.chance(Action::Train, 0) <= 1.0);
    let solver = Solver::new(Config { train_mod: -0.9, ..level(1) });
    assert!(solver.chance(Action::Train, WORST_TIER) >= 0.0);
}

// ------------------------------------------------------------------ analysis

#[test]
fn the_distribution_is_a_distribution() {
    for lvl in [1, 4, 8] {
        let a = Solver::new(level(lvl)).analyse(None);
        assert!(close(a.dist.total(), 1.0, 1e-9), "level {lvl}");
        for ((g, d), p) in a.dist.iter() {
            assert!(p >= 0.0);
            assert!(g.max(d) <= a.cfg.slots);
        }
    }
}

#[test]
fn the_forward_sweep_matches_direct_recursion() {
    for lvl in [1, 2, 4] {
        let mut solver = Solver::new(level(lvl));
        let a = solver.analyse(None);
        let start = solver.start_state();
        let (g, d, w) = forward_expectations(&mut solver, start);
        assert!(close(a.e_glory, g, 1e-9), "level {lvl}");
        assert!(close(a.e_despair_success, d, 1e-9), "level {lvl}");
        assert!(close(a.p_dead_end, w, 1e-9), "level {lvl}");
    }
}

#[test]
fn amplification_is_summed_over_outcomes_not_over_averages() {
    // the zero floor makes amplification non-linear, so the expectation is
    // summed outcome by outcome; the linear shortcut under-states it
    for (wg, wd) in [(5.0, 2.0), (1.0, 1.0), (3.0, 4.0)] {
        let mut solver = Solver::new(Config::for_level(1, Strategy::weighted(wg, wd)).unwrap());
        let a = solver.analyse(None);
        let summed: f64 = a.dist.iter().map(|((g, d), p)| solver.amplification(g, d) * p).sum();
        assert!(close(a.e_amplification, summed, 1e-9));
        assert!(a.e_amplification + 1e-12 >= wg * a.e_glory - wd * a.e_despair_success);
    }
}

#[test]
fn equal_weights_stay_close_to_the_unfloored_numbers() {
    let a = Solver::new(Config::for_level(1, Strategy::weighted(1.0, 1.0)).unwrap()).analyse(None);
    assert!(close(a.e_glory, 3.4583, 0.05));
    assert!(close(a.e_despair_success, 2.1060, 0.05));
}

#[test]
fn safety_first_minimises_the_wipe() {
    let plain = Solver::new(level(1)).analyse(None);
    let safe = Solver::new(Config { safety_first: true, ..level(1) }).analyse(None);
    assert!(safe.p_dead_end < plain.p_dead_end);
    assert!(safe.e_amplification <= plain.e_amplification + 1e-12); // it can only cost score
}

#[test]
fn bars_are_filled_exactly_once() {
    let a = Solver::new(level(1)).analyse(None);
    let slots = f64::from(a.cfg.slots);
    for action in [Action::Glory, Action::Despair] {
        assert!(a.action_counts.get(action) <= slots + 1e-9);
        assert!(a.action_counts.get(action) >= slots * (1.0 - a.p_dead_end) - 1e-9);
    }
    assert!(a.action_counts.get(Action::Train) <= f64::from(a.cfg.start_mental()) + 1e-9);
}

#[test]
fn monte_carlo_agrees_with_the_exact_solve() {
    let mut solver = Solver::new(level(1));
    let a = solver.analyse(None);
    let mut rng = Rng::for_run(20260922, 0);
    let trials = 120_000;
    let (mut g, mut d, mut wipes) = (0.0, 0.0, 0.0);
    for _ in 0..trials {
        let res = solver.attempt(&mut rng, None);
        g += f64::from(res.glory_success);
        d += f64::from(res.despair_success);
        wipes += f64::from(u8::from(res.dead_end));
    }
    let n = f64::from(trials);
    assert!(close(g / n, a.e_glory, 0.02));
    assert!(close(d / n, a.e_despair_success, 0.02));
    assert!(close(wipes / n, a.p_dead_end, 0.005));
}

#[test]
fn simulated_attempts_obey_the_rules() {
    let mut solver = Solver::new(level(1));
    let mut rng = Rng::for_run(7, 0);
    for _ in 0..500 {
        let mut log = Vec::new();
        let res = solver.attempt(&mut rng, Some(&mut log));
        for step in &log {
            let s = step.state;
            assert!(solver.legal_actions(s).contains(&step.action));
            assert!(s.sp <= solver.cfg.max_spirit);
            assert!(s.ms <= solver.cfg.start_mental());
            assert!(usize::from(s.tier) < TIERS.len());
            assert!(s.gs <= s.gf && s.ds <= s.df);
        }
        if !res.dead_end {
            assert!(res.glory_success <= solver.cfg.slots && res.despair_success <= solver.cfg.slots);
        }
    }
}

// ------------------------------------------------------------------ strategies

#[test]
fn the_default_is_the_weighted_objective() {
    let cfg = level(1);
    assert!(!cfg.strategy.is_target());
    assert_eq!((cfg.strategy.w_glory, cfg.strategy.w_despair), (5.0, 2.0));
    assert_eq!(cfg.strategy.name(), "weighted (+5%/-2%)");
}

#[test]
fn amplification_is_five_glory_minus_two_despair_floored() {
    let solver = Solver::new(level(1));
    assert_eq!(solver.amplification(3, 1), 13.0);
    assert_eq!(solver.amplification(4, 2), 16.0);
    assert_eq!(solver.max_amplification(), 25.0); // a full 5-slot glory bar
    assert_eq!(solver.min_amplification(), 0.0);
    assert_eq!(solver.amplification(0, 5), 0.0); // would be -10
    assert_eq!(solver.amplification(1, 3), 0.0); // would be -1
    assert_eq!(solver.amplification(1, 2), 1.0);
    // every slot counts at its own rate
    assert_eq!(solver.amplification(5, 2) - solver.amplification(4, 2), 5.0);
    assert_eq!(solver.amplification(4, 1) - solver.amplification(4, 2), 2.0);
}

#[test]
fn the_floor_makes_the_solver_accept_more_wipes() {
    // a wipe costs 0 instead of -2 x slots, so dodging one is worth less
    assert!(Solver::new(level(8)).analyse(None).p_dead_end > 0.0419);
}

#[test]
fn targets_are_all_or_nothing() {
    let solver = Solver::new(Config::for_level(1, Strategy::target(4, 2)).unwrap());
    assert_eq!(solver.objective(4, 2), 1.0);
    assert_eq!(solver.objective(5, 0), 1.0);
    assert_eq!(solver.objective(3, 2), 0.0);
    assert_eq!(solver.objective(4, 3), 0.0);
    assert_eq!(solver.objective(3, 2), solver.objective(0, 5)); // no partial credit
    assert!(solver.hits_target(4, 2) && solver.hits_target(5, 0));
    assert!(!solver.hits_target(3, 2) && !solver.hits_target(4, 3));
}

#[test]
fn weighted_strategies_have_no_target() {
    let mut solver = Solver::new(level(1));
    assert!(!solver.hits_target(5, 0));
    assert_eq!(solver.analyse(None).p_target, 0.0);
}

#[test]
fn targets_clamp_to_the_bar_size() {
    let cfg = Config::for_level(1, Strategy::target(99, 99)).unwrap(); // 5 slots
    assert_eq!((cfg.want_glory(), cfg.allow_despair()), (5, 5));
}

#[test]
fn p_target_matches_the_distribution() {
    for strat in [Strategy::target(4, 2), Strategy::target(5, 1)] {
        let mut solver = Solver::new(Config::for_level(4, strat).unwrap());
        let a = solver.analyse(None);
        let manual: f64 = a.dist.iter().filter(|&((g, d), _)| solver.hits_target(g, d)).map(|(_, p)| p).sum();
        assert!(close(a.p_target, manual, 1e-12), "{}", strat.name());
    }
}

#[test]
fn a_target_beats_the_weighted_policy_on_its_own_box() {
    let target = Solver::new(Config::for_level(1, Strategy::target(4, 2)).unwrap()).analyse(None);
    let plain = Solver::new(level(1)).analyse(None);
    let rate: f64 = plain.dist.iter().filter(|&((g, d), _)| g >= 4 && d <= 2).map(|(_, p)| p).sum();
    assert!(target.p_target + 1e-12 >= rate);
    assert!(target.e_amplification <= plain.e_amplification + 1e-12);
}

#[test]
fn an_incomplete_attempt_records_as_zero_and_zero() {
    let mut solver = Solver::new(level(1));
    let a = solver.analyse(None);
    assert!(a.dist.get((0, 0)) >= a.p_dead_end - 1e-12);
    let mut rng = Rng::for_run(1, 0);
    let wipe =
        (0..4000).map(|_| solver.attempt(&mut rng, None)).find(|r| r.dead_end).expect("a wipe in 4000");
    assert_eq!((wipe.glory_success, wipe.despair_success), (0, 0));
}

#[test]
fn an_incomplete_attempt_never_hits_even_a_lenient_target() {
    // target(0, 0) is met by the (0, 0) cell on paper, but an attempt that
    // never finished must not count
    let mut solver = Solver::new(Config::for_level(1, Strategy::target(0, 0)).unwrap());
    let a = solver.analyse(None);
    assert!(a.p_dead_end > 0.0);
    let by_cell: f64 = a.dist.iter().filter(|&((g, d), _)| solver.hits_target(g, d)).map(|(_, p)| p).sum();
    assert!(close(a.p_target, by_cell - a.p_dead_end, 1e-12));
}

#[test]
fn recording_a_wipe_as_zero_zero_leaves_the_policy_alone() {
    let a = Solver::new(level(7)).analyse(None);
    assert!(close(a.e_glory, 4.2309, 5e-4));
    assert!(close(a.e_amplification, 16.8113, 5e-4));
}

#[test]
fn the_score_objective_is_unfloored_and_never_negative() {
    let solver = Solver::new(Config::for_level(1, Strategy::score(1.0, 3.0)).unwrap()); // 5 slots
    // a 5/2 and a 1/5 both floor to 0 as amplification; score keeps them apart
    assert_eq!(solver.objective(5, 2), 5.0 + 3.0 * 3.0);
    assert_eq!(solver.objective(1, 5), 1.0);
    for g in 0..=5 {
        for d in 0..=5 {
            assert!(solver.objective(g, d) >= 0.0);
        }
    }
    assert_eq!(solver.cfg.strategy.name(), "score (1 glory : 3 despair)");
}

// ------------------------------------------------------------------ edge cases

#[test]
fn training_is_a_tier_lever_not_just_a_spirit_power_refill() {
    // spirit power alone covers both bars, so training is never needed for
    // fuel - and it still gets used: a failed training costs no slot and
    // pushes the rate back up the ladder
    let mut solver = Solver::new(Config { slots: 2, max_spirit: 7, ..level(1) });
    let a = solver.analyse(None);
    assert_eq!(a.p_dead_end, 0.0);
    assert!(a.action_counts.get(Action::Train) > 0.0);
    assert_eq!(solver.best_action(st(1, 1, 2, 0, 1, 5, WORST_TIER)), Choice::Act(Action::Train));
    assert_eq!(solver.best_action(st(1, 1, 2, 0, 1, 5, BEST_TIER)), Choice::Act(Action::Glory));
}

#[test]
fn the_wipe_chance_is_never_zero_when_training_is_required() {
    for lvl in [1, 4, 8] {
        let a = Solver::new(Config { safety_first: true, ..level(lvl) }).analyse(None);
        assert!(a.p_dead_end > 0.0, "level {lvl}");
    }
}

#[test]
fn reaching_a_mark_is_all_or_nothing_on_amplification() {
    // P(amplification >= mark) from an attempt's outcome distribution
    let p_reach = |strategy: Strategy, mark: f64| {
        let mut solver = Solver::new(Config::for_level(20, strategy).unwrap());
        let a = solver.analyse(None);
        a.dist.iter().filter(|&((g, d), _)| solver.amplification(g, d) >= mark).map(|(_, p)| p).sum::<f64>()
    };
    // 50% is a full glory bar with no despair: the same box as target (10, 0)
    assert!(close(p_reach(Strategy::reach(50), 50.0), p_reach(Strategy::target(10, 0), 50.0), 1e-12));
    // playing for a mark beats playing for amplification at that mark
    for mark in [42, 45, 48] {
        let (reach, weighted) =
            (p_reach(Strategy::reach(mark), f64::from(mark)), p_reach(Strategy::default(), f64::from(mark)));
        assert!(reach >= weighted - 1e-12, "{mark}%: {reach} vs {weighted}");
    }
}

#[test]
fn a_settled_target_is_played_for_amplification() {
    // all or nothing for 8+ glory, 2- despair at level 18; one glory slot
    // left, 3 despair successes already: out of reach, every action scores 0
    let mut solver = Solver::new(Config::for_level(18, Strategy::target(8, 2)).unwrap());
    let lost = st(8, 5, 9, 3, 3, 4, 3);
    assert_eq!(solver.value(lost).score, 0.0);
    // the tie goes to amplification: a 35% training (usually a fail, the
    // ladder up to 50%) before the glory roll, not glory at 35% + 10% now
    assert_eq!(solver.best_action(lost), Choice::Act(Action::Train));
    // while the box can still be hit, the same holds on its own merit
    let open = st(8, 7, 9, 2, 3, 4, 3);
    assert!(solver.value(open).score > 0.0);
    assert_eq!(solver.best_action(open), Choice::Act(Action::Train));
}

/// P(finishing with glory >= g, despair <= d) following `play`'s choices from `s`.
fn chance_of(play: &mut Solver, s: State, g: u8, d: u8) -> f64 {
    match play.best_action(s) {
        Choice::Done => f64::from(u8::from(s.gs >= g && s.ds <= d)),
        Choice::Act(a) => play.transitions(s, a).iter().filter(|t| t.0 > 0.0).map(|&(p, n, _)| p * chance_of(play, n, g, d)).sum(),
        _ => 0.0,
    }
}

#[test]
fn a_target_out_of_reach_is_played_for_the_nearest_box() {
    // all or nothing for 7/1, but 2 despair successes already: 7/2 is the
    // nearest result left, and it is played for as hard as if it were the
    // target (amplification alone would settle for 6/2 more: 43% not 54%)
    let mut for_7_1 = Solver::new(Config::for_level(18, Strategy::target(7, 1)).unwrap());
    let mut for_7_2 = Solver::new(Config::for_level(18, Strategy::target(7, 2)).unwrap());
    let s = st(7, 5, 9, 2, 2, 1, 0);
    assert_eq!(for_7_1.value(s).score, 0.0);
    let best = for_7_2.value(s).score;
    assert!(close(best, 0.54, 1e-9), "{best}");
    assert!(close(chance_of(&mut for_7_1, s, 7, 2), best, 1e-12));
    assert!(close(for_7_1.value(s).near, best, 1e-12));
    // and so from every state where 7/1 is gone and 7/2 is not
    for gf in 0..=9 {
        for gs in 0..=gf {
            for df in 2..=9 {
                for (ms, sp, tier) in [(0, 3, 1), (1, 6, 0), (2, 10, 2), (3, 8, 4)] {
                    let s = st(gf, gs, df, 2, ms, sp, tier);
                    let want = for_7_2.value(s).score;
                    if want > 0.0 {
                        assert!(close(chance_of(&mut for_7_1, s, 7, 2), want, 1e-9), "{s:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn settled_ties_change_nothing_but_what_follows_the_target() {
    // a whole attempt for the target: P(hit) is the plain optimum's; the
    // tie-breaks (the nearest box, then amplification) only shape the play
    // once it is settled (relic solve --level 7 --target 5 1)
    let mut solver = Solver::new(Config::for_level(7, Strategy::target(5, 1)).unwrap());
    let a = solver.analyse(None);
    assert!(close(a.p_target, 0.114972, 1e-6), "{}", a.p_target);
    assert!(close(a.e_amplification, 16.377, 1e-3), "{}", a.e_amplification);
}
