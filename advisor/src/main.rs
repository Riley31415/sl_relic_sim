//! `relic-advisor`: read a Hero's Legacy screenshot and say what the
//! look-ahead plan does next - which relic to attempt and how to play it,
//! whether to keep the result, or to summon.
//!
//!     relic-advisor shot.png                  advise toward level 20 + 43%
//!     relic-advisor shot.png --target 20      a different look-ahead target
//!     relic-advisor shot.png --level 19       when the level can't be read
//!     relic-advisor --level 18 --board 7/1/207 7/1/115 ...   no screenshot
//!     relic-advisor shot.png --dump           what every number was read from

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use image::{Rgb, RgbImage};
use relic::levels::report::{bar_text, describe_step, requirement_text};
use relic::levels::{
    self, Advice, Bar, CRIT, Game, Move, Next, Quest, RELICS, Relic, TableKey, Target, amplification,
    expected_closing, outcomes,
};
use relic::solver::Config;
use relic_advisor::vision::{self, Frame, Screen, Spot};

#[derive(Parser)]
#[command(
    name = "relic-advisor",
    about = "What the look-ahead plan does next, from a Hero's Legacy screenshot"
)]
struct Args {
    /// a screenshot of Hero's Legacy showing all twelve relics (PNG, JPEG, WebP, BMP)
    screenshot: Option<PathBuf>,
    /// the inheritor level now (by default worked out from the requirement shown)
    #[arg(long)]
    level: Option<usize>,
    /// what the plan is for: a level (2-20), or 20+MARK for the crit relic
    /// farmed to MARK% once at level 20 (43, 46, 48 or 50)
    #[arg(long, default_value = "20+43")]
    target: String,
    /// the twelve relics typed in, in screen order, as glory/despair/on-hand
    #[arg(long, num_args = 12, value_name = "G/D/N")]
    board: Option<Vec<String>>,
    /// how full the pity bar is, in percent: a plan that rolls spare relics
    /// for the pity only once they can fill it needs this to do so
    #[arg(long, value_name = "PERCENT")]
    pity: Option<f64>,
    /// show the ink every number was read from
    #[arg(long)]
    dump: bool,
    /// write the screenshot with every box read outlined (green read, red not)
    #[arg(long, value_name = "PATH")]
    debug: Option<PathBuf>,
}

