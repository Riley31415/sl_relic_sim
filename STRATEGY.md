# Relic inheritance - strategy

How to climb the inheritor from level 1 to any goal - a level from 2 to 20, or
level 20 plus a Demon Eye of Weakness farmed to 43, 46, 48 or 50% - and how to
play a single inheritance attempt on the way. What every level costs in
diamonds, goal by goal, is in [LEVELS.md](LEVELS.md); farming a crit relic is
in [COST.md](COST.md).

Every number about a single attempt is exact: the solver enumerates every
branch of an attempt and weights it by its probability, rather than sampling.
The climb's costs are simulated (20,000 runs).

## The board at every level

| level | memory slots | spirit power | glory rate | despair rate | starts with | first rate | average amplification | best possible | wipe chance |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 5 | 8 | +0% | +0% | - | 80% | 13.12% | 25% | 0.36% |
| 2 | 5 | 8 | +2% | +0% | - | 80% | 13.55% | 25% | 0.39% |
| 3 | 5 | 8 | +2% | -2% | - | 80% | 13.73% | 25% | 0.35% |
| 4 | 6 | 8 | +2% | -2% | - | 80% | 16.07% | 30% | 2.09% |
| 5 | 6 | 9 | +2% | -2% | - | 80% | 16.10% | 30% | 1.99% |
| 6 | 6 | 9 | +4% | -2% | - | 80% | 16.58% | 30% | 2.06% |
| 7 | 6 | 9 | +4% | -4% | - | 80% | 16.81% | 30% | 2.02% |
| 8 | 7 | 9 | +4% | -4% | - | 80% | 18.53% | 35% | 4.95% |
| 9 | 7 | 10 | +4% | -4% | - | 80% | 19.97% | 35% | 0.90% |
| 10 | 7 | 10 | +6% | -4% | - | 80% | 20.54% | 35% | 0.94% |
| 11 | 7 | 10 | +6% | -6% | - | 80% | 20.81% | 35% | 0.88% |
| 12 | 8 | 10 | +6% | -6% | - | 80% | 23.14% | 40% | 2.55% |
| 13 | 8 | 10 | +8% | -6% | - | 80% | 23.73% | 40% | 2.60% |
| 14 | 8 | 10 | +8% | -8% | - | 80% | 24.09% | 40% | 2.44% |
| 15 | 8 | 10 | +8% | -8% | 1 glory success | 65% | 24.55% | 40% | 2.48% |
| 16 | 9 | 10 | +8% | -8% | 1 glory success | 65% | 26.23% | 45% | 4.49% |
| 17 | 9 | 10 | +10% | -8% | 1 glory success | 65% | 26.77% | 45% | 4.35% |
| 18 | 9 | 10 | +10% | -10% | 1 glory success | 65% | 27.24% | 45% | 4.32% |
| 19 | 9 | 10 | +10% | -10% | 1 glory success, 1 despair fail | 80% | 30.74% | 45% | 0.98% |
| 20 | 10 | 10 | +10% | -10% | 1 glory success, 1 despair fail | 80% | 33.10% | 50% | 2.25% |

"First rate" is the chance the attempt opens on. "Average amplification" is one
attempt played for the most amplification (+5% a glory success, -2% a despair
success, floored at 0%); "best possible" is a full glory bar with no despair.

Two kinds of level shape the boards:

- **Slots cost fuel.** Every slot added (levels 4, 8, 12, 16 and 20) has to be
  filled from the same spirit power, so the wipe chance jumps at each of
  them - to 4.95% at level 8, the worst board in the game. Spirit power grows
  only at levels 5 and 9 and stops at 10, so from level 12 on every new slot
  is paid for with mental training.
- **Head starts save fuel, but move the rate like real results.** From level
  15 every attempt starts with one glory slot already a success, and from
  level 19 one despair slot already a failure. Each is a slot that costs no
  spirit power and cannot go wrong - but a success makes the next action
  harder, so levels 15-18 open at 65% instead of 80%. The despair failure at
  19 moves it back up to 80%. That opening step down takes back much of what
  the free glory slot gives: level 15 is only 0.46 points better than level
  14, and levels 16-18 wipe more than any other late board (4.3-4.5%).
  Level 19 gets both halves and is the best board below 20: 16 fills against
  10 spirit power, only 3 successful trainings needed, an 80% start, and a
  wipe in 1 attempt in 100.

## Every goal

A level is reached with attempts made on the level before it: level 13 with
attempts on the level-12 board, and so on. Each goal has its own plan - the
cheapest route to it, found by the look-ahead search - so what a level costs
depends on what lies beyond it. The requirement is what the board must show
to level up; pity levels up without it.

