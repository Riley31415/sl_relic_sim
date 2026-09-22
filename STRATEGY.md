# Relic inheritance — optimal strategy at inheritor level 1

## Running it

```
python main.py --level 1                  # exact solve, distribution, policy, trade-offs
python main.py --level 7 --target 5 1     # chase an all-or-nothing target
python main.py --level 20 --amp-table     # P(combination or better) grid
python main.py --strategies --target 4 2  # weighted vs target, every level
python main.py --mc 200000                # cross-check the exact numbers with rollouts
python main.py --play 3                   # sample attempts, move by move
python main.py --all-levels               # summary table, levels 1-20
python heuristics.py                      # hand-written rules vs the optimum (level 7)
python test_relic.py                      # 35 checks
```

`simulate(level)` in `relic.py` is the one-argument entry point: it plays a single
attempt under the optimal policy.

## The objective

**Maximise glory successes, minimise despair successes.** Both bars are reported
as *successes*, so glory wants a high number and despair wants a low one.

Note that minimising despair successes and maximising despair fails are the same
optimisation — they differ only by the constant `w_despair * slots` — so the
policy below is identical under either framing. Only the reported number changes.

The single combined figure, "good slots", is glory successes plus the despair
slots that did *not* come up success, on a 0–10 scale at level 1.

## What the solver does

Not a Monte Carlo sampler. The whole reachable state space at level 1 is only
~49,000 states, so `relic.py` solves it **exactly** by expectimax: every branch is
enumerated and weighted by its probability. The numbers below are exact, not
estimates. A 200,000-run Monte Carlo agrees to three decimals, which is what
`--mc` is for.

State is `(glory attempted, glory successes, despair attempted, despair
successes, mental strength, spirit power, rate)`. Every action either takes a
slot or spends mental strength, so the graph is acyclic and one backward pass
solves it.

## Headline numbers (level 1)

| | |
|---|---|
| Expected glory successes | **3.4583 / 5** (69.2% — higher is better) |
| Expected despair successes | **2.1060 / 5** (42.1% — **lower** is better) |
| Expected good slots | 6.3524 / 10 |
| Chance of a dead-end wipe | **0.25%** |
| Conditional on not wiping | 3.4670 glory, 2.0987 despair successes |
| P(4 or 5 glory successes) | 49.9% |
| P(3+ glory and 2 or fewer despair successes) | 62.1% |

The glory bar finishes 69.2% good; the despair bar only 57.9% good. That gap comes
entirely from the **starting tier**: you open at 80%, which is glory's best rate
and despair's worst. It is exactly mirrored — start the attempt at 20% instead and
despair becomes the strong bar (1.55 despair successes against 2.85 glory
successes). The head start is worth about 0.56 slots and you never fully give it
back.

## The core idea: the two bars are opposites, so alternate them

The success ladder (80/65/50/35/20) is a **single shared resource**. Every roll
moves it: success steps it down, failure steps it up.

- Glory wants a **high** rate — it wants its rolls to succeed.
- Despair wants a **low** rate — it wants its rolls to fail.
- A glory success pushes the rate **down**, into despair's good zone.
- A despair failure pushes the rate **up**, into glory's good zone.

So the two bars refill each other's ladder. The engine of the whole strategy is:

> Play glory while the rate is high. Its successes drag the rate down to where
> despair rolls are likely to fail. Play despair there. Those failures drag the
> rate back up into glory's zone. Repeat.

Playing one bar out and then the other — the obvious approach — wrecks this and
scores 5.69 instead of 6.35. That is the single most expensive mistake available.

What optimal play chooses, by tier, with **both bars still open**:

| Rate | Choice |
|---|---|
| 80% | **attempt glory** |
| 65% | **attempt glory** |
| 50% | **mental training** |
| 35% | **attempt despair** |
| 20% | **attempt despair** |

Clean bands, with 50% as a neutral pivot that belongs to neither bar.

## Mental training is a rate lever, not just a fuel pump

This is the least obvious part, and it is worth more than it looks.

Mental training is the **only action that does not consume a slot**. That makes
it a free reroll of the rate. At a bad rate a mental training *failure* is
actively good for you: it costs nothing but mental strength and steps the rate
back up the ladder.

The effect is strong enough that mental training is correct even when spirit
power is irrelevant. With one glory slot left, the despair bar finished, and spare mental
strength and spirit power (figures are expected good slots):

| Rate | Attempt the slot | Mental training instead |
|---|---|---|
| 80% | **6.800** | 6.680 |
| 65% | **6.650** | 6.605 |
| 50% | 6.500 | **6.523** |
| 35% | 6.350 | **6.437** |
| 20% | 6.200 | **6.380** |

At 20% you are better off burning mental strength than committing a glory slot
at a 20% rate — an 80% chance the mental training fails and hands you 35% free.

This gives the **stall rule**, and it is neatly symmetric around the 50% pivot:

- Only glory left: attempt glory at 80/65/50, and take **mental training** at
  35/20 rather than spend a slot at a rate that low.
- Only despair left: attempt despair at 50/35/20, and take **mental training**
  at 80/65 rather than spend a slot at a rate that high.

Once one bar is finished, mental strength has no other job, so spend it rather
than commit a slot at a rate that suits the wrong bar. This is worth +0.22 over
the same rule without it.

If mental training did *not* move the ladder, the score would drop from 6.35 to 5.86 and
this entire section would disappear. See assumption 2.

## The fuel math, and why the wipe is a non-issue at level 1

Completing both bars costs **10 spirit power**. You start with **8**. Mental
training gives +2 but cannot overcap, so it is only worth full value at 6 spirit
power or below.

> You need **1 successful mental training taken at 6 or fewer spirit power**,
> out of 5 mental strength.

One. That is why the wipe rate is only 0.25% — you would have to fail five
mental trainings in a row, and each failure makes the next one easier. Optimal
play still spends 4.55 of its 5 mental strength, but for rate control, not fuel.

