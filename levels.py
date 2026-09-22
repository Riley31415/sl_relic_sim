#!/usr/bin/env python3
"""Monte Carlo: what it costs in diamonds to raise the inheritor level.

Levelling the inheritor needs the twelve relics to hit stated glory/despair
conditions, and a higher inheritor level is what gives the relics more memory
slots and spirit power.  So the two feed each other, and the question is which
order to do the work in.

Everything is simulated: the multinomial summon stream (you do not choose which
relic type drops, so the per-type counts drift apart), the inheritance attempt
itself, and the accept-or-keep decision on each result.

    python levels.py --runs 2000
    python levels.py --compare          # score every strategy
    python levels.py --strategy prep --chart > levels.svg
    python levels.py --greedy           # cheapest level-up at every step
    python levels.py --lookahead        # cheapest route to level 9 overall
    python levels.py --markdown LEVELS.md   # regenerate the whole document
"""

from __future__ import annotations

import argparse
import bisect
import math
import os
import random
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass, replace

from relic import cached_analysis, config_for_level, score, target

# ------------------------------------------------------------------ economy
DIAMONDS_PER_SUMMON = 5000
RELICS_PER_SUMMON = 11
RELICS_PER_ATTEMPT = 10

RELICS = [
    "Giant's Right Hand",
    "Demon Eye of Weakness",      # the crit relic
    "Oath of Immortality",
    "Sacred Tree of Rebirth",
    "Ring of Lightning",
    "Golden Star",
    "Seal of the Legendary Archer",
    "Veil of the Night",
    "Spark of Eternity",
    "Mermaid's Tear",
    "Eye of the Sky",
    "Crown of the Great Mountain",
]
N_RELICS = len(RELICS)
# two-word names for tables, in the same order
RELIC_SHORT = [
    "Giant Hand", "Demon Eye", "Immortal Oath", "Sacred Tree", "Lightning Ring",
    "Golden Star", "Archer Seal", "Night Veil", "Eternal Spark", "Mermaid Tear",
    "Sky Eye", "Mountain Crown",
]
ATK = 0                        # Giant's Right Hand, the attack relic
CRIT = 1                       # Demon Eye of Weakness
MAX_LEVEL = 9                  # as far as the requirement table is known

# What the inheritor needs in order to BE at each level.  Level 1 is free.
#   ("relics", [indices], glory, despair)  each listed relic needs >=glory, <=despair
#   ("each",   glory, despair)             every one of the twelve does
#   ("total",  glory, despair)             summed over all twelve
REQUIREMENTS = {
    2: ("relics", [0], 3, None),
    3: ("relics", [1, 2], 3, 2),
    4: ("total", 25, 21),
    5: ("relics", [3, 4, 5], 5, 2),
    6: ("total", 44, 18),
    7: ("each", 4, 2),
    8: ("total", 59, 17),
    9: ("relics", [6, 7, 8], 6, 2),
}


# Pity: every attempt, on any relic, fills a meter; a full meter levels the
# inheritor up without the quest.  The meter empties on every level-up, however
# it was reached.
PITY_PER_LEVEL = 500           # level 2 needs 500, level 3 1000 ... level 9 4000


def pity_needed(goal: int) -> int:
    """Pity that levels the inheritor up to `goal` without its quest."""
    return PITY_PER_LEVEL * (goal - 1)


