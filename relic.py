#!/usr/bin/env python3
"""Slayer Legend - relic inheritance simulator and optimal-policy solver.

One "inheritance attempt" is one simulation and takes a single argument: the
inheritor level.  Memory slots, max spirit power and the glory/despair rate
modifiers are all derived from that level.

The solver is an exact expectimax dynamic program over the whole reachable
state space, not a random sampler: every branch is enumerated and weighted by
its probability, so the expectations it reports are exact under optimal play.
Monte Carlo playouts are available (--mc) purely as a cross-check.
"""

from __future__ import annotations

import random
from collections import Counter, defaultdict
from dataclasses import dataclass

# The shared success-chance ladder.  Index 0 is the easiest tier.
# A successful roll moves one tier DOWN the list (harder next time),
# a failed roll moves one tier UP (easier next time).  Both ends clamp.
TIERS = (0.80, 0.65, 0.50, 0.35, 0.20)
BEST_TIER = 0
WORST_TIER = len(TIERS) - 1

GLORY = "attempt glory"
DESPAIR = "attempt despair"
TRAIN = "mental training"
ACTION_ORDER = (GLORY, DESPAIR, TRAIN)
DEAD_END = "dead-end"

# Level 1 is the base board.
BASE_SLOTS = 5
BASE_MAX_SPIRIT = 8

# From level 2 the bonuses repeat on a four-level cycle, keyed by level % 4:
#   2 -> +2% glory,  3 -> -2% despair,  0 -> +1 memory slot,  1 -> +1 spirit power
# which reproduces the spelled-out levels 2-9 exactly (4 and 8 give the 6th and
# 7th slot, 5 and 9 give the 9th and 10th spirit power).
LEVEL_CYCLE = {
    2: ("glory_mod", +0.02),
    3: ("despair_mod", -0.02),
    0: ("slots", +1),
    1: ("max_spirit", +1),
}

# TODO.md spells the cycle out as far as level 20.  Levels past that are the
# same cycle continued, which is an extrapolation rather than a stated rule.
DOCUMENTED_MAX_LEVEL = 20


class UnknownLevelError(ValueError):
    """Raised for inheritor levels whose bonuses we do not know yet."""


WEIGHTED = "weighted"
TARGET = "target"


@dataclass(frozen=True)
class Strategy:
    """What an attempt is trying to achieve.  Two kinds, and only two.

    weighted(w_glory, w_despair)
        Maximise the relic's amplification, w_glory * glory successes minus
        w_despair * despair successes.  Every slot counts, at its own rate;
        there is no threshold to clear.  This is the general-purpose objective.

    target(glory, despair)
        All or nothing.  Maximise the probability of finishing with at least
        `glory` glory successes AND at most `despair` despair successes.  Every
        outcome outside that box is equally worthless, so the solver will chase
        the box even when that costs it amplification.

    A target strategy still reports amplification, using the weights carried
    alongside it, so the two kinds stay comparable on one scale.
    """

    kind: str = WEIGHTED
    want_glory: int = 0        # target only: glory successes needed
    allow_despair: int = 0     # target only: despair successes tolerated
    w_glory: float = 5.0      # percentage points gained per glory success
    w_despair: float = 2.0    # percentage points lost per despair success
    label: str = ""

    def __post_init__(self):
        if self.kind not in (WEIGHTED, TARGET):
            raise ValueError(f"kind must be {WEIGHTED!r} or {TARGET!r}, got {self.kind!r}")
        if self.kind == TARGET and (self.want_glory < 0 or self.allow_despair < 0):
            raise ValueError("targets cannot be negative")

    @property
    def is_target(self) -> bool:
        return self.kind == TARGET

    @property
    def name(self) -> str:
        if self.label:
            return self.label
        if self.is_target:
            return f"target ({self.want_glory}, {self.allow_despair})"
        return f"weighted (+{self.w_glory:g}%/-{self.w_despair:g}%)"


def weighted(w_glory: float = 5.0, w_despair: float = 2.0, label: str = "") -> Strategy:
    """Maximise amplification %: +w_glory% per glory success, -w_despair% per despair."""
    return Strategy(kind=WEIGHTED, w_glory=w_glory, w_despair=w_despair, label=label)


def target(want_glory: int, allow_despair: int, **kwargs) -> Strategy:
    """All or nothing: maximise P(>= want_glory glory AND <= allow_despair despair)."""
    return Strategy(kind=TARGET, want_glory=want_glory, allow_despair=allow_despair, **kwargs)


