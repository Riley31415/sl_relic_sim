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
    assert_eq!(q[11], Some(Quest::Total { glory: 71, despair: 18 }));
    assert_eq!(q[12], Some(Quest::Total { glory: 73, despair: 17 }));
    assert_eq!(q[13], Some(Quest::Relics { which: vec![9, 10, 11], bar: Bar::new(7, Some(2)) }));
    assert_eq!(q[17], Some(Quest::Total { glory: 89, despair: 21 }));
    assert_eq!(q[20], Some(Quest::Total { glory: 96, despair: 16 }));
    assert_eq!(q[1], None); // level 1 is free
    assert_eq!(game().max_level(), MAX_LEVEL);
    assert_eq!(RELICS[CRIT], "Demon Eye of Weakness");
    assert_eq!(RELICS[ATK], "Giant's Right Hand");
}

#[test]
fn banking_looks_at_the_next_named_relics() {
    assert_eq!(game().next_named(4), Some((5, set_of(&[3, 4, 5]))));
    assert_eq!(game().next_named(6), Some((9, set_of(&[6, 7, 8]))));
    assert_eq!(game().next_named(9), Some((13, set_of(&[9, 10, 11]))));
    assert_eq!(game().next_named(13), None);
    assert_eq!(game().future_named(9), set_of(&[9, 10, 11]));
    assert_eq!(game().future_named(13), 0);
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
    let steps: Vec<f64> = (1..=12).map(level_multiplier).collect();
    assert_eq!(steps[0], 0.0);
    for pair in steps[..7].windows(2) {
        assert!(close(pair[1] - pair[0], 0.05, 1e-12)); // levels 2-7: +5% each
    }
    assert!(close(steps[8], 0.50, 1e-12)); // level 9
    assert!(close(steps[9], 0.60, 1e-12)); // level 10
    assert!(close(steps[10], 0.70, 1e-12)); // level 11
    assert!(close(steps[11], 0.80, 1e-12)); // level 12
    let at = |lvl| level_multiplier(lvl);
    assert!(close(at(15), 1.10, 1e-12) && close(at(16), 1.25, 1e-12));
    assert!(close(at(19), 1.70, 1e-12) && close(at(20), 1.90, 1e-12));
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
    let p = Plan::new([(8, StepChoice::default().bank()), (14, StepChoice::default().bank())]);
    assert_eq!(p.protected(game(), 8), set_of(&[6, 7, 8]));
    assert_eq!(p.protected(game(), 14), 0); // nothing named after level 13
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
    assert!(!levels::knobs(game(), 14).contains(&"bank"));
}