The only ways to actually wipe:

1. **Mental training at 7 or 8 spirit power**, where the +2 is partly or wholly
   wasted against the cap. At 8 it gains you nothing at all.
2. **Leaving it until the rate has been dragged to 20%**, where mental trainings
   start failing in clusters.

Neither is easy to do by accident if you follow the rule below.

### "Absolutely avoid the wipe" — what it costs

An incomplete attempt inherits nothing: **0 glory successes and 0 despair
successes**, which amplifies to the 0% floor.

| Objective | E[glory] | E[despair succ] | Good slots | P(wipe) |
|---|---|---|---|---|
| maximise score (default) | 3.4583 | 2.1060 | 6.3524 | 0.250% |
| wipe penalty 10 | 3.4580 | 2.1058 | 6.3523 | 0.236% |
| wipe penalty 30 | 3.4314 | 2.1037 | 6.3277 | 0.122% |
| minimise wipe first | 3.3407 | 2.1221 | 6.2186 | 0.084% |

**The wipe cannot be eliminated** — 0.084% is a hard floor, since every mental
training can fail. But at level 1 this dial barely matters: even the paranoid setting costs
only 0.13 points. Run `--wipe-penalty 30` if you want the wipe rate halved for
0.4% of the score; the default is fine either way. (At level 8 the same dial
matters considerably more — the wipe rate there is 3.5%.)

## A rule you can actually play — tuned at level 7

The optimal policy is a 49,000-entry lookup table. This five-line rule gets
**97.5% of it** at level 7 (7.7518 vs 7.9501):

1. **Out of spirit power?** Mental training. (forced)
2. **Fuel:** if missing at least 2 spirit power, total glory + despair slots
   remaining exceed spirit power, **and the rate is 50% or better** — mental
   training.
3. **Both bars open:** attempt glory at 80/65, attempt despair at 50/35/20.
4. **Only glory left:** attempt glory at 80/65/50, otherwise mental training.
5. **Only despair left:** attempt despair at 50/35/20, otherwise mental training.

"Missing at least 2 spirit power" is just the readable form of "the +2 will not
be wasted": spirit power is hard-capped at the maximum (8 at levels 1-4, 9 from
level 5), so a mental training taken at 1 short returns only +1, and at the cap
returns nothing at all.

Note there is no middle mental training band any more: every rate belongs to a
bar, and all mental training comes from the fuel clause and the two stall
clauses.

### Why level 7 is the tuning baseline

Level 1 **cannot exercise the rule**. You start there with 8 spirit power against
10 slots (a deficit of 2), so a single mental training always closes the gap and
the fuel clause never fires: measured at **0.00% of decisions**. Tuning against
level 1 was tuning dead code, which is how the earlier version ended up with a
rate-blind fuel clause that is actively harmful from level 4 up.

Level 7 starts 3 short (12 slots, 9 spirit power), so every clause is live. The
fuel clause decides 8.7% of decisions at level 4 and 12.7% at level 8.

### How the pieces earn their keep (level 7)

| Rule | E[glory] | E[despair succ] | Good slots | vs optimal | P(wipe) |
|---|---|---|---|---|---|
| glory bar first, then despair bar | 3.7900 | 2.7712 | 7.0188 | -0.93 | 2.86% |
| tier split only | 3.9403 | 2.6236 | 7.3167 | -0.63 | 7.20% |
| + fuel rule | 3.9873 | 2.4141 | 7.5732 | -0.38 | 2.75% |
| old rule, tuned at level 1 | 4.0199 | 2.3674 | 7.6525 | -0.30 | 4.00% |
| **full rule, tuned at level 7** | **4.1015** | **2.3497** | **7.7518** | **-0.20** | **0.94%** |

### Where the fuel clause is gated — this matters a lot

Mental training is a roll on the same ladder as everything else. At 20% it fails
four times in five, so buying fuel at a bad rate just burns mental strength for
nothing. Gating it produces a clean interior optimum at 50%:

| Fuel mental training fires | Good slots | P(wipe) |
|---|---|---|
| never | 7.6026 | 3.58% |
| only at 80% | 7.5103 | 3.00% |
| at 65% or better | 7.5556 | 1.64% |
| **at 50% or better** | **7.7518** | **0.94%** |
| at 35% or better | 7.7043 | 1.38% |
| at any rate (old behaviour) | 7.6794 | 1.60% |

Being *pickier* about when to take mental training **lowers** the wipe rate, which
is the counterintuitive part. Mental training at 80% converts mental strength into
fuel at 80% efficiency; at 20% it is 20%. Restricting the clause conserves mental
strength, so you end up with more total fuel, not less.

The glory end of the split is the other sensitive parameter — attempting glory
only at 80% scores 7.02 against 7.75 for glory at 80/65. The despair end and the
two stall thresholds barely move anything.

### The same rule across every level

| lvl | optimal | old rule (tuned @ L1) | new rule (tuned @ L7) |
|---|---|---|---|
| 1 | 6.3524 (0.25%) | 6.3104 −0.042 (0.59%) | 6.1698 −0.183 (**0.09%**) |
| 2 | 6.4473 (0.26%) | 6.4040 −0.043 (0.64%) | 6.2631 −0.184 (**0.09%**) |
| 3 | 6.5248 (0.25%) | 6.4815 −0.043 (0.61%) | 6.3494 −0.175 (**0.09%**) |
| 4 | 7.7324 (1.27%) | 7.1430 −0.589 (7.77%) | **7.5402 −0.192** (0.94%) |
| 5 | 7.7473 (1.42%) | 7.4655 −0.282 (3.82%) | **7.5402 −0.207** (0.94%) |
| 6 | 7.8537 (1.46%) | 7.5599 −0.294 (4.06%) | **7.6442 −0.210** (0.97%) |
| 7 | 7.9501 (1.43%) | 7.6525 −0.298 (4.00%) | **7.7518 −0.198** (0.94%) |
| 8 | 8.9726 (3.49%) | 7.6902 −1.282 (16.99%) | **8.7411 −0.231** (3.95%) |

