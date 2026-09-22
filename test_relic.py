#!/usr/bin/env python3
"""Checks for the relic inheritance solver.

Run with:  python test_relic.py
"""

from __future__ import annotations

import random
import unittest

from relic import (
    BEST_TIER,
    DEAD_END,
    DESPAIR,
    GLORY,
    TIERS,
    TRAIN,
    WORST_TIER,
    Inheritance,
    UnknownLevelError,
    Strategy,
    config_for_level,
    target,
    weighted,
)


def forward_expectations(solver: Inheritance, state=None):
    """Independent recursive evaluation of the policy.

    Deliberately does NOT reuse Analysis' layered sweep, so the two can be
    compared against each other.  Returns (E[glory], E[despair successes],
    P(wipe)).
    """
    if state is None:
        state = solver.start_state()
    if solver.is_terminal(state):
        _gf, gs, _df, ds, _ms, _sp, _t = state
        return float(gs), float(ds), 0.0
    action = solver.best_action(state)
    if action == DEAD_END:
        # an incomplete attempt inherits nothing: 0 glory, 0 despair
        return 0.0, 0.0, 1.0
    g = d = w = 0.0
    for prob, nxt, _ok in solver.transitions(state, action):
        if not prob:
            continue
        sub = forward_expectations(solver, nxt)
        g += prob * sub[0]
        d += prob * sub[1]
        w += prob * sub[2]
    return g, d, w


class TestLevels(unittest.TestCase):
    def test_unknown_levels_raise(self):
        for bad in (0, -1, -100):
            with self.assertRaises(UnknownLevelError):
                config_for_level(bad)

    def test_non_integer_level_raises(self):
        for bad in (1.5, "1", None, True):
            with self.assertRaises(UnknownLevelError):
                config_for_level(bad)

    def test_cumulative_bonuses(self):
        expected = {
            #        slots, max_spirit, glory_mod, despair_mod
            1: (5, 8, 0.00, 0.00),
            2: (5, 8, 0.02, 0.00),
            3: (5, 8, 0.02, -0.02),
            4: (6, 8, 0.02, -0.02),
            5: (6, 9, 0.02, -0.02),
            6: (6, 9, 0.04, -0.02),
            7: (6, 9, 0.04, -0.04),
            8: (7, 9, 0.04, -0.04),
            9: (7, 10, 0.04, -0.04),
            10: (7, 10, 0.06, -0.04),
            11: (7, 10, 0.06, -0.06),
            12: (8, 10, 0.06, -0.06),
            13: (8, 11, 0.06, -0.06),
            20: (10, 12, 0.10, -0.10),
        }
        for level, (slots, sp, gm, dm) in expected.items():
            cfg = config_for_level(level)
            self.assertEqual((cfg.slots, cfg.max_spirit), (slots, sp), f"level {level}")
            self.assertAlmostEqual(cfg.glory_mod, gm, msg=f"level {level}")
            self.assertAlmostEqual(cfg.despair_mod, dm, msg=f"level {level}")

    def test_high_levels_keep_cycling(self):
        # four levels add exactly one slot, one spirit power, +2% and -2%
        for level in (2, 5, 9, 13, 17):
            low = config_for_level(level)
            high = config_for_level(level + 4)
            self.assertEqual(high.slots, low.slots + 1, f"level {level}+4")
            self.assertEqual(high.max_spirit, low.max_spirit + 1, f"level {level}+4")
            self.assertAlmostEqual(high.glory_mod, low.glory_mod + 0.02)
            self.assertAlmostEqual(high.despair_mod, low.despair_mod - 0.02)

    def test_mental_strength_equals_slots(self):
        for level in range(1, 25):
            cfg = config_for_level(level)
            self.assertEqual(cfg.start_mental, cfg.slots)


class TestRules(unittest.TestCase):
    def setUp(self):
        self.solver = Inheritance(config_for_level(1))

    def test_tier_ladder_clamps(self):
        # success at the easiest tier stays on the ladder, fail at the easiest
        # tier clamps rather than running off the top
        state = (0, 0, 0, 0, 5, 7, BEST_TIER)
        outs = {ok: nxt for _p, nxt, ok in self.solver.transitions(state, GLORY)}
        self.assertEqual(outs[True][6], BEST_TIER + 1)
        self.assertEqual(outs[False][6], BEST_TIER)
        state = (0, 0, 0, 0, 5, 7, WORST_TIER)
        outs = {ok: nxt for _p, nxt, ok in self.solver.transitions(state, GLORY)}
        self.assertEqual(outs[True][6], WORST_TIER)
        self.assertEqual(outs[False][6], WORST_TIER - 1)

    def test_training_cannot_overcap_spirit_power(self):
        state = (0, 0, 0, 0, 5, 7, BEST_TIER)  # 7 of a max 8
        outs = {ok: nxt for _p, nxt, ok in self.solver.transitions(state, TRAIN)}
        self.assertEqual(outs[True][5], 8)   # +2 would be 9, capped at 8
        self.assertEqual(outs[False][5], 7)  # a failed training gains nothing
        self.assertEqual(outs[True][4], 4)   # mental strength spent either way
        self.assertEqual(outs[False][4], 4)

    def test_legality(self):
        # no spirit power: only training is available
        self.assertEqual(self.solver.legal_actions((0, 0, 0, 0, 3, 0, 0)), (TRAIN,))
        # no mental strength: only the two bars
        self.assertEqual(self.solver.legal_actions((0, 0, 0, 0, 0, 3, 0)), (GLORY, DESPAIR))
        # a full glory bar removes only glory
        self.assertEqual(self.solver.legal_actions((5, 3, 0, 0, 0, 3, 0)), (DESPAIR,))
        # nothing left at all
        self.assertEqual(self.solver.legal_actions((2, 1, 2, 1, 0, 0, 0)), ())

    def test_dead_end_is_a_total_wipe(self):
        stuck = (2, 2, 2, 0, 0, 0, 0)  # bars unfinished, no resources
        p_finish, score, action = self.solver.value(stuck)
        self.assertEqual(action, DEAD_END)
        self.assertEqual(p_finish, 0.0)
        # the two glory successes already banked are lost and every despair slot
        # counts as a success - but amplification is floored, so a wipe pays 0
        # rather than going negative
        self.assertEqual(score, self.solver.min_amplification())
        self.assertEqual(score, 0.0)

    def test_terminal_scoring(self):
        done = (5, 4, 5, 1, 2, 3, 0)  # 4 glory successes, 1 despair success
        p_finish, score, action = self.solver.value(done)
        self.assertIsNone(action)
        self.assertEqual(p_finish, 1.0)
        self.assertEqual(score, 5 * 4 - 2 * 1)  # amplification = 18

    def test_chance_modifiers_apply_per_action(self):
        solver = Inheritance(config_for_level(8))  # +4% glory, -4% despair
        self.assertAlmostEqual(solver.chance(GLORY, 0), 0.84)
        self.assertAlmostEqual(solver.chance(DESPAIR, 0), 0.76)
        self.assertAlmostEqual(solver.chance(TRAIN, 0), 0.80)

    def test_chance_stays_in_range(self):
        cfg = config_for_level(1, train_mod=0.5)
        solver = Inheritance(cfg)
        self.assertLessEqual(solver.chance(TRAIN, 0), 1.0)
        cfg = config_for_level(1, train_mod=-0.9)
        solver = Inheritance(cfg)
        self.assertGreaterEqual(solver.chance(TRAIN, WORST_TIER), 0.0)


