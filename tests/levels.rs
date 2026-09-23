//! Raising the inheritor: the rules, plans, the Monte Carlo, the searches and
//! the LEVELS.md report built on them.

use relic::levels::report::{self, level_rows};
use relic::levels::{
    self, ATK, Bar, CRIT, FILLERS, Game, MAX_LEVEL, N_RELICS, Plan, Profile, Quest, RELIC_SHORT, RELICS,
    RunOptions, SearchOptions, StepChoice, amplification, damage, level_multiplier, pity_gain, pity_needed,
    set_of, strategy,
};

fn game() -> &'static Game {
    Game::standard()
}

fn plan(name: &str) -> Plan {
    strategy(name).expect("a named plan")
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

// ------------------------------------------------------------------ rules

#[test]
fn the_requirement_table_matches_the_sheet() {
    let q = &game().quests;
    assert_eq!(q[2], Some(Quest::Relics { which: vec![0], bar: Bar::new(3, None) }));
    assert_eq!(q[7], Some(Quest::Each(Bar::new(4, Some(2)))));
    assert_eq!(q[8], Some(Quest::Total { glory: 59, despair: 17 }));
    assert_eq!(q[9], Some(Quest::Relics { which: vec![6, 7, 8], bar: Bar::new(6, Some(2)) }));
    assert_eq!(q[10], Some(Quest::Total { glory: 68, despair: 19 }));
    assert_eq!(q[1], None); // level 1 is free
    assert_eq!(game().max_level(), MAX_LEVEL);
    assert_eq!(RELICS[CRIT], "Demon Eye of Weakness");
    assert_eq!(RELICS[ATK], "Giant's Right Hand");
}

#[test]
fn the_banked_relics_are_level_9s() {
    assert_eq!(game().bank_level(), 9);
    assert_eq!(game().banked_relics(), set_of(&[6, 7, 8]));
    assert_eq!(game().future_named(4), set_of(&[3, 4, 5, 6, 7, 8]));
    assert_eq!(game().future_named(9), 0);
}

#[test]
fn pity_meter_sizes_and_gains() {
    assert_eq!([2, 3, 9, 10].map(pity_needed), [500, 1000, 4000, 4500]);
    assert_eq!([1, 2, 3].map(pity_gain), [100; 3]);
    assert_eq!([4, 7].map(pity_gain), [120; 2]);
    assert_eq!([8, 11].map(pity_gain), [140; 2]);
    assert_eq!(pity_gain(19), 180);
}

#[test]
fn level_multiplier_and_damage() {
    let steps: Vec<f64> = (1..=10).map(level_multiplier).collect();
    assert_eq!(steps[0], 0.0);
    for pair in steps[..7].windows(2) {
        assert!(close(pair[1] - pair[0], 0.05, 1e-12)); // levels 2-7: +5% each
    }
    assert!(close(steps[8], 0.50, 1e-12)); // level 9
    assert!(close(steps[9], 0.60, 1e-12)); // level 10
    assert_eq!(damage(1, 0.0, 0.0), 1.0);
    assert!(close(damage(9, 25.0, 10.0), 1.5 * 1.2 * (1.0 + 2.2 / 209.0), 1e-12));
}

#[test]
fn amplification_is_floored() {
    assert_eq!(amplification((4, 1)), 18.0);
    assert_eq!(amplification((0, 5)), 0.0);
}

#[test]
fn every_relic_has_a_two_word_name() {
    assert_eq!(RELIC_SHORT.len(), N_RELICS);
    assert!(RELIC_SHORT.iter().all(|n| n.split(' ').count() == 2));
}

// ------------------------------------------------------------------ plans

#[test]
fn a_level_left_out_uses_the_default() {
    assert_eq!(Plan::default().step(6), StepChoice::default());
}

#[test]
fn banking_only_covers_relics_still_to_come() {
    let p = Plan::new([(8, StepChoice::default().bank()), (9, StepChoice::default().bank())]);
    assert_eq!(p.protected(game(), 8), set_of(&[6, 7, 8]));
    assert_eq!(p.protected(game(), 9), 0); // its own level
    assert_eq!(p.protected(game(), 6), 0); // not banked there
}

#[test]
fn a_choice_replaces_rather_than_stacks() {
    let first = StepChoice::build(Profile::Bar(5, 1)).closer().score().bank().with_filler((4, 2));
    let p = Plan::default().with(6, first).with(6, StepChoice::build(Profile::Repair));
    assert_eq!(p.step(6), StepChoice::build(Profile::Repair));
    assert_eq!(p.protected(game(), 6), 0);
}

#[test]
fn named_relic_levels_only_decide_their_filler() {
    for lvl in [2, 3, 5, 9] {
        let choices = levels::step_choices(game(), lvl);
        assert_eq!(choices.iter().map(|c| c.filler).collect::<Vec<_>>(), FILLERS.to_vec());
        assert!(choices.iter().all(|c| c.profile.is_none()));
    }
    assert!(levels::step_choices(game(), 4).len() > 100);
    assert_eq!(levels::knobs(game(), 4), ["filler", "bank", "profile", "closer", "score"]);
    assert!(!levels::knobs(game(), 9).contains(&"bank"));
}

#[test]
fn the_saved_plans() {
    let (look, greedy) = (plan("lookahead"), plan("greedy"));
    assert_eq!(look.step(6).profile, Some(Profile::Repair));
    assert_eq!(look.step(2).filler, None); // look-ahead saves its early stock
    assert_eq!(greedy.step(2).filler, Some((4, 2))); // greedy spends it on pity
    for lvl in [8, 10] {
        assert_eq!(greedy.step(lvl).profile, Some(Profile::Repair));
    }
    assert_eq!(plan("minimal"), Plan::default());
}

#[test]
fn labels() {
    assert_eq!(StepChoice::filler((4, 2)).label(), "roll the named relics to the bar, filler to 4+ / 2-");
    assert!(StepChoice::build(Profile::Bar(5, 1)).closer().label().contains("keep gap-closers"));
}

// ------------------------------------------------------------------ the Monte Carlo

#[test]
fn a_run_climbs_to_the_top_in_whole_summons() {
    for run in game().simulate(&plan("lookahead"), 50, 4, &RunOptions::default()).unwrap() {
        assert_eq!(run.levels.len(), MAX_LEVEL);
        assert_eq!((run.levels[0].diamonds, run.levels[0].attempts, run.levels[0].by_pity), (0, 0, false));
        assert!(run.levels.windows(2).all(|p| p[0].diamonds <= p[1].diamonds));
        assert!(run.levels.iter().all(|l| l.diamonds % 5000 == 0));
        assert_eq!(run.board.len(), N_RELICS);
    }
}

#[test]
fn the_same_seed_gives_the_same_runs() {
    let (p, o) = (plan("greedy"), RunOptions::default());
    assert_eq!(game().simulate(&p, 100, 9, &o).unwrap(), game().simulate(&p, 100, 9, &o).unwrap());
    assert_ne!(game().simulate(&p, 100, 9, &o).unwrap(), game().simulate(&p, 100, 10, &o).unwrap());
}

#[test]
fn runs_are_paired_across_plans() {
    // two plans that only differ at level 8 play levels 2-7 identically
    let base = plan("lookahead");
    let other = base.with(8, StepChoice::build(Profile::Bar(5, 1)));
    let o = RunOptions::default();
    let (a, b) = (game().simulate(&base, 200, 3, &o).unwrap(), game().simulate(&other, 200, 3, &o).unwrap());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.levels[..7], y.levels[..7]);
    }
}