| goal | requirement | total diamonds | the last level-up | reached by pity |
|---|---|---|---|---|
| 2 | Giant Hand: 3+ / any | 47K | 47K | 59% |
| 3 | Demon Eye, Immortal Oath: 3+ / 2- | 78K | 30K | 62% |
| 4 | all twelve together: 25+ / 21- | 78K | 0.5K | 0% |
| 5 | Sacred Tree, Lightning Ring, Golden Star: 5+ / 2- | 142K | 64K | 89% |
| 6 | all twelve: 44+ / 18- | 179K | 37K | 13% |
| 7 | every relic: 4+ / 2- | 258K | 79K | 60% |
| 8 | all twelve: 59+ / 17- | 327K | 69K | 37% |
| 9 | Archer Seal, Night Veil, Eternal Spark: 6+ / 2- | 421K | 94K | 87% |
| 10 | all twelve: 68+ / 19- | 485K | 64K | 24% |
| 11 | all twelve: 71+ / 18- | 587K | 102K | 41% |
| 12 | all twelve: 73+ / 17- | 696K | 109K | 41% |
| 13 | Mermaid Tear, Sky Eye, Mountain Crown: 7+ / 2- | 814K | 118K | 85% |
| 14 | all twelve: 80+ / 20- | 868K | 54K | 11% |
| 15 | all twelve: 83+ / 20- | 963K | 95K | 19% |
| 16 | all twelve: 85+ / 20- | 1.07M | 107K | 21% |
| 17 | all twelve: 89+ / 21- | 1.15M | 83K | 19% |
| 18 | all twelve: 91+ / 20- | 1.23M | 75K | 14% |
| 19 | all twelve: 94+ / 19- | 1.36M | 127K | 30% |
| 20 | all twelve: 96+ / 16- | 1.51M | 158K | 39% |
| 20 + 43% | Demon Eye at 43% | 1.56M | 50K | - |
| 20 + 46% | Demon Eye at 46% | 1.68M | 119K | - |
| 20 + 48% | Demon Eye at 48% | 1.96M | 279K | - |
| 20 + 50% | Demon Eye at 50% | 5.31M | 3.35M | - |

"The last level-up" is what the goal costs over the goal before it, each from
its own plan.

How the plans differ by goal:

- **The goal's own level spends everything.** Nearly every goal's plan rolls
  spare relics for the pity at its last step: whatever is left after the goal
  is worth nothing to it, so it goes into the pity bar.
- **A named-relic goal is usually reached by pity.** Three named relics at a
  bar - 5+ / 2-, 6+ / 2-, 7+ / 2- - is a long shot on the board it is rolled
  on, so goals 5, 9 and 13 finish by pity 85-89% of the time, and goals 2, 3
  and 7 about 60%.
- **A later goal buys more early.** On the way to a far goal, the plan
  summons where a near goal would roll spare relics for pity, and banks the
  relics the next named level needs: reaching level 5 costs 142K when 5 is
  the goal, but 358K on the way to 20 + 43%. The stock bought early is spent
  on the later levels, which is why they come cheap.
- **The Demon Eye tiers share the climb.** 20 + 43% and 20 + 46% use the same
  plan as far as level 20; 48% and 50% build some totals levels to a bar
  instead of repairing them. Past 46% each point gets much dearer: 50% - a
  full glory bar and no despair - costs 3.35M more than 48%.

## The climb to 20 + 43%

The cheapest route to level 20 and a 43% Demon Eye, the extension's default
goal, step by step. The route to 20 + 46% is the same as far as level 20. Diamonds are what each level-up costs on this route; the
strategy names are the extension's.

| to level | requirement | how attempts are played | diamonds | by pity |
|---|---|---|---|---|
| 2 | Giant Hand: 3+ / any | **maximize glory, minimize despair above a target**: the best chance of 3+ glory | 64K | 0% |
| 3 | Demon Eye, Immortal Oath: 3+ / 2- | above a target: the best chance of 3+ / 2- on each | 56K | 0% |
| 4 | 25+ / 21- in total | build relics to 3 / 2; **maximize glory, minimize despair until a limit**, toward the totals ahead | 0.4K | 0% |
| 5 | Sacred Tree, Lightning Ring, Golden Star: 5+ / 2- | above a target: 5+ / 2- on each | 237K | 13% |
| 6 | 44+ / 18- | repair; until a limit, toward this level's totals | 1.5K | 2% |
| 7 | every relic: 4+ / 2- | above a target: 4+ / 2- on every relic short of it | 42K | 0% |
| 8 | 59+ / 17- | repair; until a limit | 17K | 23% |
| 9 | Archer Seal, Night Veil, Eternal Spark: 6+ / 2- | above a target: 6+ / 2- on each | 312K | 6% |
| 10 | 68+ / 19- | build relics to 5 / 2; until a limit, toward the totals ahead | 80K | 10% |
| 11 | 71+ / 18- | repair; until a limit, toward the totals ahead | 8K | 18% |
| 12 | 73+ / 17- | repair; until a limit | 36K | 21% |
| 13 | Mermaid Tear, Sky Eye, Mountain Crown: 7+ / 2- | above a target: 7+ / 2- on each | 352K | 5% |
| 14 | 80+ / 20- | repair; until a limit, toward the totals ahead | 2.6K | 7% |
| 15 | 83+ / 20- | repair; until a limit | 13K | 11% |
| 16 | 85+ / 20- | repair; until a limit | 26K | 14% |
| 17 | 89+ / 21- | repair; until a limit, toward the totals ahead | 37K | 20% |
| 18 | 91+ / 20- | repair; until a limit, toward the totals ahead | 49K | 9% |
| 19 | 94+ / 19- | repair; until a limit, toward the totals ahead | 83K | 24% |
| 20 | 96+ / 16- | repair; until a limit | 108K | 30% |
| 20 + 43% | Demon Eye at 43% | **maximize amplification above a target**, 43% as the Minimum Useful Amplification | 37K | - |