/// The board, however it was found.
struct Board {
    states: Vec<Relic>,
    stock: Vec<u32>,
    /// the requirement shown for the next level, if read
    need: Option<(u32, u32)>,
    /// the screen's own totals, if read
    total: Option<(u32, u32)>,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
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

fn run(a: Args) -> Result<String, String> {
    let game = Game::standard();
    let target = Target::parse(&a.target)?;
    let plan = levels::lookahead_plan(target)?;
    let mut out = String::new();
    let board = match (&a.board, &a.screenshot) {
        (Some(cells), _) => typed_board(cells)?,
        (None, Some(path)) => read_board(path, &a, &mut out)?,
        (None, None) => return Err("give a screenshot, or the relics with --board".into()),
    };

    out.push_str(&board_table(&board));
    let read_level = board.need.and_then(|need| level_for(game, need));
    let level = match (a.level, read_level, board.need) {
        (Some(level), Some(read), _) if level != read => {
            out.push_str(&format!(
                "  note: the requirement shown is level {}'s, so the screen says level {read}; going with --level {level}\n",
                read + 1
            ));
            level
        }
        (Some(level), _, _) => level,
        (None, Some(read), _) => read,
        (None, None, Some((g, d))) => {
            return Err(format!(
                "{out}\nthe requirement read as {g}+ glory / {d}- despair, which no level asks for - give --level"
            ));
        }
        (None, None, None) => return Err(format!("{out}\ncould not read the requirement - give --level")),
    };

    let pity = a.pity.map(|pct| {
        let needed = levels::pity_needed((level + 1).min(levels::MAX_LEVEL));
        (f64::from(needed) * pct.clamp(0.0, 100.0) / 100.0).round() as u32
    });
    let advice = game.advise(&plan, target, level, &board.states, &board.stock, pity)?;
    out.push_str(&situation(game, &advice, &board));
    let step = describe_step(game, &plan, advice.goal);
    let goal = advice.goal;
    out.push_str(&wrap(
        &format!("plan: look-ahead to {}; level {goal} step: {step}", target_text(target)),
        2,
        8,
    ));
    out.push('\n');
    out.push_str(&advice_text(game, &advice, &board));
    Ok(out)
}

// ------------------------------------------------------------------ input

fn typed_board(cells: &[String]) -> Result<Board, String> {
    let mut states = Vec::new();
    let mut stock = Vec::new();
    for cell in cells {
        let parts: Vec<u32> = cell
            .split('/')
            .map(|p| p.trim().parse().map_err(|_| format!("'{cell}' is not glory/despair/on-hand")))
            .collect::<Result<_, _>>()?;
        let [g, d, n] = parts[..] else { return Err(format!("'{cell}' is not glory/despair/on-hand")) };
        states.push((g as u8, d as u8));
        stock.push(n);
    }
    Ok(Board { states, stock, need: None, total: None })
}

fn read_board(path: &PathBuf, a: &Args, out: &mut String) -> Result<Board, String> {
    let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgb8();
    let screen = Screen::read(&img, vision::templates())?;
    out.push_str(&format!("Read {}  (game drawn at {:.2}x the reference)\n", path.display(), screen.frame.s));
    if screen.frame.s < 0.95 {
        out.push_str("  warning: the game is smaller on screen than the program learned it - enlarge the window if numbers misread\n");
    }
    if a.dump {
        out.push_str(&dump(&screen));
    }
    if let Some(debug) = &a.debug {
        outline(&img, &screen).save(debug).map_err(|e| format!("{}: {e}", debug.display()))?;
        out.push_str(&format!("  boxes read: {}\n", debug.display()));
    }
    let failed: Vec<String> =
        screen.reads.iter().filter(|r| r.value.is_none()).map(|r| r.spot.name()).collect();
    let relic_failed: Vec<&String> =
        failed.iter().filter(|n| !n.starts_with("total") && !n.starts_with("req")).collect();
    if !relic_failed.is_empty() {
        let names: Vec<&str> = relic_failed.iter().map(|s| s.as_str()).collect();
        return Err(format!(
            "could not read: {} - check with --dump and --debug, or type the board with --board",
            names.join(", ")
        ));
    }
    let value = |spot| screen.value(spot).expect("checked above");
    let both = |g, d| screen.value(g).zip(screen.value(d));
    Ok(Board {
        states: (0..vision::RELICS)
            .map(|i| (value(Spot::Glory(i)) as u8, value(Spot::Despair(i)) as u8))
            .collect(),
        stock: (0..vision::RELICS).map(|i| value(Spot::Stock(i))).collect(),
        need: both(Spot::NeedGlory, Spot::NeedDespair),
        total: both(Spot::TotalGlory, Spot::TotalDespair),
    })
}

/// The inheritor level whose next level asks for (glory, despair).
fn level_for(game: &Game, (glory, despair): (u32, u32)) -> Option<usize> {
    let matches = |q: &Quest| match q {
        Quest::Total { glory: g, despair: d } => (*g, *d) == (glory, despair),
        Quest::Relics { bar, .. } | Quest::Each(bar) => {
            (u32::from(bar.glory), bar.despair.map(u32::from)) == (glory, Some(despair))
        }
    };
    let goals: Vec<usize> =
        (2..game.quests.len()).filter(|&lvl| game.quests[lvl].as_ref().is_some_and(matches)).collect();
    match goals[..] {
        [goal] => Some(goal - 1),
        _ => None,
    }
}

fn dump(screen: &Screen) -> String {
    let mut out = String::new();
    for r in &screen.reads {
        let text = r.reading.as_ref().map_or("-".to_string(), |x| x.text.clone());
        let (fit, margin) = r.reading.as_ref().map_or((f32::NAN, f32::NAN), |x| (x.fit, x.margin));
        out.push_str(&format!(
            "\n{}: read '{text}' (unexplained ink {:.1}%, lead over the next-best glyph {margin:.1}, font {:.2})\n",
            r.spot.name(),
            100.0 * fit,
            r.font
        ));
        if let Some(line) = &r.line {
            out.push_str(&line.art());
        }
    }
    out.push('\n');
    out
}

/// The screenshot with every box read outlined.
fn outline(img: &RgbImage, screen: &Screen) -> RgbImage {
    let mut out = img.clone();
    for r in &screen.reads {
        let colour = if r.value.is_some() { Rgb([0, 255, 0]) } else { Rgb([255, 0, 0]) };
        draw_rect(&mut out, &screen.frame, r.spot, colour);
    }
    out
}

fn draw_rect(img: &mut RgbImage, frame: &Frame, spot: Spot, colour: Rgb<u8>) {
    let (x, y, w, h) = spot.rect(frame);
    let (x0, y0) = (x.round() as i64, y.round() as i64);
    let (x1, y1) = ((x + w).round() as i64, (y + h).round() as i64);
    let mut put = |px: i64, py: i64| {
        if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
            img.put_pixel(px as u32, py as u32, colour);
        }
    };
    for px in x0..=x1 {
        put(px, y0);
        put(px, y1);
    }
    for py in y0..=y1 {
        put(x0, py);
        put(x1, py);
    }
}

