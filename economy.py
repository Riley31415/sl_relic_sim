#!/usr/bin/env python3
"""Monte Carlo: how many diamonds it takes to reach each amplification mark.

The relic solver says what one inheritance attempt is worth.  This says what it
costs to keep attempting until the relic is good enough.

    5000 diamonds buys 11 random relics
    there are 12 relic types; type 1 is the crit relic, the only one we want
    one inheritance attempt burns 10 crit relics

So a summon yields Binomial(11, 1/12) crit relics - 0.9167 on average - and an
attempt needs 10 of them, which works out to about 54,545 diamonds per attempt
once leftovers carry over.

Each attempt re-rolls the relic, and you keep the best result you have ever hit,
so the question is: how many attempts (and therefore diamonds) before the best
so far reaches 20%, 30%, 45%, 50%?

    python economy.py --level 20 --sims 5000
    python economy.py --chart > cost.svg

The per-attempt outcome is drawn from the solver's exact outcome distribution
rather than by replaying the policy move by move.  Those are the same
distribution - analyse() enumerates every branch - so this is a speedup, not an
approximation.
"""

from __future__ import annotations

import argparse
import bisect
import math
import random

from relic import Inheritance, config_for_level, weighted

DIAMONDS_PER_SUMMON = 5000
RELICS_PER_SUMMON = 11
RELIC_TYPES = 12          # type 1 is the crit relic
RELICS_PER_ATTEMPT = 10   # crit relics burned by one inheritance attempt


def summon_cdf(relics: int = RELICS_PER_SUMMON, types: int = RELIC_TYPES):
    """Cumulative distribution of crit relics from one summon: Binomial(11, 1/12)."""
    p = 1.0 / types
    cum, running = [], 0.0
    for k in range(relics + 1):
        running += math.comb(relics, k) * p ** k * (1 - p) ** (relics - k)
        cum.append(running)
    cum[-1] = 1.0
    return cum


def amplification_cdf(solver: Inheritance, analysis):
    """Distinct amplifications and their cumulative probability, ascending."""
    by_amp = {}
    for (g, d), prob in analysis.dist.items():
        key = solver.amplification(g, d)
        by_amp[key] = by_amp.get(key, 0.0) + prob
    values = sorted(v for v, prob in by_amp.items() if prob > 0)
    cum, running = [], 0.0
    for v in values:
        running += by_amp[v]
        cum.append(running)
    cum[-1] = 1.0
    return values, cum


def diamonds_per_attempt() -> float:
    """Long-run diamonds per attempt, once leftover crit relics carry over."""
    crits_per_summon = RELICS_PER_SUMMON / RELIC_TYPES
    return RELICS_PER_ATTEMPT / crits_per_summon * DIAMONDS_PER_SUMMON


