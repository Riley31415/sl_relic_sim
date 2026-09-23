//! `relic cost`: how many diamonds it takes to farm a crit relic up to each
//! amplification mark - and COST.md, generated from it.
//!
//! 5000 diamonds buys 11 random relics over 12 types; only type 1 (crit) is
//! wanted, and an attempt burns 10 of them.  Each attempt re-rolls the relic
//! and the best result ever hit is kept, so the question is how many attempts
//! (and so diamonds) before the best reaches 20%, 30%, 45%, 50%.
//!
//! The per-attempt outcome is drawn from the solver's exact outcome
//! distribution rather than by replaying the policy move by move - the same
//! distribution, so a speedup, not an approximation.

use std::fmt::Write as _;

use crate::format::{commas, general, human, pct};
use crate::rng::Rng;
use crate::solver::{Analysis, Config, Solver, Strategy, Tally};

pub const DIAMONDS_PER_SUMMON: u64 = 5000;
pub const RELICS_PER_SUMMON: u32 = 11;
/// type 1 is the crit relic
pub const RELIC_TYPES: u32 = 12;
/// crit relics burned by one inheritance attempt
pub const RELICS_PER_ATTEMPT: u32 = 10;

/// Long-run diamonds per attempt, once leftover crit relics carry over.
pub fn diamonds_per_attempt() -> f64 {
    let crits_per_summon = f64::from(RELICS_PER_SUMMON) / f64::from(RELIC_TYPES);
    f64::from(RELICS_PER_ATTEMPT) / crits_per_summon * DIAMONDS_PER_SUMMON as f64
}

/// Cumulative distribution of crit relics from `summons` summons at once:
/// Binomial(11 x summons, 1/12).
pub fn summon_cdf(summons: u32) -> Vec<f64> {
    let n = RELICS_PER_SUMMON * summons;
    let p = 1.0 / f64::from(RELIC_TYPES);
    let mut pmf = (1.0 - p).powi(n as i32);
    let mut running = 0.0;
    let mut cum = Vec::with_capacity(n as usize + 1);
    for k in 0..=n {
        running += pmf;
        cum.push(running);
        pmf *= f64::from(n - k) / f64::from(k + 1) * p / (1.0 - p);
    }
    *cum.last_mut().expect("at least one outcome") = 1.0;
    cum
}

/// Crit relics from a batch of summons, drawn exactly.  Big batches let a run
/// skip over thousands of summons in a few draws.
struct Summoner {
    batches: Vec<(u32, Vec<f64>)>,
}

impl Summoner {
    fn new() -> Self {
        Summoner { batches: [64, 8, 1].into_iter().map(|size| (size, summon_cdf(size))).collect() }
    }

    /// Summon until at least `need` crit relics have been drawn in total.
    /// Returns the summons that took; `drawn` carries the running total.
    fn until(&self, drawn: &mut u32, need: u32, rng: &mut Rng) -> u64 {
        let mut summons = 0;
        while *drawn < need {
            // the biggest batch that cannot overshoot: even 11 crits a summon
            // would stay short of `need`, so no first-reach is skipped
            let gap = need - *drawn;
            let (size, cdf) = self
                .batches
                .iter()
                .find(|(size, _)| RELICS_PER_SUMMON * size < gap)
                .unwrap_or(self.batches.last().expect("the single-summon batch"));
            let r = rng.unit();
            *drawn += cdf.partition_point(|&c| c <= r) as u32;
            summons += u64::from(*size);
        }
        summons
    }
}

/// Distinct amplifications and their cumulative probability, ascending.
pub fn amplification_cdf(solver: &Solver, analysis: &Analysis) -> (Vec<f64>, Vec<f64>) {
    let mut by_amp = Tally::default();
    for ((g, d), p) in analysis.dist.iter() {
        by_amp.add(solver.amplification(g, d).to_bits(), p);
    }
    let mut items: Vec<(f64, f64)> =
        by_amp.iter().map(|(bits, p)| (f64::from_bits(bits), p)).filter(|&(_, p)| p > 0.0).collect();
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut running = 0.0;
    let mut cum: Vec<f64> = items
        .iter()
        .map(|&(_, p)| {
            running += p;
            running
        })
        .collect();
    *cum.last_mut().expect("a distribution is never empty") = 1.0;
    (items.into_iter().map(|(v, _)| v).collect(), cum)
}

