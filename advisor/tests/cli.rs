//! The program end to end, on the fixture.

use std::process::Command;

fn advisor(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_relic-advisor")).args(args).output().expect("it runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

const SHOT: &str = "tests/fixtures/grid-l18.png";
const BOARD: [&str; 12] = [
    "7/1/207", "7/1/115", "8/3/135", "8/2/92", "8/2/109", "7/1/28", "8/2/156", "8/3/105", "7/1/36",
    "9/1/180", "7/2/4", "8/3/137",
];

#[test]
fn the_screenshot_says_to_repair_the_crown() {
    let (ok, out) = advisor(&[SHOT]);
    assert!(ok, "{out}");
    assert!(out.contains("(matches the screen's Total)"), "{out}");
    assert!(out.contains("inheritor level 18, working on level 19"), "{out}");
    assert!(
        out.contains(">> ATTEMPT  Crown of the Great Mountain  (137 on hand, now 8 glory / 3 despair)"),
        "{out}"
    );
    assert!(
        out.contains("all-or-nothing for 8+ glory / 2- despair   [autoplayer: Target, G 8, D 2]"),
        "{out}"
    );
    assert!(out.contains("0 despair and 6+ glory"), "{out}");
}

#[test]
fn a_typed_board_gets_the_same_advice() {
    let mut args = vec!["--level", "18", "--board"];
    args.extend(BOARD);
    let (ok, out) = advisor(&args);
    assert!(ok, "{out}");
    assert!(out.contains(">> ATTEMPT  Crown of the Great Mountain"), "{out}");
}

#[test]
fn a_level_given_overrides_the_screen_and_says_so() {
    let (ok, out) = advisor(&[SHOT, "--level", "19"]);
    assert!(ok, "{out}");
    assert!(out.contains("going with --level 19"), "{out}");
    assert!(out.contains("working on level 20"), "{out}");
}

#[test]
fn a_typed_board_needs_a_level() {
    let mut args = vec!["--board"];
    args.extend(BOARD);
    let (ok, out) = advisor(&args);
    assert!(!ok && out.contains("give --level"), "{out}");
}

#[test]
fn at_the_top_it_converts_then_farms() {
    let (ok, out) = advisor(&[SHOT, "--level", "20"]);
    assert!(ok, "{out}");
    assert!(out.contains(">> CONVERT"), "{out}");
    // nothing left to trade: the crit relic itself is attempted
    let mut args = vec!["--level", "20", "--board"];
    let board: Vec<String> = BOARD
        .iter()
        .enumerate()
        .map(|(i, cell)| format!("{}/{}", cell.rsplit_once('/').unwrap().0, if i == 1 { 30 } else { 0 }))
        .collect();
    args.extend(board.iter().map(String::as_str));
    let (ok, out) = advisor(&args);
    assert!(ok, "{out}");
    assert!(out.contains(">> ATTEMPT  Demon Eye of Weakness"), "{out}");
    assert!(out.contains("[autoplayer: Amplification >= 46%]"), "{out}");
}