// ------------------------------------------------------------------ output

fn board_table(board: &Board) -> String {
    let mut out = String::from("\n                                 glory  despair  on hand\n");
    for (i, (&(g, d), n)) in board.states.iter().zip(&board.stock).enumerate() {
        out.push_str(&format!("  {:<30} {g:>5} {d:>8} {n:>8}\n", RELICS[i]));
    }
    let (g, d) = levels::totals(&board.states);
    let check = match board.total {
        Some(shown) if shown == (g, d) => "  (matches the screen's Total)".to_string(),
        Some((sg, sd)) => format!("  <-- the screen's Total says {sg} / {sd}: a number above is misread"),
        None if board.need.is_some() => "  (the screen's Total could not be read)".to_string(),
        None => String::new(),
    };
    out.push_str(&format!("  {:<30} {g:>5} {d:>8}{check}\n\n", "total"));
    out
}

fn target_text(target: Target) -> String {
    match target.tier {
        Some(mark) => format!("level {} + {}% {}", target.level, mark, RELICS[CRIT]),
        None => format!("level {}", target.level),
    }
}

/// Where the board stands against what it is working toward.
fn situation(game: &Game, advice: &Advice, board: &Board) -> String {
    if advice.goal == advice.level {
        let amp = amplification(board.states[CRIT]);
        return format!("  inheritor level {} (the top): {} at {amp:.0}%\n", advice.level, RELICS[CRIT]);
    }
    let quest = game.quests[advice.goal].as_ref().expect("a real level");
    let short = match quest {
        Quest::Total { glory, despair } => {
            let (g, d) = levels::totals(&board.states);
            format!(
                "  ({} glory short, {} despair over)",
                glory.saturating_sub(g),
                d.saturating_sub(*despair)
            )
        }
        _ => String::new(),
    };
    let need = requirement_text(game, advice.goal, false);
    // the totals the rolls close the gap to, when they are not this level's own
    let ahead = match (advice.horizon, quest) {
        (Some(h), Quest::Total { glory, despair }) if h == (*glory, *despair) => String::new(),
        (Some((g, d)), _) => format!("; rolls play to the totals ahead, {g} glory / {d} despair"),
        (None, _) => String::new(),
    };
    wrap(&format!("inheritor level {}, working on level {}: {need}{short}{ahead}", advice.level, advice.goal), 2, 8)
}

fn relic_now(i: usize, board: &Board) -> String {
    let (g, d) = board.states[i];
    format!("{}  ({} on hand, now {g} glory / {d} despair)", RELICS[i], board.stock[i])
}