/// The cost of first reaching one amplification mark.
#[derive(Clone, Debug)]
pub struct MarkRow {
    pub mark: f64,
    /// chance a single attempt reaches the mark
    pub p_attempt: f64,
    pub mean_attempts: f64,
    pub mean: f64,
    pub median: f64,
    pub p90: f64,
    pub runs: usize,
}

pub struct Farm {
    pub rows: Vec<MarkRow>,
    pub solver: Solver,
    pub analysis: Analysis,
}

/// Play out `sims` farming runs to the top mark.
///
/// Only the attempts that set a new best matter, so a run jumps from one
/// record to the next - an exact rewrite of "summon, attempt, keep the best":
/// the attempts to the next record are Geometric(P(amp > best)), the record is
/// drawn conditional on beating the best, and the summons behind attempt A are
/// the first n whose crit drops reach 10A (leftovers carry).
pub fn farm(level: i64, sims: usize, seed: u64) -> Result<Farm, String> {
    let mut solver = Solver::new(Config::for_level(level, Strategy::default())?);
    let analysis = solver.analyse(None);
    let (values, amp_cum) = amplification_cdf(&solver, &analysis);
    let thresholds: Vec<f64> = values.iter().copied().filter(|&v| v > 0.0).collect();
    let summoner = Summoner::new();
    let last = values.len() - 1;
    // P(amp <= best) for the 0% floor every run starts at
    let floor_cum = match values.partition_point(|&v| v <= 0.0) {
        0 => 0.0,
        k => amp_cum[k - 1],
    };

    let mut costs: Vec<Vec<f64>> = vec![Vec::with_capacity(sims); thresholds.len()];
    let mut attempts_at: Vec<Vec<f64>> = vec![Vec::with_capacity(sims); thresholds.len()];
    for run in 0..sims {
        let mut rng = Rng::for_run(seed, run as u64);
        let (mut drawn, mut summons, mut attempts) = (0u32, 0u64, 0u64);
        let mut best_cum = floor_cum;
        let mut index = 0;
        while index < thresholds.len() {
            let beat = 1.0 - best_cum;
            attempts += if beat >= 1.0 { 1 } else { 1 + ((1.0 - rng.unit()).ln() / (-beat).ln_1p()) as u64 };
            let target = best_cum + rng.unit() * beat;
            let pick = amp_cum.partition_point(|&c| c <= target).min(last);
            let best = values[pick];
            best_cum = amp_cum[pick];
            summons += summoner.until(&mut drawn, RELICS_PER_ATTEMPT * attempts as u32, &mut rng);
            let spent = (summons * DIAMONDS_PER_SUMMON) as f64;
            while index < thresholds.len() && thresholds[index] <= best {
                costs[index].push(spent);
                attempts_at[index].push(attempts as f64);
                index += 1;
            }
        }
    }

    let mut rows = Vec::with_capacity(thresholds.len());
    for (i, &mark) in thresholds.iter().enumerate() {
        let series = &mut costs[i];
        series.sort_by(f64::total_cmp);
        let n = series.len();
        let p_reach: f64 =
            values.iter().zip(pmf(&amp_cum)).filter(|&(&v, _)| v >= mark).map(|(_, p)| p).sum();
        rows.push(MarkRow {
            mark,
            p_attempt: p_reach,
            mean_attempts: attempts_at[i].iter().sum::<f64>() / n as f64,
            mean: series.iter().sum::<f64>() / n as f64,
            median: series[n / 2],
            p90: series[(n - 1).min((0.9 * n as f64) as usize)],
            runs: n,
        });
    }
    Ok(Farm { rows, solver, analysis })
}

