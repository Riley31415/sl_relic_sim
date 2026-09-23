//! `relic`: the command line for the solver, the diamond cost model and the
//! inheritor-levelling Monte Carlo.
//!
//!     relic solve --level 1               exact solve + report for one level
//!     relic solve --level 20 --amp-table  P(each glory/despair combination)
//!     relic heuristics                    hand-written rules vs the optimum
//!     relic cost --markdown COST.md       regenerate COST.md
//!     relic levels --markdown LEVELS.md   regenerate LEVELS.md
//!     relic levels --lookahead            search for the cheapest plan

use std::fs;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use relic::format::{commas, human};
use relic::levels::report::{self, HABITS};
use relic::levels::{self, Game, Plan, RunOptions, SearchOptions};
use relic::solve_report::SolveArgs;
use relic::solver::DOCUMENTED_MAX_LEVEL;
use relic::{economy, heuristics, solve_report};

#[derive(Parser)]
#[command(name = "relic", about = "Slayer Legend relic inheritance: solver, costs and levelling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Solve one inheritance attempt exactly and report on it
    Solve(Solve),
    /// Score hand-written, human-playable rules against the exact optimum
    Heuristics(Heuristics),
    /// Diamond cost of farming a crit relic to each amplification mark
    Cost(Cost),
    /// Diamond cost of raising the inheritor level, and the plan searches
    Levels(Levels),
}

#[derive(Args)]
struct Solve {
    /// inheritor level
    #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
    level: i64,
    /// Monte Carlo attempts to cross-check the exact numbers
    #[arg(long, default_value_t = 0)]
    mc: u64,
    /// show N sample attempts, move by move
    #[arg(long, default_value_t = 0)]
    play: u64,
    #[arg(long, default_value_t = 12345)]
    seed: u64,
    /// summary table for every level up to --max-level
    #[arg(long)]
    all_levels: bool,
    /// skip the distribution tables
    #[arg(long)]
    no_dist: bool,
    /// minimise the dead-end wipe chance before maximising score
    #[arg(long)]
    safety_first: bool,
    /// extra score charged for a wipe (a middle ground; default 0)
    #[arg(long, default_value_t = 0.0)]
    wipe_penalty: f64,
    /// skip the safety trade-off table
    #[arg(long)]
    no_tradeoff: bool,
    /// chase an all-or-nothing target: >=GLORY glory and <=DESPAIR despair successes
    #[arg(long, num_args = 2, value_names = ["GLORY", "DESPAIR"])]
    target: Option<Vec<u8>>,
    /// compare the weighted objective against a target across all levels
    #[arg(long)]
    strategies: bool,
    /// render the amplification table as a shaded HTML table plus a curve SVG
    #[arg(long)]
    html: bool,
    /// P(each glory/despair combination), with its amplification
    #[arg(long)]
    amp_table: bool,
    /// amplification per glory success
    #[arg(long, default_value_t = 5.0)]
    amp_glory: f64,
    /// amplification lost per despair success
    #[arg(long, default_value_t = 2.0)]
    amp_despair: f64,
    /// highest level in the level tables
    #[arg(long, default_value_t = DOCUMENTED_MAX_LEVEL)]
    max_level: u32,
}

#[derive(Args)]
struct Heuristics {
    /// the rule is tuned at level 7
    #[arg(long, default_value_t = 7)]
    level: i64,
    #[arg(long, default_value_t = 5.0)]
    amp_glory: f64,
    #[arg(long, default_value_t = 2.0)]
    amp_despair: f64,
}

#[derive(Args)]
struct Cost {
    #[arg(long, default_value_t = 20)]
    level: i64,
    /// Monte Carlo farming runs
    #[arg(long, default_value_t = 20000)]
    sims: usize,
    #[arg(long, default_value_t = 20260922)]
    seed: u64,
    /// table granularity in amplification points (1: every achievable mark)
    #[arg(long, default_value_t = 1)]
    step: u32,
    /// lowest mark to print (0 shows all)
    #[arg(long, default_value_t = 35.0)]
    min_mark: f64,
    /// emit the cost curve as an inline SVG instead of a table
    #[arg(long)]
    chart: bool,
    /// write the whole COST.md document to PATH
    #[arg(long, value_name = "PATH")]
    markdown: Option<String>,
}

#[derive(Args)]
struct Levels {
    /// which named plan to report on
    #[arg(long, default_value = "lookahead")]
    strategy: String,
    /// runs per plan (default 2000; 20000 for --markdown)
    #[arg(long)]
    runs: Option<u64>,
    #[arg(long, default_value_t = levels::SEED)]
    seed: u64,
    #[arg(long, default_value_t = levels::MAX_LEVEL)]
    max_level: usize,
    /// relics of EACH type already on hand before the first summon
    #[arg(long, default_value_t = 0)]
    start_stock: u32,
    /// score every named plan
    #[arg(long)]
    compare: bool,
    /// emit the cost-by-level chart as inline SVG
    #[arg(long)]
    chart: bool,
    /// write the whole LEVELS.md document to PATH
    #[arg(long, value_name = "PATH")]
    markdown: Option<String>,
    /// search for the plan that makes each level-up as cheap as possible on its own
    #[arg(long)]
    greedy: bool,
    /// search for the cheapest plan to --max-level overall (starts from greedy)
    #[arg(long)]
    lookahead: bool,
}