#[test]
fn mean_costs_agree_with_the_full_runs() {
    let (p, o) = (plan("greedy"), RunOptions::default());
    let runs = game().simulate(&p, 300, 5, &o).unwrap();
    let means = game().mean_costs(&p, 300, 5, &o).unwrap();
    for (index, mean) in means.iter().enumerate() {
        let direct = runs.iter().map(|r| r.levels[index].diamonds as f64).sum::<f64>() / 300.0;
        assert!(close(*mean, direct, 1e-6));
    }
    assert_eq!(levels::total_cost(game(), &p, 300, 5, MAX_LEVEL).unwrap(), means[MAX_LEVEL - 1]);
    let step = levels::step_cost(game(), &p, 6, 300, 5).unwrap();
    assert!(close(step, means[5] - means[4], 1e-6));
}

#[test]
fn pity_levels_up_without_the_quest() {
    let mut quests = levels::requirements();
    quests[2] = Some(Quest::Relics { which: vec![0], bar: Bar::new(99, Some(0)) }); // nothing can clear it
    let game = Game::with_quests(quests);
    let p = Plan::new([(2, StepChoice::filler((4, 2)))]);
    for run in game.simulate(&p, 20, 1, &RunOptions::to(2)).unwrap() {
        assert!(run.levels[1].by_pity);
        assert_eq!(run.levels[1].attempts, 5); // 500 pity at +100 an attempt
    }
}