fn pmf(cum: &[f64]) -> Vec<f64> {
    let mut prev = 0.0;
    cum.iter()
        .map(|&c| {
            let p = c - prev;
            prev = c;
            p
        })
        .collect()
}

/// The marks worth printing: everything below about 30% is reached on the
/// first attempt, so `min_mark` cuts them; `step` thins the rest (the top mark
/// is always kept).
pub fn table_rows(rows: &[MarkRow], step: u32, min_mark: Option<f64>) -> Vec<MarkRow> {
    let mut keep: Vec<MarkRow> =
        rows.iter().filter(|r| min_mark.is_none_or(|m| r.mark >= m)).cloned().collect();
    if keep.is_empty() {
        keep = rows.to_vec();
    }
    if step > 1 {
        let mut thinned: Vec<MarkRow> =
            keep.iter().filter(|r| r.mark % f64::from(step) == 0.0).cloned().collect();
        let top = keep.last().expect("non-empty").mark;
        if !thinned.is_empty() && thinned.last().expect("non-empty").mark != top {
            thinned.push(keep.last().expect("non-empty").clone());
        }
        if !thinned.is_empty() {
            keep = thinned;
        }
    }
    keep
}

/// The cost table for the console.
pub fn report(farm: &Farm, sims: usize, step: u32, min_mark: Option<f64>) -> String {
    let rows = &farm.rows;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Diamond cost to reach each amplification mark - inheritor level {}",
        farm.solver.cfg.level
    );
    let _ = writeln!(
        out,
        "  {} diamonds = {RELICS_PER_SUMMON} relics, 1 in {RELIC_TYPES} is a crit relic, {RELICS_PER_ATTEMPT} crit relics per attempt",
        commas(DIAMONDS_PER_SUMMON as f64, 0)
    );
    let _ = writeln!(
        out,
        "  -> {:.4} crit relics per summon, about {} diamonds per attempt",
        f64::from(RELICS_PER_SUMMON) / f64::from(RELIC_TYPES),
        commas(diamonds_per_attempt(), 0)
    );
    let _ = writeln!(
        out,
        "  Monte Carlo over {} farming runs, each run keeping its best amplification so far",
        commas(sims as f64, 0)
    );
    if let Some(m) = min_mark {
        let _ = writeln!(
            out,
            "  showing marks from {m:.0}% up; everything below that is one attempt (--min-mark 0 for all)"
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  {:>5} | {:>15} {:>14} | {:>14} {:>10} {:>10}",
        "amp", "P(one attempt)", "mean attempts", "mean diamonds", "median", "90th pct"
    );
    let _ = writeln!(out, "  {}", "-".repeat(80));
    for r in table_rows(rows, step, min_mark) {
        let _ = writeln!(
            out,
            "  {:>4.0}% | {:>14} {:>14.1} | {:>14} {:>10} {:>10}",
            r.mark,
            pct(r.p_attempt, 4),
            r.mean_attempts,
            commas(r.mean, 0),
            human(r.median),
            human(r.p90)
        );
    }
    let _ = writeln!(out);
    let (top, before) = (&rows[rows.len() - 1], &rows[rows.len() - 2]);
    let _ = writeln!(
        out,
        "  the last {:.0} points cost {:.1}x what everything before them did ({} -> {})",
        top.mark - before.mark,
        top.mean / before.mean,
        human(before.mean),
        human(top.mean)
    );
    let _ = writeln!(
        out,
        "  cross-check: {:.0} attempts x {} diamonds = {}, against the simulated {}",
        top.mean_attempts,
        commas(diamonds_per_attempt(), 0),
        commas(top.mean_attempts * diamonds_per_attempt(), 0),
        commas(top.mean, 0)
    );
    out
}

