//! The solver's reports: number formatting, table colours, and golden files.
//!
//! The golden files are the reports as the original Python implementation
//! printed them; the exact solver must reproduce them character for character.

use relic::format::pct3;
use relic::heuristics;
use relic::solve_report::{self, DESPAIR_RED, GLORY_GOLD, SolveArgs, heat_colour, ramp_colour};

fn args(level: i64, target: Option<(u8, u8)>, max_level: u32) -> SolveArgs {
    SolveArgs {
        level,
        target,
        amp_glory: 5.0,
        amp_despair: 2.0,
        safety_first: false,
        wipe_penalty: 0.0,
        max_level,
        seed: 12345,
    }
}

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    let expected = std::fs::read_to_string(&path).expect("golden file");
    if actual != expected.replace("\r\n", "\n") {
        let first = actual
            .lines()
            .zip(expected.lines())
            .position(|(a, e)| a != e)
            .map_or("the line count".to_string(), |i| format!("line {}", i + 1));
        panic!("{name} differs from {path} at {first}");
    }
}

#[test]
fn golden_level_1_report() {
    golden("solve_l1", &solve_report::report(&args(1, None, 20), true, true, 0, 0).unwrap());
}

#[test]
fn golden_level_7_target_report() {
    golden("solve_l7_target", &solve_report::report(&args(7, Some((5, 1)), 20), true, true, 0, 0).unwrap());
}

#[test]
fn golden_strategy_comparison() {
    golden("strategies", &solve_report::compare_strategies(&args(1, Some((4, 2)), 12)).unwrap());
}

#[test]
fn golden_level_20_html_grid_and_curve() {
    golden("amp_html", &solve_report::amplification_table(&args(20, None, 20), true).unwrap());
}

#[test]
fn golden_heuristics() {
    golden("heur_l7", &heuristics::report(7, 5.0, 2.0).unwrap());
}

#[test]
fn three_digits_showing() {
    for (value, expected) in [
        (1.0, "100%"),
        (0.98304, "98.3%"),
        (0.0936, "9.36%"),
        (0.000658, "0.07%"),
        (0.5, "50.0%"),
        (0.0999, "9.99%"),
        (0.01, "1.00%"),
    ] {
        assert_eq!(pct3(value), expected, "{value}");
    }
}

#[test]
fn anything_under_a_hundredth_rounds_down_to_zero() {
    assert_eq!(pct3(0.0), "0%");
    assert_eq!(pct3(0.00004), "0%");
    assert_eq!(pct3(0.00001), "0%");
    assert_eq!(pct3(0.0001), "0.01%"); // the smallest value that shows
}

#[test]
fn axis_ramps_run_white_to_their_colour() {
    assert_eq!(ramp_colour(0.0, GLORY_GOLD).0, "#ffffff");
    assert_eq!(ramp_colour(1.0, GLORY_GOLD).0, "#d4af37");
    assert_eq!(ramp_colour(0.0, DESPAIR_RED).0, "#ffffff");
    assert_eq!(ramp_colour(1.0, DESPAIR_RED).0, "#8b0000");
}

#[test]
fn table_ink_is_always_black() {
    for f in [0.0, 0.25, 0.5, 0.75, 1.0] {
        assert_eq!(ramp_colour(f, DESPAIR_RED).1, "#000000");
        assert_eq!(ramp_colour(f, GLORY_GOLD).1, "#000000");
        assert_eq!(heat_colour(f).1, "#000000");
    }
}

#[test]
fn heat_scale_runs_light_red_to_green() {
    assert_eq!(heat_colour(0.0).0, "#f7beb9"); // light red, not a block
    assert_eq!(heat_colour(0.5).0, "#ffffbf");
    assert_eq!(heat_colour(1.0).0, "#1a9850");
    assert_eq!(heat_colour(-1.0).0, "#f7beb9"); // clamped outside 0..1
    assert_eq!(heat_colour(2.0).0, "#1a9850");
}