class TestAnalysis(unittest.TestCase):
    def test_distribution_is_a_distribution(self):
        for level in (1, 4, 8):
            a = Inheritance(config_for_level(level)).analyse()
            self.assertAlmostEqual(sum(a.dist.values()), 1.0, places=9, msg=f"level {level}")
            self.assertTrue(all(p >= 0 for p in a.dist.values()))
            for g, d in a.dist:
                self.assertLessEqual(max(g, d), a.cfg.slots)

    def test_layered_sweep_matches_direct_recursion(self):
        for level in (1, 2, 4):
            solver = Inheritance(config_for_level(level))
            a = solver.analyse()
            g, d, w = forward_expectations(solver)
            self.assertAlmostEqual(a.e_glory, g, places=9, msg=f"level {level}")
            self.assertAlmostEqual(a.e_despair_success, d, places=9, msg=f"level {level}")
            self.assertAlmostEqual(a.p_dead_end, w, places=9, msg=f"level {level}")

    def test_amplification_is_summed_over_outcomes_not_over_averages(self):
        # the zero floor makes amplification non-linear, so the expectation has
        # to be summed outcome by outcome; the linear shortcut UNDER-states it
        for w_g, w_d in ((5.0, 2.0), (1.0, 1.0), (3.0, 4.0)):
            cfg = config_for_level(1, strategy=weighted(w_g, w_d))
            solver = Inheritance(cfg)
            a = solver.analyse()
            summed = sum(solver.amplification(g, d) * p for (g, d), p in a.dist.items())
            self.assertAlmostEqual(a.e_amplification, summed, places=9)
            linear = w_g * a.e_glory - w_d * a.e_despair_success
            self.assertGreaterEqual(a.e_amplification + 1e-12, linear)

    def test_equal_weights_stay_close_to_the_unfloored_numbers(self):
        # weighted(1, 1) is the old "most glory, least despair" objective, but
        # the zero floor blunts the downside, so the policy drifts slightly
        # rather than matching the old 3.4583 / 2.1060 exactly
        a = Inheritance(config_for_level(1, strategy=weighted(1, 1))).analyse()
        self.assertAlmostEqual(a.e_glory, 3.4583, delta=0.05)
        self.assertAlmostEqual(a.e_despair_success, 2.1060, delta=0.05)

    def test_safety_first_minimises_the_wipe(self):
        plain = Inheritance(config_for_level(1)).analyse()
        safe = Inheritance(config_for_level(1, safety_first=True)).analyse()
        self.assertLess(safe.p_dead_end, plain.p_dead_end)
        # and it can only cost score, never gain it
        self.assertLessEqual(safe.e_amplification, plain.e_amplification + 1e-12)

    def test_bars_are_filled_exactly_once(self):
        # a run that does not wipe fills every slot in both bars, so the
        # expected number of bar actions sits between "every finished run" and
        # "the hard cap"
        a = Inheritance(config_for_level(1)).analyse()
        slots = a.cfg.slots
        for action in (GLORY, DESPAIR):
            self.assertLessEqual(a.action_counts[action], slots + 1e-9)
            self.assertGreaterEqual(a.action_counts[action], slots * (1 - a.p_dead_end) - 1e-9)
        self.assertLessEqual(a.action_counts[TRAIN], a.cfg.start_mental + 1e-9)

    def test_monte_carlo_agrees_with_the_exact_solve(self):
        solver = Inheritance(config_for_level(1))
        a = solver.analyse()
        rng = random.Random(20260922)
        trials = 120_000
        g = d = wipes = 0
        for _ in range(trials):
            res = solver.attempt(rng)
            g += res["glory_success"]
            d += res["despair_success"]
            wipes += res["dead_end"]
        # 3 standard errors on a mean of ~120k draws of a 0..5 variable
        self.assertAlmostEqual(g / trials, a.e_glory, delta=0.02)
        self.assertAlmostEqual(d / trials, a.e_despair_success, delta=0.02)
        self.assertAlmostEqual(wipes / trials, a.p_dead_end, delta=0.005)

    def test_simulated_attempts_obey_the_rules(self):
        solver = Inheritance(config_for_level(1))
        rng = random.Random(7)
        for _ in range(500):
            log = []
            res = solver.attempt(rng, log)
            for state, action, _ok, _nxt in log:
                self.assertIn(action, solver.legal_actions(state))
                gf, gs, df, ds, ms, sp, t = state
                self.assertTrue(0 <= sp <= solver.cfg.max_spirit)
                self.assertTrue(0 <= ms <= solver.cfg.start_mental)
                self.assertTrue(0 <= t < len(TIERS))
                self.assertLessEqual(gs, gf)
                self.assertLessEqual(ds, df)
            if not res["dead_end"]:
                self.assertLessEqual(res["glory_success"], solver.cfg.slots)
                self.assertLessEqual(res["despair_success"], solver.cfg.slots)