The level-7 rule is within −0.18 to −0.23 at *every* level, where the old rule
collapses at 4 and 8. Its wipe rate is below the optimal policy's own at levels
1–7.

The one honest trade-off: at levels 1–3 the old rule scores better (−0.04 against
−0.18), because the level-7 rule spends extra mental trainings on safety that a
level-1 board does not need. It buys a 6–7x lower wipe rate for that. From level 4 up the
new rule wins on both counts at once.

## The objective barely matters

Worth knowing before you tune weights: whether you value glory 1:1, 2:1 or even
3:1 against despair produces an **identical** policy and identical numbers at
level 1 (3.4583 glory / 2.1060 despair successes in every case). The bars are
attempted in the order the ladder dictates, not the order your preferences say,
so there is no glory/despair trade-off dial to turn. Tune the wipe penalty
instead.

## All known levels

| lvl | slots | max SP | glory% | desp% | E[glory] | E[desp succ] | Good slots | per slot | P(wipe) |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 5 | 8 | +0% | +0% | 3.4583 | 2.1060 | 6.3524 | 63.5% | 0.25% |
| 2 | 5 | 8 | +2% | +0% | 3.5389 | 2.0917 | 6.4473 | 64.5% | 0.26% |
| 3 | 5 | 8 | +2% | -2% | 3.5457 | 2.0209 | 6.5248 | 65.2% | 0.24% |
| 4 | 6 | 8 | +2% | -2% | 4.1138 | 2.3813 | 7.7324 | 64.4% | 1.27% |
| 5 | 6 | 9 | +2% | -2% | 4.1214 | 2.3741 | 7.7473 | 64.6% | 1.42% |
| 6 | 6 | 9 | +4% | -2% | 4.2099 | 2.3562 | 7.8537 | 65.4% | 1.46% |
| 7 | 6 | 9 | +4% | -4% | 4.2251 | 2.2750 | 7.9501 | 66.3% | 1.43% |
| 8 | 7 | 9 | +4% | -4% | 4.6309 | 2.6583 | 8.9726 | 64.1% | 3.49% |

Despair successes rise with level only because there are more despair slots to
attempt; the "per slot" column is the like-for-like comparison.

What each level is worth, in expected good slots:

| Level | Gives | Gain |
|---|---|---|
| 2 | +2% glory | +0.09 |
| 3 | -2% despair | +0.08 |
| **4** | **6th memory slot** | **+1.21** |
| 5 | max spirit power 9 | +0.01 |
| 6 | +2% glory | +0.11 |
| 7 | -2% despair | +0.10 |
| **8** | **7th memory slot** | **+1.02** |

**Memory slots are the only bonus that matters.** Levels 4 and 8 are worth +2.23
between them; the other six levels combined are worth +0.39 — roughly twelve times
the value of any single rate modifier. The "per slot" column shows why: quality
per slot stays flat at about 64–66% whatever you do, so the only real lever on
total output is having more slots.

The +1 max spirit power at level 5 is worth **+0.01** — effectively nothing, since
8 spirit power already covers 10 slots with one mental training. It would have
been a big deal on a 7 spirit power base; on an 8 base it is dead weight. (It does not even
cut the wipe rate, because the solver spends the slack on score instead.)

## Two objectives

`relic.py` offers exactly two, and they answer different questions.

### weighted(w_glory, w_despair) — the default

Maximise the relic's **amplification**:

> amplification = **+`w_glory`% per glory success, -`w_despair`% per despair
> success**

with 5 and 2 as the defaults, so **+5% per glory, -2% per despair**, and
**floored at zero** - a relic never comes out worse than no relic. Every slot
pays at its own rate and there is no threshold to clear, which makes this the
general-purpose objective. At level 20 it runs from 0% (a wipe, or any run
washed out below the floor) to 50% (a full glory bar
with no despair success).

A despair success costs 2%, a glory success pays 5%, so **five despair successes
cancel exactly two glory successes** - until the floor bites, below which nothing
counts at all.

The floor has two consequences worth knowing:

**Expected amplification is not `5% x E[glory] - 2% x E[despair]`.**
`max(0, ...)` is non-linear, so the average has to be summed outcome by outcome.
The linear shortcut under-states the true figure.

**It makes optimal play more willing to risk a wipe.** A wipe used to cost
-20%; now it costs 0%, so dodging one is worth less.
The wipe rate rises across every level as a direct result:

| lvl | wipe, unfloored | wipe, floored | change |
|---|---|---|---|
| 1 | 0.36% | 0.36% | -0% |
| 4 | 2.09% | 2.09% | -0% |
| 8 | 4.95% | 4.95% | -0% |
| 12 | 2.55% | 2.55% | -0% |
| 16 | 4.47% | 4.47% | -0% |
| 20 | 2.35% | 2.35% | -0% |

If that is not what you want, `--wipe-penalty` still charges a wipe extra on top
of the floor.

### target(glory, despair) — all or nothing

Maximise `P(at least this many glory successes AND at most this many despair
successes)`. Every outcome outside the box is equally worthless — there is no
partial credit for landing near it — so the solver will chase the box even when
that costs amplification and raises the wipe rate.

```
python main.py --level 7                    # weighted, the default
python main.py --level 7 --target 5 1       # chase 5+ glory and 1 or fewer despair
python main.py --level 20 --amp-table       # the grid below
python main.py --level 20 --amp-table --html
python main.py --strategies --target 4 2    # both, every level
```

Both kinds report amplification, so they stay comparable on one scale.

### Is chasing a box worth it?

Weighted play against `target(4, 2)`, both measured on the same box:

