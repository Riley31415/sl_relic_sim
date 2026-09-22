#!/usr/bin/env python3
"""Command line front end for the relic inheritance solver.

    python main.py                  # exact solve + report for inheritor level 1
    python main.py --level 4
    python main.py --mc 200000      # cross-check the exact numbers with rollouts
    python main.py --play 3         # show sample attempts, move by move
    python main.py --all-levels     # summary table for every known level
"""

from __future__ import annotations

import argparse
import math
import random

from relic import (
    DEAD_END,
    DESPAIR,
    GLORY,
    DOCUMENTED_MAX_LEVEL,
    TIERS,
    TRAIN,
    Inheritance,
    UnknownLevelError,
    config_for_level,
    target,
    weighted,
)

LETTER = {GLORY: "G", DESPAIR: "D", TRAIN: "T"}


def strategy_from_args(args):
    """A target strategy when --target was given, otherwise the weighted one."""
    if args.target:
        return target(args.target[0], args.target[1],
                      w_glory=args.amp_glory, w_despair=args.amp_despair)
    return weighted(args.amp_glory, args.amp_despair)


def pct(x: float, places: int = 2) -> str:
    return f"{100 * x:.{places}f}%"


def amp_pct(value: float, places: int = 2) -> str:
    """An amplification as a percentage - 34%, 31.15%, 0%."""
    if value == int(value) and places == 0:
        return f"{value:g}%"
    return f"{value:.{places}f}%"


def pct3(x: float) -> str:
    """A probability as a percentage with three digits showing.

    100%, 98.3%, 9.36%, 0.07% - the decimal places shrink as the number grows
    so every cell reads at the same glance-width.  Anything too small to show at
    two decimals rounds down to a plain 0%.
    """
    v = 100 * x
    if v >= 100:
        return f"{v:.0f}%"
    if v >= 10:
        return f"{v:.1f}%"
    if v >= 0.005:
        return f"{v:.2f}%"
    return "0%"