class TestStrategies(unittest.TestCase):
    def test_default_is_the_weighted_objective(self):
        cfg = config_for_level(1)
        self.assertFalse(cfg.is_target)
        self.assertEqual((cfg.w_glory, cfg.w_despair), (5.0, 2.0))
        self.assertEqual(cfg.strategy.name, "weighted (+5%/-2%)")

    def test_amplification_is_a_percentage(self):
        # 5 percentage points per glory success, 2 lost per despair success
        solver = Inheritance(config_for_level(1))
        self.assertEqual(solver.amplification(3, 1), 5 * 3 - 2 * 1)   # 13%
        self.assertEqual(solver.max_amplification(), 25.0)            # 5 slots x 5%

    def test_amplification_is_five_glory_minus_two_despair(self):
        solver = Inheritance(config_for_level(1))
        self.assertEqual(solver.amplification(4, 2), 5 * 4 - 2 * 2)
        self.assertEqual(solver.max_amplification(), 5 * 5)    # full glory bar
        self.assertEqual(solver.min_amplification(), 0.0)      # floored

    def test_amplification_never_goes_below_zero(self):
        solver = Inheritance(config_for_level(1))   # 5 slots, 5 glory / 2 despair
        self.assertEqual(solver.amplification(0, 5), 0.0)   # a wipe: would be -10
        self.assertEqual(solver.amplification(1, 4), 0.0)   # would be -3
        self.assertEqual(solver.amplification(1, 3), 0.0)   # would be -1, still 0
        self.assertEqual(solver.amplification(1, 2), 1.0)   # 5 - 4, above the floor
        for g in range(6):
            for d in range(6):
                self.assertGreaterEqual(solver.amplification(g, d), 0.0)

    def test_the_floor_makes_the_solver_accept_more_wipes(self):
        # a wipe costs 0 instead of -2 * slots, so dodging one is worth less
        floored = Inheritance(config_for_level(8)).analyse()
        self.assertGreater(floored.p_dead_end, 0.0419)  # was 4.19% unfloored

    def test_weighted_gives_credit_for_every_slot(self):
        # unlike the old clamped scoring, a 5th glory success is still worth 5
        solver = Inheritance(config_for_level(1))
        self.assertEqual(solver.amplification(5, 2) - solver.amplification(4, 2), 5)
        self.assertEqual(solver.amplification(4, 1) - solver.amplification(4, 2), 2)

    def test_targets_are_all_or_nothing(self):
        solver = Inheritance(config_for_level(1, strategy=target(4, 2)))
        # inside the box scores 1 whatever the margin, outside scores 0
        self.assertEqual(solver.objective(4, 2), 1.0)
        self.assertEqual(solver.objective(5, 0), 1.0)
        self.assertEqual(solver.objective(3, 2), 0.0)
        self.assertEqual(solver.objective(4, 3), 0.0)
        # no partial credit: 3 glory is worth the same as 0
        self.assertEqual(solver.objective(3, 2), solver.objective(0, 5))

    def test_weighted_strategies_have_no_target(self):
        solver = Inheritance(config_for_level(1))
        self.assertFalse(solver.hits_target(5, 0))
        self.assertAlmostEqual(solver.analyse().p_target, 0.0)

    def test_targets_clamp_to_the_bar_size(self):
        cfg = config_for_level(1, strategy=target(999, 99))  # 5 slots
        self.assertEqual(cfg.want_glory, 5)
        self.assertEqual(cfg.allow_despair, 5)

    def test_hits_target(self):
        solver = Inheritance(config_for_level(1, strategy=target(4, 2)))
        self.assertTrue(solver.hits_target(4, 2))
        self.assertTrue(solver.hits_target(5, 0))
        self.assertFalse(solver.hits_target(3, 2))
        self.assertFalse(solver.hits_target(4, 3))

    def test_p_target_matches_the_distribution(self):
        for strat in (target(4, 2), target(5, 1)):
            solver = Inheritance(config_for_level(4, strategy=strat))
            a = solver.analyse()
            manual = sum(p for (g, d), p in a.dist.items() if solver.hits_target(g, d))
            self.assertAlmostEqual(a.p_target, manual, places=12, msg=strat.name)

    def test_a_target_beats_the_weighted_policy_on_its_own_box(self):
        # chasing the box must maximise the box, even though it gives up
        # amplification to do it
        box = Inheritance(config_for_level(1, strategy=target(4, 2))).analyse()
        plain = Inheritance(config_for_level(1)).analyse()
        rate = sum(p for (g, d), p in plain.dist.items() if g >= 4 and d <= 2)
        self.assertGreaterEqual(box.p_target + 1e-12, rate)
        self.assertLessEqual(box.e_amplification, plain.e_amplification + 1e-12)

    def test_an_incomplete_attempt_records_as_zero_and_zero(self):
        # nothing is inherited, so neither bar scores; it amplifies to the floor
        solver = Inheritance(config_for_level(1))
        a = solver.analyse()
        self.assertEqual(solver.amplification(0, 0), solver.min_amplification())
        self.assertGreaterEqual(a.dist[(0, 0)], a.p_dead_end - 1e-12)
        # and the simulator agrees
        import random
        rng = random.Random(1)
        for _ in range(4000):
            res = solver.attempt(rng)
            if res["dead_end"]:
                self.assertEqual((res["glory_success"], res["despair_success"]), (0, 0))
                break

    def test_an_incomplete_attempt_never_hits_even_a_lenient_target(self):
        # target(0, 0) is satisfied by the (0, 0) cell on paper, but an attempt
        # that never finished must not be counted as having hit it
        solver = Inheritance(config_for_level(1, strategy=target(0, 0)))
        a = solver.analyse()
        self.assertGreater(a.p_dead_end, 0.0)
        by_cell = sum(p for (g, d), p in a.dist.items() if solver.hits_target(g, d))
        self.assertAlmostEqual(a.p_target, by_cell - a.p_dead_end, places=12)

    def test_recording_a_wipe_as_zero_zero_leaves_the_policy_alone(self):
        # amp(0, 0) and amp(0, slots) both floor to 0, so the solver cannot tell
        # them apart - glory and amplification must be untouched
        a = Inheritance(config_for_level(7)).analyse()
        self.assertAlmostEqual(a.e_glory, 4.2309, places=3)
        self.assertAlmostEqual(a.e_amplification, 16.8113, places=3)

    def test_bad_strategy_rejected(self):
        with self.assertRaises(ValueError):
            Strategy(kind="whatever")
        with self.assertRaises(ValueError):
            target(-1, 2)