| lvl | E[glory] | E[despair] | E[amp] | hits (4,2) | E[amp] chasing | hits chasing |
|---|---|---|---|---|---|---|
| 1 | 3.459 | 2.091 | 13.12% | 37.25% | 12.45% | 37.47% |
| 2 | 3.539 | 2.076 | 13.55% | 40.33% | 12.91% | 40.56% |
| 3 | 3.547 | 2.008 | 13.73% | 42.18% | 13.10% | 42.44% |
| 4 | 4.124 | 2.276 | 16.07% | 48.01% | 15.27% | 48.48% |
| 5 | 4.128 | 2.270 | 16.10% | 48.29% | 15.25% | 48.64% |
| 6 | 4.217 | 2.251 | 16.58% | 50.58% | 15.70% | 51.05% |
| 7 | 4.231 | 2.172 | 16.81% | 53.68% | 15.98% | 54.24% |
| 8 | 4.657 | 2.379 | 18.53% | 45.89% | 17.15% | 47.30% |
| 9 | 4.997 | 2.507 | 19.97% | 46.71% | 18.00% | 48.10% |

Chasing the box buys between 0.2 and 1.4 points of hit rate, and pays 0.7 to 1.9
percentage points of amplification for it - roughly a tenth of the total at the
higher levels. Unless you need that exact result, weighted play is the better
default.

## The amplification table (level 20)

**The chance of finishing on exactly that many glory and despair successes.**
The grid sums to 100%. Glory increases left to right, despair **decreases** top
to bottom, so the best corner is the bottom right.

Cells are shaded **relative to the most likely cell** - green there, light red at
zero - because no single combination comes close to 100% on its own; the peak is
14.1%. Anything too small to show at two decimals
reads as a plain 0%. The axes carry their own shading: the glory
header deepens white to **gold** as the count rises, the despair header white to
**dark red**, so the corner you want is the gold end of the top row crossed with
the white end of the side column.

Level 20: 10 slots per bar, 12 max spirit power,
glory +10%, despair -10%.
Expected amplification **31.15%** from 7.37 glory
and 2.86 despair successes, with a
2.35% wipe rate.

<table style="border-collapse:collapse;font-size:13px;">
<thead><tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;color:#000000;background:#ffffff;">despair &darr; / glory &rarr;</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ffffff;color:#000000;">0</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#fbf7eb;color:#000000;">1</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#f6efd7;color:#000000;">2</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#f2e7c3;color:#000000;">3</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#eedfaf;color:#000000;">4</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ead79b;color:#000000;">5</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#e5cf87;color:#000000;">6</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#e1c773;color:#000000;">7</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ddbf5f;color:#000000;">8</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#d8b74b;color:#000000;">9</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#d4af37;color:#000000;">10</th></tr></thead>
<tbody>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#8b0000;color:#000000;">10</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="5 glory, 10 despair - amplifies 5%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="6 glory, 10 despair - amplifies 10%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="7 glory, 10 despair - amplifies 15%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="8 glory, 10 despair - amplifies 20%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="9 glory, 10 despair - amplifies 25%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="10 glory, 10 despair - amplifies 30%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#971a1a;color:#000000;">9</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 9 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 9 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 9 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 9 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 9 despair - amplifies 2%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="5 glory, 9 despair - amplifies 7%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="6 glory, 9 despair - amplifies 12%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="7 glory, 9 despair - amplifies 17%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="8 glory, 9 despair - amplifies 22%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="9 glory, 9 despair - amplifies 27%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="10 glory, 9 despair - amplifies 32%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#a23333;color:#000000;">8</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 8 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 8 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 8 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 8 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 8 despair - amplifies 4%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="5 glory, 8 despair - amplifies 9%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="6 glory, 8 despair - amplifies 14%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="7 glory, 8 despair - amplifies 19%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="8 glory, 8 despair - amplifies 24%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="9 glory, 8 despair - amplifies 29%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="10 glory, 8 despair - amplifies 34%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ae4c4c;color:#000000;">7</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 7 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 7 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 7 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 7 despair - amplifies 1%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 7 despair - amplifies 6%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="5 glory, 7 despair - amplifies 11%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="6 glory, 7 despair - amplifies 16%">0.01%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="7 glory, 7 despair - amplifies 21%">0.01%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="8 glory, 7 despair - amplifies 26%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="9 glory, 7 despair - amplifies 31%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="10 glory, 7 despair - amplifies 36%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#b96666;color:#000000;">6</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 6 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 6 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 6 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 6 despair - amplifies 3%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 6 despair - amplifies 8%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb8;color:#000000;" title="5 glory, 6 despair - amplifies 13%">0.05%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7bdb6;color:#000000;" title="6 glory, 6 despair - amplifies 18%">0.22%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7bdb5;color:#000000;" title="7 glory, 6 despair - amplifies 23%">0.24%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb8;color:#000000;" title="8 glory, 6 despair - amplifies 28%">0.07%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="9 glory, 6 despair - amplifies 33%">0.01%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="10 glory, 6 despair - amplifies 38%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#c58080;color:#000000;">5</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 5 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 5 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 5 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 5 despair - amplifies 5%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 5 despair - amplifies 10%">0.02%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7bdb5;color:#000000;" title="5 glory, 5 despair - amplifies 15%">0.24%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f9b8a4;color:#000000;" title="6 glory, 5 despair - amplifies 20%">1.39%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fbb292;color:#000000;" title="7 glory, 5 despair - amplifies 25%">2.59%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f9b9a7;color:#000000;" title="8 glory, 5 despair - amplifies 30%">1.17%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7bdb7;color:#000000;" title="9 glory, 5 despair - amplifies 35%">0.13%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="10 glory, 5 despair - amplifies 40%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#d19999;color:#000000;">4</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 4 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 4 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 4 despair - amplifies 2%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 4 despair - amplifies 7%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb8;color:#000000;" title="4 glory, 4 despair - amplifies 12%">0.05%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f8bbb0;color:#000000;" title="5 glory, 4 despair - amplifies 17%">0.57%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fcaf85;color:#000000;" title="6 glory, 4 despair - amplifies 22%">3.56%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#cbe98d;color:#000000;" title="7 glory, 4 despair - amplifies 27%">9.10%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fafdba;color:#000000;" title="8 glory, 4 despair - amplifies 32%">7.23%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f9b8a4;color:#000000;" title="9 glory, 4 despair - amplifies 37%">1.39%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb8;color:#000000;" title="10 glory, 4 despair - amplifies 42%">0.05%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#dcb2b2;color:#000000;">3</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 3 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 3 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 3 despair - amplifies 4%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 3 despair - amplifies 9%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb8;color:#000000;" title="4 glory, 3 despair - amplifies 14%">0.04%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f8bbb0;color:#000000;" title="5 glory, 3 despair - amplifies 19%">0.59%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fcb98c;color:#000000;" title="6 glory, 3 despair - amplifies 24%">4.01%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#67bc5e;color:#000000;" title="7 glory, 3 despair - amplifies 29%">12.2%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#1a9850;color:#000000;" title="8 glory, 3 despair - amplifies 34%">14.1%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fdcf9c;color:#000000;" title="9 glory, 3 despair - amplifies 39%">4.95%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f8bcb4;color:#000000;" title="10 glory, 3 despair - amplifies 44%">0.36%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#e8cccc;color:#000000;">2</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 2 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 2 despair - amplifies 1%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 2 despair - amplifies 6%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 2 despair - amplifies 11%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 2 despair - amplifies 16%">0.02%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7bdb5;color:#000000;" title="5 glory, 2 despair - amplifies 21%">0.25%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fab59c;color:#000000;" title="6 glory, 2 despair - amplifies 26%">1.92%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fffabb;color:#000000;" title="7 glory, 2 despair - amplifies 31%">6.83%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#b1de75;color:#000000;" title="8 glory, 2 despair - amplifies 36%">10.1%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fedba5;color:#000000;" title="9 glory, 2 despair - amplifies 41%">5.47%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f8bbae;color:#000000;" title="10 glory, 2 despair - amplifies 46%">0.73%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#f3e6e6;color:#000000;">1</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="0 glory, 1 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 1 despair - amplifies 3%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 1 despair - amplifies 8%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 1 despair - amplifies 13%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 1 despair - amplifies 18%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb8;color:#000000;" title="5 glory, 1 despair - amplifies 23%">0.04%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f8bcb4;color:#000000;" title="6 glory, 1 despair - amplifies 28%">0.36%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f9b7a2;color:#000000;" title="7 glory, 1 despair - amplifies 33%">1.54%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fbb18e;color:#000000;" title="8 glory, 1 despair - amplifies 38%">2.86%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fab59a;color:#000000;" title="9 glory, 1 despair - amplifies 43%">2.07%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f8bcb2;color:#000000;" title="10 glory, 1 despair - amplifies 48%">0.43%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ffffff;color:#000000;">0</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fab396;color:#000000;" title="0 glory, 0 despair - amplifies 0%">2.35%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="1 glory, 0 despair - amplifies 5%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="2 glory, 0 despair - amplifies 10%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="3 glory, 0 despair - amplifies 15%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="4 glory, 0 despair - amplifies 20%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="5 glory, 0 despair - amplifies 25%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb9;color:#000000;" title="6 glory, 0 despair - amplifies 30%">0.02%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb7;color:#000000;" title="7 glory, 0 despair - amplifies 35%">0.11%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7bdb5;color:#000000;" title="8 glory, 0 despair - amplifies 40%">0.25%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7bdb5;color:#000000;" title="9 glory, 0 despair - amplifies 45%">0.24%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f7beb8;color:#000000;" title="10 glory, 0 despair - amplifies 50%">0.07%</td></tr>
</tbody></table>