def run(level: int = 20, sims: int = 5000, seed: int = 20260922, marks=None):
    """Play out `sims` farming runs; return the cost of first reaching each mark.

    A run summons until it can afford an attempt, attempts, keeps the best
    amplification so far, and stops once the top mark is reached.  The diamond
    total at the moment each mark is first met is recorded.
    """
    solver = Inheritance(config_for_level(level, strategy=weighted()))
    analysis = solver.analyse()
    values, amp_cum = amplification_cdf(solver, analysis)
    scum = summon_cdf()

    thresholds = [v for v in values if v > 0] if marks is None else sorted(marks)
    top = thresholds[-1]
    rng = random.Random(seed)
    # costs[i] collects, across runs, the diamonds spent to first reach thresholds[i]
    costs = [[] for _ in thresholds]
    attempts_at = [[] for _ in thresholds]

    rand = rng.random
    bis = bisect.bisect
    for _ in range(sims):
        crit = summons = attempts = 0
        best = 0.0
        index = 0
        while index < len(thresholds):
            while crit < RELICS_PER_ATTEMPT:
                crit += bis(scum, rand())
                summons += 1
            crit -= RELICS_PER_ATTEMPT
            attempts += 1
            amp = values[bis(amp_cum, rand())]
            if amp > best:
                best = amp
                spent = summons * DIAMONDS_PER_SUMMON
                while index < len(thresholds) and thresholds[index] <= best:
                    costs[index].append(spent)
                    attempts_at[index].append(attempts)
                    index += 1
            if best >= top and index >= len(thresholds):
                break

    rows = []
    for i, mark in enumerate(thresholds):
        series = sorted(costs[i])
        n = len(series)
        p_reach = sum(prob for v, prob in zip(values, _pmf(amp_cum)) if v >= mark)
        rows.append({
            "mark": mark,
            "p_attempt": p_reach,
            "mean_attempts": sum(attempts_at[i]) / n,
            "mean": sum(series) / n,
            "median": series[n // 2],
            "p90": series[min(n - 1, int(0.90 * n))],
            "runs": n,
        })
    return rows, analysis, solver


def _pmf(cum):
    out, prev = [], 0.0
    for c in cum:
        out.append(c - prev)
        prev = c
    return out


def human(value: float) -> str:
    """Diamonds, abbreviated: 54.5K, 1.23M, 81.6M."""
    for cut, suffix in ((1e9, "B"), (1e6, "M"), (1e3, "K")):
        if value >= cut:
            return f"{value / cut:.3g}{suffix}"
    return f"{value:.0f}"


def table_rows(rows, step: int = 1, min_mark: float | None = 35.0):
    """Pick the marks worth printing.

    Everything below about 30% is reached on the first attempt, so the low marks
    are dozens of identical lines; `min_mark` cuts them.  `step` thins what is
    left, and the top mark is always kept.
    """
    keep = [r for r in rows if min_mark is None or r["mark"] >= min_mark]
    if not keep:
        keep = rows
    if step > 1:
        thinned = [r for r in keep if r["mark"] % step == 0]
        if thinned and thinned[-1] is not keep[-1]:
            thinned.append(keep[-1])
        keep = thinned or keep
    return keep


def print_table(rows, analysis, solver, sims: int, step: int = 1,
                min_mark: float | None = 35.0) -> None:
    cfg = solver.cfg
    print(f"Diamond cost to reach each amplification mark - inheritor level {cfg.level}")
    print(f"  {DIAMONDS_PER_SUMMON:,} diamonds = {RELICS_PER_SUMMON} relics, "
          f"1 in {RELIC_TYPES} is a crit relic, "
          f"{RELICS_PER_ATTEMPT} crit relics per attempt")
    print(f"  -> {RELICS_PER_SUMMON / RELIC_TYPES:.4f} crit relics per summon, "
          f"about {diamonds_per_attempt():,.0f} diamonds per attempt")
    print(f"  Monte Carlo over {sims:,} farming runs, each run keeping its best "
          f"amplification so far")
    if min_mark is not None:
        print(f"  showing marks from {min_mark:.0f}% up; everything below that is "
              f"one attempt (--min-mark 0 for all)")
    print()
    print(f"  {'amp':>5} | {'P(one attempt)':>15} {'mean attempts':>14} | "
          f"{'mean diamonds':>14} {'median':>10} {'90th pct':>10}")
    print("  " + "-" * 80)
    for r in table_rows(rows, step, min_mark):
        print(f"  {r['mark']:>4.0f}% | {r['p_attempt']:>14.4%} {r['mean_attempts']:>14.1f} | "
              f"{r['mean']:>14,.0f} {human(r['median']):>10} {human(r['p90']):>10}")


def chart_svg(rows, solver) -> str:
    """Cost against amplification, as an inline SVG. Log y - the range is 1000x."""
    cfg = solver.cfg
    marks = [r["mark"] for r in rows]
    costs = [r["mean"] for r in rows]
    lo_x, hi_x = marks[0], marks[-1]
    lo_e = math.floor(math.log10(min(costs)))
    hi_e = math.ceil(math.log10(max(costs)))

    ml, mr, mt, mb = 58, 16, 26, 40
    plot_w, plot_h = 560, 230
    width, height = ml + plot_w + mr, mt + plot_h + mb

    def x_of(mark):
        return ml + (mark - lo_x) / (hi_x - lo_x) * plot_w

    def y_of(cost):
        frac = (math.log10(cost) - lo_e) / (hi_e - lo_e)
        return mt + plot_h * (1 - frac)

    out = [
        f'<svg viewBox="0 0 {width} {height}" width="{width}" height="{height}" '
        f'role="img" xmlns="http://www.w3.org/2000/svg" '
        f'aria-label="Diamonds needed to reach each amplification mark at level {cfg.level}">',
        f'<title>Diamond cost by amplification mark, inheritor level {cfg.level}</title>',
        f'<desc>Mean diamonds to first reach each amplification, log scale. '
        f'{human(costs[0])} at {marks[0]:.0f} percent rising to {human(costs[-1])} '
        f'at {marks[-1]:.0f} percent.</desc>',
        "<style>"
        ".ec-line{fill:none;stroke:#2a78d6;stroke-width:2}"
        ".ec-dot{fill:#2a78d6}"
        ".ec-grid{stroke:#e1e0d9;stroke-width:1}.ec-axis{stroke:#c3c2b7;stroke-width:1}"
        ".ec-ink{fill:#52514e}.ec-muted{fill:#898781}"
        ".ec-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}"
        ".ec-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}"
        "@media(prefers-color-scheme:dark){"
        ".ec-line{stroke:#3987e5}.ec-dot{fill:#3987e5}"
        ".ec-grid{stroke:#2c2c2a}.ec-axis{stroke:#383835}.ec-ink{fill:#c3c2b7}}"
        "</style>",
    ]

    for e in range(lo_e, hi_e + 1):
        y = y_of(10 ** e)
        out.append(f'<line class="ec-grid" x1="{ml}" y1="{y:.1f}" '
                   f'x2="{ml + plot_w}" y2="{y:.1f}"/>')
        out.append(f'<text class="ec-t ec-muted" x="{ml - 8}" y="{y + 4:.1f}" '
                   f'text-anchor="end">{human(10 ** e)}</text>')
    out.append(f'<text class="ec-t ec-muted" x="{ml - 8}" y="{mt - 9}" '
               f'text-anchor="end">diamonds</text>')

    points = " ".join(f"{x_of(m):.1f},{y_of(c):.1f}" for m, c in zip(marks, costs))
    out.append(f'<polyline class="ec-line" points="{points}"/>')
    for m, c, r in zip(marks, costs, rows):
        out.append(f'<circle class="ec-dot" cx="{x_of(m):.1f}" cy="{y_of(c):.1f}" r="2.6">'
                   f'<title>{m:.0f}%: {c:,.0f} diamonds on average '
                   f'({r["mean_attempts"]:.1f} attempts)</title></circle>')

    base_y = mt + plot_h
    out.append(f'<line class="ec-axis" x1="{ml}" y1="{base_y}" '
               f'x2="{ml + plot_w}" y2="{base_y}"/>')
    for mark in range(0, int(hi_x) + 1, 5):
        if mark < lo_x:
            continue
        out.append(f'<text class="ec-t ec-muted" x="{x_of(mark):.1f}" '
                   f'y="{base_y + 15}" text-anchor="middle">{mark}%</text>')
    out.append(f'<text class="ec-t ec-muted" x="{ml + plot_w / 2:.1f}" '
               f'y="{height - 6}" text-anchor="middle">'
               f'amplification reached (or better)</text>')

    # label the two ends: the cheap entry and the jackpot
    out.append(f'<text class="ec-b ec-ink" x="{x_of(marks[0]) + 6:.1f}" '
               f'y="{y_of(costs[0]) + 14:.1f}">{human(costs[0])}</text>')
    out.append(f'<text class="ec-b ec-ink" x="{x_of(marks[-1]) - 6:.1f}" '
               f'y="{y_of(costs[-1]) - 8:.1f}" text-anchor="end">'
               f'{human(costs[-1])}</text>')
    out.append("</svg>")
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--level", type=int, default=20)
    parser.add_argument("--sims", type=int, default=5000,
                        help="Monte Carlo farming runs (default 5000)")
    parser.add_argument("--seed", type=int, default=20260922)
    parser.add_argument("--step", type=int, default=1,
                        help="table granularity in amplification points "
                             "(default 1: every achievable mark)")
    parser.add_argument("--min-mark", type=float, default=35.0,
                        help="lowest mark to print (default 35; 0 shows all)")
    parser.add_argument("--chart", action="store_true",
                        help="emit the cost curve as an inline SVG instead of a table")
    args = parser.parse_args()

    rows, analysis, solver = run(args.level, args.sims, args.seed)
    if args.chart:
        print(chart_svg(rows, solver))
        return 0
    print_table(rows, analysis, solver, args.sims, args.step,
                args.min_mark if args.min_mark > 0 else None)
    print()
    top = rows[-1]
    print(f"  the last {rows[-1]['mark'] - rows[-2]['mark']:.0f} points cost "
          f"{top['mean'] / rows[-2]['mean']:.1f}x what everything before them did "
          f"({human(rows[-2]['mean'])} -> {human(top['mean'])})")
    print(f"  cross-check: {top['mean_attempts']:.0f} attempts x "
          f"{diamonds_per_attempt():,.0f} diamonds = "
          f"{top['mean_attempts'] * diamonds_per_attempt():,.0f}, "
          f"against the simulated {top['mean']:,.0f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
