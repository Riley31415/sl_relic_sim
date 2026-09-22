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