def print_setup(solver: Inheritance) -> None:
    cfg = solver.cfg
    ladder = "  ".join(pct(t, 0) for t in TIERS)
    print(f"Inheritor level {cfg.level}")
    print(f"  memory slots per bar : {cfg.slots}")
    print(f"  max spirit power     : {cfg.max_spirit}  (spirit power starts here)")
    print(f"  starting mental str. : {cfg.start_mental}")
    print(f"  glory rate modifier  : {cfg.glory_mod:+.0%}")
    print(f"  despair rate modifier: {cfg.despair_mod:+.0%}")
    print(f"  chance ladder        : {ladder}   (starts at {pct(TIERS[cfg.start_tier], 0)})")
    st = cfg.strategy
    print(f"  strategy             : {st.name}")
    if cfg.is_target:
        print(f"  objective            : ALL OR NOTHING - maximise P({cfg.want_glory}+ glory "
              f"successes and {cfg.allow_despair} or fewer despair successes)")
    else:
        print(f"  objective            : maximise amplification "
              f"(+{cfg.w_glory:g}% per glory success, "
              f"-{cfg.w_despair:g}% per despair success)")
    need = max(0, -(-(2 * cfg.slots - cfg.max_spirit) // 2))
    print(f"  spirit power needed  : {2 * cfg.slots} to complete both bars, "
          f"{cfg.max_spirit} on hand -> at least {need} successful mental "
          f"training{'' if need == 1 else 's'} (worth full value at "
          f"{cfg.max_spirit - 2} spirit power or below)")


def print_results(analysis, solver_max: float) -> None:
    cfg = analysis.cfg
    s = cfg.slots
    print()
    print(f"OPTIMAL PLAY  (exact expectimax, {analysis.states_explored:,} states)")
    print(f"  expected glory successes : {analysis.e_glory:.4f} / {s}"
          f"   ({pct(analysis.e_glory / s)} of the bar, higher is better)")
    print(f"  expected despair success : {analysis.e_despair_success:.4f} / {s}"
          f"   ({pct(analysis.e_despair_success / s)} of the bar, LOWER is better)")
    print(f"  expected amplification   : {amp_pct(analysis.e_amplification, 3):>8}"
          f" of a possible {amp_pct(solver_max, 0)}")
    if cfg.is_target:
        print(f"  P(hit target {cfg.want_glory}+ glory,"
              f" <={cfg.allow_despair} despair) : {pct(analysis.p_target, 4)}")
    print(f"  P(dead-end wipe)         : {pct(analysis.p_dead_end, 4)}")
    total_actions = sum(analysis.action_counts.values())
    print(f"  expected actions taken   : {total_actions:.2f}"
          f"  (attempt glory {analysis.action_counts[GLORY]:.2f},"
          f" attempt despair {analysis.action_counts[DESPAIR]:.2f},"
          f" mental training {analysis.action_counts[TRAIN]:.2f} of {cfg.start_mental})")


def print_distribution(analysis) -> None:
    s = analysis.cfg.slots
    glory_marginal = [0.0] * (s + 1)
    despair_marginal = [0.0] * (s + 1)
    for (g, d), p in analysis.dist.items():
        glory_marginal[g] += p
        despair_marginal[d] += p
    print()
    print("OUTCOME DISTRIBUTION  (glory successes: want high. despair successes: want low)")
    print(f"  {'count':>5} | {'glory successes':>16} | {'despair succ.':>14}")
    for i in range(s + 1):
        gbar = "#" * round(glory_marginal[i] * 40)
        print(f"  {i:>5} | {pct(glory_marginal[i]):>16} | {pct(despair_marginal[i]):>14}  {gbar}")
    print("  (bars show the glory marginal)")

    print()
    print("  joint table, rows = glory successes, cols = despair successes")
    header = "        " + "".join(f"{d:>9}" for d in range(s + 1))
    print(header)
    for g in range(s + 1):
        cells = "".join(f"{pct(analysis.dist.get((g, d), 0.0), 2):>9}" for d in range(s + 1))
        print(f"  g={g}  {cells}")


def print_policy(solver: Inheritance, analysis) -> None:
    cfg = solver.cfg
    print()
    print("POLICY: what optimal play actually does")
    print()
    print("  share of decisions by chance tier (probability weighted)")
    print(f"  {'tier':>6} | {'attempt glory':>14} {'attempt despair':>16} "
          f"{'mental training':>16} | {'weight':>8}")
    for t in range(len(TIERS)):
        counts = analysis.action_by_tier.get(t)
        if not counts:
            continue
        total = sum(counts.values())
        print(f"  {pct(TIERS[t], 0):>6} | {pct(counts[GLORY] / total, 1):>14}"
              f" {pct(counts[DESPAIR] / total, 1):>16}"
              f" {pct(counts[TRAIN] / total, 1):>16} | {total:>8.3f}")

    print()
    print("  dominant choice by tier x spirit power"
          "  (G attempt glory / D attempt despair / T mental training)")
    print("  lowercase = the choice is mixed, it depends on how full the bars are")
    header = "         " + "".join(f"{sp:>4}" for sp in range(cfg.max_spirit + 1))
    print(f"  {'':>6}   spirit power")
    print(header)
    for t in range(len(TIERS)):
        row = []
        for sp in range(cfg.max_spirit + 1):
            counts = analysis.action_by_sp.get((t, sp))
            if not counts:
                row.append("   .")
                continue
            total = sum(counts.values())
            action, mass = counts.most_common(1)[0]
            letter = LETTER[action]
            row.append(f"{letter if mass / total > 0.999 else letter.lower():>4}")
        print(f"  {pct(TIERS[t], 0):>6} " + "".join(row))


def print_opening(solver: Inheritance) -> None:
    state = solver.start_state()
    print()
    print("  opening move, expected score of each option from the start state")
    best = solver.best_action(state)
    for action in (GLORY, DESPAIR, TRAIN):
        if action not in solver.legal_actions(state):
            continue
        ev = sum(p * solver.value(n)[1] for p, n, _ok in solver.transitions(state, action) if p)
        mark = "  <== optimal" if action == best else ""
        print(f"    {action:<16} {ev:7.4f}{mark}")


def print_safety_tradeoff(args) -> None:
    """How much score does dodging the dead-end wipe actually cost?

    The wipe penalty is an extra charge, in score points, applied to a wiped
    attempt.  0 is the plain expected-score maximiser; "lexicographic" drives
    the wipe chance as low as the rules allow before it looks at score at all.
    """
    print()
    print("SAFETY TRADE-OFF: cost of dodging the dead-end wipe")
    print(f"  {'objective':>16} | {'E[glory]':>9} {'E[desp succ]':>13} "
          f"{'E[amp]':>9} {'P(wipe)':>9}")
    rows = [(f"wipe penalty {p:g}", dict(wipe_penalty=p)) for p in (0, 3, 10, 30)]
    rows.append(("min wipe first", dict(safety_first=True)))
    for label, kwargs in rows:
        cfg = config_for_level(args.level, strategy=strategy_from_args(args), **kwargs)
        a = Inheritance(cfg).analyse()
        print(f"  {label:>16} | {a.e_glory:>9.4f} {a.e_despair_success:>13.4f} "
              f"{amp_pct(a.e_amplification, 3):>9} {pct(a.p_dead_end, 4):>9}")


def run_monte_carlo(solver: Inheritance, trials: int, seed: int) -> None:
    rng = random.Random(seed)
    g = d = 0.0
    dead = 0
    for _ in range(trials):
        res = solver.attempt(rng)
        g += res["glory_success"]
        d += res["despair_success"]
        dead += res["dead_end"]
    print()
    print(f"MONTE CARLO CROSS-CHECK ({trials:,} attempts, seed {seed})")
    print(f"  mean glory successes : {g / trials:.4f}")
    print(f"  mean despair success : {d / trials:.4f}")
    print(f"  dead-end rate        : {pct(dead / trials, 4)}")


def play(solver: Inheritance, count: int, seed: int) -> None:
    rng = random.Random(seed)
    for i in range(count):
        log = []
        res = solver.attempt(rng, log)
        print()
        print(f"SAMPLE ATTEMPT {i + 1}   (SUCCESS = the roll succeeded: good on "
              f"glory, bad on despair; bars read successes/attempted)")
        for state, action, ok, nxt in log:
            gf, gs, df, ds, ms, sp, t = state
            chance = solver.chance(action, t)
            print(f"  sp={sp} ms={ms} tier={pct(TIERS[t], 0):>3} "
                  f"glory={gs}/{gf} despair={ds}/{df}  ->  "
                  f"{action:<16} @ {pct(chance, 0):>3}  "
                  f"{'SUCCESS' if ok else 'fail':<7} -> next tier {pct(TIERS[nxt[6]], 0)}")
        if res["dead_end"]:
            print(f"  DEAD END - attempt wiped (0 glory successes, "
                  f"{res['despair_success']} despair successes)")
        else:
            print(f"  result: {res['glory_success']} glory successes, "
                  f"{res['despair_success']} despair successes "
                  f"(sp left {res['spirit_left']}, ms left {res['mental_left']})")


def all_levels(args) -> None:
    print(f"{'lvl':>4} {'slots':>6} {'maxSP':>6} {'glory%':>7} {'desp%':>7} "
          f"{'E[glory]':>9} {'E[desp succ]':>13} {'E[amp]':>9} {'per slot':>9} {'P(wipe)':>9}")
    for level in range(1, args.max_level + 1):
        cfg = config_for_level(level, strategy=strategy_from_args(args))
        a = Inheritance(cfg).analyse()
        print(f"{level:>4} {cfg.slots:>6} {cfg.max_spirit:>6} {cfg.glory_mod:>+7.0%} "
              f"{cfg.despair_mod:>+7.0%} {a.e_glory:>9.4f} {a.e_despair_success:>13.4f} "
              f"{amp_pct(a.e_amplification, 2):>9} "
              f"{a.e_amplification / (cfg.w_glory * cfg.slots):>8.1%} "
              f"{pct(a.p_dead_end, 4):>9}")


def compare_strategies(args) -> None:
    """Every level x {the weighted objective, the all-or-nothing target}."""
    tg, td = (args.target or (4, 2))
    strategies = [
        weighted(args.amp_glory, args.amp_despair),
        target(tg, td, w_glory=args.amp_glory, w_despair=args.amp_despair),
    ]
    print(f"Strategy comparison: weighted amplification vs the all-or-nothing "
          f"target ({tg}, {td})")
    print("  E[desp] is despair SUCCESSES (lower is better); failure = dead-end wipe")
    print()
    print(f"  {'lvl':>3} {'slots':>5} | {'strategy':>22} | {'E[glory]':>8} "
          f"{'E[desp]':>8} {'E[amp]':>8} {'failure':>8} | {'hit rate':>9}")
    for level in range(1, args.max_level + 1):
        for i, strat in enumerate(strategies):
            cfg = config_for_level(level, strategy=strat)
            a = Inheritance(cfg).analyse()
            lvl = f"{level:>3} {cfg.slots:>5}" if i == 0 else " " * 9
            hit = pct(a.p_target, 3) if cfg.is_target else "-"
            print(f"  {lvl} | {strat.name:>22} | {a.e_glory:>8.4f} "
                  f"{a.e_despair_success:>8.4f} {amp_pct(a.e_amplification, 2):>8} "
                  f"{pct(a.p_dead_end, 3):>8} | {hit:>9}")
        print()

    print(f"  Is chasing the box worth it?  P(>={tg} glory and <={td} despair), "
          f"measured the same way for both")
    print(f"  {'lvl':>3} | {'weighted':>10} {'all-or-nothing':>15} {'gain':>8} | "
          f"{'amp given up':>13}")
    for level in range(1, args.max_level + 1):
        rates, amps = [], []
        for strat in strategies:
            a = Inheritance(config_for_level(level, strategy=strat)).analyse()
            rates.append(sum(p for (g, d), p in a.dist.items() if g >= tg and d <= td))
            amps.append(a.e_amplification)
        print(f"  {level:>3} | {pct(rates[0], 3):>10} {pct(rates[1], 3):>15} "
              f"{pct(rates[1] - rates[0], 3):>8} | "
              f"{amp_pct(amps[1] - amps[0], 2):>13}")


def print_amplification_table(args) -> None:
    """P(this glory/despair combination or better), ranked by amplification.

    "Better" means a higher amplification, w_glory * glory - w_despair * despair,
    so each cell sums the probability of every outcome whose amplification is at
    least that cell's.  Glory increases left to right and despair DEcreases top
    to bottom, which puts the best corner (full glory bar, no despair success)
    at the bottom right and the worst at the top left.
    """
    strat = weighted(args.amp_glory, args.amp_despair)
    cfg = config_for_level(args.level, strategy=strat)
    solver = Inheritance(cfg)
    a = solver.analyse()
    s = cfg.slots

    amp_of = {(g, d): solver.amplification(g, d)
              for g in range(s + 1) for d in range(s + 1)}
    e_amp = sum(solver.amplification(g, d) * p for (g, d), p in a.dist.items())

    print(f"Inheritor level {cfg.level}: amplification = "
          f"+{args.amp_glory:g}% per glory success, "
          f"-{args.amp_despair:g}% per despair success")
    print(f"  {cfg.slots} slots per bar, {cfg.max_spirit} max spirit power, "
          f"glory {cfg.glory_mod:+.0%}, despair {cfg.despair_mod:+.0%}")
    print(f"  expected amplification : {amp_pct(e_amp, 3)}"
          f"   (range {amp_pct(solver.min_amplification(), 0)} to "
          f"{amp_pct(solver.max_amplification(), 0)})")
    print(f"  expected glory         : {a.e_glory:.4f} / {s}")
    print(f"  expected despair       : {a.e_despair_success:.4f} / {s}")
    print(f"  dead-end wipe          : {pct(a.p_dead_end, 3)}  "
          f"(scores {amp_pct(solver.amplification(0, s), 0)}, the worst possible)")
    print(f"  most likely single cell: {pct3(max(a.dist.values()))} "
          f"of attempts, and the grid below sums to 100%")

    # bucket the outcome distribution by amplification, then take upper tails
    by_amp = {}
    for (g, d), prob in a.dist.items():
        key = amp_of[(g, d)]
        by_amp[key] = by_amp.get(key, 0.0) + prob
    tail = {}
    running = 0.0
    for value in sorted(by_amp, reverse=True):
        running += by_amp[value]
        tail[value] = running

    def at_least(value):
        """P(amplification >= value), for a value that may not itself occur."""
        above = [v for v in tail if v >= value]
        return tail[min(above)] if above else 0.0

    if args.html:
        print()
        print_amplification_html(solver, a, amp_of)
        print()
        print_amplification_curve_svg(solver, a)
        return

    print()
    print("  P(exactly this combination), percent.  The grid sums to 100%.")
    print("  glory increases left to right; despair decreases top to bottom,")
    print("  so the best outcome is the bottom-right corner.")
    print()
    header = "  desp |" + "".join(f"{g:>8}" for g in range(s + 1)) + "   <- glory"
    print(header)
    print("  " + "-" * (len(header) - 2))
    for d in range(s, -1, -1):
        cells = "".join(f"{pct3(a.dist.get((g, d), 0.0)):>8}" for g in range(s + 1))
        print(f"  {d:>4} |{cells}")
    print()
    print("  the same cells as amplification values:")
    print(header)
    print("  " + "-" * (len(header) - 2))
    for d in range(s, -1, -1):
        cells = "".join(f"{amp_pct(amp_of[(g, d)], 0):>8}" for g in range(s + 1))
        print(f"  {d:>4} |{cells}")


# RdYlGn, but with a LIGHT red at the bottom: most of a joint-probability grid
# sits near zero, and a saturated red there reads as a heavy block rather than
# as "rare".  Light red keeps the rare field calm so the likely cells carry the
# eye.
_HEAT_STOPS = (
    (0.00, (247, 190, 185)),
    (0.25, (252, 174, 132)),
    (0.50, (255, 255, 191)),
    (0.75, (166, 217, 106)),
    (1.00, (26, 152, 80)),
)


def heat_colour(fraction: float):
    """Interpolate the red-to-green scale; returns (background, text) hex."""
    f = min(1.0, max(0.0, fraction))
    for (lo, c_lo), (hi, c_hi) in zip(_HEAT_STOPS, _HEAT_STOPS[1:]):
        if f <= hi:
            span = hi - lo
            k = 0.0 if span == 0 else (f - lo) / span
            rgb = tuple(round(a + (b - a) * k) for a, b in zip(c_lo, c_hi))
            break
    else:
        rgb = _HEAT_STOPS[-1][1]
    # readable text: white on the dark ends, near-black in the pale middle
    return "#%02x%02x%02x" % rgb, _readable_text(rgb)


GLORY_GOLD = (212, 175, 55)      # metallic gold, the glory axis
DESPAIR_RED = (139, 0, 0)        # dark red, the despair axis


def _readable_text(rgb) -> str:
    """Table ink is always black, by choice.

    Flipping to white over the dark end of each ramp would hold contrast better,
    but it makes the grid read as two different tables.  One ink keeps it a
    single surface; see the contrast note beside the table in STRATEGY.md for
    which cells this costs.
    """
    return "#000000"


def ramp_colour(fraction: float, rgb):
    """White at 0 through to `rgb` at 1; returns (background, text) hex."""
    f = min(1.0, max(0.0, fraction))
    mixed = tuple(round(255 + (c - 255) * f) for c in rgb)
    return "#%02x%02x%02x" % mixed, _readable_text(mixed)


def print_amplification_html(solver, a, amp_of) -> None:
    """The joint outcome grid as an HTML table.

    Each cell is the chance of finishing on EXACTLY that many glory and despair
    successes, so the grid sums to 100%.  The shading is relative to the most
    likely cell - green there, red at zero - because no single combination gets
    anywhere near 100% on its own.
    """
    cfg = solver.cfg
    s = cfg.slots
    peak = max(a.dist.values()) if a.dist else 1.0
    cell = ("padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums;"
            "border:1px solid rgba(128,128,128,.35);")
    head = ("padding:3px 7px;text-align:right;border:1px solid rgba(128,128,128,.35);"
            "font-weight:600;")
    # the corner label has no ramp behind it, so it states its own colours
    corner = head + "color:#000000;background:#ffffff;"
    print('<table style="border-collapse:collapse;font-size:13px;">')
    # glory across the top, shaded white -> gold as the count rises
    glory_heads = []
    for g in range(s + 1):
        bg, fg = ramp_colour(g / s, GLORY_GOLD)
        glory_heads.append(f'<th style="{head}background:{bg};color:{fg};">{g}</th>')
    print("<thead><tr>"
          f'<th style="{corner}">despair &darr; / glory &rarr;</th>'
          + "".join(glory_heads)
          + "</tr></thead>")
    print("<tbody>")
    for d in range(s, -1, -1):
        # despair down the side, shaded white -> dark red as the count rises
        d_bg, d_fg = ramp_colour(d / s, DESPAIR_RED)
        cells = []
        for g in range(s + 1):
            p = a.dist.get((g, d), 0.0)
            bg, fg = heat_colour(p / peak if peak else 0.0)
            cells.append(f'<td style="{cell}background:{bg};color:{fg};" '
                         f'title="{g} glory, {d} despair - amplifies '
                         f'{amp_of[(g, d)]:g}%">{pct3(p)}</td>')
        print(f'<tr><th style="{head}background:{d_bg};color:{d_fg};">{d}</th>'
              + "".join(cells) + "</tr>")
    print("</tbody></table>")


def print_amplification_curve_svg(solver, a) -> None:
    """The amplification distribution as an inline SVG column chart.

    One column per achievable amplification, height = P(exactly that value).
    A single series, so no legend: the title names it.  The wipe column is the
    one qualitatively different outcome (a failed attempt, not a low roll), so
    it carries the reserved critical colour and a direct label.  Colours follow
    prefers-color-scheme because a markdown file has no theme toggle to hang a
    data-theme scope on.
    """
    cfg = solver.cfg
    by_amp = {}
    for (g, d), prob in a.dist.items():
        key = solver.amplification(g, d)
        by_amp[key] = by_amp.get(key, 0.0) + prob

    lo = int(solver.min_amplification())
    hi = int(solver.max_amplification())
    span = hi - lo + 1
    peak = max(by_amp.values())
    y_top = 0.05 * math.ceil(peak / 0.05)          # 0.15 for a 13.9% peak
    mean = sum(v * p for v, p in by_amp.items())

    pitch, bar_w = 9, 7
    ml, mr, mt, mb = 46, 14, 26, 36
    plot_w, plot_h = span * pitch, 210
    width, height = ml + plot_w + mr, mt + plot_h + mb

    def x_of(value):      # left edge of that value's column
        return ml + (value - lo) * pitch + (pitch - bar_w) / 2

    def x_mid(value):     # continuous position, for the mean marker
        return ml + (value - lo) * pitch + pitch / 2

    def y_of(p):
        return mt + plot_h * (1 - p / y_top)

    out = [
        f'<svg viewBox="0 0 {width} {height}" width="{width}" height="{height}" '
        f'role="img" xmlns="http://www.w3.org/2000/svg" '
        f'aria-label="Distribution of relic amplification at inheritor level {cfg.level}">',
        f'<title>Amplification distribution, inheritor level {cfg.level}</title>',
        f'<desc>Chance of each exact amplification under optimal weighted play. '
        f'Mean {mean:.2f} percent. Peak {100 * peak:.1f} percent of attempts at '
        f'an amplification of {max(by_amp, key=by_amp.get):g} percent.</desc>',
        "<style>"
        ".ac-bar{fill:#2a78d6}.ac-wipe{fill:#d03b3b}"
        ".ac-grid{stroke:#e1e0d9;stroke-width:1}.ac-axis{stroke:#c3c2b7;stroke-width:1}"
        ".ac-ink{fill:#52514e}.ac-muted{fill:#898781}.ac-mean{stroke:#52514e}"
        ".ac-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}"
        ".ac-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}"
        "@media(prefers-color-scheme:dark){"
        ".ac-bar{fill:#3987e5}.ac-grid{stroke:#2c2c2a}.ac-axis{stroke:#383835}"
        ".ac-ink{fill:#c3c2b7}.ac-mean{stroke:#c3c2b7}}"
        "</style>",
    ]

    # recessive horizontal grid + y labels
    step = 0.05
    ticks = int(round(y_top / step))
    for i in range(ticks + 1):
        p = i * step
        y = y_of(p)
        out.append(f'<line class="ac-grid" x1="{ml}" y1="{y:.1f}" '
                   f'x2="{ml + plot_w}" y2="{y:.1f}"/>')
        out.append(f'<text class="ac-t ac-muted" x="{ml - 8}" y="{y + 4:.1f}" '
                   f'text-anchor="end">{100 * p:.0f}%</text>')
    out.append(f'<text class="ac-t ac-muted" x="{ml - 8}" y="{mt - 9}" '
               f'text-anchor="end">chance</text>')

    # columns, rounded data-end, 2px surface gap comes from pitch - bar_w
    radius = min(3, bar_w / 2)
    for value in sorted(by_amp):
        p = by_amp[value]
        if p <= 0:
            continue
        x = x_of(value)
        y = y_of(p)
        h = mt + plot_h - y
        r = min(radius, h)
        cls = "ac-wipe" if value == lo else "ac-bar"
        out.append(
            f'<path class="{cls}" d="M{x:.1f},{mt + plot_h} V{y + r:.1f} '
            f'Q{x:.1f},{y:.1f} {x + r:.1f},{y:.1f} H{x + bar_w - r:.1f} '
            f'Q{x + bar_w:.1f},{y:.1f} {x + bar_w:.1f},{y + r:.1f} '
            f'V{mt + plot_h} Z">'
            f'<title>amplification {value:g}: {pct3(p)}</title></path>')

    # baseline and x labels every 10
    base_y = mt + plot_h
    out.append(f'<line class="ac-axis" x1="{ml}" y1="{base_y}" '
               f'x2="{ml + plot_w}" y2="{base_y}"/>')
    for value in range(lo, hi + 1):
        if value % 10:
            continue
        out.append(f'<text class="ac-t ac-muted" x="{x_mid(value):.1f}" '
                   f'y="{base_y + 15}" text-anchor="middle">{value}%</text>')
    out.append(f'<text class="ac-t ac-muted" x="{ml + plot_w / 2:.1f}" '
               f'y="{height - 6}" text-anchor="middle">amplification '
               f'(+{cfg.w_glory:g}% per glory success, '
               f'-{cfg.w_despair:g}% per despair success)</text>')

    # selective direct labels: the mean, the mode, and the wipe
    mx = x_mid(mean)
    out.append(f'<line class="ac-mean" x1="{mx:.1f}" y1="{mt - 6}" x2="{mx:.1f}" '
               f'y2="{base_y}" stroke-dasharray="3 3" stroke-width="1.5"/>')
    out.append(f'<text class="ac-b ac-ink" x="{mx + 5:.1f}" y="{mt - 12}">'
               f'mean {mean:.1f}%</text>')
    mode_v = max(by_amp, key=by_amp.get)
    out.append(f'<text class="ac-b ac-ink" x="{x_mid(mode_v):.1f}" '
               f'y="{y_of(peak) - 6:.1f}" text-anchor="middle">{pct3(peak)}</text>')
    # the floored column holds the wipe, plus any run washed out to zero
    wipe_p = by_amp.get(lo, 0.0)
    if wipe_p > 0:
        # only call out washouts when they are big enough to show at all:
        # under 0.005% they round away to 0.00% and the column is just the wipe
        washouts = wipe_p - a.p_dead_end
        name = "wipe" if washouts < 5e-5 else "wipe + washouts"
        out.append(f'<text class="ac-b ac-wipe" x="{x_mid(lo) + 6:.1f}" '
                   f'y="{y_of(wipe_p) - 8:.1f}" text-anchor="start">{name} '
                   f'{pct3(wipe_p)}</text>')
    out.append("</svg>")
    print("\n".join(out))


def main() -> int:
    parser = argparse.ArgumentParser(description="Slayer Legend relic inheritance solver")
    parser.add_argument("--level", type=int, default=1, help="inheritor level (1-8)")

    parser.add_argument("--mc", type=int, default=0, help="Monte Carlo attempts to cross-check")
    parser.add_argument("--play", type=int, default=0, help="show N sample attempts")
    parser.add_argument("--seed", type=int, default=12345)
    parser.add_argument("--all-levels", action="store_true", help="summary table for levels 1-8")
    parser.add_argument("--no-dist", action="store_true", help="skip the distribution tables")
    parser.add_argument("--safety-first", action="store_true",
                        help="minimise the dead-end wipe chance before maximising score")
    parser.add_argument("--wipe-penalty", type=float, default=0.0,
                        help="extra score charged for a wipe (middle ground; default 0)")
    parser.add_argument("--no-tradeoff", action="store_true",
                        help="skip the safety trade-off table")
    parser.add_argument("--target", type=int, nargs=2, metavar=("GLORY", "DESPAIR"),
                        help="chase an all-or-nothing target: >=GLORY glory successes "
                             "and <=DESPAIR despair successes")
    parser.add_argument("--strategies", action="store_true",
                        help="compare (999,0) against (4,2) across all levels")
    parser.add_argument("--html", action="store_true",
                        help="render the amplification table as a shaded HTML table")
    parser.add_argument("--amp-table", action="store_true",
                        help="P(combination or better) grid, ranked by amplification")
    parser.add_argument("--amp-glory", type=float, default=5.0,
                        help="amplification per glory success (default 5)")
    parser.add_argument("--amp-despair", type=float, default=2.0,
                        help="amplification lost per despair success (default 2)")
    parser.add_argument("--max-level", type=int, default=DOCUMENTED_MAX_LEVEL,
                        help=f"highest level in the level tables (default {DOCUMENTED_MAX_LEVEL})")
    args = parser.parse_args()

    try:
        if args.amp_table:
            print_amplification_table(args)
            return 0
        if args.strategies:
            compare_strategies(args)
            return 0
        if args.all_levels:
            all_levels(args)
            return 0
        cfg = config_for_level(
            args.level,
            strategy=strategy_from_args(args),
            safety_first=args.safety_first,
            wipe_penalty=args.wipe_penalty,
        )
    except UnknownLevelError as exc:
        parser.error(str(exc))
        return 2

    solver = Inheritance(cfg)
    analysis = solver.analyse()
    print_setup(solver)
    print_results(analysis, Inheritance(cfg).max_amplification())
    if not args.no_dist:
        print_distribution(analysis)
    print_policy(solver, analysis)
    print_opening(solver)
    if not args.no_tradeoff:
        print_safety_tradeoff(args)
    if args.mc:
        run_monte_carlo(solver, args.mc, args.seed)
    if args.play:
        play(solver, args.play, args.seed)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