The whole route costs 1.56M diamonds on average: 1.53M to level 20, then
37K to bring the Demon Eye to 43%. Farming to 46% instead costs 157K
from level 20, 1.68M in all.

The climb rests on a few habits:

- **The named levels are where the money goes.** Levels 2, 3, 5, 9 and 13 ask
  for named relics at a bar, and take two thirds of everything spent on the
  way to 20 - level 13 alone, 7 glory with at most 2 despair on an 8-slot
  board (a 1-in-7 shot), nearly a quarter. Each attempt on a named relic
  plays for the best chance of the whole bar - missing by one is worth almost
  nothing - and only once that is out of reach for the attempt does it play
  on toward the totals.
- **The totals levels after them come cheap.** A named level leaves the board
  well built, so the totals level after it costs little: level 4 after 3,
  level 6 after 5, level 14 after 13.
- **Build early, repair late.** The early totals levels 4 and 10 raise every
  relic to a bar - 3 / 2, then 5 / 2 - and level 7 asks every relic for
  4 / 2 outright. From level 11 on the board already sits near each total,
  so the levels are repairs: while the total despair is over the budget,
  every relic with 2 or more despair is asked for one less (the worst are
  the easiest steps); otherwise every relic with room is asked for one more
  glory. Of those, the relic rolled is whichever closes the most of the gap
  per attempt.
- **Every attempt on a totals level closes the gap, step by step.** It is
  played to maximize glory, minimize despair until a limit: the limit is
  what that one relic would have to become to bring the board to the totals,
  every glory gained or despair shed toward it counts, and past it the
  attempt goes for more glory and less despair, weighed by what the totals
  still need. A roll is kept if it brings the board nearer the totals, or as
  near with more glory and less despair - 9/0 beats a 9/1 that already
  clears the level. When every relic meets its bar and the totals are still
  short, the bars go a step further; at level 10 a relic traded off its bar
  for a result the keep rule ranks as high counts as meeting it.
- **Look ahead where it pays.** At levels 4, 10, 11, 14 and 17-19 the plan
  aims at the totals ahead rather than the level's own: the most glory and
  the least despair any level up to the goal asks. On the way to 20 that is
  96 / 16, and level 20's despair cap of 16 is the tightest in the climb, so
  despair shed early is despair not paid for later.
- **Summon rather than roll for pity.** When nothing the work needs has 10 on
  hand, the plan summons instead of rolling spare relics just to fill the
  pity bar - except at level 20, where it rolls them when the spare stock can
  fill the bar within 30 attempts. About 1 run in 4 still finishes level 19
  by pity, and 3 in 10 finish level 20 that way.

Every decision is made from the board as it stands - the relics, the stock
and the pity bar - with nothing remembered from earlier attempts, so the
advisor and the extension play exactly as these numbers were simulated.

At level 20 the plan converts every leftover relic into Demon Eyes (10 of a
type for 7) and farms it, summoning when short, every attempt played for the
best chance of 43% and then the most amplification.

## Playing one attempt on a 9-slot board (levels 16-18)

Optimal play at level 16:

| | |
|---|---|
| glory successes | 6.32 of 9 (one of them free) |
| despair successes | 2.69 of 9 (lower is better) |
| amplification | **26.23%** of a possible 45% |
| wipe | 4.49% |
| mental training | 8.8 of the 9 available |
| first rate | 65% (the free glory success counts as a success) |

**The fuel math decides everything.** With the free glory success, both bars
need 17 fills and you start with 10 spirit power, so at least 4 of your 9
mental trainings must succeed. Optimal play spends almost all of them. The
attempt also opens at 65%, not 80%: still a glory rate, but optimal play
trains there a third of the time to climb back to 80%.

What it actually does, share of decisions at each rate:

| rate | attempt glory | attempt despair | mental training |
|---|---|---|---|
| 80% | **59%** | 7% | 33% |
| 65% | **61%** | 4% | 35% |
| 50% | 16% | 33% | **51%** |
| 35% | 6% | **80%** | 14% |
| 20% | 15% | **72%** | 13% |

The two bars are opposites - a glory success is good, a despair success is bad -
so the good rates go to glory and the bad rates to despair. The middle rate is
for mental training: it refills spirit power, and a failed training costs no
slot and moves the rate back up.

### A rule you can play by hand

1. **Out of spirit power:** mental training.
2. **Fuel:** mental training when you are at least 2 below the spirit power cap,
   the slots left in both bars outnumber your spirit power, and the rate is
   50% or better.
3. **Both bars open:** attempt glory at 80% and 65%; attempt despair at 50% and
   below.
4. **One bar left:** attempt it only at the rates that suit it (glory at
   80/65/50%, despair at 50/35/20%) and take mental training otherwise -
   mental strength has no other use by then.

Scored exactly against the optimum:

| level | the rule | optimal | gap | wipe (rule) |
|---|---|---|---|---|
| 16 | 24.98% | 26.23% | -1.25 | 3.63% |
| 19 | 29.24% | 30.74% | -1.50 | 0.35% |
| 20 | 31.50% | 33.10% | -1.60 | 1.32% |

About 1.25-1.6 points short of perfect play, and it wipes less than the
optimum on every one of these boards. The one clause not to change is the fuel
gate. At level 16, training at 50% or better is best; never training wipes 14.8%
of attempts, and training at any rate still wipes 9.1% because a 20% training
fails four times in five.

