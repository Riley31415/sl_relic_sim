//! The diamond cost of farming a crit relic.

use relic::economy::{self, DIAMONDS_PER_SUMMON, Stock};

#[test]
fn diamonds_per_attempt() {
    // 11/12 crits a summon, plus 121/12 others converted at 10 for 7
    let per_summon = 11.0 / 12.0 + 121.0 / 12.0 * 0.7;
    assert!((economy::crits_per_summon() - per_summon).abs() < 1e-12);
    assert!((economy::diamonds_per_attempt() - 10.0 / per_summon * 5000.0).abs() < 1e-6);
    assert!((economy::diamonds_per_attempt() - 6_269.592_5).abs() < 1e-3);
}

#[test]
fn ten_of_one_type_convert_to_seven() {
    let mut stock = Stock::default();
    for _ in 0..3 {
        stock.add(0);
    }
    // nine each of two types: 18 others, but no full ten of either
    for _ in 0..9 {
        stock.add(1);
        stock.add(2);
    }
    assert_eq!(stock.crit_supply(), 3);
    stock.add(2);
    assert_eq!(stock.crit_supply(), 10);
    assert_eq!((stock.others[0], stock.others[1]), (9, 0)); // the full pile is spent
    for _ in 0..10 {
        stock.add(11);
    }
    assert_eq!(stock.crit_supply(), 17);
}

#[test]
fn cost_rises_with_the_mark() {
    let farm = economy::farm(4, 300, 11).unwrap();
    for pair in farm.rows.windows(2) {
        assert!(pair[0].mean <= pair[1].mean + 1e-9, "{} -> {}", pair[0].mark, pair[1].mark);
        assert!(pair[0].mean_attempts <= pair[1].mean_attempts + 1e-9);
    }
}

#[test]
fn spend_is_always_whole_summons() {
    for row in economy::farm(4, 200, 3).unwrap().rows {
        assert_eq!(row.median % DIAMONDS_PER_SUMMON as f64, 0.0);
        assert_eq!(row.p90 % DIAMONDS_PER_SUMMON as f64, 0.0);
    }
}

#[test]
fn attempts_match_the_geometric_expectation() {
    // first reaching a mark is geometric in P(one attempt reaches it)
    for row in economy::farm(4, 3000, 5).unwrap().rows.iter().filter(|r| r.p_attempt >= 0.02) {
        let expected = 1.0 / row.p_attempt;
        assert!((row.mean_attempts - expected).abs() <= 0.25 * expected + 0.5, "{}%", row.mark);
    }
}

#[test]
fn every_run_reaches_the_top_mark() {
    for row in economy::farm(4, 200, 9).unwrap().rows {
        assert_eq!(row.runs, 200, "{}% was not reached by every run", row.mark);
    }
}

#[test]
fn cost_markdown_is_complete() {
    let farm = economy::farm(20, 2000, 1).unwrap();
    let doc = economy::markdown(&farm, 2000).unwrap();
    for heading in ["# What a crit relic costs", "## Cost by mark", "## Cross-check", "## Assumptions"] {
        assert!(doc.contains(heading), "{heading}");
    }
    assert_eq!(doc.matches("<svg").count(), 1);
    assert!(doc.contains("| **50%** |"));
}