fn solve(a: Solve) -> Result<String, String> {
    let target = match a.target.as_deref() {
        Some([g, d]) => Some((*g, *d)),
        _ => None,
    };
    let args = SolveArgs {
        level: a.level,
        target,
        amp_glory: a.amp_glory,
        amp_despair: a.amp_despair,
        safety_first: a.safety_first,
        wipe_penalty: a.wipe_penalty,
        max_level: a.max_level,
        seed: a.seed,
    };
    if a.amp_table {
        solve_report::amplification_table(&args, a.html)
    } else if a.strategies {
        solve_report::compare_strategies(&args)
    } else if a.all_levels {
        solve_report::all_levels(&args)
    } else {
        solve_report::report(&args, !a.no_dist, !a.no_tradeoff, a.mc, a.play)
    }
}

fn cost(a: Cost) -> Result<String, String> {
    let farm = economy::farm(a.level, a.sims, a.seed)?;
    if let Some(path) = a.markdown {
        fs::write(&path, economy::markdown(&farm, a.sims)?).map_err(|e| format!("{path}: {e}"))?;
        return Ok(format!("wrote {path}\n"));
    }
    if a.chart {
        return Ok(economy::chart_svg(&farm) + "\n");
    }
    Ok(economy::report(&farm, a.sims, a.step, (a.min_mark > 0.0).then_some(a.min_mark)))
}

fn levels(a: Levels) -> Result<String, String> {
    let game = Game::standard();
    if let Some(path) = a.markdown {
        fs::write(&path, report::markdown(a.runs.unwrap_or(20000), a.seed)?)
            .map_err(|e| format!("{path}: {e}"))?;
        return Ok(format!("wrote {path}\n"));
    }
    let runs = a.runs.unwrap_or(2000);
    if a.greedy || a.lookahead {
        return search(game, &a);
    }
    if a.compare {
        return report::compare(game, runs, a.seed, a.max_level);
    }
    let plan = levels::strategy(&a.strategy).ok_or(format!("no plan called '{}'", a.strategy))?;
    let options = RunOptions::to(a.max_level).with_stock(a.start_stock);
    let rows = report::level_rows(game, &plan, runs, a.seed, &options)?;
    if a.chart {
        return Ok(report::chart_svg(&rows, "Cost to raise the inheritor", true) + "\n");
    }
    let mut out = report::print_table(&rows, &a.strategy, runs);
    out.push_str(&format!("\n  the plan, level by level  ('{}')\n  every step: {HABITS}\n", a.strategy));
    for level in 2..=a.max_level {
        out.push_str(&format!("  {level:>3} | {}\n", report::describe_step(game, &plan, level)));
    }
    Ok(out)
}

fn search(game: &Game, a: &Levels) -> Result<String, String> {
    let quiet = |_: &str| {};
    let greedy_options = SearchOptions { max_level: a.max_level, ..SearchOptions::greedy() };
    let mut out = String::new();
    if a.greedy {
        let (_plan, rows) = levels::greedy_search(game, greedy_options, &quiet)?;
        out.push_str("Greedy search: each level-up made as cheap as it can be on its own\n");
        let mut total = 0.0;
        for row in rows {
            total += row.cost;
            let alt = row
                .runner_up
                .map(|(choice, cost)| format!("   next best: {} {}", choice.label(), human(cost)))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {:>3} | {:>6} (total {:>6}) | {}{alt}\n",
                row.level,
                human(row.cost),
                human(total),
                row.choice.label()
            ));
        }
        return Ok(out);
    }
    println!("Look-ahead search: cheapest plan to level {} overall", a.max_level);
    let (start, _rows) = levels::greedy_search(game, greedy_options, &quiet)?;
    let log = |line: &str| println!("  {line}");
    let options = SearchOptions { max_level: a.max_level, ..SearchOptions::lookahead() };
    let (plan, cost): (Plan, f64) = levels::lookahead_search(game, &start, options, &log)?;
    for (level, choice) in &plan.steps {
        out.push_str(&format!("  {level:>3} | {}\n", choice.label()));
    }
    out.push_str(&format!("  mean to level {}: {}\n", a.max_level, commas(cost, 0)));
    Ok(out)
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Solve(a) => solve(a),
        Command::Heuristics(a) => heuristics::report(a.level, a.amp_glory, a.amp_despair),
        Command::Cost(a) => cost(a),
        Command::Levels(a) => levels(a),
    };
    match result {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}