| mental training at | amplification | wipe |
|---|---|---|
| never | 22.79% | 14.79% |
| 80% or better | 22.52% | 12.81% |
| 65% or better | 23.45% | 5.64% |
| **50% or better** | **24.98%** | **3.63%** |
| 35% or better | 24.13% | 7.53% |
| any rate | 23.72% | 9.09% |

The same rule works on the 8-slot boards (levels 12-15), where only 3
successful trainings are needed and wipes are rarer (about 2.4-2.6%).

### Dodging the wipe

The optimum already accepts a small wipe chance because a wipe only costs the
floor (0%). Charging a wipe extra buys safety cheaply at first: at level 16 a
penalty of 10 cuts the wipe from 4.49% to 3.96% for 0.03 points of
amplification, and a penalty of 30 gets it to 2.86% for 0.23 points. Driving
it as low as the rules allow (1.69%) costs 6.7 points.

## Level 20: the 10th slot

Level 20 is reached with level-19 attempts, so its extra slot only matters for
attempts made once you are there - farming a relic at level 20, which is what
[COST.md](COST.md) prices.

- **Four successful trainings again.** The 10th slot brings the fills back to
  18 (20 slots less the two head starts) against the same 10 spirit power, so
  4 of your 10 mental trainings must land - one more than at level 19. The
  wipe chance goes from 0.98% to **2.25%**, still well below levels 16-18.
- **More amplification, a little less per slot.** The average rises to
  **33.10%** of a possible 50%: 66.2% of a perfect bar against 68.3% at
  level 19. The extra slot is paid for in fuel.
- **Play barely changes.** Next to level 19, optimal play trains a little more
  at 65% (33% against 32%) and attempts glory a little more at 50% (11%
  against 8%); the rule above needs no adjustment.
- **The hand rule still holds** at 31.50% (1.60 points short) with a 1.32%
  wipe.
- **Safety is almost free.** A wipe penalty of 10 takes the wipe from 2.25% to
  1.90% for 0.01 points; lowering it all the way (0.51%) costs 7.2 points.

### The level-20 outcome grid

The chance of finishing one level-20 attempt on each exact combination of glory
and despair successes, under optimal play. Glory rises left to right and despair
falls from top to bottom, so the best corner (a full glory bar, no despair) is
bottom right. Cells are shaded relative to the most likely one (15.2%): green
there, white near zero. The grid sums to 100%.