class TestPercentFormatting(unittest.TestCase):
    def test_three_digits_showing(self):
        from main import pct3
        cases = {
            1.0: "100%",
            0.98304: "98.3%",
            0.0936: "9.36%",
            0.000658: "0.07%",
            0.5: "50.0%",
            0.0999: "9.99%",
            0.01: "1.00%",
        }
        for value, expected in cases.items():
            self.assertEqual(pct3(value), expected, f"{value}")

    def test_anything_under_a_hundredth_rounds_down_to_zero(self):
        from main import pct3
        self.assertEqual(pct3(0.0), "0%")
        self.assertEqual(pct3(0.00004), "0%")     # too small to show, reads 0%
        self.assertEqual(pct3(0.00001), "0%")
        self.assertEqual(pct3(0.0001), "0.01%")   # the smallest value that shows


class TestTableColours(unittest.TestCase):
    def test_axis_ramps_run_white_to_their_colour(self):
        from main import DESPAIR_RED, GLORY_GOLD, ramp_colour
        self.assertEqual(ramp_colour(0.0, GLORY_GOLD)[0], "#ffffff")
        self.assertEqual(ramp_colour(1.0, GLORY_GOLD)[0], "#d4af37")
        self.assertEqual(ramp_colour(0.0, DESPAIR_RED)[0], "#ffffff")
        self.assertEqual(ramp_colour(1.0, DESPAIR_RED)[0], "#8b0000")

    def test_table_ink_is_always_black(self):
        from main import DESPAIR_RED, GLORY_GOLD, heat_colour, ramp_colour
        # one ink across the whole grid, at both ends of every ramp
        for f in (0.0, 0.25, 0.5, 0.75, 1.0):
            self.assertEqual(ramp_colour(f, DESPAIR_RED)[1], "#000000")
            self.assertEqual(ramp_colour(f, GLORY_GOLD)[1], "#000000")
            self.assertEqual(heat_colour(f)[1], "#000000")

    def test_heat_scale_runs_light_red_to_green(self):
        from main import heat_colour
        self.assertEqual(heat_colour(0.0)[0], "#f7beb9")   # light red, not a block
        self.assertEqual(heat_colour(0.5)[0], "#ffffbf")
        self.assertEqual(heat_colour(1.0)[0], "#1a9850")
        # and it is clamped outside 0..1
        self.assertEqual(heat_colour(-1.0)[0], "#f7beb9")
        self.assertEqual(heat_colour(2.0)[0], "#1a9850")


class TestEconomy(unittest.TestCase):
    def test_summon_distribution(self):
        import economy
        cum = economy.summon_cdf()
        self.assertAlmostEqual(cum[-1], 1.0, places=12)
        pmf = [c - (cum[i - 1] if i else 0.0) for i, c in enumerate(cum)]
        self.assertAlmostEqual(sum(pmf), 1.0, places=12)
        mean = sum(k * q for k, q in enumerate(pmf))
        self.assertAlmostEqual(mean, economy.RELICS_PER_SUMMON / economy.RELIC_TYPES, places=12)

    def test_diamonds_per_attempt(self):
        import economy
        # 10 crit relics at 11/12 per summon, 5000 diamonds a summon
        self.assertAlmostEqual(economy.diamonds_per_attempt(), 10 / (11 / 12) * 5000, places=6)
        self.assertAlmostEqual(economy.diamonds_per_attempt(), 54545.4545, places=3)

    def test_cost_rises_with_the_mark(self):
        import economy
        rows, _a, _s = economy.run(level=4, sims=300, seed=11)
        for lo, hi in zip(rows, rows[1:]):
            self.assertLessEqual(lo["mean"], hi["mean"] + 1e-9,
                                 f"{lo['mark']} -> {hi['mark']}")
            self.assertLessEqual(lo["mean_attempts"], hi["mean_attempts"] + 1e-9)

    def test_spend_is_always_whole_summons(self):
        import economy
        rows, _a, _s = economy.run(level=4, sims=200, seed=3)
        for r in rows:
            self.assertEqual(r["median"] % economy.DIAMONDS_PER_SUMMON, 0)
            self.assertEqual(r["p90"] % economy.DIAMONDS_PER_SUMMON, 0)

    def test_attempts_match_the_geometric_expectation(self):
        # first reaching a mark is geometric in P(one attempt reaches it),
        # so mean attempts should track 1/p
        import economy
        rows, _a, _s = economy.run(level=4, sims=3000, seed=5)
        for r in rows:
            if r["p_attempt"] < 0.02:
                continue          # too rare to pin down at this sample size
            expected = 1.0 / r["p_attempt"]
            self.assertAlmostEqual(r["mean_attempts"], expected,
                                   delta=0.25 * expected + 0.5, msg=f"{r['mark']}%")

    def test_every_run_reaches_the_top_mark(self):
        import economy
        rows, _a, _s = economy.run(level=4, sims=200, seed=9)
        for r in rows:
            self.assertEqual(r["runs"], 200, f"{r['mark']}% was not reached by every run")


