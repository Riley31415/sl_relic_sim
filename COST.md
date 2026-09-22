# What a crit relic costs

How many diamonds it takes to farm a level 20 crit relic up to each
amplification mark. The strategy side of this - what one attempt is worth and how
to play it - is in [STRATEGY.md](STRATEGY.md); this is only the price tag.

```
python economy.py --level 20 --sims 20000
python economy.py --chart > cost.svg
python economy.py --min-mark 0      # include the cheap marks below 35% too
```

## The economy

| | |
|---|---|
| 1 summon | 5,000 diamonds, 11 random relics |
| relic types | 12, of which type 1 (crit) is the one we want |
| crit relics per summon | Binomial(11, 1/12) = **0.9167** on average |
| 1 inheritance attempt | 10 crit relics |
| **so 1 attempt costs** | **about 54,545 diamonds** once leftovers carry over |

Each attempt re-rolls the relic and you keep the best result you have ever hit,
so reaching a mark is a matter of attempting until one lands. That makes the
number of attempts geometric in the per-attempt chance, and the diamond cost
follows from it.

## Method

A Monte Carlo over **20,000 farming runs**. Each run summons until it can afford
an attempt, attempts, keeps its best amplification, and stops once it hits 50%.
The diamond total at the moment each mark is first met is recorded, then averaged.

The per-attempt outcome is drawn from the solver's **exact** outcome
distribution rather than by replaying the policy move by move. `analyse()`
enumerates every branch, so those are the same distribution - a speedup, not an
approximation. The summon side is simulated properly, one batch at a time, so
the leftover crit relics that carry between attempts are handled exactly.

## Cost by mark

Every achievable mark from 35% up. Below that the table is not worth
printing: a single attempt clears 30% 60% of the time,
so everything cheaper than 35% costs about one attempt
(93.9K or less). Run with `--min-mark 0` for the
full 48 marks.

| amp | P(one attempt) | mean attempts | mean diamonds | median | 90th pct |
|---|---|---|---|---|---|
| **35%** | 29.2472% | 3.4 | 188,407 | 140K | 395K |
| **36%** | 29.0089% | 3.4 | 189,674 | 140K | 400K |
| **37%** | 18.8890% | 5.3 | 291,034 | 210K | 630K |
| **38%** | 17.4950% | 5.8 | 315,824 | 225K | 695K |
| **39%** | 14.6375% | 6.9 | 377,015 | 270K | 835K |
| **40%** | 9.6883% | 10.3 | 566,284 | 400K | 1.27M |
| **41%** | 9.4297% | 10.6 | 581,718 | 415K | 1.31M |
| **42%** | 3.9551% | 25.5 | 1,391,221 | 975K | 3.19M |
| **43%** | 3.9010% | 25.8 | 1,409,309 | 990K | 3.23M |
| **44%** | 1.8328% | 54.6 | 2,980,756 | 2.06M | 6.85M |
| **45%** | 1.4729% | 67.8 | 3,703,813 | 2.56M | 8.52M |
| **46%** | 1.2348% | 81.2 | 4,431,286 | 3.07M | 10.2M |
| **48%** | 0.5033% | 200.5 | 10,938,315 | 7.6M | 25.3M |
| **50%** | 0.0689% | 1,452.6 | 79,234,326 | 55.3M | 181M |