DEFAULT_STRATEGY = weighted()


@dataclass(frozen=True)
class Config:
    """Everything one attempt needs, derived from the inheritor level."""

    level: int
    slots: int          # memory slots per bar (glory and despair each get this many)
    max_spirit: int     # spirit power cap, and the starting spirit power
    glory_mod: float    # additive modifier to the glory success chance
    despair_mod: float  # additive modifier to the despair success chance
    train_mod: float    # additive modifier to the mental training success chance
    start_tier: int     # index into TIERS at the start of the attempt
    strategy: Strategy  # what this attempt is trying to achieve
    safety_first: bool  # minimise the chance of a dead-end wipe before anything else
    wipe_penalty: float  # extra score charged for a wipe (ignored if safety_first)

    @property
    def start_mental(self) -> int:
        return self.slots

    @property
    def w_glory(self) -> float:
        return self.strategy.w_glory

    @property
    def w_despair(self) -> float:
        return self.strategy.w_despair

    @property
    def is_target(self) -> bool:
        return self.strategy.is_target

    @property
    def want_glory(self) -> int:
        """The glory target, clamped to what the bar can actually hold."""
        return min(self.strategy.want_glory, self.slots)

    @property
    def allow_despair(self) -> int:
        return min(self.strategy.allow_despair, self.slots)


def config_for_level(
    level,
    *,
    strategy: Strategy = DEFAULT_STRATEGY,
    start_tier: int = BEST_TIER,
    train_mod: float = 0.0,
    safety_first: bool = False,
    wipe_penalty: float = 0.0,
) -> Config:
    if not isinstance(level, int) or isinstance(level, bool):
        raise UnknownLevelError("inheritor level must be an integer")
    if level < 1:
        raise UnknownLevelError(f"inheritor level {level} is not a real level (1 and up)")
    totals = {"slots": BASE_SLOTS, "max_spirit": BASE_MAX_SPIRIT,
              "glory_mod": 0.0, "despair_mod": 0.0}
    for lv in range(2, level + 1):
        field, delta = LEVEL_CYCLE[lv % 4]
        totals[field] += delta
    slots = totals["slots"]
    max_spirit = totals["max_spirit"]
    glory_mod = totals["glory_mod"]
    despair_mod = totals["despair_mod"]
    return Config(
        level=level,
        slots=slots,
        max_spirit=max_spirit,
        glory_mod=glory_mod,
        despair_mod=despair_mod,
        train_mod=train_mod,
        start_tier=start_tier,
        strategy=strategy,
        safety_first=safety_first,
        wipe_penalty=wipe_penalty,
    )


# A state is a plain tuple so it is cheap as a dict key:
#   (glory_filled, glory_success, despair_filled, despair_success,
#    mental_strength, spirit_power, tier)
State = tuple


@dataclass
class Analysis:
    """Exact outcome statistics for optimal play."""

    cfg: Config
    states_explored: int
    e_amplification: float  # mean amplification %, +w_glory% / -w_despair% per slot
    e_glory: float
    e_despair_success: float  # the bad ones - lower is better
    p_dead_end: float
    p_target: float         # target strategies: share landing inside the box
    dist: Counter           # (glory_success, despair_success) -> probability
    action_counts: Counter  # action -> expected number of times taken
    action_by_tier: dict    # tier -> Counter(action -> probability mass)
    action_by_sp: dict      # (tier, spirit_power) -> Counter(action -> mass)