class TestInheritorLevelling(unittest.TestCase):
    def test_requirement_table_matches_the_sheet(self):
        import levels
        self.assertEqual(levels.REQUIREMENTS[2], ("relics", [0], 3, None))
        self.assertEqual(levels.REQUIREMENTS[7], ("each", 4, 2))
        self.assertEqual(levels.REQUIREMENTS[8], ("total", 59, 17))
        self.assertEqual(levels.REQUIREMENTS[9], ("relics", [6, 7, 8], 6, 2))
        self.assertNotIn(1, levels.REQUIREMENTS)      # level 1 is free
        self.assertEqual(len(levels.RELICS), 12)
        self.assertEqual(levels.RELICS[levels.CRIT], "Demon Eye of Weakness")

    def test_meets(self):
        import levels
        self.assertTrue(levels.meets((4, 2), 4, 2))
        self.assertTrue(levels.meets((5, 0), 4, 2))
        self.assertFalse(levels.meets((3, 2), 4, 2))
        self.assertFalse(levels.meets((4, 3), 4, 2))
        self.assertTrue(levels.meets((3, 9), 3, None))   # no despair bar

    def test_level_conditions(self):
        import levels
        good = [(5, 1)] * 12
        self.assertTrue(levels.level_satisfied(7, good))     # each 4/2
        self.assertTrue(levels.level_satisfied(8, good))     # 60 glory, 12 despair
        weak = [(4, 2)] * 12                                 # 48 glory, 24 despair
        self.assertTrue(levels.level_satisfied(7, weak))
        self.assertFalse(levels.level_satisfied(8, weak))    # despair over budget
        blank = [(0, 0)] * 12
        self.assertFalse(levels.level_satisfied(4, blank))
        self.assertTrue(levels.level_satisfied(1, blank))    # level 1 is free

    def test_untouched_relics_count_as_zero_zero(self):
        import levels
        states = [(0, 0)] * 12
        states[0] = (5, 1)
        states[1] = (5, 1)
        states[2] = (5, 1)
        states[3] = (5, 1)
        states[4] = (5, 1)
        # 25 glory, 5 despair from five relics; the other seven add nothing
        self.assertTrue(levels.level_satisfied(4, states))

    def test_accept_rule(self):
        import levels
        # a dominating roll is always taken
        self.assertTrue(levels._accept((3, 2), (4, 1), (9, 0)))
        # a roll that clears the profile is taken even if it adds despair
        self.assertTrue(levels._accept((0, 0), (4, 2), (4, 2)))
        # a worse roll is refused
        self.assertFalse(levels._accept((5, 1), (3, 3), (4, 2)))
        # a dead end lands as (0, 0) and can never be used
        self.assertFalse(levels._accept((4, 2), (0, 0), (4, 2)))

    def test_a_run_reaches_the_top_and_costs_rise(self):
        import random
        import levels
        rng = random.Random(4)
        reached, states = levels.simulate_run(levels.strategy_tuned, rng)
        self.assertEqual(max(reached), levels.MAX_LEVEL)
        self.assertEqual(reached[1][0], 0)                    # level 1 is free
        costs = [reached[lvl][0] for lvl in sorted(reached)]
        for lo, hi in zip(costs, costs[1:]):
            self.assertLessEqual(lo, hi)
        for c in costs:
            self.assertEqual(c % levels.DIAMONDS_PER_SUMMON, 0)
        self.assertTrue(levels.level_satisfied(levels.MAX_LEVEL, states))

    def test_every_relic_is_a_legal_choice(self):
        # the picker must be able to return any relic it is handed
        import levels
        states = [(0, 0)] * 12
        profiles = {i: (4, 2) for i in range(12)}
        inventory = [10] * 12
        for name, picker in levels.PICKERS.items():
            got = picker(list(range(12)), states, profiles, inventory)
            self.assertIn(got, range(12), name)

    def test_tuned_beats_the_naive_schedules(self):
        import random
        import levels

        def mean(strategy, runs=120, seed=77):
            rng = random.Random(seed)
            return sum(levels.simulate_run(strategy, rng)[0][9][0]
                       for _ in range(runs)) / runs

        tuned = mean(levels.strategy_tuned)
        self.assertLess(tuned, mean(levels.strategy_prep))
        self.assertLess(tuned, mean(levels.strategy_balanced))

    def test_banking_level_9_relics_beats_repair_alone(self):
        # the early decisions (lock the level-9 relics, accept anything that
        # closes a total) are worth ~15% on top of the level-8 repair
        import random
        import levels

        def mean(strategy, runs=300, seed=99):
            rng = random.Random(seed)
            return sum(levels.simulate_run(strategy, rng)[0][9][0]
                       for _ in range(runs)) / runs

        self.assertLess(mean(levels.strategy_tuned), mean(levels.strategy_repair))


class TestPlanKnobs(unittest.TestCase):
    def test_future_named(self):
        import levels
        self.assertEqual(levels.future_named(4), {3, 4, 5, 6, 7, 8})
        self.assertEqual(levels.future_named(5), {6, 7, 8})
        self.assertEqual(levels.future_named(9), set())

    def test_reserve_protects_only_future_named_relics(self):
        from levels import Plan
        everything = Plan(reserve=True)
        self.assertEqual(everything.protected(4), {3, 4, 5, 6, 7, 8})
        self.assertEqual(everything.protected(9), frozenset())   # its own level
        only_l9 = Plan(reserve=frozenset({6, 7, 8}))
        self.assertEqual(only_l9.protected(4), {6, 7, 8})
        self.assertEqual(Plan().protected(4), frozenset())

    def test_a_locked_relic_sits_out_shared_total_work(self):
        from levels import Plan
        states = [(0, 0)] * 12
        profiles = Plan(reserve=frozenset({6, 7, 8}))(4, states)   # level 4 is a total
        self.assertNotIn(6, profiles)
        self.assertNotIn(8, profiles)
        self.assertIn(9, profiles)

    def test_every_relic_still_counts_when_the_level_needs_every_relic(self):
        from levels import Plan
        profiles = Plan(reserve=True)(7, [(0, 0)] * 12)            # level 7 is "each"
        self.assertEqual(sorted(profiles), list(range(12)))

    def test_surgical_repair_stays_inside_its_pool(self):
        import levels
        states = [(4, 2)] * 12
        profiles = levels.surgical_total(states, 59, 17, pool=[0, 1, 2])
        self.assertEqual(sorted(profiles), [0, 1, 2])
        for i in profiles:
            self.assertEqual(profiles[i], (4, 1))     # over budget: shave despair

    def test_prework_only_touches_idle_unlocked_relics(self):
        from levels import Plan
        plan = Plan(reserve=frozenset({6, 7, 8}), prework=((5, (4, 2)),))
        profiles = plan(5, [(0, 0)] * 12)
        for i in (3, 4, 5):
            self.assertEqual(profiles[i], (5, 2))     # the level's own relics
        for i in (0, 1, 2, 9, 10, 11):
            self.assertEqual(profiles[i], (4, 2))     # idle stock put to work
        for i in (6, 7, 8):
            self.assertNotIn(i, profiles)             # still banked for level 9

    def test_chase_changes_the_aim_not_the_bar(self):
        from levels import Plan
        plan = Plan(chase=((7, 6, (6, 2)),))
        profiles = plan(7, [(0, 0)] * 12)
        self.assertEqual(profiles[6], (4, 2))                     # the bar to accept
        self.assertEqual(plan.aim(7, 6, profiles[6]), (6, 2))     # what it rolls for
        self.assertEqual(plan.aim(7, 5, profiles[5]), (4, 2))

    def test_every_plan_knob_reaches_the_top(self):
        import random
        import levels
        from levels import Plan
        plans = [
            Plan(reserve=True, quota=40),
            Plan(reserve=frozenset({6, 7, 8}), accept=frozenset({4, 8})),
            Plan(prework=((5, (4, 2)),), picker="likeliest"),
            Plan(chase=((7, 6, (6, 2)),), surgical=frozenset({8})),
        ]
        for plan in plans:
            reached, states = levels.simulate_run(plan, random.Random(3))
            self.assertEqual(max(reached), levels.MAX_LEVEL, plan)
            self.assertTrue(levels.level_satisfied(levels.MAX_LEVEL, states))