/// Cost against amplification as an inline SVG; log y, the range is 1000x.
pub fn chart_svg(farm: &Farm) -> String {
    let rows = &farm.rows;
    let level = farm.solver.cfg.level;
    let marks: Vec<f64> = rows.iter().map(|r| r.mark).collect();
    let costs: Vec<f64> = rows.iter().map(|r| r.mean).collect();
    let (lo_x, hi_x) = (marks[0], marks[marks.len() - 1]);
    let lo_e = costs.iter().copied().fold(f64::MAX, f64::min).log10().floor() as i32;
    let hi_e = costs.iter().copied().fold(f64::MIN, f64::max).log10().ceil() as i32;

    let (ml, mr, mt, mb) = (58.0, 16.0, 26.0, 40.0);
    let (plot_w, plot_h) = (560.0, 230.0);
    let (width, height) = (ml + plot_w + mr, mt + plot_h + mb);
    let x_of = |mark: f64| ml + (mark - lo_x) / (hi_x - lo_x) * plot_w;
    let y_of = |cost: f64| mt + plot_h * (1.0 - (cost.log10() - f64::from(lo_e)) / f64::from(hi_e - lo_e));
    let n = general;

    let mut out = vec![
        format!(
            "<svg viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" xmlns=\"http://www.w3.org/2000/svg\" aria-label=\"Diamonds needed to reach each amplification mark at level {level}\">",
            w = n(width),
            h = n(height)
        ),
        format!("<title>Diamond cost by amplification mark, inheritor level {level}</title>"),
        format!(
            "<desc>Mean diamonds to first reach each amplification, log scale. {} at {:.0} percent rising to {} at {:.0} percent.</desc>",
            human(costs[0]),
            marks[0],
            human(costs[costs.len() - 1]),
            marks[marks.len() - 1]
        ),
        concat!(
            "<style>",
            ".ec-line{fill:none;stroke:#2a78d6;stroke-width:2}",
            ".ec-dot{fill:#2a78d6}",
            ".ec-grid{stroke:#e1e0d9;stroke-width:1}.ec-axis{stroke:#c3c2b7;stroke-width:1}",
            ".ec-ink{fill:#52514e}.ec-muted{fill:#898781}",
            ".ec-t{font:11px system-ui,-apple-system,'Segoe UI',sans-serif}",
            ".ec-b{font:600 11px system-ui,-apple-system,'Segoe UI',sans-serif}",
            "@media(prefers-color-scheme:dark){",
            ".ec-line{stroke:#3987e5}.ec-dot{fill:#3987e5}",
            ".ec-grid{stroke:#2c2c2a}.ec-axis{stroke:#383835}.ec-ink{fill:#c3c2b7}}",
            "</style>"
        )
        .to_string(),
    ];
    for e in lo_e..=hi_e {
        let y = y_of(10f64.powi(e));
        out.push(format!(
            "<line class=\"ec-grid\" x1=\"{}\" y1=\"{y:.1}\" x2=\"{}\" y2=\"{y:.1}\"/>",
            n(ml),
            n(ml + plot_w)
        ));
        out.push(format!(
            "<text class=\"ec-t ec-muted\" x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            n(ml - 8.0),
            y + 4.0,
            human(10f64.powi(e))
        ));
    }
    out.push(format!(
        "<text class=\"ec-t ec-muted\" x=\"{}\" y=\"{}\" text-anchor=\"end\">diamonds</text>",
        n(ml - 8.0),
        n(mt - 9.0)
    ));
    let points: Vec<String> =
        marks.iter().zip(&costs).map(|(&m, &c)| format!("{:.1},{:.1}", x_of(m), y_of(c))).collect();
    out.push(format!("<polyline class=\"ec-line\" points=\"{}\"/>", points.join(" ")));
    for r in rows {
        out.push(format!(
            "<circle class=\"ec-dot\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.6\"><title>{:.0}%: {} diamonds on average ({:.1} attempts)</title></circle>",
            x_of(r.mark),
            y_of(r.mean),
            r.mark,
            commas(r.mean, 0),
            r.mean_attempts
        ));
    }
    let base_y = mt + plot_h;
    out.push(format!(
        "<line class=\"ec-axis\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
        n(ml),
        n(base_y),
        n(ml + plot_w),
        n(base_y)
    ));
    for mark in (0..=hi_x as i64).step_by(5).filter(|&m| m as f64 >= lo_x) {
        out.push(format!(
            "<text class=\"ec-t ec-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">{mark}%</text>",
            x_of(mark as f64),
            n(base_y + 15.0)
        ));
    }
    out.push(format!(
        "<text class=\"ec-t ec-muted\" x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\">amplification reached (or better)</text>",
        ml + plot_w / 2.0,
        n(height - 6.0)
    ));
    // label the two ends: the cheap entry and the jackpot
    let (first, last) = (costs[0], costs[costs.len() - 1]);
    out.push(format!(
        "<text class=\"ec-b ec-ink\" x=\"{:.1}\" y=\"{:.1}\">{}</text>",
        x_of(marks[0]) + 6.0,
        y_of(first) + 14.0,
        human(first)
    ));
    out.push(format!(
        "<text class=\"ec-b ec-ink\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
        x_of(hi_x) - 6.0,
        y_of(last) - 8.0,
        human(last)
    ));
    out.push("</svg>".to_string());
    out.join("\n")
}

