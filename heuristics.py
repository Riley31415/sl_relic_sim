#!/usr/bin/env python3
"""Hand-written, human-playable policies, scored against the exact optimum.

The solver's policy is a 45,000 entry lookup table - useless at the keyboard.
These are rules a player can hold in their head, evaluated on exactly the same
footing (the same exhaustive probability sweep, not sampling), so the gap to
optimal play is an exact number rather than an estimate.

    python heuristics.py            # level 7, the tuning baseline
    python heuristics.py --level 1
"""

from __future__ import annotations

import argparse

from relic import (
    DESPAIR,
    GLORY,
    TIERS,
    TRAIN,
    Inheritance,
    config_for_level,
    weighted,
)


def _pick(legal, *preferences):
    """First preference that is actually legal."""
    for action in preferences:
        if action in legal:
            return action
    return legal[0]


def always_glory_first(cfg):
    """Baseline: run the glory bar out, then despair, mental training only when dry."""

    def policy(state, legal):
        _gf, _gs, _df, _ds, _ms, sp, _t = state
        if sp == 0:
            return TRAIN
        return _pick(legal, GLORY, DESPAIR, TRAIN)

    return policy


def tier_split(cfg, split=2):
    """Attempt glory on the good rates, despair on the bad ones, train when dry."""

    def policy(state, legal):
        _gf, _gs, _df, _ds, _ms, sp, t = state
        if sp == 0:
            return TRAIN
        if t < split:
            return _pick(legal, GLORY, DESPAIR, TRAIN)
        if t > split:
            return _pick(legal, DESPAIR, GLORY, TRAIN)
        return _pick(legal, TRAIN, GLORY, DESPAIR)

    return policy


def tier_split_with_fuel(cfg, split=2, reserve=-1):
    """Tier split, plus mental training early enough that the fuel never runs out.

    The fuel rule only fires while missing at least 2 spirit power, so the full +2
    is never wasted against the cap, and only while the bars still need more fuel
    than is on hand: waiting until the tank is empty is what causes most wipes.
    """

    def policy(state, legal):
        gf, _gs, df, _ds, _ms, sp, t = state
        if sp == 0:
            return TRAIN
        fills_left = (cfg.slots - gf) + (cfg.slots - df)
        if TRAIN in legal and sp <= cfg.max_spirit - 2 and fills_left - sp > reserve:
            return TRAIN
        if t < split:
            return _pick(legal, GLORY, DESPAIR, TRAIN)
        if t > split:
            return _pick(legal, DESPAIR, GLORY, TRAIN)
        return _pick(legal, TRAIN, GLORY, DESPAIR)

    return policy


def tier_split_fuel_and_stall(cfg, glory_max=1, despair_min=2, reserve=0,
                              glory_stall=2, despair_stall=2, fuel_max_tier=2):
    """The full rule of thumb, tuned at inheritor level 7.

    Level 7 is the tuning baseline because level 1 cannot exercise the rule:
    there you start with 8 spirit power against 10 slots, so one mental training
    always closes the gap and the fuel clause never fires at all.  Level 7
    starts 3 short, so every clause is live.

    Four ideas stacked up:

    1. fuel - take mental training when missing at least 2 spirit power (so the
       +2 is not wasted against the cap), the total glory + despair slots still
       remaining exceed spirit power, and the rate is 50% or better.  Mental
       training is a roll on the same ladder: at 20% it fails four times out of
       five, so buying fuel at a bad rate just burns mental strength.
    2. tier split - with both bars open, attempt glory on the good rates and
       attempt despair on the rest.  There is no middle mental training band;
       every rate belongs to a bar.
    3. stall - with only one bar left, attempt that bar on the rates that suit
       it (glory 80/65/50, despair 50/35/20) and take mental training on the
       rest.  Mental strength has no other job by then, so spend it rather than
       commit a slot at a rate that suits the bar you already finished.  A
       failed mental training costs no slot and pushes the rate back up.
    4. never strand yourself - out of spirit power, take mental training.

    `fuel_max_tier` is the tier index the fuel clause is gated on; -1 disables
    the clause and len(TIERS)-1 lets it fire at any rate (the old level-1
    behaviour, which is actively harmful from level 4 up).
    """

    def policy(state, legal):
        gf, _gs, df, _ds, _ms, sp, t = state
        if sp == 0:
            return TRAIN
        glory_left = cfg.slots - gf
        despair_left = cfg.slots - df
        fills_left = glory_left + despair_left
        if (TRAIN in legal and sp <= cfg.max_spirit - 2
                and fills_left - sp > reserve and t <= fuel_max_tier):
            return TRAIN
        if glory_left and despair_left:
            if t <= glory_max:
                return GLORY
            if t >= despair_min:
                return DESPAIR
            return _pick(legal, TRAIN, GLORY, DESPAIR)
        if glory_left:
            if t > glory_stall and TRAIN in legal:
                return TRAIN
            return GLORY
        if t < despair_stall and TRAIN in legal:
            return TRAIN
        return DESPAIR

    return policy