#[test]
fn the_saved_plans() {
    let (look, greedy) = (plan("lookahead"), plan("greedy"));
    assert_eq!(look.step(6).profile, Some(Profile::Repair));
    assert_eq!(look.step(2).filler, None); // look-ahead saves its early stock
    assert_eq!(greedy.step(2).filler, Some((4, 2))); // greedy spends it on pity
    for lvl in [8, 11, 12, 14, 20] {
        assert_eq!(greedy.step(lvl).profile, Some(Profile::Repair));
        assert_eq!(look.step(lvl).profile, Some(Profile::Repair));
    }
    assert_eq!(look.step(13).filler, None); // no filler on the last named relics
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
fn filler_makes_its_own_step_cheaper() {
    // spare stock spent for pity shortens the wait on a named-relic level
    // (though to level 20 the stock it burns is missed later on)
    for lvl in [5, 9] {
        let filled = Plan::new([(lvl, StepChoice::filler((4, 2)))]);
        let plain = levels::step_cost(game(), &Plan::default(), lvl, 2000, 7).unwrap();
        assert!(levels::step_cost(game(), &filled, lvl, 2000, 7).unwrap() < plain, "level {lvl}");
    }
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
    let o = SearchOptions { runs: 100, final_runs: 200, seed: 1, max_level: 4, tier: None };
    let (p, rows) = levels::greedy_search(game(), o, &|_| {}).unwrap();
    assert_eq!(rows.iter().map(|r| r.level).collect::<Vec<_>>(), [2, 3, 4]);
    assert!(rows[2].runner_up.is_some() && rows[2].bank_cost.is_some());
    assert_eq!(p.step(4), rows[2].choice);
}

#[test]
fn a_lookahead_search_never_makes_the_start_worse() {
    let before = levels::total_cost(game(), &Plan::default(), 2000, 99, 4).unwrap();
    let o = SearchOptions { runs: 200, final_runs: 800, seed: 3, max_level: 4, tier: None };
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
fn the_cost_chart_stands_a_bar_at_every_level() {
    let rows = rows();
    let svg = report::cost_chart_svg(&rows, "t");
    assert_eq!(svg.matches("<rect class=\"lv-bar\"").count(), rows.len() - 1);
    assert!(svg.contains(">log dmg</text>") && svg.contains(">+0%</text>"));
    for r in &rows[1..] {
        assert!(svg.contains(&format!(
            "<title>level {}: {} diamonds",
            r.level,
            relic::format::commas(r.mean, 0)
        )));
    }
    // one climb, nothing far out on its own: no break in the axis
    assert!(!svg.contains("axis broken"));
}

#[test]
fn the_cost_axis_breaks_before_a_far_last_tier() {
    let o = RunOptions::default().with_tiers(&levels::GREEDY_TIERS);
    let rows = level_rows(game(), &plan("greedy"), 60, 1, &o).unwrap();
    let svg = report::cost_chart_svg(&rows, "t");
    assert!(svg.contains("(axis broken before the last bar)"));
    assert_eq!(svg.matches("<polyline class=\"lv-axis\"").count(), 1); // the zig-zag
    assert!(svg.contains(&format!(">{}</text>", relic::format::human(rows[rows.len() - 1].mean))));
}

#[test]
fn the_efficiency_chart_plots_every_level_up_against_the_reference() {
    let rows = rows();
    let svg = report::efficiency_chart_svg(&rows, "e");
    let from = report::EFFICIENCY_FROM;
    assert_eq!(svg.matches("<circle class=\"lv-effdot\"").count(), rows.len() - from + 1);
    assert_eq!(svg.matches("<polyline class=\"lv-eff\"").count(), 1);
    assert!(svg.contains("<line class=\"lv-ref\"") && svg.contains(">19* Orr: 16% dmg for 156k dia</text>"));
    for r in &rows[1..] {
        assert_eq!(svg.contains(&format!(">L{}</text>", r.level)), r.level >= from, "L{}", r.level);
    }
}

#[test]
fn the_cost_chart_names_every_bar() {
    let o = RunOptions::default().with_tiers(&levels::GREEDY_TIERS);
    let rows = level_rows(game(), &plan("greedy"), 60, 1, &o).unwrap();
    let svg = report::cost_chart_svg(&rows, "t");
    for r in &rows[1..] {
        let tick = r.tier.map_or_else(|| format!("L{}", r.level), |m| format!("{m}%"));
        assert!(svg.contains(&format!(">{tick}</text>")), "{tick}");
    }
}

#[test]
fn efficiency_is_log_damage_per_million_diamonds() {
    let (_, mult, cost) = report::EFFICIENCY_REFERENCE;
    assert!(close(report::efficiency(mult, cost), 1.1664f64.ln() / 156_700.0 * 1e6, 1e-12));
    assert!(close(report::efficiency(mult, cost), 0.982, 0.001));
    // twice the damage (in log terms) for the same diamonds is twice as efficient
    assert!(close(report::efficiency(1.21, 1e5), 2.0 * report::efficiency(1.1, 1e5), 1e-12));
}

#[test]
fn dmg_ticks_step_evenly_to_just_past_the_top() {
    assert_eq!(report::dmg_ticks(81.6, 20), [0, 20, 40, 60, 80, 100]);
    assert_eq!(report::dmg_ticks(80.0, 20), [0, 20, 40, 60, 80]);
    assert_eq!(report::dmg_ticks(3.0, 10), [0, 10]);
}

/// Gaps between the damage-axis labels, top to bottom.
fn tick_gaps(svg: &str) -> Vec<f64> {
    let ys: Vec<f64> = svg
        .lines()
        .filter(|l| l.contains("lv-muted\" x=") && l.contains("\">+"))
        .map(|l| l.split("y=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap())
        .collect();
    ys.windows(2).map(|p| p[0] - p[1]).collect()
}

#[test]
fn log_ticks_bunch_up_as_they_rise() {
    let log = tick_gaps(&report::cost_chart_svg(&rows(), "t"));
    assert!(log.iter().all(|&g| g > 0.0));
    assert!(log[0] > log[log.len() - 1]);
}

#[test]
fn the_markdown_has_both_modes_and_every_level() {
    let doc = report::markdown(60, 1).unwrap();
    for heading in
        ["## Greedy mode", "## Look-ahead mode", "## Strategy", "### Greedy mode", "### Look-ahead mode"]
    {
        assert!(doc.contains(heading), "{heading}");
    }
    assert!(!doc.contains("### Level ")); // grouped, not one section a level
    assert!(doc.contains("Keeps its spare relics instead of spending them on pity"));
    assert!(doc.contains("**Pays more up front**") && doc.contains("**and gets it back**"));
    let strategy = &doc[doc.find("## Strategy").unwrap()..];
    for lvl in 2..=MAX_LEVEL {
        // every level appears in both modes' grouped lists
        let listed = |s: &str| {
            s.lines().filter(|l| l.starts_with("- **Level")).any(|l| {
                let head = &l[..l.find(":**").unwrap()];
                head.split(|c: char| !c.is_ascii_digit()).any(|n| n == lvl.to_string())
            })
        };
        let (greedy, look) = strategy.split_at(strategy.find("### Look-ahead mode").unwrap());
        assert!(listed(greedy) && listed(look), "level {lvl}");
        // a row in each mode's table and one in the attempt table
        assert_eq!(doc.matches(&format!("| **{lvl}** |")).count(), 3);
    }
    assert!(doc.contains("60 simulations."));
    assert!(doc.contains("## One attempt at each level"));
    assert_eq!(doc.matches("<svg").count(), 4); // 2 modes x 2 charts
    assert_eq!(doc.matches(">log dmg</text>").count(), 2);
    assert_eq!(doc.matches(">19* Orr: 16% dmg for 156k dia</text>").count(), 2);
}

// ------------------------------------------------------------------ tiers

#[test]
fn the_greedy_tiers_are_level_20_marks() {
    let marks: std::collections::BTreeSet<u8> =
        (0..=10u8).flat_map(|g| (0..=10u8).map(move |d| amplification((g, d)) as u8)).collect();
    assert!(levels::GREEDY_TIERS.iter().all(|m| marks.contains(m)));
    assert_eq!(levels::GREEDY_TIERS, [43, 46, 48, 50]);
    assert_eq!(levels::tiers("greedy"), levels::GREEDY_TIERS);
    assert!(levels::tiers("lookahead").is_empty());
}

#[test]
fn each_tier_farms_the_crit_relic_from_level_20() {
    let o = RunOptions::default().with_tiers(&levels::GREEDY_TIERS);
    for run in game().simulate(&plan("greedy"), 30, 5, &o).unwrap() {
        assert_eq!(run.levels.len(), MAX_LEVEL + levels::GREEDY_TIERS.len());
        for (at, &mark) in run.levels[MAX_LEVEL..].iter().zip(&levels::GREEDY_TIERS) {
            assert!(amplification(at.crit) >= f64::from(mark));
            assert!(!at.by_pity);
        }
        for pair in run.levels[..MAX_LEVEL].windows(2) {
            assert!(pair[0].diamonds <= pair[1].diamonds);
        }
        // each tier is its own farm from level 20
        assert!(run.levels[MAX_LEVEL..].iter().all(|t| t.diamonds >= run.levels[MAX_LEVEL - 1].diamonds));
        // the climb itself is untouched by the tiers after it
        assert_eq!(run.levels[MAX_LEVEL - 1].atk, run.levels[MAX_LEVEL].atk);
    }
    // and the same runs without tiers climb exactly the same way
    let plain = game().simulate(&plan("greedy"), 30, 5, &RunOptions::default()).unwrap();
    let tiered = game().simulate(&plan("greedy"), 30, 5, &o).unwrap();
    let mut extra = 0;
    for (a, b) in plain.iter().zip(&tiered) {
        assert_eq!(a.levels[..MAX_LEVEL - 1], b.levels[..MAX_LEVEL - 1]);
        // at level 20 the leftovers go into the crit relic: more attempts, no
        // more diamonds, and a crit relic at least as good
        let (a, b) = (a.levels[MAX_LEVEL - 1], b.levels[MAX_LEVEL - 1]);
        assert_eq!((a.diamonds, a.by_pity, a.atk), (b.diamonds, b.by_pity, b.atk));
        assert!(b.attempts >= a.attempts);
        assert!(amplification(b.crit) >= amplification(a.crit));
        extra += b.attempts - a.attempts;
    }
    assert!(extra > 0);
}

#[test]
fn conversion_makes_the_tiers_far_cheaper_than_the_climb() {
    // leftover stock plus 10-for-7 conversion: 42% is under a tenth of the climb
    let rows = level_rows(game(), &plan("greedy"), 200, 3, &RunOptions::default().with_tiers(&[42])).unwrap();
    let (top, tier) = (&rows[MAX_LEVEL - 1], &rows[MAX_LEVEL]);
    assert_eq!((tier.level, tier.tier, tier.label()), (MAX_LEVEL, Some(42), "20 + 42%".to_string()));
    assert!(tier.crit_amp >= 42.0);
    assert!(tier.mean - top.mean < 0.1 * top.mean, "{} vs {}", tier.mean, top.mean);
}

#[test]
fn the_markdown_lists_the_tiers_in_both_modes() {
    let doc = report::markdown(40, 2).unwrap();
    let look = doc.find("## Look-ahead mode").unwrap();
    for mark in levels::GREEDY_TIERS {
        let row = format!("| **20 + {mark}%** |");
        assert_eq!(doc.matches(&row).count(), 2, "{row}");
        assert!(doc.find(&row).unwrap() < look && doc.rfind(&row).unwrap() > look);
    }
    assert!(doc.contains("Every row is its own look-ahead"));
    assert!(doc.contains("- **Past level 20, tiers 43%, 46%, 48% and 50%:**"));
    assert!(doc.contains(">43%</text>") && doc.contains(">50%</text>"));
    assert!(!doc.contains("20 + 41%") && !doc.contains("20 + 45%"));
}

// ------------------------------------------------------------------ look-ahead targets

#[test]
fn step_codes_round_trip() {
    for level in [2, 4, 7, 8, 13] {
        for choice in levels::step_choices(game(), level) {
            for c in [choice, choice.bank()] {
                assert_eq!(StepChoice::parse(&c.code()).unwrap(), c, "{}", c.code());
            }
        }
    }
    assert_eq!(StepChoice::default().code(), "-");
    assert_eq!(StepChoice::build(Profile::Repair).closer().score().with_filler((4, 1)).code(), "R,c,s,f41");
    assert!(StepChoice::parse("R,x").is_err() && StepChoice::parse("B5").is_err());
}

#[test]
fn plan_codes_round_trip() {
    for (_, p) in levels::strategies() {
        assert_eq!(Plan::parse(&p.code()).unwrap(), p);
    }
    assert_eq!(plan("lookahead").up_to(6).code(), "4=B51,c,s; 6=R,c,s");
}

#[test]
fn targets_are_every_level_then_every_tier() {
    let targets = levels::targets();
    assert_eq!(targets.len(), MAX_LEVEL - 1 + levels::GREEDY_TIERS.len());
    assert_eq!(targets[0], levels::Target::level(2));
    assert_eq!(targets[targets.len() - 1], levels::Target::tier(50));
    for t in &targets {
        assert_eq!(levels::Target::parse(&t.code()).unwrap(), *t);
    }
    assert_eq!(levels::Target::tier(41).code(), "20+41");
}

#[test]
fn every_target_has_a_plan_that_stops_at_it() {
    let plans = levels::lookahead_plans().unwrap();
    assert_eq!(plans.iter().map(|(t, _)| *t).collect::<Vec<_>>(), levels::targets());
    for (t, p) in &plans {
        assert!(p.steps.keys().all(|&l| l <= t.level), "{}", t.code());
    }
}

#[test]
fn saved_plans_parse_and_format_back() {
    let text = "# a comment

4: 4=B52,c,s
20+50: 8=R,c,s,b,f41; 20=R,c,f42
";
    let plans = levels::parse_plans(text).unwrap();
    assert_eq!(plans.len(), 2);
    assert_eq!(levels::parse_plans(&levels::format_plans(&plans)).unwrap(), plans);
}

#[test]
fn each_look_ahead_row_is_its_own_target() {
    let rows = report::target_rows(game(), 30, 4).unwrap();
    let labels: Vec<String> = rows.iter().map(|r| r.label()).collect();
    let mut expect = vec!["1".to_string()];
    expect.extend(levels::targets().iter().map(|t| match t.tier {
        Some(m) => format!("{} + {m}%", t.level),
        None => t.level.to_string(),
    }));
    assert_eq!(labels, expect);
    for r in rows.iter().filter(|r| r.tier.is_some()) {
        assert!(r.crit_amp >= f64::from(r.tier.unwrap()));
    }
}

#[test]
fn a_tier_search_scores_the_climb_and_the_farm_together() {
    let t = levels::Target::tier(41);
    let climb =
        levels::target_cost(game(), &plan("greedy"), 100, 7, levels::Target::level(MAX_LEVEL)).unwrap();
    let whole = levels::target_cost(game(), &plan("greedy"), 100, 7, t).unwrap();
    assert!(whole >= climb);
    assert_eq!(levels::SearchOptions::per_target(t).tier, Some(41));
}