/// COST.md, generated end to end from one farming run.
pub fn markdown(farm: &Farm, sims: usize) -> Result<String, String> {
    const MIN_MARK: f64 = 35.0;
    let rows = &farm.rows;
    let level = farm.solver.cfg.level;
    let at = |mark: f64| -> Result<&MarkRow, String> {
        rows.iter().find(|r| r.mark == mark).ok_or(format!("no {mark}% mark at level {level}"))
    };
    let top = &rows[rows.len() - 1];
    let (m25, m30, m35, m40, m45) = (at(25.0)?, at(30.0)?, at(35.0)?, at(40.0)?, at(45.0)?);
    let dpa = diamonds_per_attempt();
    let h = human;

    let mut table = vec![
        "| amp | P(one attempt) | mean attempts | mean diamonds | median | 90th pct |".to_string(),
        "|---|---|---|---|---|---|".to_string(),
    ];
    for r in table_rows(rows, 1, Some(MIN_MARK)) {
        table.push(format!(
            "| **{:.0}%** | {} | {} | {} | {} | {} |",
            r.mark,
            pct(r.p_attempt, 4),
            commas(r.mean_attempts, 1),
            commas(r.mean, 0),
            h(r.median),
            h(r.p90)
        ));
    }

    let mut out = String::new();
    let _ = write!(
        out,
        r#"# What a crit relic costs

<!-- generated by `relic cost --markdown COST.md` - do not edit -->

How many diamonds it takes to farm a level {level} crit relic up to each
amplification mark. The strategy side of this - what one attempt is worth and how
to play it - is in [STRATEGY.md](STRATEGY.md); this is only the price tag.

```
relic cost --level {level} --sims {sims}
relic cost --chart > cost.svg
relic cost --min-mark 0      # include the cheap marks below 35% too
```

## The economy

| | |
|---|---|
| 1 summon | {summon} diamonds, {RELICS_PER_SUMMON} random relics |
| relic types | {RELIC_TYPES}, of which type 1 (crit) is the one we want |
| crit relics per summon | Binomial({RELICS_PER_SUMMON}, 1/{RELIC_TYPES}) = **{per_summon:.4}** on average |
| 1 inheritance attempt | {RELICS_PER_ATTEMPT} crit relics |
| **so 1 attempt costs** | **about {dpa_s} diamonds** once leftovers carry over |

Each attempt re-rolls the relic and you keep the best result you have ever hit,
so reaching a mark is a matter of attempting until one lands. That makes the
number of attempts geometric in the per-attempt chance, and the diamond cost
follows from it.

## Method

A Monte Carlo over **{sims_s} farming runs**. Each run summons until it can afford
an attempt, attempts, keeps its best amplification, and stops once it hits {top_mark:.0}%.
The diamond total at the moment each mark is first met is recorded, then averaged.

The per-attempt outcome is drawn from the solver's **exact** outcome
distribution rather than by replaying the policy move by move. The solver
enumerates every branch, so those are the same distribution - a speedup, not an
approximation. The summon side is simulated properly, so the leftover crit relics
that carry between attempts are handled exactly.

## Cost by mark

Every achievable mark from {MIN_MARK:.0}% up. Below that the table is not worth
printing: a single attempt clears 30% {p30} of the time,
so everything cheaper than {MIN_MARK:.0}% costs about one attempt
({m30_mean} or less). Run with `--min-mark 0` for the
full {marks} marks.

{table}

{svg}

<sub>Log scale - the range spans three orders of magnitude. The chart needs a
renderer that keeps inline SVG; the VS Code preview does, GitHub strips it.</sub>

## What this says

**Everything below 35% is close to free**, which is why the table starts there.
25% costs {m25_mean} and lands first try
{p25} of the time; 30% costs
{m30_mean}. The real spending starts above that.

**The multiplier itself keeps growing** - each extra 5 points costs more,
relative to the step before it, than the last one did:

| step | mean diamonds | multiplier |
|---|---|---|
| 35% to 40% | {m40_mean} | {x40:.1}x |
| 40% to 45% | {m45_mean} | {x45:.1}x |
| 45% to 50% | {top_mean} | {x50:.1}x |

**The last 5 points cost {x50:.0}x everything before them.**
Getting to 45% averages {m45_mean}; going from there to a perfect
50% averages {top_mean}, because a perfect relic needs a flawless attempt
and that happens {top_p} of the time - once per
{top_attempts} attempts.

**The averages hide a very long tail.** The median run reaches 50% for
{top_median}, but the 90th percentile is {top_p90}. First-reach times
are geometric, so the spread is as wide as the mean: budgeting the average is a
coin flip, not a plan.

**Where to stop is a judgement call, but 40% is the value corner.**
{m40_mean} buys 40%; the next 10 points cost
{x4050:.0}x that again. Put the other way: the
diamonds behind one perfect relic would farm about
**{x4050:.0} separate relics at 40%**, or
{x50:.0} at 45%.

## Cross-check

Two independent routes to the same number, which is the reason to trust it:

| | |
|---|---|
| simulated mean to 50% | {top_mean_full} diamonds |
| {top_attempts} attempts x {dpa_s} per attempt | {product} diamonds |
| gap | {gap} |

The simulated attempt count also matches the geometric expectation: {top_attempts}
attempts against a predicted 1 / {top_p6} = {predicted}.

## Assumptions

1. A summon is {RELICS_PER_SUMMON} **independent** relics, each equally
   likely to be any of the {RELIC_TYPES} types. No pity, no duplicate
   protection, no banner weighting.
2. Only crit relics have any value; the other {others} types are
   discarded. If they are worth something, the true cost per crit relic is lower.
3. Leftover crit relics carry over between attempts, so nothing is wasted except
   within the final partial summon.
4. Attempts are independent and the relic keeps its best amplification ever
   rolled - an attempt can never make an existing relic worse.
5. The inheritance itself is played to the solver's optimal weighted policy
   (+5% per glory success, -2% per despair success, floored at 0%). A worse
   policy costs more diamonds for the same mark.
"#,
        summon = commas(DIAMONDS_PER_SUMMON as f64, 0),
        per_summon = f64::from(RELICS_PER_SUMMON) / f64::from(RELIC_TYPES),
        dpa_s = commas(dpa, 0),
        sims_s = commas(sims as f64, 0),
        top_mark = top.mark,
        p30 = pct(m30.p_attempt, 0),
        p25 = pct(m25.p_attempt, 0),
        m25_mean = h(m25.mean),
        m30_mean = h(m30.mean),
        marks = rows.len(),
        table = table.join("\n"),
        svg = chart_svg(farm),
        m40_mean = h(m40.mean),
        m45_mean = h(m45.mean),
        top_mean = h(top.mean),
        x40 = m40.mean / m35.mean,
        x45 = m45.mean / m40.mean,
        x50 = top.mean / m45.mean,
        x4050 = top.mean / m40.mean,
        top_p = pct(top.p_attempt, 4),
        top_p6 = pct(top.p_attempt, 6),
        top_attempts = commas(top.mean_attempts, 0),
        top_median = h(top.median),
        top_p90 = h(top.p90),
        top_mean_full = commas(top.mean, 0),
        product = commas(top.mean_attempts * dpa, 0),
        gap = pct((top.mean - top.mean_attempts * dpa).abs() / top.mean, 3),
        predicted = commas(1.0 / top.p_attempt, 0),
        others = RELIC_TYPES - 1,
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summon_tables_are_distributions() {
        for size in [1, 8, 64] {
            let cum = summon_cdf(size);
            assert!((cum[cum.len() - 1] - 1.0).abs() < 1e-12);
            let mean: f64 = pmf(&cum).iter().enumerate().map(|(k, p)| k as f64 * p).sum();
            let expect = f64::from(RELICS_PER_SUMMON * size) / f64::from(RELIC_TYPES);
            assert!((mean - expect).abs() < 1e-9, "batch of {size}");
        }
    }

    #[test]
    fn batched_summons_match_one_at_a_time() {
        // the same first-passage count, drawn two ways
        let summoner = Summoner::new();
        let single = &summoner.batches[2].1;
        let (mut batched, mut stepped) = (0.0, 0.0);
        let runs = 20_000;
        for run in 0..runs {
            let mut rng = Rng::for_run(3, run);
            let mut drawn = 0;
            batched += summoner.until(&mut drawn, 2000, &mut rng) as f64;
            let mut rng = Rng::for_run(4, run);
            let (mut drawn, mut summons) = (0u32, 0u64);
            while drawn < 2000 {
                let r = rng.unit();
                drawn += single.partition_point(|&c| c <= r) as u32;
                summons += 1;
            }
            stepped += summons as f64;
        }
        let (b, s) = (batched / runs as f64, stepped / runs as f64);
        assert!((b / s - 1.0).abs() < 0.003, "{b} vs {s}");
    }

    #[test]
    fn record_jumping_is_exact() {
        // every mark's attempts must be Geometric(P(amp >= mark)), mean 1/p,
        // and cost ~ attempts x diamonds per attempt; the top mark at level 4
        // is a 0.3% shot, the hardest case for the jump
        let farm = farm(4, 20_000, 5).unwrap();
        for row in [&farm.rows[farm.rows.len() - 1], &farm.rows[farm.rows.len() / 2]] {
            let expect = 1.0 / row.p_attempt;
            assert!(
                (row.mean_attempts / expect - 1.0).abs() < 0.03,
                "{}%: {} vs {expect}",
                row.mark,
                row.mean_attempts
            );
            let cost = expect * diamonds_per_attempt();
            assert!((row.mean / cost - 1.0).abs() < 0.04, "{}%: {} vs {cost}", row.mark, row.mean);
        }
    }

    #[test]
    fn table_rows_thin_but_keep_the_top() {
        let row = |mark: f64| MarkRow {
            mark,
            p_attempt: 0.0,
            mean_attempts: 0.0,
            mean: 0.0,
            median: 0.0,
            p90: 0.0,
            runs: 0,
        };
        let rows: Vec<MarkRow> = [20.0, 25.0, 33.0, 40.0, 42.0].into_iter().map(row).collect();
        let kept: Vec<f64> = table_rows(&rows, 5, Some(25.0)).iter().map(|r| r.mark).collect();
        assert_eq!(kept, vec![25.0, 40.0, 42.0]);
        let all: Vec<f64> = table_rows(&rows, 1, None).iter().map(|r| r.mark).collect();
        assert_eq!(all.len(), 5);
    }
}