<sub>The shading needs a renderer that keeps inline styles — the VS Code preview
does, GitHub strips them and shows the numbers unshaded. All text in the grid is
black; on the two darkest despair headers (10 and 9) that sits below the
usual contrast floor, so read those row labels against the row order rather than
the ink.</sub>

Two things to read off it:

Note the bottom-left cell: **an incomplete attempt records as 0 glory and 0
despair**, since nothing is inherited. Despair decreases downward, so that puts
it at the bottom of the glory-0 column rather than in the all-despair corner at
the top. Both amplify to 0% either way, so the solver cannot tell
them apart and the policy is identical - but recording it at the origin keeps
E[despair successes] honest, because a failed attempt did not actually inflict
10 despair successes on you.

**Almost all the mass sits in a handful of cells.** 19 of the
121 combinations carry more than 1% each, and together they account
for 94.8% of attempts. The rest of the grid is real but rare -
75 of the cells round to 0% - which is why it reads as a pale field.
The light red end is deliberate: a saturated red across that many cells would
read as a heavy block rather than as "this barely happens".

**Equal-amplification cells lie on diagonals, but they are not equally likely.**
(8 glory, 4 despair) and (6 glory, 0 despair) both amplify 32%, yet the first
happens 7.23% of the time and the second
0.02%. The payoff is the same; the odds are not.

The same cells, as amplification values:

| despair \ glory | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **10** | 0% | 0% | 0% | 0% | 0% | 5% | 10% | 15% | 20% | 25% | 30% |
| **9** | 0% | 0% | 0% | 0% | 2% | 7% | 12% | 17% | 22% | 27% | 32% |
| **8** | 0% | 0% | 0% | 0% | 4% | 9% | 14% | 19% | 24% | 29% | 34% |
| **7** | 0% | 0% | 0% | 1% | 6% | 11% | 16% | 21% | 26% | 31% | 36% |
| **6** | 0% | 0% | 0% | 3% | 8% | 13% | 18% | 23% | 28% | 33% | 38% |
| **5** | 0% | 0% | 0% | 5% | 10% | 15% | 20% | 25% | 30% | 35% | 40% |
| **4** | 0% | 0% | 2% | 7% | 12% | 17% | 22% | 27% | 32% | 37% | 42% |
| **3** | 0% | 0% | 4% | 9% | 14% | 19% | 24% | 29% | 34% | 39% | 44% |
| **2** | 0% | 1% | 6% | 11% | 16% | 21% | 26% | 31% | 36% | 41% | 46% |
| **1** | 0% | 3% | 8% | 13% | 18% | 23% | 28% | 33% | 38% | 43% | 48% |
| **0** | 0% | 5% | 10% | 15% | 20% | 25% | 30% | 35% | 40% | 45% | 50% |