/// `text` in lines of at most 100 characters, the first indented `first`
/// spaces and the rest `rest`.
fn wrap(text: &str, first: usize, rest: usize) -> String {
    let mut out = String::new();
    let mut line = " ".repeat(first);
    let mut empty = true;
    for word in text.split_whitespace() {
        if !empty && line.len() + 1 + word.len() > 100 {
            out.push_str(&line);
            out.push('\n');
            line = " ".repeat(rest);
            empty = true;
        }
        if !empty {
            line.push(' ');
        }
        line.push_str(word);
        empty = false;
    }
    out.push_str(&line);
    out.push('\n');
    out
}

/// How an attempt is played, and the autoplayer setting that plays it so.
fn play_text(key: TableKey) -> String {
    match key {
        TableKey::Close { glory, despair, gap, tie: (wg, wd), .. } => format!(
            "to maximize glory, minimize despair until a limit - max useful glory {glory}, min useful despair {despair}: what closes the totals gap for this relic, {gap} useful steps off now (each glory gained or despair shed counts) - then for more glory, less despair, weighed {wg} : {wd} as those totals still need them   [autoplayer: Maximize glory, minimize despair until a limit - Max useful Glory {glory}, Min useful Despair {despair}]"
        ),
        TableKey::Target { glory, despair, then: (tg, td, gap), .. } => format!(
            "to maximize glory, minimize despair above a target - the best chance of {glory}+ glory / {despair}- despair (its bar) - then until a limit, max useful glory {tg}, min useful despair {td}: what closes the totals gap for this relic, {gap} useful steps off now   [autoplayer: Maximize glory, minimize despair above a target - Glory Target {glory}, Despair Target {despair}]"
        ),
        TableKey::Above { mark, least, .. } => format!(
            "to maximize amplification above a target - the best chance of {mark}% or more, then the most amplification (a result is kept from {least}%; if {mark}% slips out of reach, for the highest still possible)   [autoplayer: Maximize amplification above a target - Minimum Useful Amplification {mark}%]"
        ),
    }
}

/// For each despair count, the least glory a result needs to be kept.
fn keep_rows(game: &Game, advice: &Advice, board: &Board) -> Vec<String> {
    let slots = Config::for_level(advice.level as i64, Default::default()).map_or(10, |c| c.slots);
    let mut rows = Vec::new();
    for d in 0..=slots {
        let kept: Vec<u8> =
            (0..=slots).filter(|&g| game.keeps(advice, &board.states, (g, d)) == Some(true)).collect();
        let Some(&low) = kept.first() else { continue };
        let row = if kept.len() == usize::from(slots - low) + 1 {
            format!("{d} despair and {low}+ glory")
        } else {
            let list: Vec<String> = kept.iter().map(u8::to_string).collect();
            format!("{d} despair and glory {}", list.join(", "))
        };
        rows.push(row);
    }
    rows
}

fn chance(advice: &Advice, test: impl Fn(Relic) -> bool) -> f64 {
    let (_, key) = advice.roll().expect("a roll");
    outcomes(key).into_iter().filter(|&(r, _)| test(r)).map(|(_, p)| p).sum()
}

fn work_text(advice: &Advice, board: &Board) -> String {
    let list: Vec<String> = advice
        .work
        .iter()
        .map(|&(i, bar)| {
            let (g, d) = board.states[i];
            let n = board.stock[i];
            format!("{} {g}/{d} -> {}, {n} on hand", RELICS[i], bar_text(bar.glory, bar.despair))
        })
        .collect();
    list.join("; ")
}