class TestScorePolicy(unittest.TestCase):
    def test_score_objective_is_unfloored_and_never_negative(self):
        from relic import score
        solver = Inheritance(config_for_level(1, strategy=score(1, 3)))   # 5 slots
        # a 5/2 and a 1/5 both floor to 0 under amplification; score keeps them apart
        self.assertEqual(solver.objective(5, 2), 5 + 3 * 3)
        self.assertEqual(solver.objective(1, 5), 1 + 3 * 0)
        self.assertGreater(solver.objective(5, 2), solver.objective(1, 5))
        for g in range(6):
            for d in range(6):
                self.assertGreaterEqual(solver.objective(g, d), 0)
        self.assertEqual(solver.cfg.strategy.name, "score (1 glory : 3 despair)")

    def test_policy_changes_the_roll_not_the_bar(self):
        import levels
        plan = levels.Plan(policy=((7, (1, 1)),))
        profiles = plan(7, [(0, 0)] * 12)
        self.assertEqual(profiles[0], (4, 2))           # the bar to keep is unchanged
        a = levels.score_sampler(6, 1, 1)
        b = levels.outcome_sampler(6, 4, 2)
        self.assertNotEqual(a, b)                        # but the roll is different

    def test_tuned_plays_the_building_levels_for_score(self):
        import levels
        policy = dict(levels.strategy_tuned.policy)
        self.assertEqual(sorted(policy), [4, 6, 7, 8])
        self.assertEqual(policy[8], (1, 1))

    def test_stop8_does_not_bank_for_level_9(self):
        import levels
        self.assertEqual(levels.strategy_stop8.protected(4), frozenset())
        self.assertEqual(levels.strategy_tuned.protected(4), {6, 7, 8})


class TestStartingStock(unittest.TestCase):
    def test_stock_on_hand_is_used_before_summoning(self):
        import levels
        # 20 of every relic: level 2 (one attempt on Giant's Right Hand) never
        # needs a summon unless two attempts in a row fail
        reached, _s = levels.simulate_run(levels.strategy_tuned, random.Random(1),
                                          max_level=2, start_stock=20)
        self.assertEqual(reached[2][0], 0)

    def test_more_stock_costs_less(self):
        import levels

        def mean(stock, runs=150):
            rng = random.Random(8)
            return sum(levels.simulate_run(levels.strategy_stop8, rng, max_level=8,
                                           start_stock=stock)[0][8][0]
                       for _ in range(runs)) / runs

        self.assertLess(mean(40), mean(0))

    def test_stock_can_be_given_per_type(self):
        import levels
        stock = [0] * 12
        stock[0] = 10
        reached, _s = levels.simulate_run(levels.strategy_tuned, random.Random(2),
                                          max_level=2, start_stock=stock)
        self.assertIn(2, reached)


class TestPlanDescriptions(unittest.TestCase):
    def test_every_level_is_described_from_the_plan(self):
        import levels
        plan = levels.strategy_tuned
        text = {lvl: levels.describe_step(plan, lvl) for lvl in range(1, 10)}
        self.assertIn("free", text[1])
        self.assertIn("Giant's Right Hand", text[2])
        self.assertIn("except Seal of the Legendary Archer", text[4])   # banked
        self.assertIn("repair", text[8])
        self.assertIn("max glory and min despair", text[7])            # the policy
        self.assertIn("banked during levels 4, 6, 8", text[9])
        self.assertIn("banked during level 8", levels.describe_step(levels.strategy_lookahead, 9))
        self.assertNotIn("except", levels.describe_step(levels.strategy_stop8, 4))

    def test_chart_labels_every_bar(self):
        import levels
        rows, _states = levels.run("tuned", runs=30, seed=1)
        svg = levels.chart_svg(rows)
        for r in rows[1:]:
            self.assertIn(f">{levels.human(r['mean'])}</text>", svg)