## What a single attempt pays (level 20)

The chance of finishing on **exactly** each amplification, under optimal weighted
play. One bar per achievable value; hover any bar for its exact figure.

<svg viewBox="0 0 519 272" width="519" height="272" role="img" xmlns="http://www.w3.org/2000/svg" aria-label="Distribution of relic amplification at inheritor level 20">
<title>Amplification distribution, inheritor level 20</title>
<desc>Chance of each exact amplification under optimal weighted play. Mean 31.15 percent. Peak 14.1 percent of attempts at an amplification of 34 percent.</desc>
<style>.ac-bar{fill:#2a78d6}.ac-wipe{fill:#d03b3b}.ac-grid{stroke:#e1e0d9;stroke-width:1}.ac-axis{stroke:#c3c2b7;stroke-width:1}.ac-ink{fill:#52514e}.ac-muted{fill:#898781}.ac-mean{stroke:#52514e}.ac-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}.ac-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}@media(prefers-color-scheme:dark){.ac-bar{fill:#3987e5}.ac-grid{stroke:#2c2c2a}.ac-axis{stroke:#383835}.ac-ink{fill:#c3c2b7}.ac-mean{stroke:#c3c2b7}}</style>
<line class="ac-grid" x1="46" y1="236.0" x2="505" y2="236.0"/>
<text class="ac-t ac-muted" x="38" y="240.0" text-anchor="end">0%</text>
<line class="ac-grid" x1="46" y1="166.0" x2="505" y2="166.0"/>
<text class="ac-t ac-muted" x="38" y="170.0" text-anchor="end">5%</text>
<line class="ac-grid" x1="46" y1="96.0" x2="505" y2="96.0"/>
<text class="ac-t ac-muted" x="38" y="100.0" text-anchor="end">10%</text>
<line class="ac-grid" x1="46" y1="26.0" x2="505" y2="26.0"/>
<text class="ac-t ac-muted" x="38" y="30.0" text-anchor="end">15%</text>
<text class="ac-t ac-muted" x="38" y="17" text-anchor="end">chance</text>
<path class="ac-wipe" d="M47.0,236 V206.1 Q47.0,203.1 50.0,203.1 H51.0 Q54.0,203.1 54.0,206.1 V236 Z"><title>amplification 0: 2.35%</title></path>
<path class="ac-bar" d="M56.0,236 V236.0 Q56.0,236.0 56.0,236.0 H63.0 Q63.0,236.0 63.0,236.0 V236 Z"><title>amplification 1: 0%</title></path>
<path class="ac-bar" d="M65.0,236 V236.0 Q65.0,236.0 65.0,236.0 H72.0 Q72.0,236.0 72.0,236.0 V236 Z"><title>amplification 2: 0%</title></path>
<path class="ac-bar" d="M74.0,236 V236.0 Q74.0,236.0 74.0,236.0 H81.0 Q81.0,236.0 81.0,236.0 V236 Z"><title>amplification 3: 0%</title></path>
<path class="ac-bar" d="M83.0,236 V236.0 Q83.0,236.0 83.0,236.0 H90.0 Q90.0,236.0 90.0,236.0 V236 Z"><title>amplification 4: 0%</title></path>
<path class="ac-bar" d="M92.0,236 V236.0 Q92.0,236.0 92.0,236.0 H99.0 Q99.0,236.0 99.0,236.0 V236 Z"><title>amplification 5: 0%</title></path>
<path class="ac-bar" d="M101.0,236 V236.0 Q101.0,236.0 101.0,236.0 H108.0 Q108.0,236.0 108.0,236.0 V236 Z"><title>amplification 6: 0%</title></path>
<path class="ac-bar" d="M110.0,236 V236.0 Q110.0,236.0 110.0,236.0 H117.0 Q117.0,236.0 117.0,236.0 V236 Z"><title>amplification 7: 0%</title></path>
<path class="ac-bar" d="M119.0,236 V236.0 Q119.0,235.9 119.1,235.9 H125.9 Q126.0,235.9 126.0,236.0 V236 Z"><title>amplification 8: 0%</title></path>
<path class="ac-bar" d="M128.0,236 V236.0 Q128.0,236.0 128.0,236.0 H135.0 Q135.0,236.0 135.0,236.0 V236 Z"><title>amplification 9: 0%</title></path>
<path class="ac-bar" d="M137.0,236 V236.0 Q137.0,235.7 137.3,235.7 H143.7 Q144.0,235.7 144.0,236.0 V236 Z"><title>amplification 10: 0.02%</title></path>
<path class="ac-bar" d="M146.0,236 V236.0 Q146.0,235.9 146.1,235.9 H152.9 Q153.0,235.9 153.0,236.0 V236 Z"><title>amplification 11: 0.01%</title></path>
<path class="ac-bar" d="M155.0,236 V236.0 Q155.0,235.4 155.6,235.4 H161.4 Q162.0,235.4 162.0,236.0 V236 Z"><title>amplification 12: 0.05%</title></path>
<path class="ac-bar" d="M164.0,236 V236.0 Q164.0,235.3 164.7,235.3 H170.3 Q171.0,235.3 171.0,236.0 V236 Z"><title>amplification 13: 0.05%</title></path>
<path class="ac-bar" d="M173.0,236 V236.0 Q173.0,235.4 173.6,235.4 H179.4 Q180.0,235.4 180.0,236.0 V236 Z"><title>amplification 14: 0.05%</title></path>
<path class="ac-bar" d="M182.0,236 V235.7 Q182.0,232.7 185.0,232.7 H186.0 Q189.0,232.7 189.0,235.7 V236 Z"><title>amplification 15: 0.24%</title></path>
<path class="ac-bar" d="M191.0,236 V236.0 Q191.0,235.6 191.4,235.6 H197.6 Q198.0,235.6 198.0,236.0 V236 Z"><title>amplification 16: 0.03%</title></path>
<path class="ac-bar" d="M200.0,236 V231.1 Q200.0,228.1 203.0,228.1 H204.0 Q207.0,228.1 207.0,231.1 V236 Z"><title>amplification 17: 0.57%</title></path>
<path class="ac-bar" d="M209.0,236 V235.9 Q209.0,232.9 212.0,232.9 H213.0 Q216.0,232.9 216.0,235.9 V236 Z"><title>amplification 18: 0.22%</title></path>
<path class="ac-bar" d="M218.0,236 V230.7 Q218.0,227.7 221.0,227.7 H222.0 Q225.0,227.7 225.0,230.7 V236 Z"><title>amplification 19: 0.59%</title></path>
<path class="ac-bar" d="M227.0,236 V219.5 Q227.0,216.5 230.0,216.5 H231.0 Q234.0,216.5 234.0,219.5 V236 Z"><title>amplification 20: 1.39%</title></path>
<path class="ac-bar" d="M236.0,236 V235.4 Q236.0,232.4 239.0,232.4 H240.0 Q243.0,232.4 243.0,235.4 V236 Z"><title>amplification 21: 0.26%</title></path>
<path class="ac-bar" d="M245.0,236 V189.2 Q245.0,186.2 248.0,186.2 H249.0 Q252.0,186.2 252.0,189.2 V236 Z"><title>amplification 22: 3.56%</title></path>
<path class="ac-bar" d="M254.0,236 V235.1 Q254.0,232.1 257.0,232.1 H258.0 Q261.0,232.1 261.0,235.1 V236 Z"><title>amplification 23: 0.28%</title></path>
<path class="ac-bar" d="M263.0,236 V182.8 Q263.0,179.8 266.0,179.8 H267.0 Q270.0,179.8 270.0,182.8 V236 Z"><title>amplification 24: 4.01%</title></path>
<path class="ac-bar" d="M272.0,236 V202.8 Q272.0,199.8 275.0,199.8 H276.0 Q279.0,199.8 279.0,202.8 V236 Z"><title>amplification 25: 2.59%</title></path>
<path class="ac-bar" d="M281.0,236 V212.1 Q281.0,209.1 284.0,209.1 H285.0 Q288.0,209.1 288.0,212.1 V236 Z"><title>amplification 26: 1.92%</title></path>
<path class="ac-bar" d="M290.0,236 V111.6 Q290.0,108.6 293.0,108.6 H294.0 Q297.0,108.6 297.0,111.6 V236 Z"><title>amplification 27: 9.10%</title></path>
<path class="ac-bar" d="M299.0,236 V233.0 Q299.0,230.0 302.0,230.0 H303.0 Q306.0,230.0 306.0,233.0 V236 Z"><title>amplification 28: 0.43%</title></path>
<path class="ac-bar" d="M308.0,236 V68.8 Q308.0,65.8 311.0,65.8 H312.0 Q315.0,65.8 315.0,68.8 V236 Z"><title>amplification 29: 12.2%</title></path>
<path class="ac-bar" d="M317.0,236 V222.4 Q317.0,219.4 320.0,219.4 H321.0 Q324.0,219.4 324.0,222.4 V236 Z"><title>amplification 30: 1.19%</title></path>
<path class="ac-bar" d="M326.0,236 V143.4 Q326.0,140.4 329.0,140.4 H330.0 Q333.0,140.4 333.0,143.4 V236 Z"><title>amplification 31: 6.83%</title></path>
<path class="ac-bar" d="M335.0,236 V137.8 Q335.0,134.8 338.0,134.8 H339.0 Q342.0,134.8 342.0,137.8 V236 Z"><title>amplification 32: 7.23%</title></path>
<path class="ac-bar" d="M344.0,236 V217.3 Q344.0,214.3 347.0,214.3 H348.0 Q351.0,214.3 351.0,217.3 V236 Z"><title>amplification 33: 1.55%</title></path>
<path class="ac-bar" d="M353.0,236 V41.8 Q353.0,38.8 356.0,38.8 H357.0 Q360.0,38.8 360.0,41.8 V236 Z"><title>amplification 34: 14.1%</title></path>
<path class="ac-bar" d="M362.0,236 V235.7 Q362.0,232.7 365.0,232.7 H366.0 Q369.0,232.7 369.0,235.7 V236 Z"><title>amplification 35: 0.24%</title></path>
<path class="ac-bar" d="M371.0,236 V97.3 Q371.0,94.3 374.0,94.3 H375.0 Q378.0,94.3 378.0,97.3 V236 Z"><title>amplification 36: 10.1%</title></path>
<path class="ac-bar" d="M380.0,236 V219.5 Q380.0,216.5 383.0,216.5 H384.0 Q387.0,216.5 387.0,219.5 V236 Z"><title>amplification 37: 1.39%</title></path>
<path class="ac-bar" d="M389.0,236 V199.0 Q389.0,196.0 392.0,196.0 H393.0 Q396.0,196.0 396.0,199.0 V236 Z"><title>amplification 38: 2.86%</title></path>
<path class="ac-bar" d="M398.0,236 V169.7 Q398.0,166.7 401.0,166.7 H402.0 Q405.0,166.7 405.0,169.7 V236 Z"><title>amplification 39: 4.95%</title></path>
<path class="ac-bar" d="M407.0,236 V235.4 Q407.0,232.4 410.0,232.4 H411.0 Q414.0,232.4 414.0,235.4 V236 Z"><title>amplification 40: 0.26%</title></path>
<path class="ac-bar" d="M416.0,236 V162.4 Q416.0,159.4 419.0,159.4 H420.0 Q423.0,159.4 423.0,162.4 V236 Z"><title>amplification 41: 5.47%</title></path>
<path class="ac-bar" d="M425.0,236 V236.0 Q425.0,235.2 425.8,235.2 H431.2 Q432.0,235.2 432.0,236.0 V236 Z"><title>amplification 42: 0.05%</title></path>
<path class="ac-bar" d="M434.0,236 V210.0 Q434.0,207.0 437.0,207.0 H438.0 Q441.0,207.0 441.0,210.0 V236 Z"><title>amplification 43: 2.07%</title></path>
<path class="ac-bar" d="M443.0,236 V234.0 Q443.0,231.0 446.0,231.0 H447.0 Q450.0,231.0 450.0,234.0 V236 Z"><title>amplification 44: 0.36%</title></path>
<path class="ac-bar" d="M452.0,236 V235.7 Q452.0,232.7 455.0,232.7 H456.0 Q459.0,232.7 459.0,235.7 V236 Z"><title>amplification 45: 0.24%</title></path>
<path class="ac-bar" d="M461.0,236 V228.8 Q461.0,225.8 464.0,225.8 H465.0 Q468.0,225.8 468.0,228.8 V236 Z"><title>amplification 46: 0.73%</title></path>
<path class="ac-bar" d="M479.0,236 V232.9 Q479.0,229.9 482.0,229.9 H483.0 Q486.0,229.9 486.0,232.9 V236 Z"><title>amplification 48: 0.43%</title></path>
<path class="ac-bar" d="M497.0,236 V236.0 Q497.0,235.0 498.0,235.0 H503.0 Q504.0,235.0 504.0,236.0 V236 Z"><title>amplification 50: 0.07%</title></path>
<line class="ac-axis" x1="46" y1="236" x2="505" y2="236"/>
<text class="ac-t ac-muted" x="50.5" y="251" text-anchor="middle">0%</text>
<text class="ac-t ac-muted" x="140.5" y="251" text-anchor="middle">10%</text>
<text class="ac-t ac-muted" x="230.5" y="251" text-anchor="middle">20%</text>
<text class="ac-t ac-muted" x="320.5" y="251" text-anchor="middle">30%</text>
<text class="ac-t ac-muted" x="410.5" y="251" text-anchor="middle">40%</text>
<text class="ac-t ac-muted" x="500.5" y="251" text-anchor="middle">50%</text>
<text class="ac-t ac-muted" x="275.5" y="266" text-anchor="middle">amplification (+5% per glory success, -2% per despair success)</text>
<line class="ac-mean" x1="330.9" y1="20" x2="330.9" y2="236" stroke-dasharray="3 3" stroke-width="1.5"/>
<text class="ac-b ac-ink" x="335.9" y="14">mean 31.2%</text>
<text class="ac-b ac-ink" x="356.5" y="32.8" text-anchor="middle">14.1%</text>
<text class="ac-b ac-wipe" x="56.5" y="195.1" text-anchor="start">wipe 2.35%</text>
</svg>

The shape is worth reading. It is **not** a bell curve around the mean — it is a
tight hump between about 20% and 45%, with a lone red spike on the floor.

**The floor is a separate outcome, not a bad roll.** At 2.35%
of attempts it sits at 0%, cut off from the hump by a gap of roughly 15 points
that almost never gets landed in. Most of that is the incomplete attempt, which
inherits nothing at all - 0 glory and 0 despair. An attempt either works and pays somewhere in the hump, or
it bottoms out. There is very little in between — which is exactly why flooring
at zero changes the incentives: the gap, not the depth, is what the solver is
trading against.

**The mean is not the typical result.** The average is
31.2%, but the single most likely outcome is 34%
(on 14.1% of attempts), and the mean is dragged down by
that spike on the floor. 52% of attempts beat the average.

The same distribution as a table, binned by 5:

| amplification | chance | cumulative |
|---|---|---|
| 0% to 4% | 2.35% | 2.35% |
| 5% to 9% | 0.01% | 2.36% |
| 10% to 14% | 0.17% | 2.53% |
| 15% to 19% | 1.65% | 4.17% |
| 20% to 24% | 9.51% | 13.68% |
| 25% to 29% | 26.19% | 39.87% |
| 30% to 34% | 30.88% | 70.75% |
| 35% to 39% | 19.56% | 90.31% |
| 40% to 44% | 8.22% | 98.53% |
| 45% to 49% | 1.40% | 99.93% |
| 50% to 50% | 0.07% | 100.00% |

## Assumptions

These are readings of the rules that the numbers depend on. Worth confirming
against the game.

1. **The attempt starts at 80%** (the top of the ladder). This drives the
   glory-first opening and the entire glory/despair asymmetry. Starting at 50%
   instead gives 2.98 glory successes and 1.94 despair successes — the despair bar
   would end up the better of the two.
2. **Mental training rolls on the same ladder and shifts it**, as "after any of
   these 3 steps" says. Load-bearing: without it the score drops 6.35 to 5.86 and
   the stall rule disappears entirely.
3. **A "success" on despair takes the slot as a success** (bad for you) and steps
   the ladder down, same as anywhere else. The ladder reacts to the roll, not to
   whether the result helped you.
4. "All slots are filled" blocks **that bar only**, not both.
5. An incomplete attempt (the dead end) inherits nothing at all: 0 glory
   successes and 0 despair successes, discarding whatever was already
   attempted. It amplifies to the 0% floor, which is the same value the old
   "all despair slots succeed" convention gave, so the policy is unaffected.
6. Rate modifiers are additive percentage points (level 8: glory 80 to 84%).
7. The ladder clamps at both ends: failing at 80% leaves you at 80%.