fn advice_text(game: &Game, advice: &Advice, board: &Board) -> String {
    let mut out = String::new();
    let keep_block = |out: &mut String, hit: Option<Bar>| {
        out.push_str("     then pick Replace with New Effect if the new result has\n");
        let rows = keep_rows(game, advice, board);
        if rows.is_empty() {
            out.push_str("       (nothing it can roll beats what it has)\n");
        }
        for row in rows {
            out.push_str(&format!("       {row}\n"));
        }
        out.push_str("     and Keep Current Effect on anything else\n");
        let kept = chance(advice, |r| game.keeps(advice, &board.states, r) == Some(true));
        let mut odds = match hit {
            Some(bar) => format!(
                "{:.1}% to hit {}, {:.1}% to get a result worth keeping",
                100.0 * chance(advice, |r| bar.met_by(r)),
                bar_text(bar.glory, bar.despair),
                100.0 * kept
            ),
            None => format!("{:.1}% to get a result worth keeping", 100.0 * kept),
        };
        if let Some((relic, key @ TableKey::Close { glory, despair, .. })) = advice.roll() {
            let closed = expected_closing(key, (glory, despair), board.states[relic]);
            odds.push_str(&format!(", {closed:.2} useful steps until the limit on average"));
        }
        out.push_str(&format!("     odds: {odds}\n"));
    };
    match &advice.next {
        Next::LevelUp => {
            out.push_str(&format!(">> LEVEL UP - the board meets level {}'s requirement\n", advice.goal));
            out.push_str("   (already done it? run again with the new level's screen, or --level)\n");
        }
        Next::Done => {
            let amp = amplification(board.states[CRIT]);
            out.push_str(&format!(">> DONE - {} is at {amp:.0}%, the target is reached\n", RELICS[CRIT]));
        }
        Next::Move(Move::Summon) => {
            out.push_str(">> SUMMON - nothing the plan would attempt has 10 on hand\n");
            if !advice.work.is_empty() {
                out.push_str(&wrap(&format!("waiting on: {}", work_text(advice, board)), 3, 5));
            }
        }
        Next::Move(Move::Quest { relic, bar, key, .. }) => {
            out.push_str(&format!(">> ATTEMPT  {}\n", relic_now(*relic, board)));
            out.push_str(&format!("     play it {}\n", play_text(*key)));
            keep_block(&mut out, Some(*bar));
            let why = format!(
                "why: the plan is working on {} - and attempts, of those with 10 on hand, the one that closes the most of the gap per attempt (the one you hold the most of, of equals)",
                work_text(advice, board)
            );
            out.push_str(&wrap(&why, 5, 10));
        }
        Next::Move(Move::Filler { relic, key, .. }) => {
            out.push_str(&format!(">> ATTEMPT (filler, for the pity)  {}\n", relic_now(*relic, board)));
            out.push_str(&format!("     play it {}\n", play_text(*key)));
            keep_block(&mut out, None);
            let why = format!(
                "why: nothing the quest needs has 10 on hand ({}), so the plan rolls the spare relic you hold \
                 the most of to fill the pity meter instead of summoning - the pity comes whatever it rolls, \
                 and a result is kept only if it narrows the gap or is better on both bars",
                work_text(advice, board)
            );
            out.push_str(&wrap(&why, 5, 10));
        }
        Next::Convert(lots) => {
            let economy = &game.economy;
            let gained: u32 = lots.iter().map(|&(_, n)| n * economy.convert_to).sum();
            out.push_str(&format!(
                ">> CONVERT - every {} of each other relic into {} {}  (+{gained})\n",
                economy.convert_from, economy.convert_to, RELICS[CRIT]
            ));
            for &(i, n) in lots {
                out.push_str(&format!(
                    "     {:<30} {:>4} -> {:>4}\n",
                    RELICS[i],
                    n * economy.convert_from,
                    n * economy.convert_to
                ));
            }
        }
        Next::Farm { key } => {
            out.push_str(&format!(">> ATTEMPT  {}\n", relic_now(CRIT, board)));
            out.push_str(&format!("     play it {}\n", play_text(*key)));
            let now = amplification(board.states[CRIT]);
            out.push_str(&format!(
                "     (it is at {now:.0}% now; amplification is 5% a glory, -2% a despair)\n"
            ));
            keep_block(&mut out, None);
        }
    }
    out
}