<table style="border-collapse:collapse;font-size:13px;">
<thead><tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;color:#000000;background:#ffffff;">despair &darr; / glory &rarr;</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ffffff;color:#000000;">0</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#fbf7eb;color:#000000;">1</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#f6efd7;color:#000000;">2</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#f2e7c3;color:#000000;">3</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#eedfaf;color:#000000;">4</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ead79b;color:#000000;">5</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#e5cf87;color:#000000;">6</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#e1c773;color:#000000;">7</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ddbf5f;color:#000000;">8</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#d8b74b;color:#000000;">9</th><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#d4af37;color:#000000;">10</th></tr></thead>
<tbody>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#8b0000;color:#000000;">10</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 10 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="5 glory, 10 despair - amplifies 5%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="6 glory, 10 despair - amplifies 10%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="7 glory, 10 despair - amplifies 15%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="8 glory, 10 despair - amplifies 20%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="9 glory, 10 despair - amplifies 25%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="10 glory, 10 despair - amplifies 30%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#971a1a;color:#000000;">9</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 9 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 9 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 9 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 9 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 9 despair - amplifies 2%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="5 glory, 9 despair - amplifies 7%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="6 glory, 9 despair - amplifies 12%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="7 glory, 9 despair - amplifies 17%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="8 glory, 9 despair - amplifies 22%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="9 glory, 9 despair - amplifies 27%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="10 glory, 9 despair - amplifies 32%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#a23333;color:#000000;">8</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 8 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 8 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 8 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 8 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 8 despair - amplifies 4%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="5 glory, 8 despair - amplifies 9%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="6 glory, 8 despair - amplifies 14%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="7 glory, 8 despair - amplifies 19%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="8 glory, 8 despair - amplifies 24%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="9 glory, 8 despair - amplifies 29%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="10 glory, 8 despair - amplifies 34%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ae4c4c;color:#000000;">7</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 7 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 7 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 7 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 7 despair - amplifies 1%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 7 despair - amplifies 6%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="5 glory, 7 despair - amplifies 11%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="6 glory, 7 despair - amplifies 16%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="7 glory, 7 despair - amplifies 21%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="8 glory, 7 despair - amplifies 26%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="9 glory, 7 despair - amplifies 31%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="10 glory, 7 despair - amplifies 36%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#b96666;color:#000000;">6</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 6 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 6 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 6 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 6 despair - amplifies 3%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 6 despair - amplifies 8%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="5 glory, 6 despair - amplifies 13%">0.01%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fefffe;color:#000000;" title="6 glory, 6 despair - amplifies 18%">0.05%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fefffd;color:#000000;" title="7 glory, 6 despair - amplifies 23%">0.09%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fffffe;color:#000000;" title="8 glory, 6 despair - amplifies 28%">0.04%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="9 glory, 6 despair - amplifies 33%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="10 glory, 6 despair - amplifies 38%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#c58080;color:#000000;">5</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 5 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 5 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 5 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 5 despair - amplifies 5%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 5 despair - amplifies 10%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fefffe;color:#000000;" title="5 glory, 5 despair - amplifies 15%">0.06%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f9fdf5;color:#000000;" title="6 glory, 5 despair - amplifies 20%">0.50%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f0f9e6;color:#000000;" title="7 glory, 5 despair - amplifies 25%">1.27%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f6fbf0;color:#000000;" title="8 glory, 5 despair - amplifies 30%">0.76%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fefefd;color:#000000;" title="9 glory, 5 despair - amplifies 35%">0.11%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="10 glory, 5 despair - amplifies 40%">0%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#d19999;color:#000000;">4</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 4 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 4 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 4 despair - amplifies 2%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 4 despair - amplifies 7%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 4 despair - amplifies 12%">0.01%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fdfefb;color:#000000;" title="5 glory, 4 despair - amplifies 17%">0.20%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#eaf6dd;color:#000000;" title="6 glory, 4 despair - amplifies 22%">1.76%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#b8e188;color:#000000;" title="7 glory, 4 despair - amplifies 27%">6.10%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#b6e086;color:#000000;" title="8 glory, 4 despair - amplifies 32%">6.20%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#eef8e3;color:#000000;" title="9 glory, 4 despair - amplifies 37%">1.43%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fefffe;color:#000000;" title="10 glory, 4 despair - amplifies 42%">0.06%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#dcb2b2;color:#000000;">3</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 3 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 3 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 3 despair - amplifies 4%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 3 despair - amplifies 9%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 3 despair - amplifies 14%">0.02%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fcfefa;color:#000000;" title="5 glory, 3 despair - amplifies 19%">0.28%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#e1f2cc;color:#000000;" title="6 glory, 3 despair - amplifies 24%">2.59%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#72c160;color:#000000;" title="7 glory, 3 despair - amplifies 29%">10.4%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#1a9850;color:#000000;" title="8 glory, 3 despair - amplifies 34%">15.2%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#b5df83;color:#000000;" title="9 glory, 3 despair - amplifies 39%">6.32%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f9fcf5;color:#000000;" title="10 glory, 3 despair - amplifies 44%">0.52%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#e8cccc;color:#000000;">2</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 2 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 2 despair - amplifies 1%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 2 despair - amplifies 6%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 2 despair - amplifies 11%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 2 despair - amplifies 16%">0.01%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fdfefc;color:#000000;" title="5 glory, 2 despair - amplifies 21%">0.15%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#edf7e1;color:#000000;" title="6 glory, 2 despair - amplifies 26%">1.54%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#acdc74;color:#000000;" title="7 glory, 2 despair - amplifies 31%">7.11%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#41aa57;color:#000000;" title="8 glory, 2 despair - amplifies 36%">13.1%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#99d368;color:#000000;" title="9 glory, 2 despair - amplifies 41%">8.34%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f0f9e7;color:#000000;" title="10 glory, 2 despair - amplifies 46%">1.25%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#f3e6e6;color:#000000;">1</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="0 glory, 1 despair - amplifies 0%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 1 despair - amplifies 3%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 1 despair - amplifies 8%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 1 despair - amplifies 13%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 1 despair - amplifies 18%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fffffe;color:#000000;" title="5 glory, 1 despair - amplifies 23%">0.03%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fbfdf8;color:#000000;" title="6 glory, 1 despair - amplifies 28%">0.34%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#e9f6da;color:#000000;" title="7 glory, 1 despair - amplifies 33%">1.87%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#cdeaab;color:#000000;" title="8 glory, 1 despair - amplifies 38%">4.28%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#d5edb8;color:#000000;" title="9 glory, 1 despair - amplifies 43%">3.62%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#f5fbee;color:#000000;" title="10 glory, 1 despair - amplifies 48%">0.86%</td></tr>
<tr><th style="padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);font-weight:600;background:#ffffff;color:#000000;">0</th><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#e5f4d3;color:#000000;" title="0 glory, 0 despair - amplifies 0%">2.25%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="1 glory, 0 despair - amplifies 5%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="2 glory, 0 despair - amplifies 10%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="3 glory, 0 despair - amplifies 15%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="4 glory, 0 despair - amplifies 20%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="5 glory, 0 despair - amplifies 25%">0%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#ffffff;color:#000000;" title="6 glory, 0 despair - amplifies 30%">0.02%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fdfefc;color:#000000;" title="7 glory, 0 despair - amplifies 35%">0.15%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fafdf7;color:#000000;" title="8 glory, 0 despair - amplifies 40%">0.43%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fafdf6;color:#000000;" title="9 glory, 0 despair - amplifies 45%">0.47%</td><td style="padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;border:1px solid rgba(128,128,128,.35);background:#fdfefc;color:#000000;" title="10 glory, 0 despair - amplifies 50%">0.15%</td></tr>
</tbody></table>

The same attempts by the amplification they pay. The red column is the 0%
floor: the 2.2% of attempts that wipe, plus any that wash out below zero.