<svg viewBox="0 0 634 296" width="634" height="296" role="img" xmlns="http://www.w3.org/2000/svg" aria-label="Diamonds needed to reach each amplification mark at level 20">
<title>Diamond cost by amplification mark, inheritor level 20</title>
<desc>Mean diamonds to first reach each amplification, log scale. 58.3K at 1 percent rising to 79.2M at 50 percent.</desc>
<style>.ec-line{fill:none;stroke:#2a78d6;stroke-width:2}.ec-dot{fill:#2a78d6}.ec-grid{stroke:#e1e0d9;stroke-width:1}.ec-axis{stroke:#c3c2b7;stroke-width:1}.ec-ink{fill:#52514e}.ec-muted{fill:#898781}.ec-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}.ec-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}@media(prefers-color-scheme:dark){.ec-line{stroke:#3987e5}.ec-dot{fill:#3987e5}.ec-grid{stroke:#2c2c2a}.ec-axis{stroke:#383835}.ec-ink{fill:#c3c2b7}}</style>
<line class="ec-grid" x1="58" y1="256.0" x2="618" y2="256.0"/>
<text class="ec-t ec-muted" x="50" y="260.0" text-anchor="end">10K</text>
<line class="ec-grid" x1="58" y1="198.5" x2="618" y2="198.5"/>
<text class="ec-t ec-muted" x="50" y="202.5" text-anchor="end">100K</text>
<line class="ec-grid" x1="58" y1="141.0" x2="618" y2="141.0"/>
<text class="ec-t ec-muted" x="50" y="145.0" text-anchor="end">1M</text>
<line class="ec-grid" x1="58" y1="83.5" x2="618" y2="83.5"/>
<text class="ec-t ec-muted" x="50" y="87.5" text-anchor="end">10M</text>
<line class="ec-grid" x1="58" y1="26.0" x2="618" y2="26.0"/>
<text class="ec-t ec-muted" x="50" y="30.0" text-anchor="end">100M</text>
<text class="ec-t ec-muted" x="50" y="17" text-anchor="end">diamonds</text>
<polyline class="ec-line" points="58.0,212.0 69.4,212.0 80.9,212.0 92.3,212.0 103.7,212.0 115.1,212.0 126.6,212.0 138.0,212.0 149.4,212.0 160.9,212.0 172.3,212.0 183.7,212.0 195.1,212.0 206.6,211.9 218.0,211.9 229.4,211.9 240.9,211.9 252.3,211.7 263.7,211.7 275.1,211.5 286.6,211.2 298.0,211.1 309.4,210.2 320.9,210.1 332.3,209.0 343.7,208.2 355.1,207.6 366.6,204.9 378.0,204.7 389.4,200.1 400.9,199.6 412.3,196.7 423.7,193.2 435.1,192.4 446.6,182.7 458.0,182.5 469.4,171.8 480.9,169.8 492.3,165.4 503.7,155.2 515.1,154.5 526.6,132.8 538.0,132.4 549.4,113.7 560.9,108.3 572.3,103.8 595.1,81.3 618.0,31.8"/>
<circle class="ec-dot" cx="58.0" cy="212.0" r="2.6"><title>1%: 58,318 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="69.4" cy="212.0" r="2.6"><title>2%: 58,318 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="80.9" cy="212.0" r="2.6"><title>3%: 58,318 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="92.3" cy="212.0" r="2.6"><title>4%: 58,318 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="103.7" cy="212.0" r="2.6"><title>5%: 58,318 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="115.1" cy="212.0" r="2.6"><title>6%: 58,318 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="126.6" cy="212.0" r="2.6"><title>7%: 58,318 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="138.0" cy="212.0" r="2.6"><title>8%: 58,320 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="149.4" cy="212.0" r="2.6"><title>9%: 58,324 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="160.9" cy="212.0" r="2.6"><title>10%: 58,324 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="172.3" cy="212.0" r="2.6"><title>11%: 58,329 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="183.7" cy="212.0" r="2.6"><title>12%: 58,333 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="195.1" cy="212.0" r="2.6"><title>13%: 58,354 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="206.6" cy="211.9" r="2.6"><title>14%: 58,370 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="218.0" cy="211.9" r="2.6"><title>15%: 58,406 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="229.4" cy="211.9" r="2.6"><title>16%: 58,552 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="240.9" cy="211.9" r="2.6"><title>17%: 58,566 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="252.3" cy="211.7" r="2.6"><title>18%: 58,886 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="263.7" cy="211.7" r="2.6"><title>19%: 59,004 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="275.1" cy="211.5" r="2.6"><title>20%: 59,380 diamonds on average (1.0 attempts)</title></circle>
<circle class="ec-dot" cx="286.6" cy="211.2" r="2.6"><title>21%: 60,193 diamonds on average (1.1 attempts)</title></circle>
<circle class="ec-dot" cx="298.0" cy="211.1" r="2.6"><title>22%: 60,358 diamonds on average (1.1 attempts)</title></circle>
<circle class="ec-dot" cx="309.4" cy="210.2" r="2.6"><title>23%: 62,615 diamonds on average (1.1 attempts)</title></circle>
<circle class="ec-dot" cx="320.9" cy="210.1" r="2.6"><title>24%: 62,818 diamonds on average (1.1 attempts)</title></circle>
<circle class="ec-dot" cx="332.3" cy="209.0" r="2.6"><title>25%: 65,747 diamonds on average (1.2 attempts)</title></circle>
<circle class="ec-dot" cx="343.7" cy="208.2" r="2.6"><title>26%: 67,804 diamonds on average (1.2 attempts)</title></circle>
<circle class="ec-dot" cx="355.1" cy="207.6" r="2.6"><title>27%: 69,357 diamonds on average (1.2 attempts)</title></circle>
<circle class="ec-dot" cx="366.6" cy="204.9" r="2.6"><title>28%: 77,535 diamonds on average (1.4 attempts)</title></circle>
<circle class="ec-dot" cx="378.0" cy="204.7" r="2.6"><title>29%: 77,996 diamonds on average (1.4 attempts)</title></circle>
<circle class="ec-dot" cx="389.4" cy="200.1" r="2.6"><title>30%: 93,865 diamonds on average (1.7 attempts)</title></circle>
<circle class="ec-dot" cx="400.9" cy="199.6" r="2.6"><title>31%: 95,859 diamonds on average (1.7 attempts)</title></circle>
<circle class="ec-dot" cx="412.3" cy="196.7" r="2.6"><title>32%: 107,509 diamonds on average (1.9 attempts)</title></circle>
<circle class="ec-dot" cx="423.7" cy="193.2" r="2.6"><title>33%: 123,857 diamonds on average (2.2 attempts)</title></circle>
<circle class="ec-dot" cx="435.1" cy="192.4" r="2.6"><title>34%: 127,899 diamonds on average (2.3 attempts)</title></circle>
<circle class="ec-dot" cx="446.6" cy="182.7" r="2.6"><title>35%: 188,407 diamonds on average (3.4 attempts)</title></circle>
<circle class="ec-dot" cx="458.0" cy="182.5" r="2.6"><title>36%: 189,674 diamonds on average (3.4 attempts)</title></circle>
<circle class="ec-dot" cx="469.4" cy="171.8" r="2.6"><title>37%: 291,034 diamonds on average (5.3 attempts)</title></circle>
<circle class="ec-dot" cx="480.9" cy="169.8" r="2.6"><title>38%: 315,824 diamonds on average (5.8 attempts)</title></circle>
<circle class="ec-dot" cx="492.3" cy="165.4" r="2.6"><title>39%: 377,015 diamonds on average (6.9 attempts)</title></circle>
<circle class="ec-dot" cx="503.7" cy="155.2" r="2.6"><title>40%: 566,284 diamonds on average (10.3 attempts)</title></circle>
<circle class="ec-dot" cx="515.1" cy="154.5" r="2.6"><title>41%: 581,718 diamonds on average (10.6 attempts)</title></circle>
<circle class="ec-dot" cx="526.6" cy="132.8" r="2.6"><title>42%: 1,391,221 diamonds on average (25.5 attempts)</title></circle>
<circle class="ec-dot" cx="538.0" cy="132.4" r="2.6"><title>43%: 1,409,309 diamonds on average (25.8 attempts)</title></circle>
<circle class="ec-dot" cx="549.4" cy="113.7" r="2.6"><title>44%: 2,980,756 diamonds on average (54.6 attempts)</title></circle>
<circle class="ec-dot" cx="560.9" cy="108.3" r="2.6"><title>45%: 3,703,813 diamonds on average (67.8 attempts)</title></circle>
<circle class="ec-dot" cx="572.3" cy="103.8" r="2.6"><title>46%: 4,431,286 diamonds on average (81.2 attempts)</title></circle>
<circle class="ec-dot" cx="595.1" cy="81.3" r="2.6"><title>48%: 10,938,315 diamonds on average (200.5 attempts)</title></circle>
<circle class="ec-dot" cx="618.0" cy="31.8" r="2.6"><title>50%: 79,234,326 diamonds on average (1452.6 attempts)</title></circle>
<line class="ec-axis" x1="58" y1="256" x2="618" y2="256"/>
<text class="ec-t ec-muted" x="103.7" y="271" text-anchor="middle">5%</text>
<text class="ec-t ec-muted" x="160.9" y="271" text-anchor="middle">10%</text>
<text class="ec-t ec-muted" x="218.0" y="271" text-anchor="middle">15%</text>
<text class="ec-t ec-muted" x="275.1" y="271" text-anchor="middle">20%</text>
<text class="ec-t ec-muted" x="332.3" y="271" text-anchor="middle">25%</text>
<text class="ec-t ec-muted" x="389.4" y="271" text-anchor="middle">30%</text>
<text class="ec-t ec-muted" x="446.6" y="271" text-anchor="middle">35%</text>
<text class="ec-t ec-muted" x="503.7" y="271" text-anchor="middle">40%</text>
<text class="ec-t ec-muted" x="560.9" y="271" text-anchor="middle">45%</text>
<text class="ec-t ec-muted" x="618.0" y="271" text-anchor="middle">50%</text>
<text class="ec-t ec-muted" x="338.0" y="290" text-anchor="middle">amplification reached (or better)</text>
<text class="ec-b ec-ink" x="64.0" y="226.0">58.3K</text>
<text class="ec-b ec-ink" x="612.0" y="23.8" text-anchor="end">79.2M</text>
</svg>

<sub>Log scale - the range spans three orders of magnitude. The shading needs a
renderer that keeps inline SVG; the VS Code preview does, GitHub strips it.</sub>

## What this says

**Everything below 35% is close to free**, which is why the table starts there.
25% costs 65.7K and lands first try
86% of the time; 30% costs
93.9K. The real spending starts above that.

**The multiplier itself keeps growing** - each extra 5 points costs more,
relative to the step before it, than the last one did:

| step | mean diamonds | multiplier |
|---|---|---|
| 35% to 40% | 566K | 3.0x |
| 40% to 45% | 3.7M | 6.5x |
| 45% to 50% | 79.2M | 21.4x |

**The last 5 points cost 21x everything before them.**
Getting to 45% averages 3.7M; going from there to a perfect
50% averages 79.2M, because a perfect relic needs a flawless attempt
and that happens 0.0689% of the time - once per
1,453 attempts.

**The averages hide a very long tail.** The median run reaches 50% for
55.3M, but the 90th percentile is 181M. First-reach times
are geometric, so the spread is as wide as the mean: budgeting the average is a
coin flip, not a plan.

**Where to stop is a judgement call, but 40% is the value corner.**
566K buys 40%; the next 10 points cost
140x that again. Put the other way: the
diamonds behind one perfect relic would farm about
**140 separate relics at 40%**, or
21 at 45%.

## Cross-check

Two independent routes to the same number, which is the reason to trust it:

| | |
|---|---|
| simulated mean to 50% | 79,234,326 diamonds |
| 1,453 attempts x 54,545 per attempt | 79,232,686 diamonds |
| gap | 0.002% |

The simulated attempt count also matches the geometric expectation: 1,453
attempts against a predicted 1 / 0.068871% = 1,452.

## Assumptions

1. A summon is 11 **independent** relics, each equally
   likely to be any of the 12 types. No pity, no duplicate
   protection, no banner weighting.
2. Only crit relics have any value; the other 11 types are
   discarded. If they are worth something, the true cost per crit relic is lower.
3. Leftover crit relics carry over between attempts, so nothing is wasted except
   within the final partial summon.
4. Attempts are independent and the relic keeps its best amplification ever
   rolled - an attempt can never make an existing relic worse.
5. The inheritance itself is played to the solver's optimal weighted policy
   (+5% per glory success, -2% per despair success, floored at 0%). A worse
   policy costs more diamonds for the same mark.