def pity_gain(level: int) -> int:
    """Pity one attempt adds at inheritor `level`: 100 at levels 1-3, 120 at
    4-7, 140 at 8-11, and +20 every four levels after (180 at 16-19)."""
    return 100 + 20 * (level // 4)


def meets(state, glory, despair) -> bool:
    """Does one relic's (glory, despair) clear a >=glory / <=despair bar?"""
    g, d = state
    return g >= glory and (despair is None or d <= despair)


def level_satisfied(level: int, states) -> bool:
    """Is the requirement for `level` met by the current twelve relics?"""
    req = REQUIREMENTS.get(level)
    if req is None:
        return True
    kind = req[0]
    if kind == "relics":
        _, which, glory, despair = req
        return all(meets(states[i], glory, despair) for i in which)
    if kind == "each":
        _, glory, despair = req
        return all(meets(s, glory, despair) for s in states)
    _, glory, despair = req
    return (sum(g for g, _d in states) >= glory
            and sum(d for _g, d in states) <= despair)


def requirement_text(level: int, short: bool = False) -> str:
    """The level's demand as `glory+ / despair-`, e.g. 4+ / 2-.  `short` uses
    the two-word relic names."""
    req = REQUIREMENTS.get(level)
    if req is None:
        return "-"
    if req[0] == "relics":
        _, which, glory, despair = req
        names = ", ".join((RELIC_SHORT if short else RELICS)[i] for i in which)
        bar = "any" if despair is None else f"{despair}-"
        return f"{names}: {glory}+ / {bar}"
    if req[0] == "each":
        return f"every relic: {req[1]}+ / {req[2]}-"
    return f"all twelve together: {req[1]}+ / {req[2]}- (totals)"


# --------------------------------------------------------------- the solver
_OUTCOMES: dict = {}


def _table(key, strategy, level: int):
    """(values, cumulative) for one attempt at `level`, solved once and cached."""
    hit = _OUTCOMES.get(key)
    if hit is not None:
        return hit
    analysis = cached_analysis(config_for_level(level, strategy=strategy))
    items = sorted(analysis.dist.items())
    values = [gd for gd, _p in items]
    cum, running = [], 0.0
    for _gd, prob in items:
        running += prob
        cum.append(running)
    cum[-1] = 1.0
    _OUTCOMES[key] = (values, cum)
    return values, cum


def score_sampler(level: int, w_glory: float, w_despair: float):
    """Cached (values, cumulative) for one attempt played for a weighted score.

    Instead of chasing a bar all-or-nothing, the attempt maximises
    w_glory * glory + w_despair * (despair slots left clean); whether the result
    is kept is still decided by the relic's profile.
    """
    return _table(("score", level, w_glory, w_despair), score(w_glory, w_despair), level)


def outcome_sampler(level: int, want_glory: int, allow_despair: int):
    """Cached (values, cumulative) for one attempt at `level` chasing a target.

    The attempt is played under the all-or-nothing policy for that target, which
    is the policy that maximises the chance of clearing it.  A dead end leaves
    the relic untouched, and lands in the (0, 0) cell, so it is already folded
    into the distribution.
    """
    return _table((level, want_glory, allow_despair), target(want_glory, allow_despair), level)


def amplification(state, w_glory: float = 5.0, w_despair: float = 2.0) -> float:
    g, d = state
    return max(0.0, w_glory * g - w_despair * d)


def level_multiplier(level: int) -> float:
    """The inheritor's own damage multiplier: +5% for each of levels 2-7, then
    +10% for each level from 8 (50% at level 9)."""
    return 0.05 * min(level - 1, 6) + 0.10 * max(0, level - 7)


def damage(level: int, crit_amp: float, atk_amp: float) -> float:
    """Damage relative to a level-1 inheritor with bare relics (1.0 = no gain).

    crit_amp and atk_amp are the two relics' amplification in percent.
    """
    return ((1 + level_multiplier(level))
            * (1 + 4 * crit_amp / 100 / 5)
            * (1 + 22 * atk_amp / 100 / 209))


# ------------------------------------------------------------- strategies
def future_named(level: int) -> set:
    """Relics that a LATER level names outright.

    Their stock is worth protecting: every one of them spent on shared work now
    is one that has to be summoned again when their own level comes round.
    """
    named = set()
    for lvl, req in REQUIREMENTS.items():
        if lvl > level and req[0] == "relics":
            named.update(req[1])
    return named


@dataclass(frozen=True)
class Plan:
    """Every decision a player makes, bundled so schedules can be compared.

    base      the (glory, despair) profile a relic is rolled toward when a level
              asks for "all twelve" rather than naming relics
    bases     per-level overrides of `base`, as ((level, (g, d)), ...)
    surgical  total levels planned as single-step repairs instead of `base`
    reserve   protect relics that a later level names by hand, so their stock
              is still banked when that level comes.  True protects every such
              relic; a frozenset protects only those listed
    quota     how protection works: None locks protected relics out of shared
              total work entirely; a number lets them join in, but only with
              stock above that many relics - the quota itself stays banked
    policy    ((level, (w_glory, w_despair)), ...) - play that level's attempts
              for a weighted score rather than all-or-nothing for the bar; the
              bar still decides whether a result is kept
    prework   ((level, (glory, despair)), ...) - while a level that names relics
              is waiting on their stock, put the idle stock of every other
              (unlocked) relic to work toward this profile
    chase     ((level, relic, (glory, despair)), ...) - aim a relic higher than
              the level asks while it is being rolled anyway; any result that
              clears the level still counts.  Ignored on a level in `policy`:
              an attempt played for score has no bar to aim at
    picker    which affordable relic gets the next attempt
    accept    how a result is judged against what the relic already has:
              "dominate"  take it if it is no worse on either bar, or it clears
                          the profile the old result failed
              "total"     on a total level, also take anything that brings the
                          board closer to the requirement
              frozenset   the same, but only on the total levels listed
    patient   summon for the relic we want rather than attempting what is ready
    filler    what to do when no relic the quest needs is affordable:
              None          summon (pity still builds from quest attempts)
              (g, d)        attempt the best-stocked spare relic instead, for
                            the pity, rolling it toward (g, d)
              ((level, (g, d) or None), ...)  the same, set per level
              Protected relics (see `reserve`) are never used as filler.
    prefer    ((level, relics, "soft" or "hard"), ...) - which relics a level's
              quest work goes to first.  soft: a preferred relic is used
              whenever one is affordable.  hard: while any preferred relic still
              needs work, the others wait (filler or a summon instead).
    """

    base: tuple = (4, 2)
    bases: tuple = ()
    surgical: frozenset = frozenset()
    reserve: object = False
    quota: object = None
    chase: tuple = ()
    prework: tuple = ()
    policy: tuple = ()
    picker: str = "stocked"
    accept: object = "dominate"
    patient: bool = False
    filler: object = None
    prefer: tuple = ()

    def base_for(self, level: int):
        return dict(self.bases).get(level, self.base)

    def protected(self, level: int) -> frozenset:
        """Relics whose stock this plan is banking while working on `level`.

        `reserve` may also be a tuple of (level, frozenset) pairs, to protect a
        different set at each level - e.g. only the next level's named relics.
        """
        if not self.reserve:
            return frozenset()
        named = frozenset(future_named(level))
        if self.reserve is True:
            return named
        if isinstance(self.reserve, tuple):
            return named & frozenset(dict(self.reserve).get(level, ()))
        return named & frozenset(self.reserve)

    def __call__(self, level: int, states):
        return plan_profiles(self, level, states)

    def prefer_for(self, level: int):
        """(relics, "soft" or "hard") preferred while working on `level`, or None."""
        for lvl, relics, how in self.prefer:
            if lvl == level:
                return frozenset(relics), how
        return None

    def filler_for(self, level: int):
        """The profile filler attempts roll toward while working on `level`."""
        if not self.filler or isinstance(self.filler[0], int):
            return self.filler or None
        return dict(self.filler).get(level)

    def aim(self, level: int, relic: int, profile):
        """What an attempt on this relic actually chases - usually its profile."""
        for lvl, r, target_profile in self.chase:
            if lvl == level and r == relic:
                return target_profile
        return profile


def plan_profiles(plan: Plan, level: int, states):
    """Assign relics a (glory, despair) profile to roll toward for `level`."""
    req = REQUIREMENTS.get(level)
    if req is None:
        return {}
    base = plan.base_for(level)
    if req[0] == "relics":
        _, which, glory, despair = req
        profiles = {i: (glory, despair) for i in which}
        extra = dict(plan.prework).get(level)
        if extra:
            locked = plan.protected(level) if plan.quota is None else frozenset()
            for i in range(N_RELICS):
                if i not in profiles and i not in locked:
                    profiles[i] = extra
        return profiles
    if req[0] == "each":
        # every relic has to clear this one, protected or not
        _, glory, despair = req
        return {i: (max(glory, base[0]), min(despair, base[1]))
                for i in range(N_RELICS)}
    pool = list(range(N_RELICS))
    if plan.quota is None:
        # a hard lock: protected relics sit the shared work out entirely
        locked = plan.protected(level)
        pool = [i for i in pool if i not in locked]
    if level in plan.surgical:
        return surgical_total(states, req[1], req[2], pool)
    return {i: base for i in pool}



def surgical_total(states, need_glory, need_despair, pool=None):
    """Plan the cheapest single-step fixes for a `total` requirement.

    Entering level 8 the board is typically only ~3 glory short and ~1 despair
    over, so asking every relic for a blanket 5/1 is wasted effort.  Each relic
    is instead asked for the one step that helps: shave a despair if the budget
    is blown, otherwise add a glory.  Those are looser bars than a full profile
    - 4/1 instead of 5/1 for a relic sitting at 4/2 - so each roll lands more
    often.
    """
    pool = list(range(len(states))) if pool is None else list(pool)
    total_g = sum(g for g, _d in states)
    total_d = sum(d for _g, d in states)
    short = max(0, need_glory - total_g)
    over = max(0, total_d - need_despair)

    if over > 0:
        # cut despair where it is cheapest: 2 -> 1 beats 1 -> 0 by a mile
        worst = max(states[i][1] for i in pool)
        profiles = {i: (states[i][0], states[i][1] - 1) for i in pool
                    if states[i][1] >= max(2, worst)}
        if profiles:
            return profiles
    if short > 0:
        return {i: (states[i][0] + 1, states[i][1]) for i in pool}
    return {i: states[i] for i in pool}


def _profile_for(level: int, states, base, surgical=frozenset()):
    """Kept for callers that only vary the standing profile."""
    return Plan(base=base, surgical=frozenset(surgical))(level, states)


# ------------------------------------------------------------- simulation
def _escalate(profiles, states, req):
    """Nothing left to roll but the level is still short - ask for more.

    Raises the glory bar on the planned relic furthest from pulling its weight,
    or tightens despair if that is what the total is failing on.
    """
    if req[0] != "total" or not profiles:
        return profiles
    _, need_glory, need_despair = req
    keys = list(profiles)
    if sum(d for _g, d in states) > need_despair:
        worst = max(keys, key=lambda i: (states[i][1], -states[i][0]))
        profiles[worst] = (max(profiles[worst][0], states[worst][0]),
                           max(0, states[worst][1] - 1))
    else:
        short = max(keys, key=lambda i: (-states[i][0], states[i][1]))
        profiles[short] = (states[short][0] + 1, profiles[short][1])
    return profiles


def gap_to(state, profile) -> int:
    """How far one relic still is from its profile, in slots that must change."""
    g, d = state
    want_g, allow_d = profile
    short = max(0, want_g - g)
    over = 0 if allow_d is None else max(0, d - allow_d)
    return short + over


_HIT: dict = {}


def hit_rate(level: int, want_glory: int, allow_despair) -> float:
    """Chance one attempt at `level` clears (want_glory, allow_despair)."""
    if allow_despair is None:
        allow_despair = config_for_level(level).slots
    key = (level, want_glory, allow_despair)
    if key not in _HIT:
        values, cum = outcome_sampler(level, want_glory, allow_despair)
        prev, total = 0.0, 0.0
        for gd, c in zip(values, cum):
            if meets(gd, want_glory, allow_despair):
                total += c - prev
            prev = c
        _HIT[key] = total
    return _HIT[key]


# Which relic to spend the next attempt on.  Every one of the twelve is always
# a legal choice, so this is a real decision and not a formality.
def pick_stocked(todo, states, profiles, inventory, level=None):
    """Whichever pending relic we are best stocked for."""
    return max(todo, key=lambda i: inventory[i])


def pick_closest(todo, states, profiles, inventory, level=None):
    """The pending relic nearest its profile - bank the easy wins first."""
    return min(todo, key=lambda i: (gap_to(states[i], profiles[i]), -inventory[i]))


def pick_neediest(todo, states, profiles, inventory, level=None):
    """The pending relic furthest from its profile - start the long job first."""
    return max(todo, key=lambda i: (gap_to(states[i], profiles[i]), inventory[i]))


def pick_untouched(todo, states, profiles, inventory, level=None):
    """Spread first: prefer a relic that has never been rolled."""
    return max(todo, key=lambda i: (states[i] == (0, 0), inventory[i]))


def pick_likeliest(todo, states, profiles, inventory, level=None):
    """The pending relic whose bar is easiest to clear right now."""
    if level is None:
        return pick_stocked(todo, states, profiles, inventory)
    return max(todo, key=lambda i: (round(hit_rate(level, *profiles[i]), 3),
                                    inventory[i]))


PICKERS = {
    "stocked": pick_stocked,
    "closest": pick_closest,
    "neediest": pick_neediest,
    "untouched": pick_untouched,
    "likeliest": pick_likeliest,
}


def _board_gap(states, req) -> int:
    """Slots a total requirement is still short by: missing glory + extra despair."""
    _, need_glory, need_despair = req
    return (max(0, need_glory - sum(g for g, _d in states))
            + max(0, sum(d for _g, d in states) - need_despair))


def simulate_run(strategy, rng, max_level: int = MAX_LEVEL, picker=None,
                 patient=None, trace=None, start_stock=0, pity: bool = True):
    """One player, from level 1 to `max_level`.  Returns the cost of each level.

    Each step is a genuine choice between two moves: summon another batch, or
    spend 10 banked relics of some type on an attempt.  `patient` picks which:

    eager (default)  attempt as soon as ANY relic we still need is affordable
    patient          decide which relic we want first, then summon until we can
                     afford that one, rather than settling for whatever is ready

    `strategy` is a Plan or any callable (level, states) -> profiles; a Plan's
    own picker/patient/accept settings are used unless overridden here.  Pass a
    dict as `trace` to get the relic stock on hand at each level-up.

    Every attempt also adds pity; a level is reached by its quest or by a full
    pity meter, whichever comes first (`pity=False` switches the rule off).
    Each entry of the result is (diamonds, attempts, crit amp, reached by pity,
    atk amp).
    """
    if picker is None:
        picker = PICKERS[getattr(strategy, "picker", "stocked")]
    if patient is None:
        patient = getattr(strategy, "patient", False)
    accept_mode = getattr(strategy, "accept", "dominate")
    quota = getattr(strategy, "quota", None)

    level = 1
    states = [(0, 0)] * N_RELICS
    # relics already on hand before the first summon: one number for every
    # type, or a list of twelve
    if isinstance(start_stock, (list, tuple)):
        inventory = list(start_stock)
    else:
        inventory = [int(start_stock)] * N_RELICS
    summons = 0
    attempts = 0
    # level -> (diamonds, attempts, crit amp, by pity, atk amp)
    reached = {1: (0, 0, 0.0, False, 0.0)}
    rand = rng.random
    filler_for = getattr(strategy, "filler_for", None)
    filler_values, filler_cum = None, None

    while level < max_level:
        goal = level + 1
        req = REQUIREMENTS[goal]
        guard = 0
        escalation = 0
        banked = strategy.protected(goal) if quota is not None else frozenset()
        # a protected relic can be spent only out of stock above its quota -
        # unless this is its own level, when there is nothing left to bank for
        need = [RELICS_PER_ATTEMPT + (quota if i in banked else 0)
                for i in range(N_RELICS)]
        meter = 0
        full = pity_needed(goal) if pity else math.inf
        gain = pity_gain(level)
        filler = filler_for(goal) if filler_for else None
        prefer_for = getattr(strategy, "prefer_for", None)
        prefer = prefer_for(goal) if prefer_for else None
        if filler:
            filler_values, filler_cum = score_sampler(level, 1, 1)
            no_filler = strategy.protected(goal) if quota is None else frozenset()
        while meter < full and not level_satisfied(goal, states):
            profiles = strategy(goal, states)
            for _ in range(escalation):
                profiles = _escalate(profiles, states, req)
            todo = [i for i, p in profiles.items() if not meets(states[i], *p)]
            if not todo:
                escalation += 1
                guard += 1
                if guard > 400:      # cannot happen with a sane requirement table
                    raise RuntimeError(f"stuck trying to reach level {goal}")
                continue
            # summon, or attempt with what is already banked?
            if patient:
                pick = picker(todo, states, profiles, inventory, level)
                while inventory[pick] < RELICS_PER_ATTEMPT:
                    for _ in range(RELICS_PER_SUMMON):
                        inventory[int(rand() * N_RELICS)] += 1
                    summons += 1
            else:
                spare = None
                if prefer and prefer[1] == "hard" and prefer[0] & set(todo):
                    todo = [i for i in todo if i in prefer[0]]
                affordable = [i for i in todo if inventory[i] >= need[i]]
                while not affordable:
                    if filler:
                        # nothing the quest needs is ready: attempt a spare
                        # relic for the pity rather than summon
                        spare = [i for i in range(N_RELICS)
                                 if inventory[i] >= RELICS_PER_ATTEMPT
                                 and i not in no_filler and i not in todo]
                        if spare:
                            break
                    for _ in range(RELICS_PER_SUMMON):
                        inventory[int(rand() * N_RELICS)] += 1
                    summons += 1
                    affordable = [i for i in todo if inventory[i] >= need[i]]
                if not affordable:
                    pick = max(spare, key=lambda i: inventory[i])
                    inventory[pick] -= RELICS_PER_ATTEMPT
                    attempts += 1
                    meter += gain
                    rolled = filler_values[bisect.bisect(filler_cum, rand())]
                    if _filler_keeps(states, pick, rolled, filler, goal, req):
                        states[pick] = rolled
                    continue
                if prefer:
                    affordable = [i for i in affordable if i in prefer[0]] or affordable
                pick = picker(affordable, states, profiles, inventory, level)
            inventory[pick] -= RELICS_PER_ATTEMPT
            attempts += 1
            meter += gain

            aim = getattr(strategy, "aim", None)
            want_g, allow_d = aim(goal, pick, profiles[pick]) if aim else profiles[pick]
            weights = dict(getattr(strategy, "policy", ())).get(goal)
            if weights:
                values, cum = score_sampler(level, *weights)
            else:
                values, cum = outcome_sampler(level, want_g,
                                              allow_d if allow_d is not None
                                              else config_for_level(level).slots)
            rolled = values[bisect.bisect(cum, rand())]
            old = states[pick]
            take = _accept(old, rolled, profiles[pick])
            if (not take and req[0] == "total"
                    and (accept_mode == "total"
                         or (isinstance(accept_mode, frozenset) and goal in accept_mode))):
                trial = list(states)
                trial[pick] = rolled
                take = _board_gap(trial, req) < _board_gap(states, req)
            if take:
                states[pick] = rolled
        by_pity = not level_satisfied(goal, states)
        level = goal
        reached[level] = (summons * DIAMONDS_PER_SUMMON, attempts,
                          amplification(states[CRIT]), by_pity,
                          amplification(states[ATK]))
        if trace is not None:
            trace[level] = list(inventory)
    return reached, states


def _filler_keeps(states, pick, new, profile, goal, req) -> bool:
    """Keep a filler roll?  Only if it helps the relic toward `profile` and
    cannot set back the quest in progress."""
    old = states[pick]
    if not _accept(old, new, profile):
        return False
    if req[0] == "total":
        trial = list(states)
        trial[pick] = new
        return _board_gap(trial, req) <= _board_gap(states, req)
    # a relic this level names (or every relic, on an "each" level) must not
    # drop back below the bar it already clears
    named = req[1] if req[0] == "relics" else range(N_RELICS)
    if pick in named:
        glory, despair = req[-2], req[-1]
        if meets(old, glory, despair) and not meets(new, glory, despair):
            return False
    return True


def _accept(old, new, profile) -> bool:
    """Keep the new roll, or keep what the relic already had?

    A dead end cannot be used at all, and lands as (0, 0), so it is rejected by
    the same rule.  Anything that dominates is taken; otherwise a roll is only
    worth taking if it clears the profile the old one failed.
    """
    g0, d0 = old
    g1, d1 = new
    if g1 >= g0 and d1 <= d0 and (g1 > g0 or d1 < d0):
        return True
    want_g, allow_d = profile
    return meets(new, want_g, allow_d) and not meets(old, want_g, allow_d)


# Named schedules.  `tuned` is the cheapest route to level 9 found in LEVELS.md:
# a coordinate descent over every Plan knob, confirmed on a held-out seed.
# Play the building levels for max glory + min despair, weighted equally,
# rather than all-or-nothing for the bar.  The bar still decides what is kept.
SCORE_POLICY = tuple((lvl, (1, 1)) for lvl in (4, 6, 7, 8))

L9_RELICS = frozenset(REQUIREMENTS[MAX_LEVEL][1])

STRATEGIES = {
    # The two plans LEVELS.md is about, both found with the pity rule on.
    # Look-ahead: the cheapest route to level 9 overall (`--lookahead`).
    "lookahead": Plan(
        base=(4, 2),
        bases=((4, (5, 1)), (6, (5, 1)), (7, (4, 2))),
        surgical=frozenset({8}),
        reserve=((8, L9_RELICS),),
        accept=frozenset({4, 6, 8}),
        policy=((4, (1, 1)), (6, (1, 1)), (7, (1, 1)), (8, (1, 1))),
        filler=((5, (4, 2)), (6, (4, 2)), (8, (4, 1)), (9, (4, 2))),
    ),
    # Greedy: every level-up as cheap as it can be on its own (`--greedy`).
    "greedy": Plan(
        base=(4, 2),
        bases=((4, (5, 1)), (6, (5, 1)), (7, (4, 2))),
        surgical=frozenset({8}),
        reserve=(),
        accept=frozenset({4, 6, 8}),
        policy=((4, (1, 1)), (6, (1, 1)), (7, (1, 1)), (8, (1, 1))),
        filler=((2, (4, 2)), (3, (4, 2)), (5, (4, 2)), (6, (4, 2)), (7, (4, 2)),
                (8, (4, 1)), (9, (4, 2))),
    ),
    # Found before the pity rule existed (none of them use filler); kept as
    # baselines for --compare and the tests.
    "tuned": Plan(
        base=(4, 2),                    # ask each level for the minimum...
        bases=((6, (5, 2)),),           # ...except level 6, built a notch higher
        surgical=frozenset({8}),        # repair level 8 step by step, never rebuild
        reserve=frozenset({6, 7, 8}),   # bank level 9's three relics from the start
        accept="total",                 # take any roll that closes a total's gap
        policy=SCORE_POLICY,            # roll for glory + clean despair, 1:1
    ),
    # tuned without banking for level 9 - beaten by `greedy` at level 8 and 9
    "stop8": Plan(
        base=(4, 2),
        bases=((6, (5, 2)),),
        surgical=frozenset({8}),
        accept="total",
        policy=SCORE_POLICY,
    ),
    "repair": Plan(base=(4, 2), surgical=frozenset({8})),
    "minimal": Plan(base=(4, 2)),
    "wide": Plan(base=(4, 1)),
    "balanced": Plan(base=(5, 2)),
    "prep": Plan(base=(5, 1)),
}

strategy_lookahead = STRATEGIES["lookahead"]
strategy_tuned = STRATEGIES["tuned"]
strategy_stop8 = STRATEGIES["stop8"]
strategy_greedy = STRATEGIES["greedy"]
strategy_repair = STRATEGIES["repair"]
strategy_minimal = STRATEGIES["minimal"]
strategy_wide = STRATEGIES["wide"]
strategy_balanced = STRATEGIES["balanced"]
strategy_prep = STRATEGIES["prep"]


def _names(relics) -> str:
    return ", ".join(RELICS[i] for i in sorted(relics))


def _bar(glory, despair) -> str:
    return f"{glory}+ / {'any' if despair is None else f'{despair}-'}"


def _policy_note(plan, level: int) -> str:
    weights = dict(plan.policy).get(level)
    if not weights:
        return ""
    wg, wd = weights
    if wg == wd:
        how = "max glory and min despair, weighted equally"
    else:
        how = f"glory and clean despair weighted {wg:g}:{wd:g}"
    return f"; play each attempt for {how}, not all-or-nothing for the bar"


def _filler_note(plan, level: int) -> str:
    """The pity side of a level: what spare stock is spent on, and the bar."""
    attempts = math.ceil(pity_needed(level) / pity_gain(level - 1))
    pity = (f"{pity_needed(level):,} pity ({attempts} attempts at +{pity_gain(level - 1)}) "
            f"levels up without the quest")
    fill = plan.filler_for(level) if isinstance(plan, Plan) else None
    if not fill:
        return f". When nothing the quest needs is affordable, summon. {pity}"
    locked = plan.protected(level) if plan.quota is None else frozenset()
    spare = "the best-stocked spare relic"
    if locked:
        spare += f" (never {_names(locked)})"
    return (f". When nothing the quest needs is affordable, attempt {spare} "
            f"instead of summoning, rolled for max glory + min despair and kept "
            f"if it moves toward {_bar(*fill)}. {pity}")


def describe_step(plan, level: int) -> str:
    """What `plan` actually does to reach `level`, in words.

    Built from the Plan itself rather than written by hand, so it cannot drift
    from what the simulation does.
    """
    text = _describe_quest(plan, level)
    if REQUIREMENTS.get(level) is None or not isinstance(plan, Plan):
        return text
    return text + _filler_note(plan, level)


def _describe_quest(plan, level: int) -> str:
    req = REQUIREMENTS.get(level)
    if req is None:
        return "free - every run starts here"
    if not isinstance(plan, Plan):
        return "custom strategy"
    kind = req[0]
    locked = plan.protected(level) if plan.quota is None else frozenset()

    if kind == "relics":
        _, which, glory, despair = req
        text = f"roll {_names(which)} to {_bar(glory, despair)}"
        # the levels whose work (or filler) kept these relics' stock untouched
        banked_during = [lv for lv in range(2, level)
                         if set(which) & set(plan.protected(lv))
                         and (REQUIREMENTS[lv][0] == "total" or plan.filler_for(lv))]
        if banked_during and plan.quota is None:
            during = ", ".join(str(lv) for lv in banked_during)
            text += (f", paid for partly out of their stock banked during "
                     f"level{'s' if len(banked_during) > 1 else ''} {during}")
        extra = dict(plan.prework).get(level)
        if extra:
            text += f"; meanwhile put idle relics to work toward {_bar(*extra)}"
        return text

    if kind == "each":
        _, glory, despair = req
        base = plan.base_for(level)
        g, d = max(glory, base[0]), min(despair, base[1])
        return f"roll every relic that is short to {_bar(g, d)}" + _policy_note(plan, level)

    # a total
    # which relic is a stock question, not a quality one: preferring the next
    # level's relics (or ones already worked on) was tried and never paid
    pool = "whichever relics you hold the most of"
    if locked:
        pool += f", except {_names(locked)}"
    if level in plan.surgical:
        text = (f"repair, do not rebuild: using {pool}, shave one despair off the "
                f"worst relics until the total fits, then add one glory at a time")
    else:
        text = f"roll {pool} to {_bar(*plan.base_for(level))} until the totals clear"
    text += _policy_note(plan, level)
    if plan.accept == "total" or (isinstance(plan.accept, frozenset)
                                  and level in plan.accept):
        text += "; take any roll that closes the gap, even if it is not better on both bars"
    if locked:
        keep_for = sorted({lv for lv, req2 in REQUIREMENTS.items()
                           if lv > level and req2[0] == "relics"
                           and set(req2[1]) & set(locked)})
        text += f" (their stock is banked for level {', '.join(map(str, keep_for))})"
    return text


def describe_habits(plan) -> str:
    """The rules a Plan applies on every single step, whatever the level."""
    if not isinstance(plan, Plan):
        return "custom strategy"
    pick = {
        "stocked": "use whichever needed relic you hold the most of",
        "likeliest": "use the needed relic whose roll is most likely to land",
        "closest": "use the needed relic closest to its target",
        "neediest": "use the needed relic furthest from its target",
        "untouched": "prefer needed relics that have never been rolled",
    }.get(plan.picker, plan.picker)
    when = ("summon until the relic you want is affordable" if plan.patient
            else "attempt as soon as any relic the quest needs is affordable")
    return f"{when}; {pick}; keep a result only if it helps"


def run_seed(seed: int, index: int) -> int:
    """Run `index` always gets the same random stream, however the work is split.

    That keeps results identical across worker counts, and pairs runs between
    plans: run 17 under one plan sees the same luck as run 17 under another.
    """
    return seed * 1_000_003 + index


def _simulate_chunk(job):
    strategy, seed, first, count, max_level, picker_name, patient, start_stock, pity = job
    picker = PICKERS[picker_name] if picker_name else None
    return [simulate_run(strategy, random.Random(run_seed(seed, i)), max_level,
                         picker, patient, start_stock=start_stock, pity=pity)
            for i in range(first, first + count)]


# below this many runs, starting worker processes costs more than it saves
PARALLEL_MIN_RUNS = 400


def simulate_many(strategy, runs: int, seed: int = 20260922, max_level: int = MAX_LEVEL,
                  picker_name=None, patient=None, start_stock=0, workers=None,
                  pity: bool = True):
    """`runs` independent simulate_run results, spread over every CPU core.

    Returns [(reached, states), ...] in run order.  `strategy` must be picklable
    (a Plan is) and `picker_name` a key of PICKERS, so the work can cross
    process boundaries.  Callers must sit under `if __name__ == "__main__":`
    on Windows, as for any multiprocessing code.
    """
    if workers is None:
        workers = os.cpu_count() or 1
    if runs < PARALLEL_MIN_RUNS or workers <= 1:
        return _simulate_chunk((strategy, seed, 0, runs, max_level, picker_name,
                                patient, start_stock, pity))
    # solve every table this plan will want up front, so the workers load them
    # from disk instead of all solving the same ones at once
    _simulate_chunk((strategy, seed, 0, 20, max_level, picker_name, patient, start_stock,
                     pity))
    size = -(-runs // (workers * 4))
    jobs = [(strategy, seed, first, min(size, runs - first), max_level, picker_name,
             patient, start_stock, pity) for first in range(0, runs, size)]
    out = []
    for chunk in _pool(workers).map(_simulate_chunk, jobs):
        out.extend(chunk)
    return out


_POOLS: dict = {}


def _pool(workers: int) -> ProcessPoolExecutor:
    """One long-lived pool per size: starting processes on Windows costs ~1s,
    which a search running a hundred evaluations should only pay once."""
    pool = _POOLS.get(workers)
    if pool is None:
        pool = _POOLS[workers] = ProcessPoolExecutor(max_workers=workers)
    return pool


# --------------------------------------------------------- greedy, step by step
@dataclass(frozen=True)
class StepChoice:
    """Everything that can be decided for one level on its own.

    profile   what each relic is rolled toward: a (glory, despair) bar, or
              "repair" for single-step fixes to the board as it stands; None on
              a level that names its relics (they are rolled to its bar)
    closer    on a total level, also keep rolls that close the board's gap
    score     play attempts for max glory + min despair instead of all-or-nothing
    filler    when nothing the quest needs is affordable: None to summon, or a
              (glory, despair) profile to roll the best-stocked spare relic
              toward, for the pity
    bank      keep the level-9 relics out of this level's work (and out of its
              filler), so their stock is still there when level 9 comes
    """

    profile: object = None
    closer: bool = False
    score: bool = False
    bank: bool = False
    filler: object = None

    @property
    def label(self) -> str:
        if self.profile is None:
            parts = ["roll the named relics to the bar"]
        else:
            parts = ["repair" if self.profile == "repair" else _bar(*self.profile)]
            parts.append("max glory + min despair" if self.score else "all-or-nothing")
            if self.closer:
                parts.append("keep gap-closers")
        parts.append(f"filler to {_bar(*self.filler)}" if self.filler else "no filler")
        if self.bank:
            parts.append("bank level-9 relics")
        return ", ".join(parts)


# what spare relics can be rolled toward while they build pity
FILLERS = (None, (4, 2), (4, 1), (5, 2), (5, 1))
TOTAL_PROFILES = ((3, 2), (3, 1), (4, 2), (4, 1), (5, 2), (5, 1), "repair")
EACH_PROFILES = ((4, 2), (4, 1), (5, 2), (5, 1))


def step_choices(level: int) -> list:
    """Every StepChoice worth trying for `level` (banking aside).

    A level that names its relics has only filler to decide: all-or-nothing is
    the best way to clear a bar, and no other relic is part of its quest.
    """
    kind = REQUIREMENTS[level][0]
    if kind == "relics":
        return [StepChoice(filler=f) for f in FILLERS]
    if kind == "each":
        return [StepChoice(p, False, s, filler=f) for p in EACH_PROFILES
                for s in (False, True) for f in FILLERS]
    return [StepChoice(p, c, s, filler=f) for p in TOTAL_PROFILES for c in (False, True)
            for s in (False, True) for f in FILLERS]


def knob_values(level: int) -> dict:
    """Each knob a level has, and the values it can take - for a descent that
    turns one knob at a time."""
    kind = REQUIREMENTS[level][0]
    knobs = {"filler": FILLERS}
    if future_named(level) & set(REQUIREMENTS[MAX_LEVEL][1]):
        knobs["bank"] = (False, True)
    if kind == "each":
        knobs.update(profile=EACH_PROFILES, score=(False, True))
    elif kind == "total":
        knobs.update(profile=TOTAL_PROFILES, closer=(False, True), score=(False, True))
    return knobs


def with_choice(plan: Plan, level: int, choice: StepChoice) -> Plan:
    """`plan` with one level's decisions replaced by `choice`."""
    bases = tuple((lv, p) for lv, p in plan.bases if lv != level)
    surgical = plan.surgical - {level}
    if choice.profile == "repair":
        surgical |= {level}
    elif choice.profile is not None:
        bases += ((level, choice.profile),)
    accept = plan.accept if isinstance(plan.accept, frozenset) else frozenset()
    accept = (accept - {level}) | ({level} if choice.closer else frozenset())
    policy = tuple((lv, w) for lv, w in plan.policy if lv != level)
    if choice.score:
        policy += ((level, (1, 1)),)
    reserve = tuple((lv, s) for lv, s in (plan.reserve or ()) if lv != level)
    if choice.bank:
        reserve += ((level, frozenset(REQUIREMENTS[MAX_LEVEL][1])),)
    filler = plan.filler if isinstance(plan.filler, tuple) and plan.filler \
        and not isinstance(plan.filler[0], int) else ()
    filler = tuple((lv, f) for lv, f in filler if lv != level)
    if choice.filler:
        filler += ((level, choice.filler),)
    return Plan(base=plan.base, bases=bases, surgical=frozenset(surgical),
                reserve=reserve, accept=frozenset(accept), policy=policy,
                picker=plan.picker, patient=plan.patient, filler=filler,
                prefer=plan.prefer)


def build_plan(choices: dict) -> Plan:
    """A Plan from one StepChoice per level."""
    plan = Plan(base=(4, 2), accept=frozenset(), reserve=(), filler=())
    for level in sorted(choices):
        plan = with_choice(plan, level, choices[level])
    return plan


def step_cost(plan, level: int, runs: int, seed: int) -> float:
    """Mean diamonds spent on the step from level-1 to `level` alone."""
    results = simulate_many(plan, runs, seed, max_level=level)
    return sum(r[level][0] - r[level - 1][0] for r, _s in results) / len(results)


def greedy_search(runs: int = 3000, seed: int = 777, finalists: int = 3,
                  final_runs: int = 12000, max_level: int = MAX_LEVEL, log=None):
    """Make each level as cheap as possible on its own, in order.

    Level by level, every StepChoice is scored on the cost of that step alone,
    starting from the board the greedy choices so far leave behind, and the
    cheapest is locked in.  What a choice does to LATER levels is ignored -
    which is the whole difference from the `tuned` plan.  The closest few are
    re-run with more samples before one is picked, and every candidate sees the
    same luck (paired runs), so the ranking is not a coin toss.

    Returns (plan, report); the report has one row per level with the winner
    and the runner-up.
    """
    plan = build_plan({})
    report = []
    for level in range(2, max_level + 1):
        choices = step_choices(level)
        scored = sorted(((step_cost(with_choice(plan, level, c), level, runs, seed), i)
                         for i, c in enumerate(choices)))
        if len(scored) > 1:
            scored = sorted(((step_cost(with_choice(plan, level, choices[i]), level,
                                        final_runs, seed + 1), i)
                             for _c, i in scored[:finalists]))
        cost, best = scored[0]
        plan = with_choice(plan, level, choices[best])
        # does banking for later ever pay for itself on this step alone?  (no)
        bank_cost = None
        if REQUIREMENTS[level][0] == "total" and future_named(level):
            banked = with_choice(plan, level,
                                 replace(choices[best], bank=True))
            bank_cost = step_cost(banked, level, final_runs, seed + 1)
        row = {"level": level, "choice": choices[best], "cost": cost,
               "options": len(choices), "bank_cost": bank_cost,
               "runner_up": (choices[scored[1][1]], scored[1][0]) if len(scored) > 1 else None}
        report.append(row)
        if log:
            log(f"L{level}: {choices[best].label}  {cost:,.0f}")
    return plan, report


def total_cost(plan, runs: int, seed: int, max_level: int = MAX_LEVEL) -> float:
    """Mean diamonds from level 1 to `max_level`."""
    results = simulate_many(plan, runs, seed, max_level=max_level)
    return sum(r[max_level][0] for r, _s in results) / len(results)


def lookahead_search(start: dict, runs: int = 3000, seed: int = 4242,
                     final_runs: int = 12000, sweeps: int = 4,
                     max_level: int = MAX_LEVEL, log=None):
    """The cheapest plan to `max_level` overall, by coordinate descent.

    `start` is one StepChoice per level.  Every knob of every level is turned
    in turn, each value scored on the total cost to `max_level`, and a change
    is kept only if it still wins on a larger, fresh set of paired runs.
    Repeats until a sweep changes nothing.  Returns ({level: StepChoice}, cost).
    """
    choices = dict(start)
    for level in range(2, max_level + 1):
        choices.setdefault(level, StepChoice())
    for sweep in range(sweeps):
        changed = False
        for level in range(2, max_level + 1):
            for knob, values in knob_values(level).items():
                current = choices[level]
                trials = [replace(current, **{knob: v}) for v in values
                          if v != getattr(current, knob)]
                if not trials:
                    continue
                scored = sorted((total_cost(build_plan({**choices, level: c}), runs,
                                            seed + sweep, max_level), i)
                                for i, c in enumerate(trials))
                best = trials[scored[0][1]]
                check = seed + 1000 + sweep
                now = total_cost(build_plan(choices), final_runs, check, max_level)
                new = total_cost(build_plan({**choices, level: best}), final_runs,
                                 check, max_level)
                if new < now * 0.995:
                    choices[level] = best
                    changed = True
                    if log:
                        log(f"sweep {sweep} L{level} {knob}: {current.label} -> "
                            f"{best.label}  {now:,.0f} -> {new:,.0f}")
        if not changed:
            break
    return choices, total_cost(build_plan(choices), final_runs, seed + 2000, max_level)


def run(strategy_name: str = "lookahead", runs: int = 2000, seed: int = 20260922,
        max_level: int = MAX_LEVEL, picker_name=None, patient=None, start_stock=0,
        workers=None):
    strategy = STRATEGIES[strategy_name]
    per_level = defaultdict(list)
    per_level_attempts = defaultdict(list)
    per_level_amp = defaultdict(list)
    per_level_pity = defaultdict(list)
    per_level_atk = defaultdict(list)
    per_level_dmg = defaultdict(list)
    final_states = []
    for reached, states in simulate_many(strategy, runs, seed, max_level, picker_name,
                                         patient, start_stock, workers):
        for lvl, (diamonds, attempts, amp, by_pity, atk) in reached.items():
            per_level_pity[lvl].append(by_pity)
            per_level_atk[lvl].append(atk)
            per_level_dmg[lvl].append(damage(lvl, amp, atk))
            per_level[lvl].append(diamonds)
            per_level_attempts[lvl].append(attempts)
            per_level_amp[lvl].append(amp)
        final_states.append(states)

    rows = []
    for lvl in sorted(per_level):
        costs = sorted(per_level[lvl])
        n = len(costs)
        rows.append({
            "level": lvl,
            "requirement": requirement_text(lvl),
            "short": requirement_text(lvl, short=True),
            "mean": sum(costs) / n,
            "median": costs[n // 2],
            "p90": costs[min(n - 1, int(0.9 * n))],
            "attempts": sum(per_level_attempts[lvl]) / n,
            "crit_amp": sum(per_level_amp[lvl]) / n,
            "pity": sum(per_level_pity[lvl]) / n,
            "atk_amp": sum(per_level_atk[lvl]) / n,
            # averaged per run: the two relics' amps are not independent
            "damage": sum(per_level_dmg[lvl]) / n,
        })
    return rows, final_states


# ---------------------------------------------------------------- reporting
def human(value: float) -> str:
    for cut, suffix in ((1e9, "B"), (1e6, "M"), (1e3, "K")):
        if value >= cut:
            return f"{value / cut:.3g}{suffix}"
    return f"{value:.0f}"


def print_table(rows, strategy_name: str, runs: int) -> None:
    print(f"Diamond cost to raise the inheritor - strategy '{strategy_name}', "
          f"{runs:,} runs")
    print(f"  {DIAMONDS_PER_SUMMON:,} diamonds = {RELICS_PER_SUMMON} relics over "
          f"{N_RELICS} types, {RELICS_PER_ATTEMPT} of a type per attempt")
    print()
    print(f"  {'lvl':>3} | {'mean diamonds':>14} {'median':>9} {'90th':>9} "
          f"{'attempts':>9} {'crit amp':>9} | requirement (glory/despair)")
    print("  " + "-" * 110)
    for r in rows:
        print(f"  {r['level']:>3} | {r['mean']:>14,.0f} {human(r['median']):>9} "
              f"{human(r['p90']):>9} {r['attempts']:>9.1f} {r['crit_amp']:>8.2f}% | "
              f"{r['requirement'][:58]}")


LOG_TICK_STEP = 10              # right-axis labels: +0%, +10%, +20% ...


def _log_ticks(top_pct: float, step: int = LOG_TICK_STEP):
    """Evenly stepped dmg-increase labels (+0%, +10%, ...) up to the first one
    at or above `top_pct`.  Plotted at log(1 + x), so they bunch up as they
    rise."""
    last = step * max(1, math.ceil(top_pct / step - 1e-9))
    return list(range(0, last + 1, step))


def _dual_chart(levels_, bars, line, title, desc, bar_name, line_name) -> str:
    """Columns on a linear diamond axis (left), with a red line on a log axis
    (right).  `line` holds multipliers (1.25 = +25%); the right axis plots
    log(multiplier) and is labelled in percent increase."""
    top = max(bars)
    step = 10 ** math.floor(math.log10(top))
    # headroom so the tallest bar's value label never hits the edge
    y_top = step * math.ceil(top * 1.12 / step)
    # right axis tops out at the first +10% label above the line
    line_ticks = _log_ticks((max(line) - 1) * 100)
    log_top = math.log1p(line_ticks[-1] / 100)

    ml, mr, mt, mb = 58, 50, 40, 42
    pitch, bar_w = 52, 36
    plot_w, plot_h = pitch * len(bars), 210
    width, height = ml + plot_w + mr, mt + plot_h + mb
    base_y = mt + plot_h

    def x_mid(i):
        return ml + i * pitch + pitch / 2

    def y_bar(v):
        return mt + plot_h * (1 - v / y_top)

    def y_line(m):
        return mt + plot_h * (1 - math.log(m) / log_top)

    out = [
        f'<svg viewBox="0 0 {width} {height}" width="{width}" height="{height}" '
        f'role="img" xmlns="http://www.w3.org/2000/svg" aria-label="{title}">',
        f"<title>{title}</title>",
        f"<desc>{desc}</desc>",
        "<style>"
        ".lv-bar{fill:#2a78d6}.lv-grid{stroke:#e1e0d9;stroke-width:1}"
        ".lv-axis{stroke:#c3c2b7;stroke-width:1}.lv-ink{fill:#52514e}"
        ".lv-muted{fill:#898781}"
        ".lv-line{stroke:#d03b32;stroke-width:2;fill:none}.lv-dot{fill:#d03b32}"
        ".lv-halo{paint-order:stroke;stroke:#ffffff;stroke-width:3px;stroke-linejoin:round}"
        ".lv-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}"
        ".lv-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}"
        "@media(prefers-color-scheme:dark){"
        ".lv-bar{fill:#3987e5}.lv-grid{stroke:#2c2c2a}.lv-axis{stroke:#383835}"
        ".lv-ink{fill:#c3c2b7}.lv-line{stroke:#ff6a5f}.lv-dot{fill:#ff6a5f}"
        ".lv-halo{stroke:#1f1f1e}}"
        "</style>",
    ]
    # left axis: diamonds, with gridlines
    ticks = 4
    for i in range(ticks + 1):
        v = y_top * i / ticks
        y = y_bar(v)
        out.append(f'<line class="lv-grid" x1="{ml}" y1="{y:.1f}" '
                   f'x2="{ml + plot_w}" y2="{y:.1f}"/>')
        out.append(f'<text class="lv-t lv-muted" x="{ml - 8}" y="{y + 4:.1f}" '
                   f'text-anchor="end">{human(v)}</text>')
    # right axis: log(1 + dmg increase), labelled in percent
    for pct in line_ticks:
        y = y_line(1 + pct / 100)
        out.append(f'<line class="lv-axis" x1="{ml + plot_w}" y1="{y:.1f}" '
                   f'x2="{ml + plot_w + 4}" y2="{y:.1f}"/>')
        out.append(f'<text class="lv-t lv-muted" x="{ml + plot_w + 7}" y="{y + 4:.1f}">'
                   f'+{pct}%</text>')
    # axis titles and legend, above the plot
    out.append(f'<text class="lv-t lv-muted" x="{ml - 8}" y="{mt - 12}" '
               f'text-anchor="end">diamonds</text>')
    out.append(f'<text class="lv-t lv-muted" x="{ml + plot_w + 46}" y="{mt - 12}" '
               f'text-anchor="end">log dmg</text>')
    lx = ml + 4
    out.append(f'<rect class="lv-bar" x="{lx}" y="{mt - 30}" width="10" height="10" rx="2"/>')
    out.append(f'<text class="lv-t lv-ink" x="{lx + 14}" y="{mt - 21}">{bar_name}</text>')
    lx2 = lx + 22 + 6.2 * len(bar_name)
    out.append(f'<line class="lv-line" x1="{lx2}" y1="{mt - 25}" x2="{lx2 + 16}" '
               f'y2="{mt - 25}"/>')
    out.append(f'<text class="lv-t lv-ink" x="{lx2 + 20}" y="{mt - 21}">{line_name}</text>')

    # the columns, each labelled with its value
    for i, (lvl, v) in enumerate(zip(levels_, bars)):
        x, y = x_mid(i) - bar_w / 2, y_bar(v)
        h = base_y - y
        rad = min(3, h)
        out.append(
            f'<path class="lv-bar" d="M{x:.1f},{base_y} V{y + rad:.1f} '
            f'Q{x:.1f},{y:.1f} {x + rad:.1f},{y:.1f} H{x + bar_w - rad:.1f} '
            f'Q{x + bar_w:.1f},{y:.1f} {x + bar_w:.1f},{y + rad:.1f} '
            f'V{base_y} Z"><title>level {lvl}: {v:,.0f} diamonds</title></path>')
        out.append(f'<text class="lv-t lv-muted" x="{x_mid(i):.1f}" '
                   f'y="{base_y + 15}" text-anchor="middle">L{lvl}</text>')
        out.append(f'<text class="lv-b lv-ink lv-halo" x="{x_mid(i):.1f}" '
                   f'y="{y - 6:.1f}" text-anchor="middle">{human(v)}</text>')
    out.append(f'<line class="lv-axis" x1="{ml}" y1="{base_y}" '
               f'x2="{ml + plot_w}" y2="{base_y}"/>')
    out.append(f'<line class="lv-axis" x1="{ml + plot_w}" y1="{mt}" '
               f'x2="{ml + plot_w}" y2="{base_y}"/>')

    # the red line on top, with a dot per level
    pts = " ".join(f"{x_mid(i):.1f},{y_line(m):.1f}" for i, m in enumerate(line))
    out.append(f'<polyline class="lv-line" points="{pts}"/>')
    for i, (lvl, m) in enumerate(zip(levels_, line)):
        out.append(f'<circle class="lv-dot" cx="{x_mid(i):.1f}" cy="{y_line(m):.1f}" '
                   f'r="3"><title>level {lvl}: +{m - 1:.1%} {line_name}</title></circle>')
    out.append(f'<text class="lv-t lv-muted" x="{ml + plot_w / 2:.1f}" '
               f'y="{height - 6}" text-anchor="middle">inheritor level reached</text>')
    out.append("</svg>")
    return chr(10).join(out)


def chart_svg(rows, title: str = "Cost to raise the inheritor") -> str:
    """Cumulative diamonds to reach each level (columns) and the total damage
    increase so far (red line, log scale)."""
    data = [r for r in rows if r["level"] > 1]
    return _dual_chart(
        [r["level"] for r in data], [r["mean"] for r in data],
        [r["damage"] for r in data], title,
        f'Cumulative mean diamonds by inheritor level, {human(data[0]["mean"])} at level '
        f'{data[0]["level"]} rising to {human(data[-1]["mean"])} at level '
        f'{data[-1]["level"]}; damage increase +{data[-1]["damage"] - 1:.0%} by then.',
        "average diamonds", "dmg increase")


def marginal_chart_svg(rows, title: str = "Cost of each level-up") -> str:
    """Diamonds for each single level-up (columns) and the damage that level-up
    adds on top of the previous level (red line, log scale)."""
    data = [(r, prev) for prev, r in zip(rows, rows[1:])]
    return _dual_chart(
        [r["level"] for r, _p in data], [r["mean"] - p["mean"] for r, p in data],
        [r["damage"] / p["damage"] for r, p in data], title,
        "Mean diamonds spent on each level-up, and the damage increase that level-up "
        "adds over the one before.",
        "marginal diamonds", "marginal dmg increase")


DOC_MODES = (
    ("greedy", "Greedy mode",
     "Every level-up made as cheap as it can be on its own, ignoring later levels."),
    ("lookahead", "Look-ahead mode",
     "The cheapest route to level 9 overall: some levels cost more so later ones cost less."),
)


DAMAGE_NOTE = ("dmg increase = (1 + level multiplier) x (1 + 4/5 x crit amp) x "
               "(1 + 22/209 x atk amp) - 1, where the level multiplier is +5% a level "
               "for levels 2-7 and +10% for 8 and 9. Marginal columns are relative "
               "to the level before.")


def markdown(runs: int = 20000, seed: int = 20260922) -> str:
    """LEVELS.md, generated end to end from the simulation."""
    out = [
        "# Raising the inheritor",
        "",
        "<!-- generated by `python levels.py --markdown LEVELS.md` - do not edit -->",
        "",
        f"Diamonds from inheritor level 1 to {MAX_LEVEL}, starting with no relics. "
        f"{DIAMONDS_PER_SUMMON:,} diamonds buys "
        f"{RELICS_PER_SUMMON} random relics of {N_RELICS} types; an attempt uses "
        f"{RELICS_PER_ATTEMPT} of one type. A level is reached by its quest or by "
        f"a full pity meter, whichever comes first.",
        "",
        DAMAGE_NOTE,
    ]
    for name, title, blurb in DOC_MODES:
        rows, _states = run(name, runs, seed)
        out += ["", f"## {title}", "", f"{blurb} {runs:,} simulations.", "",
                "| level | requirement (glory/despair) | average diamonds | "
                "marginal diamonds | median | reached by pity | crit relic amp | "
                "atk relic amp | dmg increase | marginal dmg increase |",
                "|---|---|---|---|---|---|---|---|---|---|"]
        prev = None
        for r in rows:
            first = prev is None
            step = "-" if first else human(r["mean"] - prev["mean"])
            pity = "-" if first else f"{r['pity']:.0%}"
            more = "-" if first else f"+{r['damage'] / prev['damage'] - 1:.1%}"
            prev = r
            out.append(f"| **{r['level']}** | {r['short']} | {r['mean']:,.0f} | {step} | "
                       f"{human(r['median'])} | {pity} | {r['crit_amp']:.2f}% | "
                       f"{r['atk_amp']:.2f}% | +{r['damage'] - 1:.1%} | {more} |")
        # side by side where there is room, stacked where there is not
        out += ["", '<div style="display:flex;flex-wrap:wrap;gap:12px">',
                chart_svg(rows, f"{title}: total cost and damage"),
                marginal_chart_svg(rows, f"{title}: each level-up"),
                "</div>"]

    out += ["", "## Strategy at each level", "",
            f"On every step, both modes: {describe_habits(STRATEGIES['greedy'])}."]
    for level in range(2, MAX_LEVEL + 1):
        out += ["", f"### Level {level} - {requirement_text(level)}", ""]
        for name, title, _blurb in DOC_MODES:
            out.append(f"- **{title.split()[0]}:** {describe_step(STRATEGIES[name], level)}.")
    return "\n".join(out) + "\n"


def compare(runs: int, seed: int, max_level: int) -> None:
    print(f"Strategy comparison to level {max_level}, {runs:,} runs each")
    print()
    print(f"  {'strategy':>10} | {'mean diamonds':>14} {'median':>9} {'90th':>9} "
          f"{'attempts':>9} {'crit amp':>9}")
    print("  " + "-" * 70)
    best = None
    for name in STRATEGIES:
        rows, _states = run(name, runs, seed, max_level)
        top = rows[-1]
        print(f"  {name:>10} | {top['mean']:>14,.0f} {human(top['median']):>9} "
              f"{human(top['p90']):>9} {top['attempts']:>9.1f} "
              f"{top['crit_amp']:>8.2f}%")
        if best is None or top["mean"] < best[1]:
            best = (name, top["mean"])
    print()
    print(f"  cheapest to level {max_level}: '{best[0]}' at {best[1]:,.0f} diamonds")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--strategy", choices=sorted(STRATEGIES), default="lookahead")
    parser.add_argument("--picker", choices=sorted(PICKERS), default=None,
                        help="override the strategy's own picker")
    parser.add_argument("--patient", action="store_const", const=True, default=None,
                        help="summon until the wanted relic is affordable instead "
                             "of attempting whatever is ready")
    parser.add_argument("--runs", type=int, default=2000)
    parser.add_argument("--seed", type=int, default=20260922)
    parser.add_argument("--max-level", type=int, default=MAX_LEVEL)
    parser.add_argument("--start-stock", type=int, default=0,
                        help="relics of EACH type already on hand before the first "
                             "summon (default 0)")
    parser.add_argument("--compare", action="store_true",
                        help="score every strategy against each other")
    parser.add_argument("--chart", action="store_true",
                        help="emit the cost-by-level chart as inline SVG")
    parser.add_argument("--markdown", metavar="PATH",
                        help="write the whole LEVELS.md document to PATH")
    parser.add_argument("--lookahead", action="store_true",
                        help="search for the cheapest plan to --max-level overall "
                             "(starts from the greedy plan; a few minutes)")
    parser.add_argument("--greedy", action="store_true",
                        help="search for the plan that makes each level-up as cheap "
                             "as possible on its own, ignoring later levels")
    args = parser.parse_args()

    if args.markdown:
        with open(args.markdown, "w", encoding="utf-8") as fh:
            fh.write(markdown(args.runs if args.runs != 2000 else 20000, args.seed))
        print(f"wrote {args.markdown}")
        return 0
    if args.lookahead:
        print(f"Look-ahead search: cheapest plan to level {args.max_level} overall")
        _plan, report = greedy_search(max_level=args.max_level)
        choices, cost = lookahead_search({r["level"]: r["choice"] for r in report},
                                         max_level=args.max_level,
                                         log=lambda s: print("  " + s, flush=True))
        for level, choice in sorted(choices.items()):
            print(f"  {level:>3} | {choice.label}")
        print(f"  mean to level {args.max_level}: {cost:,.0f}")
        print(f"  {build_plan(choices)}")
        return 0
    if args.greedy:
        print("Greedy search: each level-up made as cheap as it can be on its own")
        _plan, report = greedy_search(max_level=args.max_level, log=None)
        total = 0.0
        for row in report:
            total += row["cost"]
            alt = row["runner_up"]
            alt_text = f"   next best: {alt[0].label} {human(alt[1])}" if alt else ""
            print(f"  {row['level']:>3} | {human(row['cost']):>6} (total {human(total):>6}) | "
                  f"{row['choice'].label}{alt_text}")
        return 0
    if args.compare:
        compare(args.runs, args.seed, args.max_level)
        return 0
    rows, _states = run(args.strategy, args.runs, args.seed, args.max_level,
                        args.picker, args.patient, args.start_stock)
    if args.chart:
        print(chart_svg(rows))
        return 0
    print_table(rows, args.strategy, args.runs)
    plan = STRATEGIES[args.strategy]
    print()
    print(f"  the plan, level by level  ('{args.strategy}')")
    print(f"  every step: {describe_habits(plan)}")
    for lvl in range(2, args.max_level + 1):
        print(f"  {lvl:>3} | {describe_step(plan, lvl)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