<svg viewBox="0 0 519 272" width="519" height="272" role="img" xmlns="http://www.w3.org/2000/svg" aria-label="Distribution of relic amplification at inheritor level 20">
<title>Amplification distribution, inheritor level 20</title>
<desc>Chance of each exact amplification under optimal weighted play. Mean 33.10 percent. Peak 15.2 percent of attempts at an amplification of 34 percent.</desc>
<style>.ac-bar{fill:#2a78d6}.ac-wipe{fill:#d03b3b}.ac-grid{stroke:#e1e0d9;stroke-width:1}.ac-axis{stroke:#c3c2b7;stroke-width:1}.ac-ink{fill:#52514e}.ac-muted{fill:#898781}.ac-mean{stroke:#52514e}.ac-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}.ac-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}@media(prefers-color-scheme:dark){.ac-bar{fill:#3987e5}.ac-grid{stroke:#2c2c2a}.ac-axis{stroke:#383835}.ac-ink{fill:#c3c2b7}.ac-mean{stroke:#c3c2b7}}</style>
<line class="ac-grid" x1="46" y1="236.0" x2="505" y2="236.0"/>
<text class="ac-t ac-muted" x="38" y="240.0" text-anchor="end">0%</text>
<line class="ac-grid" x1="46" y1="183.5" x2="505" y2="183.5"/>
<text class="ac-t ac-muted" x="38" y="187.5" text-anchor="end">5%</text>
<line class="ac-grid" x1="46" y1="131.0" x2="505" y2="131.0"/>
<text class="ac-t ac-muted" x="38" y="135.0" text-anchor="end">10%</text>
<line class="ac-grid" x1="46" y1="78.5" x2="505" y2="78.5"/>
<text class="ac-t ac-muted" x="38" y="82.5" text-anchor="end">15%</text>
<line class="ac-grid" x1="46" y1="26.0" x2="505" y2="26.0"/>
<text class="ac-t ac-muted" x="38" y="30.0" text-anchor="end">20%</text>
<text class="ac-t ac-muted" x="38" y="17" text-anchor="end">chance</text>
<path class="ac-wipe" d="M47.0,236 V215.4 Q47.0,212.4 50.0,212.4 H51.0 Q54.0,212.4 54.0,215.4 V236 Z"><title>amplification 0: 2.25%</title></path>
<path class="ac-bar" d="M56.0,236 V236.0 Q56.0,236.0 56.0,236.0 H63.0 Q63.0,236.0 63.0,236.0 V236 Z"><title>amplification 1: 0%</title></path>
<path class="ac-bar" d="M65.0,236 V236.0 Q65.0,236.0 65.0,236.0 H72.0 Q72.0,236.0 72.0,236.0 V236 Z"><title>amplification 2: 0%</title></path>
<path class="ac-bar" d="M74.0,236 V236.0 Q74.0,236.0 74.0,236.0 H81.0 Q81.0,236.0 81.0,236.0 V236 Z"><title>amplification 3: 0%</title></path>
<path class="ac-bar" d="M83.0,236 V236.0 Q83.0,236.0 83.0,236.0 H90.0 Q90.0,236.0 90.0,236.0 V236 Z"><title>amplification 4: 0%</title></path>
<path class="ac-bar" d="M92.0,236 V236.0 Q92.0,236.0 92.0,236.0 H99.0 Q99.0,236.0 99.0,236.0 V236 Z"><title>amplification 5: 0%</title></path>
<path class="ac-bar" d="M101.0,236 V236.0 Q101.0,236.0 101.0,236.0 H108.0 Q108.0,236.0 108.0,236.0 V236 Z"><title>amplification 6: 0%</title></path>
<path class="ac-bar" d="M110.0,236 V236.0 Q110.0,236.0 110.0,236.0 H117.0 Q117.0,236.0 117.0,236.0 V236 Z"><title>amplification 7: 0%</title></path>
<path class="ac-bar" d="M119.0,236 V236.0 Q119.0,236.0 119.0,236.0 H126.0 Q126.0,236.0 126.0,236.0 V236 Z"><title>amplification 8: 0%</title></path>
<path class="ac-bar" d="M128.0,236 V236.0 Q128.0,236.0 128.0,236.0 H135.0 Q135.0,236.0 135.0,236.0 V236 Z"><title>amplification 9: 0%</title></path>
<path class="ac-bar" d="M137.0,236 V236.0 Q137.0,236.0 137.0,236.0 H144.0 Q144.0,236.0 144.0,236.0 V236 Z"><title>amplification 10: 0%</title></path>
<path class="ac-bar" d="M146.0,236 V236.0 Q146.0,236.0 146.0,236.0 H153.0 Q153.0,236.0 153.0,236.0 V236 Z"><title>amplification 11: 0%</title></path>
<path class="ac-bar" d="M155.0,236 V236.0 Q155.0,235.9 155.1,235.9 H161.9 Q162.0,235.9 162.0,236.0 V236 Z"><title>amplification 12: 0.01%</title></path>
<path class="ac-bar" d="M164.0,236 V236.0 Q164.0,235.9 164.1,235.9 H170.9 Q171.0,235.9 171.0,236.0 V236 Z"><title>amplification 13: 0.01%</title></path>
<path class="ac-bar" d="M173.0,236 V236.0 Q173.0,235.8 173.2,235.8 H179.8 Q180.0,235.8 180.0,236.0 V236 Z"><title>amplification 14: 0.02%</title></path>
<path class="ac-bar" d="M182.0,236 V236.0 Q182.0,235.4 182.6,235.4 H188.4 Q189.0,235.4 189.0,236.0 V236 Z"><title>amplification 15: 0.06%</title></path>
<path class="ac-bar" d="M191.0,236 V236.0 Q191.0,235.9 191.1,235.9 H197.9 Q198.0,235.9 198.0,236.0 V236 Z"><title>amplification 16: 0.01%</title></path>
<path class="ac-bar" d="M200.0,236 V236.0 Q200.0,233.9 202.1,233.9 H204.9 Q207.0,233.9 207.0,236.0 V236 Z"><title>amplification 17: 0.20%</title></path>
<path class="ac-bar" d="M209.0,236 V236.0 Q209.0,235.4 209.6,235.4 H215.4 Q216.0,235.4 216.0,236.0 V236 Z"><title>amplification 18: 0.05%</title></path>
<path class="ac-bar" d="M218.0,236 V236.0 Q218.0,233.1 220.9,233.1 H222.1 Q225.0,233.1 225.0,236.0 V236 Z"><title>amplification 19: 0.28%</title></path>
<path class="ac-bar" d="M227.0,236 V233.8 Q227.0,230.8 230.0,230.8 H231.0 Q234.0,230.8 234.0,233.8 V236 Z"><title>amplification 20: 0.50%</title></path>
<path class="ac-bar" d="M236.0,236 V236.0 Q236.0,234.4 237.6,234.4 H241.4 Q243.0,234.4 243.0,236.0 V236 Z"><title>amplification 21: 0.16%</title></path>
<path class="ac-bar" d="M245.0,236 V220.6 Q245.0,217.6 248.0,217.6 H249.0 Q252.0,217.6 252.0,220.6 V236 Z"><title>amplification 22: 1.76%</title></path>
<path class="ac-bar" d="M254.0,236 V236.0 Q254.0,234.8 255.2,234.8 H259.8 Q261.0,234.8 261.0,236.0 V236 Z"><title>amplification 23: 0.12%</title></path>
<path class="ac-bar" d="M263.0,236 V211.8 Q263.0,208.8 266.0,208.8 H267.0 Q270.0,208.8 270.0,211.8 V236 Z"><title>amplification 24: 2.59%</title></path>
<path class="ac-bar" d="M272.0,236 V225.6 Q272.0,222.6 275.0,222.6 H276.0 Q279.0,222.6 279.0,225.6 V236 Z"><title>amplification 25: 1.27%</title></path>
<path class="ac-bar" d="M281.0,236 V222.8 Q281.0,219.8 284.0,219.8 H285.0 Q288.0,219.8 288.0,222.8 V236 Z"><title>amplification 26: 1.55%</title></path>
<path class="ac-bar" d="M290.0,236 V174.9 Q290.0,171.9 293.0,171.9 H294.0 Q297.0,171.9 297.0,174.9 V236 Z"><title>amplification 27: 6.10%</title></path>
<path class="ac-bar" d="M299.0,236 V235.0 Q299.0,232.0 302.0,232.0 H303.0 Q306.0,232.0 306.0,235.0 V236 Z"><title>amplification 28: 0.38%</title></path>
<path class="ac-bar" d="M308.0,236 V129.6 Q308.0,126.6 311.0,126.6 H312.0 Q315.0,126.6 315.0,129.6 V236 Z"><title>amplification 29: 10.4%</title></path>
<path class="ac-bar" d="M317.0,236 V230.8 Q317.0,227.8 320.0,227.8 H321.0 Q324.0,227.8 324.0,230.8 V236 Z"><title>amplification 30: 0.78%</title></path>
<path class="ac-bar" d="M326.0,236 V164.4 Q326.0,161.4 329.0,161.4 H330.0 Q333.0,161.4 333.0,164.4 V236 Z"><title>amplification 31: 7.11%</title></path>
<path class="ac-bar" d="M335.0,236 V173.9 Q335.0,170.9 338.0,170.9 H339.0 Q342.0,170.9 342.0,173.9 V236 Z"><title>amplification 32: 6.20%</title></path>
<path class="ac-bar" d="M344.0,236 V219.3 Q344.0,216.3 347.0,216.3 H348.0 Q351.0,216.3 351.0,219.3 V236 Z"><title>amplification 33: 1.88%</title></path>
<path class="ac-bar" d="M353.0,236 V79.1 Q353.0,76.1 356.0,76.1 H357.0 Q360.0,76.1 360.0,79.1 V236 Z"><title>amplification 34: 15.2%</title></path>
<path class="ac-bar" d="M362.0,236 V236.0 Q362.0,233.3 364.7,233.3 H366.3 Q369.0,233.3 369.0,236.0 V236 Z"><title>amplification 35: 0.26%</title></path>
<path class="ac-bar" d="M371.0,236 V101.3 Q371.0,98.3 374.0,98.3 H375.0 Q378.0,98.3 378.0,101.3 V236 Z"><title>amplification 36: 13.1%</title></path>
<path class="ac-bar" d="M380.0,236 V224.0 Q380.0,221.0 383.0,221.0 H384.0 Q387.0,221.0 387.0,224.0 V236 Z"><title>amplification 37: 1.43%</title></path>
<path class="ac-bar" d="M389.0,236 V194.1 Q389.0,191.1 392.0,191.1 H393.0 Q396.0,191.1 396.0,194.1 V236 Z"><title>amplification 38: 4.28%</title></path>
<path class="ac-bar" d="M398.0,236 V172.7 Q398.0,169.7 401.0,169.7 H402.0 Q405.0,169.7 405.0,172.7 V236 Z"><title>amplification 39: 6.32%</title></path>
<path class="ac-bar" d="M407.0,236 V234.5 Q407.0,231.5 410.0,231.5 H411.0 Q414.0,231.5 414.0,234.5 V236 Z"><title>amplification 40: 0.43%</title></path>
<path class="ac-bar" d="M416.0,236 V151.5 Q416.0,148.5 419.0,148.5 H420.0 Q423.0,148.5 423.0,151.5 V236 Z"><title>amplification 41: 8.34%</title></path>
<path class="ac-bar" d="M425.0,236 V236.0 Q425.0,235.3 425.7,235.3 H431.3 Q432.0,235.3 432.0,236.0 V236 Z"><title>amplification 42: 0.06%</title></path>
<path class="ac-bar" d="M434.0,236 V201.0 Q434.0,198.0 437.0,198.0 H438.0 Q441.0,198.0 441.0,201.0 V236 Z"><title>amplification 43: 3.62%</title></path>
<path class="ac-bar" d="M443.0,236 V233.6 Q443.0,230.6 446.0,230.6 H447.0 Q450.0,230.6 450.0,233.6 V236 Z"><title>amplification 44: 0.52%</title></path>
<path class="ac-bar" d="M452.0,236 V234.1 Q452.0,231.1 455.0,231.1 H456.0 Q459.0,231.1 459.0,234.1 V236 Z"><title>amplification 45: 0.47%</title></path>
<path class="ac-bar" d="M461.0,236 V225.9 Q461.0,222.9 464.0,222.9 H465.0 Q468.0,222.9 468.0,225.9 V236 Z"><title>amplification 46: 1.25%</title></path>
<path class="ac-bar" d="M479.0,236 V230.0 Q479.0,227.0 482.0,227.0 H483.0 Q486.0,227.0 486.0,230.0 V236 Z"><title>amplification 48: 0.86%</title></path>
<path class="ac-bar" d="M497.0,236 V236.0 Q497.0,234.4 498.6,234.4 H502.4 Q504.0,234.4 504.0,236.0 V236 Z"><title>amplification 50: 0.15%</title></path>
<line class="ac-axis" x1="46" y1="236" x2="505" y2="236"/>
<text class="ac-t ac-muted" x="50.5" y="251" text-anchor="middle">0%</text>
<text class="ac-t ac-muted" x="140.5" y="251" text-anchor="middle">10%</text>
<text class="ac-t ac-muted" x="230.5" y="251" text-anchor="middle">20%</text>
<text class="ac-t ac-muted" x="320.5" y="251" text-anchor="middle">30%</text>
<text class="ac-t ac-muted" x="410.5" y="251" text-anchor="middle">40%</text>
<text class="ac-t ac-muted" x="500.5" y="251" text-anchor="middle">50%</text>
<text class="ac-t ac-muted" x="275.5" y="266" text-anchor="middle">amplification (+5% per glory success, -2% per despair success)</text>
<line class="ac-mean" x1="348.4" y1="20" x2="348.4" y2="236" stroke-dasharray="3 3" stroke-width="1.5"/>
<text class="ac-b ac-ink" x="353.4" y="14">mean 33.1%</text>
<text class="ac-b ac-wipe" x="56.5" y="204.4" text-anchor="start">wipe 2.25%</text>
</svg>

<sub>Regenerate both with `relic solve --level 20 --amp-table --html`. They need a
renderer that keeps inline HTML and SVG; the VS Code preview does, GitHub strips
them.</sub>

## Assumptions

1. Level bonuses from 13 on follow the sheet: 13 +2% glory, 14 -2% despair, 15
   start with 1 glory success, 16 the 9th slot, 17 +2% glory, 18 -2% despair,
   19 start with 1 despair failure, 20 the 10th slot. Level 18 is read as -2%
   despair. Spirit power stays at 10.
2. The level-13 quest's "Abyss's Water Drop, Eye of Typhoon, Emperor Ring" are
   the three relics no earlier level names: Mermaid's Tear, Eye of the Sky and
   Crown of the Great Mountain.
3. Amplification is +5% a glory success and -2% a despair success, floored at 0%;
   an attempt that runs dry (a wipe) inherits nothing.
4. The chance ladder is 80 / 65 / 50 / 35 / 20%, starting at 80%; a success
   moves one step down (harder), a failure one step up. The head-start slots
   move it the same way before the first action: the level-15 glory success
   puts levels 15-18 at 65%, and the level-19 despair failure brings 19 and 20
   back to 80%.

## Running it

```
cargo install --path engine         # once: builds `relic` and puts it on your PATH
relic solve --level 16              # exact solve, distribution, policy, safety trade-off
relic solve --level 12 --target 7 2 # chase an all-or-nothing target (the level-13 quest)
relic solve --level 20 --amp-table  # P(each glory/despair combination) at level 20
relic solve --all-levels            # one line per level, 1-20
relic heuristics --level 16         # the hand-played rule vs the optimum
relic levels --strategy lookahead   # the whole climb, level by level
relic levels --cost 20+43           # the mean diamonds to a goal under its saved plan
```