class Inheritance:
    """Solver for one inheritor level: optimal policy plus exact expectations."""

    def __init__(self, cfg: Config):
        self.cfg = cfg
        self._memo: dict = {}

    # ---------------------------------------------------------------- rules

    def start_state(self) -> State:
        c = self.cfg
        return (0, 0, 0, 0, c.start_mental, c.max_spirit, c.start_tier)

    def chance(self, action: str, tier: int) -> float:
        """Success chance of an action at the given tier, after modifiers."""
        base = TIERS[tier]
        if action == GLORY:
            base += self.cfg.glory_mod
        elif action == DESPAIR:
            base += self.cfg.despair_mod
        else:
            base += self.cfg.train_mod
        return min(1.0, max(0.0, base))

    def legal_actions(self, state: State) -> tuple:
        gf, _gs, df, _ds, ms, sp, _t = state
        slots = self.cfg.slots
        out = []
        if sp > 0 and gf < slots:
            out.append(GLORY)
        if sp > 0 and df < slots:
            out.append(DESPAIR)
        if ms > 0:
            out.append(TRAIN)
        return tuple(out)

    def is_terminal(self, state: State) -> bool:
        return state[0] == self.cfg.slots and state[2] == self.cfg.slots

    def transitions(self, state: State, action: str):
        """Yield (probability, next_state, roll_succeeded) for an action."""
        gf, gs, df, ds, ms, sp, t = state
        p = self.chance(action, t)
        harder = min(t + 1, WORST_TIER)  # a successful roll makes the next one harder
        easier = max(t - 1, BEST_TIER)   # a failed roll makes the next one easier
        if action == GLORY:
            yield p, (gf + 1, gs + 1, df, ds, ms, sp - 1, harder), True
            yield 1 - p, (gf + 1, gs, df, ds, ms, sp - 1, easier), False
        elif action == DESPAIR:
            yield p, (gf, gs, df + 1, ds + 1, ms, sp - 1, harder), True
            yield 1 - p, (gf, gs, df + 1, ds, ms, sp - 1, easier), False
        else:
            boosted = min(sp + 2, self.cfg.max_spirit)
            yield p, (gf, gs, df, ds, ms - 1, boosted, harder), True
            yield 1 - p, (gf, gs, df, ds, ms - 1, sp, easier), False

    def amplification(self, glory_success: int, despair_success: int) -> float:
        """The relic's amplification, as a percentage, floored at zero.

        w_glory% per glory success minus w_despair% per despair success - with
        the defaults, +5% and -2% - but never negative: a relic cannot come out
        worse than no relic, so a wipe and a merely awful result both pay 0%.

        The floor makes amplification NON-LINEAR, which matters in two places.
        Expected amplification is no longer w_glory * E[glory] - w_despair *
        E[despair] - it has to be summed over the outcome distribution, which
        is what analyse() does.  And because the floor caps the downside of a
        wipe, it makes the solver more willing to risk one.
        """
        return max(0.0, self.cfg.w_glory * glory_success
                   - self.cfg.w_despair * despair_success)

    def max_amplification(self) -> float:
        """The best amplification % the bars allow: a full glory bar, no despair."""
        return self.cfg.w_glory * self.cfg.slots

    def min_amplification(self) -> float:
        """Zero percent - the floor.  A wipe lands here, as does any washed-out run."""
        return 0.0

    def hits_target(self, glory_success: int, despair_success: int) -> bool:
        """Did this outcome land inside the target box?  Target strategies only."""
        if not self.cfg.is_target:
            return False
        return (glory_success >= self.cfg.want_glory
                and despair_success <= self.cfg.allow_despair)

    def objective(self, glory_success: int, despair_success: int) -> float:
        """What the solver steers by.

        A target strategy is strictly all or nothing: 1 inside the box, 0
        outside it, with no credit for getting close.  A weighted strategy
        steers by amplification.
        """
        if self.cfg.is_target:
            return 1.0 if self.hits_target(glory_success, despair_success) else 0.0
        return self.amplification(glory_success, despair_success)

    # ------------------------------------------------------------- solving

    def _better(self, cand, best) -> bool:
        """Compare two (p_finish, score) pairs under the configured objective.

        safety_first is a strict lexicographic order: dodge the wipe first, and
        only break ties on score.  Otherwise the two are traded off linearly,
        wipe_penalty = 0 being the plain expected-score maximiser.
        """
        if self.cfg.safety_first:
            if cand[0] > best[0] + 1e-12:
                return True
            if cand[0] < best[0] - 1e-12:
                return False
            return cand[1] > best[1] + 1e-12
        pen = self.cfg.wipe_penalty
        return cand[1] + pen * cand[0] > best[1] + pen * best[0] + 1e-12

    def value(self, state: State):
        """Solve a state exactly: (P(finish), expected score, optimal action).

        Both numbers are honest expectations under the chosen policy - the
        objective only steers which action is picked, it never contaminates
        the reported score.  Every action either takes a slot or spends mental
        strength, so potential() strictly decreases and the graph is acyclic.
        """
        hit = self._memo.get(state)
        if hit is not None:
            return hit
        _gf, gs, df, ds, _ms, _sp, _t = state
        if self.is_terminal(state):
            result = (1.0, self.objective(gs, ds), None)
            self._memo[state] = result
            return result

        legal = self.legal_actions(state)
        best = None
        for action in ACTION_ORDER:
            if action not in legal:
                continue
            p_finish = 0.0
            ev = 0.0
            for prob, nxt, _ok in self.transitions(state, action):
                if prob:
                    sub = self.value(nxt)
                    p_finish += prob * sub[0]
                    ev += prob * sub[1]
            if best is None or self._better((p_finish, ev), best):
                best = (p_finish, ev, action)

        if best is None:
            # Dead end: no spirit power, no mental strength, bars unfinished.
            # The attempt never completed, so nothing was inherited at all:
            # 0 glory successes and 0 despair successes.  That amplifies to 0,
            # the floor, and an incomplete attempt never counts as hitting a
            # target however lenient the target is.
            result = (0.0, 0.0, DEAD_END)
        else:
            result = best
        self._memo[state] = result
        return result

    def best_action(self, state: State):
        return self.value(state)[2]

    def potential(self, state: State) -> int:
        """Strictly decreases by one per action; used to order the DP layers."""
        return state[4] + (2 * self.cfg.slots - state[0] - state[2])

    # ------------------------------------------------------------ analysis

    def analyse(self, policy=None) -> Analysis:
        """Push probability mass forward through a policy, exactly.

        `policy` is called as policy(state, legal_actions) and must return one
        of the legal actions; the default is the solver's own optimal policy.
        Passing a hand-written policy scores it on exactly the same footing.
        """
        start = self.start_state()
        top = self.potential(start)
        layers = [dict() for _ in range(top + 1)]
        layers[top][start] = 1.0

        dist = Counter()
        action_counts = Counter()
        action_by_tier = defaultdict(Counter)
        action_by_sp = defaultdict(Counter)
        p_dead = 0.0

        for level in range(top, -1, -1):
            for state, mass in layers[level].items():
                _gf, gs, df, ds, _ms, sp, t = state
                if self.is_terminal(state):
                    dist[(gs, ds)] += mass
                    continue
                legal = self.legal_actions(state)
                if not legal:
                    p_dead += mass
                    dist[(0, 0)] += mass  # incomplete: nothing inherited
                    continue
                action = policy(state, legal) if policy else self.best_action(state)
                if action not in legal:
                    raise ValueError(f"policy chose illegal action {action!r} in {state}")
                action_counts[action] += mass
                action_by_tier[t][action] += mass
                action_by_sp[(t, sp)][action] += mass
                for prob, nxt, _ok in self.transitions(state, action):
                    if prob:
                        bucket = layers[self.potential(nxt)]
                        bucket[nxt] = bucket.get(nxt, 0.0) + mass * prob

        p_target = sum(p for (g, d), p in dist.items() if self.hits_target(g, d))
        if self.hits_target(0, 0):
            # incomplete attempts share the (0, 0) cell but never count as a hit
            p_target -= p_dead

        e_glory = sum(g * p for (g, _d), p in dist.items())
        e_despair = sum(d * p for (_g, d), p in dist.items())  # despair SUCCESSES
        return Analysis(
            cfg=self.cfg,
            states_explored=len(self._memo),
            # read back off the outcome distribution so a hand-written policy
            # is scored by what it actually achieves, not by the optimal value
            e_amplification=sum(self.amplification(g, d) * p for (g, d), p in dist.items()),
            p_target=p_target,
            e_glory=e_glory,
            e_despair_success=e_despair,
            p_dead_end=p_dead,
            dist=dist,
            action_counts=action_counts,
            action_by_tier=dict(action_by_tier),
            action_by_sp=dict(action_by_sp),
        )

    # ---------------------------------------------------------- simulating

    def attempt(self, rng=None, log=None) -> dict:
        """Play out one inheritance attempt following the optimal policy."""
        rng = rng or random
        state = self.start_state()
        while True:
            if self.is_terminal(state):
                _gf, gs, _df, ds, ms, sp, _t = state
                return {
                    "dead_end": False,
                    "glory_success": gs,
                    "despair_success": ds,
                    "mental_left": ms,
                    "spirit_left": sp,
                }
            action = self.best_action(state)
            if action == DEAD_END:
                return {
                    "dead_end": True,
                    "glory_success": 0,
                    "despair_success": 0,
                    "mental_left": 0,
                    "spirit_left": 0,
                }
            succeeded = rng.random() < self.chance(action, state[6])
            for _prob, nxt, ok in self.transitions(state, action):
                if ok == succeeded:
                    if log is not None:
                        log.append((state, action, succeeded, nxt))
                    state = nxt
                    break


def simulate(level: int, rng=None, **kwargs) -> dict:
    """Run a single inheritance attempt at `level` under the optimal policy."""
    return Inheritance(config_for_level(level, **kwargs)).attempt(rng)