def level1_tuned_rule(cfg):
    """The previous rule, tuned at level 1, kept for comparison.

    Its fuel clause is rate-blind, which is inert at level 1 and costly above it.
    """
    return tier_split_fuel_and_stall(
        cfg, glory_max=1, despair_min=3, reserve=2,
        glory_stall=1, despair_stall=2, fuel_max_tier=len(TIERS) - 1,
    )


POLICIES = {
    "glory bar first (baseline)": always_glory_first,
    "tier split": tier_split,
    "tier split + fuel": tier_split_with_fuel,
    "old rule (tuned @ level 1)": level1_tuned_rule,
    "FULL RULE (tuned @ level 7)": tier_split_fuel_and_stall,
}


def main() -> int:
    parser = argparse.ArgumentParser(description="score hand-written policies")
    parser.add_argument("--level", type=int, default=7,
                        help="the rule is tuned at level 7 (default)")
    parser.add_argument("--amp-glory", type=float, default=5.0)
    parser.add_argument("--amp-despair", type=float, default=2.0)
    args = parser.parse_args()

    cfg = config_for_level(
        args.level, strategy=weighted(args.amp_glory, args.amp_despair)
    )
    solver = Inheritance(cfg)
    best = solver.analyse()

    print(f"Inheritor level {cfg.level}: hand-written rules vs the exact optimum")
    print(f"  {'policy':>27} | {'E[glory]':>9} {'E[desp succ]':>13} "
          f"{'E[amp]':>9} {'P(wipe)':>9} | {'vs optimal':>10}")
    rows = [("OPTIMAL (lookup table)", best)]
    for name, factory in POLICIES.items():
        rows.append((name, solver.analyse(factory(cfg))))
    for name, a in rows:
        gap = a.e_amplification - best.e_amplification
        gap_s = "-" if not gap else f"{gap:+.3f}%"
        print(f"  {name:>27} | {a.e_glory:>9.4f} {a.e_despair_success:>13.4f} "
              f"{a.e_amplification:>8.3f}% {100 * a.p_dead_end:>8.4f}% | {gap_s:>10}")

    print()
    print("  where the fuel clause is gated (the rest of the rule held fixed)")
    print(f"  {'mental training at':>19} | {'E[amp]':>9} {'P(wipe)':>9}")
    for f in range(-1, len(TIERS)):
        a = solver.analyse(tier_split_fuel_and_stall(cfg, fuel_max_tier=f))
        name = "never" if f == -1 else ("any rate" if f == len(TIERS) - 1
                                        else f"{TIERS[f]:.0%} or better")
        star = "  <== default" if f == 2 else ""
        print(f"  {name:>19} | {a.e_amplification:>8.3f}% {100 * a.p_dead_end:>8.4f}%{star}")

    print()
    print("  where the glory/despair split sits (despair takes everything below)")
    print(f"  {'attempt glory up to':>19} | {'E[amp]':>9} {'P(wipe)':>9}")
    for split in range(len(TIERS)):
        a = solver.analyse(tier_split_fuel_and_stall(cfg, glory_max=split,
                                                     despair_min=split + 1))
        star = "  <== default" if split == 1 else ""
        print(f"  {TIERS[split]:>18.0%} | {a.e_amplification:>8.3f}% {100 * a.p_dead_end:>8.4f}%{star}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
