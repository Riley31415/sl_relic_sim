//! Raising the inheritor: the rules, plans, the Monte Carlo, the searches and
//! the LEVELS.md report built on them.

use relic::levels::report::{self, level_rows};
use relic::levels::{
    self, ATK, Bar, CRIT, Game, MAX_LEVEL, N_RELICS, Plan, Profile, Quest, RELIC_SHORT, RELICS,
    RunOptions, SearchOptions, StepChoice, amplification, damage, level_multiplier, pity_gain, pity_needed,
    set_of, strategy,
};
use relic::solver::{Config, Solver, Strategy};

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
    let first = StepChoice::build(Profile::Bar(5, 1)).bank().with_filler();
    let p = Plan::default().with(6, first).with(6, StepChoice::build(Profile::Repair));
    assert_eq!(p.step(6), StepChoice::build(Profile::Repair));
    assert_eq!(p.protected(game(), 6), 0);
}

#[test]
fn named_relic_levels_only_decide_their_filler() {
    for lvl in [2, 3, 5, 9] {
        let choices = levels::step_choices(game(), lvl);
        // no filler, or a filler with each window
        assert_eq!(choices.len(), 1 + levels::FILLER_WINDOWS.len());
        assert!(choices.iter().all(|c| c.profile.is_none() && (c.filler || c.filler_within.is_none())));
    }
    assert!(levels::step_choices(game(), 4).len() > 30);
    assert_eq!(levels::knobs(game(), 4, MAX_LEVEL), ["filler", "window", "bank", "profile", "trades", "ahead"]);
    // a repair has nothing to trade
    assert!(levels::turn(game(), 4, "trades", StepChoice::build(Profile::Repair)).is_empty());
    assert!(!levels::knobs(game(), 14, MAX_LEVEL).contains(&"bank"));
    // a level naming its relics on the way to 20: the totals ahead are 20's either way
    assert!(!levels::knobs(game(), 13, MAX_LEVEL).contains(&"ahead"));
}

#[test]
fn the_saved_plans() {
    let look = plan("lookahead");
    assert_eq!(look.step(6).profile, Some(Profile::Repair));
    assert!(!look.step(2).filler); // look-ahead saves its early stock
    for lvl in [8, 11, 12, 14, 20] {
        assert_eq!(look.step(lvl).profile, Some(Profile::Repair));
    }
    assert!(!look.step(13).filler); // no filler on the last named relics
    assert_eq!(plan("minimal"), Plan::default());
}

#[test]
fn labels() {
    assert_eq!(StepChoice::filler().label(), "roll the named relics to the bar, filler");
    assert_eq!(StepChoice::build(Profile::Bar(5, 1)).label(), "5+ / 1-, no filler");
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
    let (p, o) = (plan("lookahead"), RunOptions::default());
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
    let (p, o) = (plan("lookahead"), RunOptions::default());
    let runs = game().simulate(&p, 300, 5, &o).unwrap();
    let means = game().mean_costs(&p, 300, 5, &o).unwrap();
    for (index, mean) in means.iter().enumerate() {
        let direct = runs.iter().map(|r| r.levels[index].diamonds as f64).sum::<f64>() / 300.0;
        assert!(close(*mean, direct, 1e-6));
    }
    assert_eq!(levels::total_cost(game(), &p, 300, 5, MAX_LEVEL).unwrap(), means[MAX_LEVEL - 1]);
}

#[test]
fn pity_levels_up_without_the_quest() {
    let mut quests = levels::requirements();
    quests[2] = Some(Quest::Relics { which: vec![0], bar: Bar::new(99, Some(0)) }); // nothing can clear it
    let game = Game::with_quests(quests);
    let p = Plan::new([(2, StepChoice::filler())]);
    for run in game.simulate(&p, 20, 1, &RunOptions::to(2)).unwrap() {
        assert!(run.levels[1].by_pity);
        assert_eq!(run.levels[1].attempts, 5); // 500 pity at +100 an attempt
    }
}