#[test]
fn the_meter_resets_on_every_level_up() {
    for run in game().simulate(&plan("greedy"), 300, 2, &RunOptions::to(6)).unwrap() {
        for goal in 2..=6 {
            let spent = run.levels[goal - 1].attempts - run.levels[goal - 2].attempts;
            assert!(spent <= u64::from(pity_needed(goal).div_ceil(pity_gain(goal - 1))));
        }
    }
}

#[test]
fn switching_pity_off() {
    let o = RunOptions { pity: false, ..RunOptions::default() };
    let runs = game().simulate(&plan("minimal"), 50, 3, &o).unwrap();
    assert!(runs.iter().all(|r| r.levels.iter().all(|l| !l.by_pity)));
}

#[test]
fn stock_on_hand_is_used_before_summoning() {
    // 20 of every relic pays for level 2 unless two attempts in a row fail
    let runs = game().simulate(&plan("lookahead"), 200, 1, &RunOptions::to(2).with_stock(20)).unwrap();
    assert!(runs.iter().filter(|r| r.levels[1].diamonds == 0).count() > 150);
    let cheap = game().mean_costs(&plan("greedy"), 500, 8, &RunOptions::to(8).with_stock(40)).unwrap();
    let dear = game().mean_costs(&plan("greedy"), 500, 8, &RunOptions::to(8)).unwrap();
    assert!(cheap[7] < dear[7]);
    let short = RunOptions { start_stock: vec![1; 3], ..RunOptions::default() };
    assert!(game().simulate(&plan("greedy"), 5, 1, &short).is_err());
}

#[test]
fn filler_makes_levelling_cheaper() {
    let filled = Plan::new([2, 3, 5, 9].map(|lvl| (lvl, StepChoice::filler((4, 2)))));
    let plain = levels::total_cost(game(), &Plan::default(), 2000, 7, MAX_LEVEL).unwrap();
    assert!(levels::total_cost(game(), &filled, 2000, 7, MAX_LEVEL).unwrap() < plain);
}

#[test]
fn a_hard_preference_improves_the_preferred_relics() {
    let l5 = set_of(&[3, 4, 5]);
    let preferring =
        StepChoice { prefer: Some((l5, true)), ..StepChoice::build(Profile::Bar(5, 1)).closer().score() };
    let p = plan("lookahead").with(4, preferring);
    let worked: usize = game()
        .simulate(&p, 50, 6, &RunOptions::to(4))
        .unwrap()
        .iter()
        .map(|r| [3, 4, 5].iter().filter(|&&i| r.board[i] != (0, 0)).count())
        .sum();
    assert!(worked > 50);
}

#[test]
fn the_searched_plans_beat_the_baseline() {
    let cost = |name| levels::total_cost(game(), &plan(name), 4000, 11, MAX_LEVEL).unwrap();
    assert!(cost("lookahead") < cost("greedy"));
    assert!(cost("greedy") < cost("minimal"));
}

// ------------------------------------------------------------------ searches

#[test]
fn a_greedy_search_reports_every_level() {
    let o = SearchOptions { runs: 100, final_runs: 200, seed: 1, max_level: 4 };
    let (p, rows) = levels::greedy_search(game(), o, &|_| {}).unwrap();
    assert_eq!(rows.iter().map(|r| r.level).collect::<Vec<_>>(), [2, 3, 4]);
    assert!(rows[2].runner_up.is_some() && rows[2].bank_cost.is_some());
    assert_eq!(p.step(4), rows[2].choice);
}

#[test]
fn a_lookahead_search_never_makes_the_start_worse() {
    let before = levels::total_cost(game(), &Plan::default(), 2000, 99, 4).unwrap();
    let o = SearchOptions { runs: 200, final_runs: 800, seed: 3, max_level: 4 };
    let (p, _cost) = levels::lookahead_search(game(), &Plan::default(), o, &|_| {}).unwrap();
    assert!(levels::total_cost(game(), &p, 2000, 99, 4).unwrap() < before * 1.02);
}

// ------------------------------------------------------------------ the report

fn rows() -> Vec<report::Row> {
    level_rows(game(), &plan("greedy"), 60, 1, &RunOptions::default()).unwrap()
}