class TestSpeedups(unittest.TestCase):
    def test_cached_analysis_is_the_same_answer_and_survives_a_restart(self):
        import tempfile
        from pathlib import Path
        import relic
        cfg = config_for_level(3, strategy=target(3, 2))
        saved_dir, saved_mem = relic.CACHE_DIR, dict(relic._SOLVED)
        try:
            with tempfile.TemporaryDirectory() as tmp:
                relic.CACHE_DIR = Path(tmp)
                relic._SOLVED.clear()
                first = relic.cached_analysis(cfg)
                self.assertEqual(first.dist, Inheritance(cfg).analyse().dist)
                self.assertTrue(any(Path(tmp).rglob("*.pkl")))
                relic._SOLVED.clear()          # a new process: memory gone, disk kept
                again = relic.cached_analysis(cfg)
                self.assertIsNot(again, first)
                self.assertEqual(again.dist, first.dist)
        finally:
            relic.CACHE_DIR = saved_dir
            relic._SOLVED.clear()
            relic._SOLVED.update(saved_mem)

    def test_parallel_runs_match_sequential_runs_exactly(self):
        import levels
        runs = levels.PARALLEL_MIN_RUNS
        one = levels.simulate_many(levels.strategy_stop8, runs, seed=4, max_level=6, workers=1)
        many = levels.simulate_many(levels.strategy_stop8, runs, seed=4, max_level=6, workers=3)
        self.assertEqual([r for r, _s in one], [r for r, _s in many])

    def test_economy_record_jumping_is_exact(self):
        # the fast loop skips straight from one new best to the next.  Every
        # mark's attempts must still be Geometric(P(amp >= mark)) - mean 1/p -
        # and cost ~ attempts x diamonds per attempt.  The top mark at level 4
        # is a 0.3% shot, the hardest case for the jump to get right.
        import economy
        rows, analysis, solver = economy.run(level=4, sims=20000, seed=5)
        for row in (rows[-1], rows[len(rows) // 2]):
            expect = 1 / row["p_attempt"]
            self.assertAlmostEqual(row["mean_attempts"] / expect, 1.0, delta=0.03)
            self.assertAlmostEqual(row["mean"] / (expect * economy.diamonds_per_attempt()),
                                   1.0, delta=0.04)


class TestPity(unittest.TestCase):
    def test_the_meter_sizes_and_gains(self):
        import levels
        self.assertEqual([levels.pity_needed(g) for g in (2, 3, 9)], [500, 1000, 4000])
        gains = {lvl: levels.pity_gain(lvl) for lvl in range(1, 20)}
        self.assertEqual({gains[1], gains[2], gains[3]}, {100})
        self.assertEqual({gains[4], gains[7]}, {120})
        self.assertEqual({gains[8], gains[11]}, {140})
        self.assertEqual(gains[19], 180)

    def test_pity_levels_up_without_the_quest(self):
        import levels

        # a quest nothing can clear: every relic asked for 99 glory
        impossible = levels.Plan(base=(4, 2), filler=(4, 2))
        saved = dict(levels.REQUIREMENTS)
        try:
            levels.REQUIREMENTS[2] = ("relics", [0], 99, 0)
            reached, _s = levels.simulate_run(impossible, random.Random(3), max_level=2)
        finally:
            levels.REQUIREMENTS.clear()
            levels.REQUIREMENTS.update(saved)
        diamonds, attempts, _amp, by_pity, _atk = reached[2]
        self.assertTrue(by_pity)
        self.assertEqual(attempts, 5)              # 500 pity at +100 an attempt

    def test_the_meter_resets_on_every_level_up(self):
        import levels
        results = levels.simulate_many(levels.strategy_greedy, 300, seed=2, max_level=4)
        for reached, _s in results:
            for goal in (2, 3, 4):
                spent = reached[goal][1] - reached[goal - 1][1]
                # never more attempts on one level than a fresh meter allows
                cap = -(-levels.pity_needed(goal) // levels.pity_gain(goal - 1))
                self.assertLessEqual(spent, cap)

    def test_switching_pity_off_means_no_level_is_reached_by_it(self):
        import levels
        for reached, _s in levels.simulate_many(levels.strategy_greedy, 50, seed=3,
                                                pity=False):
            self.assertFalse(any(v[3] for v in reached.values()))

    def test_filler_never_sets_the_quest_back(self):
        import levels
        # a named relic already clearing its bar keeps clearing it
        req = levels.REQUIREMENTS[5]
        states = [(0, 0)] * 12
        states[3] = (5, 2)
        self.assertFalse(levels._filler_keeps(states, 3, (6, 3), (4, 2), 5, req))
        # on a total level, a roll that clears the filler bar but widens the
        # board's gap is refused: 59 glory is exactly enough, 5/3 -> 4/2 drops it
        req = levels.REQUIREMENTS[8]
        states = [(5, 3)] + [(5, 1)] * 10 + [(4, 1)]
        self.assertTrue(levels._accept((5, 3), (4, 2), (4, 2)))
        self.assertFalse(levels._filler_keeps(states, 0, (4, 2), (4, 2), 8, req))

    def test_filler_makes_levelling_cheaper(self):
        import levels
        plain = levels.total_cost(levels.build_plan({}), 400, 7)
        filled = levels.total_cost(levels.build_plan(
            {lvl: levels.StepChoice(filler=(4, 2)) for lvl in (2, 3, 5, 9)}), 400, 7)
        self.assertLess(filled, plain)


class TestCharts(unittest.TestCase):
    def test_both_charts_label_every_bar_and_plot_the_damage_line(self):
        import levels
        rows, _states = levels.run("greedy", runs=40, seed=1)
        total = levels.chart_svg(rows)
        steps = levels.marginal_chart_svg(rows)
        for r, prev in zip(rows[1:], rows):
            self.assertIn(f">{levels.human(r['mean'])}</text>", total)
            self.assertIn(f">{levels.human(r['mean'] - prev['mean'])}</text>", steps)
        for svg in (total, steps):
            self.assertEqual(svg.count("<circle"), len(rows) - 1)   # one dot per level
            self.assertIn(">log dmg</text>", svg)
            self.assertIn(">+0%</text>", svg)                       # log(1) at the base

    def test_log_ticks_step_by_ten_and_cover_the_line(self):
        import levels
        self.assertEqual(levels._log_ticks(81.6), [0, 10, 20, 30, 40, 50, 60, 70, 80, 90])
        self.assertEqual(levels._log_ticks(17.5), [0, 10, 20])
        self.assertEqual(levels._log_ticks(20), [0, 10, 20])
        self.assertEqual(levels._log_ticks(3), [0, 10])

    def test_log_ticks_bunch_up_as_they_rise(self):
        import levels
        rows, _states = levels.run("greedy", runs=40, seed=1)
        svg = levels.chart_svg(rows)
        import re
        ys = [float(y) for y in re.findall(r'lv-muted" x="[0-9.]+" y="([0-9.]+)">\+', svg)]
        gaps = [a - b for a, b in zip(ys, ys[1:])]
        self.assertTrue(all(g > 0 for g in gaps))
        self.assertGreater(gaps[0], gaps[-1])       # +0->+10 wider than the top step


class TestDamage(unittest.TestCase):
    def test_level_multiplier_reaches_50_percent_at_level_9(self):
        import levels
        steps = [levels.level_multiplier(l) for l in range(1, 10)]
        self.assertEqual(steps[0], 0)
        for a, b in zip(steps[:6], steps[1:7]):          # levels 2-7: +5% each
            self.assertAlmostEqual(b - a, 0.05)
        self.assertAlmostEqual(steps[7] - steps[6], 0.10)  # level 8
        self.assertAlmostEqual(steps[8], 0.50)             # level 9

    def test_damage_formula(self):
        import levels
        self.assertEqual(levels.damage(1, 0, 0), 1.0)
        # (1 + 50%) x (1 + 4/5 x 25%) x (1 + 22/209 x 10%)
        self.assertAlmostEqual(levels.damage(9, 25, 10), 1.5 * 1.2 * (1 + 2.2 / 209))

    def test_every_relic_has_a_two_word_name(self):
        import levels
        self.assertEqual(len(levels.RELIC_SHORT), len(levels.RELICS))
        self.assertTrue(all(len(n.split()) == 2 for n in levels.RELIC_SHORT))
        self.assertEqual(levels.RELICS[levels.ATK], "Giant's Right Hand")
        self.assertEqual(levels.requirement_text(9, short=True),
                         "Archer Seal, Night Veil, Eternal Spark: 6+ / 2-")

    def test_table_has_the_damage_columns(self):
        import levels
        rows, _s = levels.run("greedy", runs=40, seed=1)
        self.assertEqual(rows[0]["damage"], 1.0)
        self.assertGreater(rows[-1]["damage"], 1 + levels.level_multiplier(9) - 1e-9)
        self.assertGreaterEqual(rows[-1]["atk_amp"], 0.0)


class TestPrefer(unittest.TestCase):
    def test_hard_preference_keeps_quest_work_on_the_preferred_relics(self):
        import levels
        from dataclasses import replace
        L5 = frozenset({3, 4, 5})
        plan = replace(levels.strategy_lookahead, prefer=((4, L5, "hard"),))
        self.assertEqual(plan.prefer_for(4), (L5, "hard"))
        self.assertIsNone(plan.prefer_for(6))
        # every relic the level-4 work touches is a level-5 relic, until they
        # are all done - so the level-5 relics come out of level 4 improved
        seen = 0
        for _ in range(20):
            reached, states = levels.simulate_run(plan, random.Random(_), max_level=4)
            seen += sum(1 for i in L5 if states[i] != (0, 0))
        self.assertGreater(seen, 20)

    def test_soft_preference_changes_the_pick_not_the_rules(self):
        import levels
        from dataclasses import replace
        plain = levels.total_cost(levels.strategy_greedy, 300, 5, max_level=6)
        soft = levels.total_cost(replace(levels.strategy_greedy,
                                         prefer=((4, frozenset({3, 4, 5}), "soft"),)),
                                 300, 5, max_level=6)
        self.assertAlmostEqual(soft / plain, 1.0, delta=0.1)


class TestGreedy(unittest.TestCase):
    def test_named_relic_levels_only_decide_their_filler(self):
        import levels
        for lvl in (2, 3, 5, 9):
            choices = levels.step_choices(lvl)
            self.assertEqual({c.filler for c in choices}, set(levels.FILLERS))
            self.assertEqual({c.profile for c in choices}, {None})
        self.assertGreater(len(levels.step_choices(4)), 10)

    def test_saved_plans_are_what_the_step_choices_build(self):
        import levels
        C = levels.StepChoice
        greedy = levels.build_plan({
            2: C(filler=(4, 2)), 3: C(filler=(4, 2)),
            4: C((5, 1), True, True), 5: C(filler=(4, 2)),
            6: C((5, 1), True, True, filler=(4, 2)), 7: C((4, 2), False, True, filler=(4, 2)),
            8: C("repair", True, True, filler=(4, 1)), 9: C(filler=(4, 2))})
        for field in ("bases", "surgical", "accept", "policy", "filler"):
            self.assertEqual(getattr(greedy, field), getattr(levels.strategy_greedy, field))
        # look-ahead banks the level-9 relics during level 8 only
        self.assertEqual(levels.strategy_lookahead.protected(8), {6, 7, 8})
        self.assertEqual(levels.strategy_lookahead.protected(6), frozenset())
        self.assertEqual(levels.strategy_greedy.protected(8), frozenset())

    def test_a_choice_only_touches_its_own_level(self):
        import levels
        base = levels.Plan(base=(4, 2), accept=frozenset(), reserve=())
        plan = levels.with_choice(base, 6, levels.StepChoice((5, 1), True, True, True))
        self.assertEqual(plan.base_for(6), (5, 1))
        self.assertEqual(plan.base_for(4), (4, 2))
        self.assertEqual(plan.accept, frozenset({6}))
        self.assertEqual(dict(plan.policy), {6: (1, 1)})
        self.assertEqual(plan.protected(6), frozenset({6, 7, 8}))
        self.assertEqual(plan.protected(4), frozenset())
        # choosing again replaces rather than stacks
        plan = levels.with_choice(plan, 6, levels.StepChoice("repair"))
        self.assertIn(6, plan.surgical)
        self.assertEqual(plan.accept, frozenset())
        self.assertEqual(plan.policy, ())
        self.assertEqual(plan.protected(6), frozenset())

    def test_greedy_search_reports_every_level(self):
        import levels
        plan, report = levels.greedy_search(runs=60, final_runs=120, max_level=4)
        self.assertEqual([r["level"] for r in report], [2, 3, 4])
        self.assertIsNotNone(report[-1]["runner_up"])
        self.assertIsNotNone(report[-1]["bank_cost"])
        self.assertIsInstance(plan, levels.Plan)


class TestDegenerateCases(unittest.TestCase):
    def test_training_is_a_tier_lever_not_just_a_spirit_power_refill(self):
        # hand-built config where spirit power alone already covers both bars,
        # so training is never needed for fuel.  It still gets used: a failed
        # training costs no slot and pushes the rate back UP the ladder, which
        # makes it the right move at a bad tier.
        cfg = config_for_level(1)
        cfg = type(cfg)(**{**cfg.__dict__, "slots": 2, "max_spirit": 7})
        solver = Inheritance(cfg)
        a = solver.analyse()
        self.assertEqual(a.p_dead_end, 0.0)  # no fuel risk at all
        self.assertGreater(a.action_counts[TRAIN], 0.0)

        # one glory slot to fill, despair done, at the worst tier: stall
        state = (1, 1, 2, 0, 1, 5, WORST_TIER)
        self.assertEqual(solver.best_action(state), TRAIN)
        # and at the best tier: take it
        self.assertEqual(solver.best_action((1, 1, 2, 0, 1, 5, BEST_TIER)), GLORY)

    def test_wipe_chance_is_never_zero_when_training_is_required(self):
        for level in (1, 4, 8):
            a = Inheritance(config_for_level(level, safety_first=True)).analyse()
            self.assertGreater(a.p_dead_end, 0.0, f"level {level}")


if __name__ == "__main__":
    unittest.main(verbosity=2)