#[test]
fn the_meter_resets_on_every_level_up() {
    for run in game().simulate(&plan("lookahead"), 300, 2, &RunOptions::to(6)).unwrap() {
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
    let cheap = game().mean_costs(&plan("lookahead"), 500, 8, &RunOptions::to(8).with_stock(40)).unwrap();
    let dear = game().mean_costs(&plan("lookahead"), 500, 8, &RunOptions::to(8)).unwrap();
    assert!(cheap[7] < dear[7]);
    let short = RunOptions { start_stock: vec![1; 3], ..RunOptions::default() };
    assert!(game().simulate(&plan("lookahead"), 5, 1, &short).is_err());
}

#[test]
fn a_hard_preference_improves_the_preferred_relics() {
    let l5 = set_of(&[3, 4, 5]);
    let preferring =
        StepChoice { prefer: Some((l5, true)), ..StepChoice::build(Profile::Bar(5, 1)) };
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
    assert!(cost("lookahead") < cost("minimal"));
}

// ------------------------------------------------------------------ searches

#[test]
fn a_lookahead_search_never_makes_the_start_worse() {
    let before = levels::total_cost(game(), &Plan::default(), 2000, 99, 4).unwrap();
    let o = SearchOptions { runs: 200, final_runs: 800, seed: 3, max_level: 4, tier: None };
    let (p, _cost) = levels::lookahead_search(game(), &Plan::default(), o, &|_| {}).unwrap();
    assert!(levels::total_cost(game(), &p, 2000, 99, 4).unwrap() < before * 1.02);
}

// ------------------------------------------------------------------ the report

fn rows() -> Vec<report::Row> {
    level_rows(game(), &plan("lookahead"), 60, 1, &RunOptions::default()).unwrap()
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
        .with(8, StepChoice::build(Profile::Repair).bank().with_filler());
    let text = |lvl| report::describe_step(game(), &banks, lvl);
    assert!(text(1).contains("free"));
    assert!(text(2).contains("Giant's Right Hand"));
    assert!(text(6).contains("repair"));
    assert!(text(8).contains("except Seal of the Legendary Archer"));
    assert!(text(9).contains("banked during level 8"));
    assert!(text(7).contains("each for the best chance of its bar, then for the goal's totals"));
    assert!(text(10).contains("4,500 pity (33 attempts at +140)"));
    assert!(report::describe_step(game(), &plan("lookahead"), 2).contains("summon"));
    assert!(report::describe_step(game(), &plan("lookahead").with(2, StepChoice::filler()), 2).contains("best-stocked spare relic"));
}

#[test]
fn the_cost_chart_stands_a_bar_at_every_level() {
    let rows = rows();
    let svg = report::cost_chart_svg(&rows, "t");
    assert_eq!(svg.matches("<rect class=\"lv-bar\"").count(), rows.len() - 1);
    assert!(svg.contains(">dmg</text>") && svg.contains(">+0%</text>"));
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
    let o = RunOptions::default().with_tiers(&levels::CRIT_TIERS);
    let rows = level_rows(game(), &plan("lookahead"), 60, 1, &o).unwrap();
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
    // a dot for every level-up from `from` that cost anything (one paid for
    // by stock left over has no damage per diamond)
    let paid = rows.windows(2).filter(|w| w[1].level >= from && w[1].mean > w[0].mean).count();
    assert!(paid >= rows.len() - from - 2);
    assert_eq!(svg.matches("<circle class=\"lv-effdot\"").count(), paid);
    assert_eq!(svg.matches("<polyline class=\"lv-eff\"").count(), 1);
    assert!(svg.contains("lv-efft lv-halo\"") && svg.contains(">Relic Inheritance by level</text>"));
    for r in &report::EFFICIENCY_REFERENCES {
        assert!(svg.contains(&format!("<line class=\"lv-ref lv-{}\"", r.colour)) && svg.contains(&format!(">{}</text>", r.label)));
    }
    for w in rows.windows(2) {
        let shown = w[1].level >= from && w[1].mean > w[0].mean;
        assert_eq!(svg.contains(&format!(">L{}</text>", w[1].level)), shown, "L{}", w[1].level);
    }
}

#[test]
fn the_cost_chart_names_every_bar() {
    let o = RunOptions::default().with_tiers(&levels::CRIT_TIERS);
    let rows = level_rows(game(), &plan("lookahead"), 60, 1, &o).unwrap();
    let svg = report::cost_chart_svg(&rows, "t");
    for r in &rows[1..] {
        let tick = r.tier.map_or_else(|| format!("L{}", r.level), |m| format!("{m}%"));
        assert!(svg.contains(&format!(">{tick}</text>")), "{tick}");
    }
}

#[test]
fn efficiency_is_log_damage_per_million_diamonds() {
    let [orr19, orr25] = &report::EFFICIENCY_REFERENCES;
    assert!(close(report::efficiency(orr19.multiplier, orr19.diamonds), 1.1664f64.ln() / 156_700.0 * 1e6, 1e-12));
    assert!(close(report::efficiency(orr19.multiplier, orr19.diamonds), 0.982, 0.001));
    assert!(close(report::efficiency(orr25.multiplier, orr25.diamonds), 0.592, 0.001));
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
fn the_markdown_has_every_level() {
    let doc = report::markdown(60, 1).unwrap();
    for heading in ["## Look-ahead plans", "## One attempt at each level", "## Strategy"] {
        assert!(doc.contains(heading), "{heading}");
    }
    assert!(!doc.contains("reedy")); // one mode: look-ahead
    assert!(!doc.contains("### Level ")); // grouped, not one section a level
    let strategy = &doc[doc.find("## Strategy").unwrap()..];
    for lvl in 2..=MAX_LEVEL {
        // every level appears in the plan's grouped list
        let listed = strategy.lines().filter(|l| l.starts_with("- **Level")).any(|l| {
            let head = &l[..l.find(":**").unwrap()];
            head.split(|c: char| !c.is_ascii_digit()).any(|n| n == lvl.to_string())
        });
        assert!(listed, "level {lvl}");
        // a row in the look-ahead table and one in the attempt table
        assert_eq!(doc.matches(&format!("| **{lvl}** |")).count(), 2);
    }
    assert!(doc.contains("60 simulations."));
    assert_eq!(doc.matches("<svg").count(), 2); // the two charts
    assert_eq!(doc.matches(">dmg</text>").count(), 1);
    assert_eq!(doc.matches(">19* Orr: 16% dmg for 156k dia</text>").count(), 1);
    assert_eq!(doc.matches(">25* Orr: 16% dmg for 251k dia</text>").count(), 1);
    assert!(doc.contains(">Damage vs Cost</text>") && doc.contains(">Level Efficiency</text>"));
    assert!(!doc.contains("After level 20, all relics"));
    assert!(doc.contains(">inheritor level reached, then crit relic amplification at level 20 (all relics converted to crit)</text>"));
}

// ------------------------------------------------------------------ tiers

#[test]
fn the_crit_tiers_are_level_20_marks() {
    let marks: std::collections::BTreeSet<u8> =
        (0..=10u8).flat_map(|g| (0..=10u8).map(move |d| amplification((g, d)) as u8)).collect();
    assert!(levels::CRIT_TIERS.iter().all(|m| marks.contains(m)));
    assert_eq!(levels::CRIT_TIERS, [43, 46, 48, 50]);
}

#[test]
fn each_tier_farms_the_crit_relic_from_level_20() {
    let o = RunOptions::default().with_tiers(&levels::CRIT_TIERS);
    for run in game().simulate(&plan("lookahead"), 30, 5, &o).unwrap() {
        assert_eq!(run.levels.len(), MAX_LEVEL + levels::CRIT_TIERS.len());
        for (at, &mark) in run.levels[MAX_LEVEL..].iter().zip(&levels::CRIT_TIERS) {
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
    // and the same runs without tiers are the same through level 20: its row
    // spends the leftovers for the most amplification either way, for no
    // tier's mark
    let plain = game().simulate(&plan("lookahead"), 30, 5, &RunOptions::default()).unwrap();
    let tiered = game().simulate(&plan("lookahead"), 30, 5, &o).unwrap();
    for (a, b) in plain.iter().zip(&tiered) {
        assert_eq!(a.levels[..MAX_LEVEL], b.levels[..MAX_LEVEL]);
    }
}

#[test]
fn conversion_makes_the_tiers_far_cheaper_than_the_climb() {
    // leftover stock plus 10-for-7 conversion: 42% is under a tenth of the climb
    let rows = level_rows(game(), &plan("lookahead"), 200, 3, &RunOptions::default().with_tiers(&[42])).unwrap();
    let (top, tier) = (&rows[MAX_LEVEL - 1], &rows[MAX_LEVEL]);
    assert_eq!((tier.level, tier.tier, tier.label()), (MAX_LEVEL, Some(42), "20 + 42%".to_string()));
    assert!(tier.crit_amp >= 42.0);
    assert!(tier.mean - top.mean < 0.1 * top.mean, "{} vs {}", tier.mean, top.mean);
}

#[test]
fn the_markdown_lists_the_tiers() {
    let doc = report::markdown(40, 2).unwrap();
    for mark in levels::CRIT_TIERS {
        let row = format!("| **20 + {mark}%** |");
        assert_eq!(doc.matches(&row).count(), 1, "{row}");
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
    assert_eq!(StepChoice::build(Profile::Repair).with_filler().code(), "R,f");
    // an older plan's filler bar and "s" are read, and dropped
    assert_eq!(StepChoice::parse("R,s,f41").unwrap(), StepChoice::build(Profile::Repair).with_filler());
    // an older plan's "c" (keep gap-closers) is what every plan does now
    assert_eq!(StepChoice::parse("R,c,s").unwrap(), StepChoice::build(Profile::Repair));
    assert!(StepChoice::parse("R,x").is_err() && StepChoice::parse("B5").is_err());
}

#[test]
fn plan_codes_round_trip() {
    for (_, p) in levels::strategies() {
        assert_eq!(Plan::parse(&p.code()).unwrap(), p);
    }
    assert_eq!(plan("lookahead").up_to(6).code(), "4=B51; 6=R");
}

#[test]
fn targets_are_every_level_then_every_tier() {
    let targets = levels::targets();
    assert_eq!(targets.len(), MAX_LEVEL - 1 + levels::CRIT_TIERS.len());
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
        levels::target_cost(game(), &plan("lookahead"), 100, 7, levels::Target::level(MAX_LEVEL)).unwrap();
    let whole = levels::target_cost(game(), &plan("lookahead"), 100, 7, t).unwrap();
    assert!(whole >= climb);
    assert_eq!(levels::SearchOptions::per_target(t).tier, Some(41));
}

// ------------------------------------------------------------------ advice

/// A level 18 board (Hero's Legacy, 92 glory / 22 despair) and its stock.
const BOARD: [(u8, u8); 12] =
    [(7, 1), (7, 1), (8, 3), (8, 2), (8, 2), (7, 1), (8, 2), (8, 3), (7, 1), (9, 1), (7, 2), (8, 3)];
const STOCK: [u32; 12] = [207, 115, 135, 92, 109, 28, 156, 105, 36, 180, 4, 137];

fn advise(target: levels::Target, level: usize, board: &[(u8, u8)], stock: &[u32]) -> levels::Advice {
    let plan = levels::lookahead_plan(target).unwrap();
    game().advise(&plan, target, level, board, stock, None).unwrap()
}

#[test]
fn a_level_plays_to_its_own_totals_or_the_totals_ahead() {
    let g = game();
    // level 19 asks 94 / 19; ahead of it on the way to 20 is 96 / 16
    assert_eq!(g.horizon_for(19, MAX_LEVEL, false), Some((94, 19)));
    assert_eq!(g.horizon_for(19, MAX_LEVEL, true), Some((96, 16)));
    // on the way to 15 (83 / 20) from level 12 (73 / 17): 12's tighter despair cap counts
    assert_eq!(g.horizon_for(12, 15, true), Some((83, 17)));
    assert_eq!(g.horizon_for(14, 15, true), Some((83, 20)));
    // a level naming its relics plays on to the goal's totals
    assert_eq!(g.horizon_for(13, MAX_LEVEL, false), Some((96, 16)));
    assert_eq!(g.horizon_for(9, 10, false), Some((68, 19)));
    // the code for it
    let step = StepChoice::parse("B52,a,t,f").unwrap();
    assert!(step.ahead && step.trades && step.filler);
    assert_eq!(step.code(), "B52,a,t,f");
}

#[test]
fn with_t_a_relic_traded_off_its_bar_counts_as_there_and_the_bars_go_further() {
    // level 9 for 10 (68 / 19), every relic built to 5/2 and playing to the
    // totals ahead (96 / 16): Seal of the Legendary Archer (relic 10) traded
    // its 5/2 for a 6/3 - as near those totals, more glory - so the bars go
    // a step further (Oath, 5/1, to 6/2) rather than the work waiting on
    // the 4 Seals on hand
    let step = StepChoice { ahead: true, ..StepChoice::build(Profile::Bar(5, 2)) };
    let board = [(6, 2), (6, 2), (5, 1), (5, 1), (6, 2), (5, 0), (6, 2), (6, 2), (6, 2), (5, 1), (6, 3), (5, 1)];
    let stock = [58, 19, 47, 99, 48, 96, 0, 55, 70, 87, 4, 70];
    let advise = |step| game().advise(&Plan::default().with(10, step), levels::Target::level(MAX_LEVEL), 9, &board, &stock, None).unwrap();
    let a = advise(StepChoice { trades: true, ..step });
    assert!(matches!(a.next, levels::Next::Move(levels::Move::Quest { relic: 2, .. })), "{a:?}");
    assert_eq!(a.work, vec![(2, Bar::new(6, Some(2))), (10, Bar::new(5, Some(2)))]);
    // without "t": the Seal is still work to do, and with 4 on hand the plan summons
    let a = advise(step);
    assert_eq!((a.next, a.work), (levels::Next::Move(levels::Move::Summon), vec![(10, Bar::new(5, Some(2)))]));
}

#[test]
fn a_windowed_filler_waits_until_the_spare_stock_can_fill_the_pity_bar() {
    // level 18 for 19 (9,000 pity, +180 an attempt): nothing the repair
    // needs is affordable, plenty of spare relics
    let plan = plan("lookahead").with(19, StepChoice::build(Profile::Repair).with_filler().within(15));
    let mut stock = STOCK;
    for i in [2, 3, 4, 6, 7, 10, 11] {
        stock[i] = 0;
    }
    let next = |stock: &[u32], pity| game().advise(&plan, levels::Target::tier(46), 18, &BOARD, stock, pity).unwrap().next;
    let summon = levels::Next::Move(levels::Move::Summon);
    // 50 attempts off: summon, keeping the spare relics
    assert_eq!(next(&stock, Some(0)), summon);
    // 6 attempts off (8,000 of 9,000), and the spare stock pays for them: fill it
    assert!(matches!(next(&stock, Some(8000)), levels::Next::Move(levels::Move::Filler { .. })));
    // 6 off, but only 5 spare attempts on hand: summon
    let few = [10, 10, 0, 0, 0, 10, 0, 0, 10, 10, 0, 0];
    assert_eq!(next(&few, Some(8000)), summon);
    // the pity not known: summon
    assert_eq!(next(&stock, None), summon);
    // a filler with no window rolls whenever stuck, as before
    let always = plan.with(19, StepChoice::build(Profile::Repair).with_filler());
    let a = game().advise(&always, levels::Target::tier(46), 18, &BOARD, &stock, Some(0)).unwrap();
    assert!(matches!(a.next, levels::Next::Move(levels::Move::Filler { .. })));
}

#[test]
fn filler_windows_have_codes_and_labels() {
    let c = StepChoice::build(Profile::Repair).with_filler().within(15);
    assert_eq!(c.code(), "R,fw15");
    assert_eq!(StepChoice::parse("R,fw15").unwrap(), c);
    assert_eq!(StepChoice::parse("R,f42w15").unwrap(), c);
    assert!(c.label().contains("when it fills the pity bar within 15"));
    assert!(StepChoice::parse("R,fwx").is_err() && StepChoice::parse("R,f4w5").is_err());
    // the search turns the window only where there is a filler
    assert!(levels::turn(game(), 19, "window", StepChoice::build(Profile::Repair)).is_empty());
    assert_eq!(levels::turn(game(), 19, "window", c).len(), levels::FILLER_WINDOWS.len() - 1);
}

#[test]
fn advice_repairs_the_worst_relic_over_the_despair_budget() {
    let target = levels::Target::tier(46);
    let a = advise(target, 18, &BOARD, &STOCK);
    assert_eq!(a.goal, 19); // 94+ / 19-: three despair over, two glory short
    // the plan's level 19 step repairs: every relic with 2+ despair is asked
    // for one less; a 3-despair relic closes the most of the gap, and
    // Mountain Crown (137) is stocked best of those, Oath (135) and Veil
    // (105).  The plan plays level 19 to the totals ahead ("a": level 20's,
    // 96 / 16): it is played to close that gap from 8/3, up to its 9 glory
    // slots, down to 0 despair
    // the rest settled 2 : 3, the board 4 glory short and 6 despair over
    assert_eq!(a.horizon, Some((96, 16)));
    let key = levels::TableKey::Close { level: 18, glory: 9, despair: 0, gap: 4, tie: (2, 3) };
    let expect = levels::Move::Quest { relic: 11, bar: Bar::new(8, Some(2)), key };
    assert_eq!(a.next, levels::Next::Move(expect));
    assert_eq!(a.roll(), Some((11, key)));
    let (three, two) = (Bar::new(8, Some(2)), Bar::new(8, Some(1)));
    assert_eq!(
        a.work,
        vec![(2, three), (3, two), (4, two), (6, two), (7, three), (10, Bar::new(7, Some(1))), (11, three)]
    );
    // kept: the bar, anything that dominates, or anything closing the gap
    assert_eq!(game().keeps(&a, &BOARD, (8, 2)), Some(true));
    assert_eq!(game().keeps(&a, &BOARD, (9, 3)), Some(true));
    // the gap to level 20's totals (96 / 16; the board 92 / 22) is 10
    assert_eq!(game().keeps(&a, &BOARD, (7, 1)), Some(true)); // gap 10 -> 9
    // gap 10 -> 10, but a despair shed outweighs a glory lost: 6 over, 4 short
    assert_eq!(game().keeps(&a, &BOARD, (7, 2)), Some(true));
    assert_eq!(game().keeps(&a, &BOARD, (8, 4)), Some(false)); // gap 10 -> 11
    assert_eq!(game().keeps(&a, &BOARD, (0, 0)), Some(false)); // a wipe
}

#[test]
fn advice_follows_the_stock() {
    let target = levels::Target::tier(46);
    // without enough Mountain Crowns the next best-stocked 3-despair relic goes
    let mut stock = STOCK;
    stock[11] = 9;
    let a = advise(target, 18, &BOARD, &stock);
    assert_eq!(a.roll().map(|(relic, _)| relic), Some(2));
    // none of the three affordable: an 8/2 asked for 8/1 - the Archer Seal
    // (156), not a filler roll
    for i in [2, 7, 11] {
        stock[i] = 0;
    }
    let a = advise(target, 18, &BOARD, &stock);
    assert_eq!(a.roll(), Some((6, levels::TableKey::Close { level: 18, glory: 9, despair: 0, gap: 3, tie: (2, 3) })));
    // no relic with 2+ despair affordable: the plan has no filler at level
    // 19, so it summons; with one, the best-stocked spare relic is rolled for
    // the pity
    for i in [3, 4, 6, 10] {
        stock[i] = 0;
    }
    let a = advise(target, 18, &BOARD, &stock);
    assert_eq!(a.next, levels::Next::Move(levels::Move::Summon));
    let plan = levels::lookahead_plan(target).unwrap();
    let plan = plan.clone().with(19, StepChoice { filler: true, ..plan.step(19) });
    let a = game().advise(&plan, target, 18, &BOARD, &stock, None).unwrap();
    assert!(matches!(a.next, levels::Next::Move(levels::Move::Filler { relic: 0, .. })), "{a:?}");
    // nothing affordable at all: summon
    let a = advise(target, 18, &BOARD, &[0; 12]);
    assert_eq!(a.next, levels::Next::Move(levels::Move::Summon));
}

#[test]
fn advice_levels_up_then_farms_the_tier() {
    let target = levels::Target::tier(46);
    let mut board = BOARD;
    board[9] = (9, 0);
    board[10] = (9, 0);
    board[2] = (8, 1); // 94 glory, 17 despair: level 19 is met
    let a = advise(target, 18, &board, &STOCK);
    assert_eq!((a.goal, a.next), (19, levels::Next::LevelUp));
    // at the top, short of 46%: trade the other relics in, then farm
    let a = advise(target, MAX_LEVEL, &board, &STOCK);
    match &a.next {
        levels::Next::Convert(lots) => {
            assert!(lots.contains(&(0, 20)) && lots.iter().all(|&(i, _)| i != CRIT && i != 10));
        }
        other => panic!("expected a trade, got {other:?}"),
    }
    let mut stock = [0; 12];
    stock[CRIT] = 10;
    let a = advise(target, MAX_LEVEL, &board, &stock);
    assert_eq!(a.next, levels::Next::Farm { key: levels::TableKey::Above { level: 20, mark: 46, least: 34 } });
    assert_eq!(game().keeps(&a, &board, (8, 1)), Some(true)); // 38% beats 33%
    assert_eq!(game().keeps(&a, &board, (7, 1)), Some(false));
    board[CRIT] = (10, 2);
    assert_eq!(advise(target, MAX_LEVEL, &board, &stock).next, levels::Next::Done);
}

#[test]
fn an_attempts_outcomes_add_up() {
    let key = levels::TableKey::Close { level: 18, glory: 9, despair: 0, gap: 4, tie: (1, 1) };
    let outcomes = levels::outcomes(key);
    assert!(close(outcomes.iter().map(|(_, p)| p).sum::<f64>(), 1.0, 1e-9));
    // the gap closed on average, from the table, is the solver's own figure
    let closed = levels::expected_closing(key, (9, 0), (8, 3));
    let mut solver = Solver::new(Config::for_level(18, Strategy::close(9, 0, (8, 3))).unwrap());
    let a = solver.analyse(None);
    let own: f64 = a.dist.iter().map(|((g, d), p)| p * solver.objective(g, d)).sum();
    assert!(close(closed, own, 1e-9), "{closed} vs {own}");
    assert!(closed > 0.0);
}

#[test]
fn runs_resumed_from_a_shared_start_cost_what_whole_runs_do() {
    // the 20 + 46% route: the climb, then the crit relic farm
    let target = levels::Target::tier(46);
    let options = target.options();
    let a = levels::lookahead_plan(target).unwrap();
    for level in [1, 6, 12, 19, 20] {
        let prefixes = game().prefixes(&a, 300, 5, &options, level).unwrap();
        assert!(prefixes.iter().all(|p| p.level() == level));
        let whole = levels::target_cost(game(), &a, 300, 5, target).unwrap();
        assert_eq!(game().mean_cost_from(&a, &prefixes, &options).unwrap(), whole, "resumed at {level}");
    }
    // a plan that differs only above the shared start resumes exactly too
    let b = a.with(13, StepChoice::build(Profile::Bar(5, 2)).with_filler());
    let prefixes = game().prefixes(&a, 300, 5, &options, 12).unwrap();
    let whole = levels::target_cost(game(), &b, 300, 5, target).unwrap();
    assert_eq!(game().mean_cost_from(&b, &prefixes, &options).unwrap(), whole);
    assert_ne!(whole, levels::target_cost(game(), &a, 300, 5, target).unwrap());
}