#[test]
fn summary_rows() {
    let rows = rows();
    assert_eq!(rows.iter().map(|r| r.level).collect::<Vec<_>>(), (1..=MAX_LEVEL).collect::<Vec<_>>());
    assert_eq!(rows[0].damage, 1.0);
    assert!(rows[MAX_LEVEL - 1].damage > 1.0 + level_multiplier(MAX_LEVEL) - 1e-9);
    assert_eq!(rows[9].short, "all twelve together: 68+ / 19- (totals)");
    assert_eq!(report::requirement_text(game(), 9, true), "Archer Seal, Night Veil, Eternal Spark: 6+ / 2-");
}

#[test]
fn every_level_is_described_from_the_plan() {
    let banks = plan("lookahead")
        .with(8, StepChoice::build(Profile::Repair).closer().score().bank().with_filler((4, 1)));
    let text = |lvl| report::describe_step(game(), &banks, lvl);
    assert!(text(1).contains("free"));
    assert!(text(2).contains("Giant's Right Hand"));
    assert!(text(6).contains("repair"));
    assert!(text(8).contains("except Seal of the Legendary Archer"));
    assert!(text(9).contains("banked during level 8"));
    assert!(text(7).contains("max glory and min despair"));
    assert!(text(10).contains("4,500 pity (33 attempts at +140)"));
    assert!(report::describe_step(game(), &plan("lookahead"), 2).contains("summon"));
    assert!(report::describe_step(game(), &plan("greedy"), 2).contains("best-stocked spare relic"));
}

#[test]
fn both_charts_label_every_bar_and_plot_the_damage_line() {
    let rows = rows();
    let total = report::chart_svg(&rows, "t", true);
    let steps = report::marginal_chart_svg(&rows, "s", true);
    for pair in rows.windows(2) {
        let (prev, r) = (&pair[0], &pair[1]);
        assert!(total.contains(&format!(">{}</text>", relic::format::human(r.mean))));
        assert!(steps.contains(&format!(">{}</text>", relic::format::human(r.mean - prev.mean))));
    }
    for svg in [&total, &steps] {
        assert_eq!(svg.matches("<circle").count(), rows.len() - 1);
        assert!(svg.contains(">log dmg</text>") && svg.contains(">+0%</text>"));
    }
}

#[test]
fn dmg_ticks_step_by_ten() {
    assert_eq!(report::dmg_ticks(81.6), [0, 10, 20, 30, 40, 50, 60, 70, 80, 90]);
    assert_eq!(report::dmg_ticks(17.5), [0, 10, 20]);
    assert_eq!(report::dmg_ticks(3.0), [0, 10]);
}

/// Gaps between the right-axis labels, top to bottom.
fn tick_gaps(svg: &str) -> Vec<f64> {
    let ys: Vec<f64> = svg
        .lines()
        .filter(|l| l.contains("lv-muted\" x=") && l.contains("\">+"))
        .map(|l| l.split("y=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap())
        .collect();
    ys.windows(2).map(|p| p[0] - p[1]).collect()
}

#[test]
fn log_ticks_bunch_up_and_linear_ticks_do_not() {
    let rows = rows();
    let log = tick_gaps(&report::chart_svg(&rows, "t", true));
    assert!(log.iter().all(|&g| g > 0.0));
    assert!(log[0] > log[log.len() - 1]);
    let linear_svg = report::chart_svg(&rows, "t", false);
    let linear = tick_gaps(&linear_svg);
    let (lo, hi) = linear.iter().fold((f64::MAX, f64::MIN), |(a, b), &g| (a.min(g), b.max(g)));
    assert!(hi - lo <= 0.2);
    assert!(linear_svg.contains(">dmg</text>") && !linear_svg.contains("log dmg"));
}

#[test]
fn the_markdown_has_both_modes_and_every_level() {
    let doc = report::markdown(60, 1).unwrap();
    for heading in ["## Greedy mode", "## Look-ahead mode", "## Strategy at each level"] {
        assert!(doc.contains(heading), "{heading}");
    }
    for lvl in 2..=MAX_LEVEL {
        assert!(doc.contains(&format!("### Level {lvl} - ")));
        assert_eq!(doc.matches(&format!("| **{lvl}** |")).count(), 2);
    }
    assert!(doc.contains("60 simulations."));
    assert_eq!(doc.matches("<svg").count(), 8); // 2 modes x log/linear x 2
    assert_eq!(doc.matches(">log dmg</text>").count(), 4);
}
