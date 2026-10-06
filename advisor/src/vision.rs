//! The Hero's Legacy grid: twelve relics, each with its glory and despair
//! successes and the number on hand, the board's totals and the next level's
//! requirement.
//!
//! Everything is placed from the grid of gem icons - each relic shows a gold
//! gem over a red one - so the game can sit anywhere in the screenshot at any
//! size.  Positions are in reference pixels: pixels of
//! tests/fixtures/grid-l18.png, from the top-left relic's gold gem.

use std::collections::VecDeque;
use std::sync::LazyLock;

use image::RgbImage;

use crate::ocr::{self, Color, Glyph, Ink, Reading};

/// Between relic columns, and between relic rows.
pub const COL_STEP: f64 = 109.5;
pub const ROW_STEP: f64 = 117.0;
/// A relic's gold gem, top to bottom.
const GEM_H: f64 = 11.0;
pub const COLS: usize = 3;
pub const ROWS: usize = 4;
pub const RELICS: usize = COLS * ROWS;

/// Where the grid sits: the top-left gold gem's centre, in pixels, and
/// pixels per reference pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x0: f64,
    pub y0: f64,
    pub s: f64,
}

impl Frame {
    pub fn at(&self, u: f64, v: f64) -> (f64, f64) {
        (self.x0 + self.s * u, self.y0 + self.s * v)
    }
}

// ------------------------------------------------------------------ the grid

struct Blob {
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
    n: i32,
}

impl Blob {
    fn w(&self) -> i32 {
        self.x1 - self.x0 + 1
    }
    fn h(&self) -> i32 {
        self.y1 - self.y0 + 1
    }
    fn cx(&self) -> f64 {
        f64::from(self.x0 + self.x1) / 2.0
    }
    fn cy(&self) -> f64 {
        f64::from(self.y0 + self.y1) / 2.0
    }
    /// Solid and about as wide as tall, as a gem is (text is neither).
    fn gem_shaped(&self) -> bool {
        let (w, h) = (self.w(), self.h());
        h >= 6 && 20 * w >= 9 * h && 4 * w <= 5 * h && 10 * self.n >= 3 * w * h
    }
}

/// Clusters of pixels passing `test`, pixels up to two apart joined (a gem's
/// highlight can split it).
fn blobs(img: &RgbImage, test: impl Fn(i32, i32, i32) -> bool) -> Vec<Blob> {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let hit: Vec<bool> =
        img.pixels().map(|p| test(i32::from(p[0]), i32::from(p[1]), i32::from(p[2]))).collect();
    let mut seen = vec![false; hit.len()];
    let mut out = Vec::new();
    let mut queue = VecDeque::new();
    for start in 0..hit.len() {
        if !hit[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        queue.push_back(start);
        let (sx, sy) = ((start as i32) % w, (start as i32) / w);
        let mut b = Blob { x0: sx, x1: sx, y0: sy, y1: sy, n: 0 };
        while let Some(i) = queue.pop_front() {
            let (x, y) = ((i as i32) % w, (i as i32) / w);
            b.n += 1;
            (b.x0, b.x1, b.y0, b.y1) = (b.x0.min(x), b.x1.max(x), b.y0.min(y), b.y1.max(y));
            for ny in (y - 2).max(0)..=(y + 2).min(h - 1) {
                for nx in (x - 2).max(0)..=(x + 2).min(w - 1) {
                    let j = (ny * w + nx) as usize;
                    if hit[j] && !seen[j] {
                        seen[j] = true;
                        queue.push_back(j);
                    }
                }
            }
        }
        out.push(b);
    }
    out
}

/// Positions within `tol` of each other, grouped: each group's mean and
/// members (indices into `values`).
fn groups(values: &[f64], tol: f64) -> Vec<(f64, Vec<usize>)> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut out: Vec<(f64, Vec<usize>)> = Vec::new();
    for i in order {
        match out.last_mut() {
            Some((_, members)) if values[i] - values[*members.last().unwrap()] <= tol => members.push(i),
            _ => out.push((0.0, vec![i])),
        }
    }
    for (mean, members) in &mut out {
        *mean = members.iter().map(|&i| values[i]).sum::<f64>() / members.len() as f64;
    }
    out
}

/// Find the grid: every relic's gold gem with its red gem under it, fitted
/// to three columns and four rows.
pub fn find_frame(img: &RgbImage) -> Result<Frame, String> {
    let gold = blobs(img, |r, g, b| r >= 190 && g >= 140 && b <= 110 && r - b >= 110 && g <= r);
    let red = blobs(img, |r, g, b| r >= 160 && g <= 70 && b <= 80 && r - g.max(b) >= 100);
    let mut pairs: Vec<(f64, f64, f64)> = Vec::new(); // gold gem (cx, cy, height)
    for g in gold.iter().filter(|b| b.gem_shaped()) {
        let h = f64::from(g.h());
        let under = red.iter().filter(|r| r.gem_shaped()).any(|r| {
            let below = (r.cy() - g.cy()) / h;
            let size = f64::from(r.h()) / h;
            (r.cx() - g.cx()).abs() <= 0.3 * h && (1.1..=2.0).contains(&below) && (0.7..=1.45).contains(&size)
        });
        if under {
            pairs.push((g.cx(), g.cy(), h));
        }
    }
    if pairs.len() < 4 {
        return Err(format!(
            "found {} relic gem icons, not the Hero's Legacy grid - is this the screen with all twelve relics?",
            pairs.len()
        ));
    }
    let mut heights: Vec<f64> = pairs.iter().map(|p| p.2).collect();
    heights.sort_by(f64::total_cmp);
    let s_guess = heights[heights.len() / 2] / GEM_H;
    let xs: Vec<f64> = pairs.iter().map(|p| p.0).collect();
    let ys: Vec<f64> = pairs.iter().map(|p| p.1).collect();
    // a column or row needs two relics' gems to count
    let cols: Vec<_> =
        groups(&xs, 0.25 * COL_STEP * s_guess).into_iter().filter(|g| g.1.len() >= 2).collect();
    let rows: Vec<_> =
        groups(&ys, 0.25 * ROW_STEP * s_guess).into_iter().filter(|g| g.1.len() >= 2).collect();
    if cols.len() != COLS || rows.len() != ROWS {
        return Err(format!(
            "the relic gems make {} columns and {} rows, not {COLS} and {ROWS} - is the whole grid on screen?",
            cols.len(),
            rows.len()
        ));
    }
    // least squares: x = x0 + s * COL_STEP * col, y = y0 + s * ROW_STEP * row
    let mut fit = Vec::new();
    for (i, &(x, y, _)) in pairs.iter().enumerate() {
        let col = cols.iter().position(|g| g.1.contains(&i));
        let row = rows.iter().position(|g| g.1.contains(&i));
        if let (Some(c), Some(r)) = (col, row) {
            fit.push((COL_STEP * c as f64, ROW_STEP * r as f64, x, y));
        }
    }
    let n = fit.len() as f64;
    let sum = |f: &dyn Fn(&(f64, f64, f64, f64)) -> f64| fit.iter().map(f).sum::<f64>();
    let (sa, sb, sx, sy) = (sum(&|p| p.0), sum(&|p| p.1), sum(&|p| p.2), sum(&|p| p.3));
    let num = sum(&|p| p.0 * p.2 + p.1 * p.3) - sx * sa / n - sy * sb / n;
    let den = sum(&|p| p.0 * p.0 + p.1 * p.1) - sa * sa / n - sb * sb / n;
    let s = num / den;
    let frame = Frame { x0: (sx - s * sa) / n, y0: (sy - s * sb) / n, s };
    let worst = fit
        .iter()
        .map(|&(a, b, x, y)| (frame.x0 + s * a - x).abs().max((frame.y0 + s * b - y).abs()))
        .fold(0.0, f64::max);
    if !(s > 0.0) || worst > 0.4 * GEM_H * s || (s / s_guess - 1.0).abs() > 0.3 {
        return Err(format!("the relic gems are not on an even grid (off by {worst:.1} px)"));
    }
    Ok(frame)
}

// ------------------------------------------------------------------ fields

/// A number on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// a relic's glory successes ("x 7"), relics in the game's order
    Glory(usize),
    /// a relic's despair successes ("x 1")
    Despair(usize),
    /// how many of a relic are on hand
    Stock(usize),
    /// the Total box
    TotalGlory,
    TotalDespair,
    /// the requirement for the next level ("'Glory' 94 or more and ...")
    NeedGlory,
    NeedDespair,
}

/// Where a number is and how it is drawn.
struct Layout {
    /// box, in reference pixels from the top-left gold gem
    u: [f64; 2],
    v: [f64; 2],
    color: Color,
    /// font sizes to try, against the relic cells' font
    fonts: &'static [f64],
    /// drawn as "x 7"
    prefix: bool,
    /// the number is the last word of a line of text
    last_word: bool,
}

impl Spot {
    pub fn all() -> Vec<Spot> {
        let mut out = Vec::new();
        for i in 0..RELICS {
            out.extend([Spot::Glory(i), Spot::Despair(i), Spot::Stock(i)]);
        }
        out.extend([Spot::TotalGlory, Spot::TotalDespair, Spot::NeedGlory, Spot::NeedDespair]);
        out
    }

    pub fn name(self) -> String {
        let relic = |i: usize| relic::levels::RELICS[i];
        match self {
            Spot::Glory(i) => format!("{} glory", relic(i)),
            Spot::Despair(i) => format!("{} despair", relic(i)),
            Spot::Stock(i) => format!("{} on hand", relic(i)),
            Spot::TotalGlory => "total glory".into(),
            Spot::TotalDespair => "total despair".into(),
            Spot::NeedGlory => "requirement glory".into(),
            Spot::NeedDespair => "requirement despair".into(),
        }
    }

    fn layout(self) -> Layout {
        const CELL: &[f64] = &[1.0];
        const TOTAL: &[f64] = &[1.15, 1.2, 1.25];
        const NEED: &[f64] = &[1.05, 1.1, 1.15];
        let cell = |i: usize, u: [f64; 2], v: [f64; 2], color, prefix| {
            let (du, dv) = (COL_STEP * (i % COLS) as f64, ROW_STEP * (i / COLS) as f64);
            Layout {
                u: [u[0] + du, u[1] + du],
                v: [v[0] + dv, v[1] + dv],
                color,
                fonts: CELL,
                prefix,
                last_word: false,
            }
        };
        let total =
            |u, color| Layout { u, v: [-83.0, -61.0], color, fonts: TOTAL, prefix: true, last_word: false };
        let need = |color| Layout {
            u: [-26.0, 272.0],
            v: [474.0, 495.0],
            color,
            fonts: NEED,
            prefix: false,
            last_word: true,
        };
        match self {
            Spot::Glory(i) => cell(i, [8.0, 38.0], [-7.0, 7.0], Color::Gold, true),
            Spot::Despair(i) => cell(i, [8.0, 38.0], [10.0, 24.0], Color::Red, true),
            Spot::Stock(i) => cell(i, [5.0, 40.0], [42.0, 55.0], Color::White, false),
            Spot::TotalGlory => total([116.0, 162.0], Color::Gold),
            Spot::TotalDespair => total([181.0, 222.0], Color::Red),
            Spot::NeedGlory => need(Color::Orange),
            Spot::NeedDespair => need(Color::Red),
        }
    }

    /// The box this number is read from, in pixels: (x, y, width, height).
    pub fn rect(self, frame: &Frame) -> (f64, f64, f64, f64) {
        let l = self.layout();
        let (x, y) = frame.at(l.u[0], l.v[0]);
        (x, y, frame.s * (l.u[1] - l.u[0]), frame.s * (l.v[1] - l.v[0]))
    }

    fn alphabet(self) -> &'static str {
        if self.layout().prefix { "x0123456789" } else { "0123456789" }
    }
}

/// The line of text holding a spot's number, at the cell font's size.
pub fn spot_line(img: &RgbImage, frame: &Frame, spot: Spot, font: f64) -> Option<Ink> {
    let l = spot.layout();
    let step = frame.s * font;
    let w = ((l.u[1] - l.u[0]) / font).round() as usize;
    let h = ((l.v[1] - l.v[0]) / font).round() as usize;
    let field = ocr::field(img, frame.at(l.u[0], l.v[0]), w, h, step, l.color)?;
    let line = ocr::line(&field)?;
    if !l.last_word {
        return Some(line);
    }
    // the number at the end of a sentence: the last word, with a little room
    let &(a, b) = ocr::words(&line, 2).last()?;
    Some(line.slice(a.saturating_sub(1), (b + 1).min(line.w)))
}

/// One number as read.
#[derive(Clone, Debug)]
pub struct Read {
    pub spot: Spot,
    pub value: Option<u32>,
    pub reading: Option<Reading>,
    pub line: Option<Ink>,
    pub font: f64,
}

/// "x7" -> 7, "207" -> 207, given whether the "x" is expected.
fn parse(text: &str, prefix: bool) -> Option<u32> {
    let digits = if prefix { text.strip_prefix('x')? } else { text };
    if digits.is_empty() || digits.len() > 5 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Read one number, at whichever of its font sizes the glyphs fit best.
pub fn read_spot(img: &RgbImage, frame: &Frame, spot: Spot, glyphs: &[Glyph]) -> Read {
    let l = spot.layout();
    let mut best: Option<Read> = None;
    for &font in l.fonts {
        let Some(line) = spot_line(img, frame, spot, font) else { continue };
        let Some(reading) = ocr::decode(&line, glyphs, spot.alphabet()) else { continue };
        let value = parse(&reading.text, l.prefix);
        let better = match &best {
            None => true,
            Some(b) => {
                (value.is_some(), -reading.fit) > (b.value.is_some(), -b.reading.as_ref().unwrap().fit)
            }
        };
        if better {
            best = Some(Read { spot, value, reading: Some(reading), line: Some(line), font });
        }
    }
    best.unwrap_or(Read { spot, value: None, reading: None, line: None, font: l.fonts[0] })
}

// ------------------------------------------------------------------ learning

/// What a screenshot is known to show.
#[derive(Clone, Debug, PartialEq)]
pub struct Truth {
    /// (glory, despair, on hand) for each relic
    pub relics: [(u32, u32, u32); RELICS],
    pub total: (u32, u32),
    pub need: (u32, u32),
}

impl Truth {
    pub fn value(&self, spot: Spot) -> u32 {
        match spot {
            Spot::Glory(i) => self.relics[i].0,
            Spot::Despair(i) => self.relics[i].1,
            Spot::Stock(i) => self.relics[i].2,
            Spot::TotalGlory => self.total.0,
            Spot::TotalDespair => self.total.1,
            Spot::NeedGlory => self.need.0,
            Spot::NeedDespair => self.need.1,
        }
    }

    /// The text a spot shows: "x7", "207".
    pub fn text(&self, spot: Spot) -> String {
        let prefix = if spot.layout().prefix { "x" } else { "" };
        format!("{prefix}{}", self.value(spot))
    }
}

/// The screenshot the templates are learned from, and what it shows.
pub const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/grid-l18.png");
pub const FIXTURE_TRUTH: Truth = Truth {
    relics: [
        (7, 1, 207),
        (7, 1, 115),
        (8, 3, 135),
        (8, 2, 92),
        (8, 2, 109),
        (7, 1, 28),
        (8, 2, 156),
        (8, 3, 105),
        (7, 1, 36),
        (9, 1, 180),
        (7, 2, 4),
        (8, 3, 137),
    ],
    total: (92, 22),
    need: (94, 19),
};

/// Sub-pixel shifts each number is learned at, in reference pixels: a glyph
/// sampled between pixels blurs differently, as it does on other screens.
const SHIFTS: [(f64, f64); 5] = [(0.0, 0.0), (0.4, 0.0), (-0.4, 0.0), (0.0, 0.4), (0.0, -0.4)];

/// Glyph templates from every number on a known screenshot, bar the spots
/// `skip` names.
pub fn learn(img: &RgbImage, truth: &Truth, skip: impl Fn(Spot) -> bool) -> Result<Vec<Glyph>, String> {
    let found = find_frame(img)?;
    let mut out = Vec::new();
    for spot in Spot::all().into_iter().filter(|&s| !skip(s)) {
        let font = spot.layout().fonts[spot.layout().fonts.len() / 2];
        let text = truth.text(spot);
        for (du, dv) in SHIFTS {
            let frame = Frame { x0: found.x0 + du * found.s, y0: found.y0 + dv * found.s, ..found };
            let line = spot_line(img, &frame, spot, font).ok_or(format!("no text for {}", spot.name()))?;
            out.extend(ocr::cut(&line, &text).ok_or(format!("could not cut '{text}' for {}", spot.name()))?);
        }
    }
    Ok(out)
}

pub fn fixture() -> RgbImage {
    image::load_from_memory(FIXTURE).expect("the fixture is a PNG").to_rgb8()
}

/// The templates, learned once from the fixture.
pub fn templates() -> &'static [Glyph] {
    static GLYPHS: LazyLock<Vec<Glyph>> =
        LazyLock::new(|| learn(&fixture(), &FIXTURE_TRUTH, |_| false).expect("the fixture reads"));
    &GLYPHS
}

// ------------------------------------------------------------------ the screen

/// Everything read off one screenshot.
#[derive(Clone, Debug)]
pub struct Screen {
    pub frame: Frame,
    pub reads: Vec<Read>,
}

impl Screen {
    pub fn read(img: &RgbImage, glyphs: &[Glyph]) -> Result<Screen, String> {
        let frame = find_frame(img)?;
        let reads = Spot::all().into_iter().map(|spot| read_spot(img, &frame, spot, glyphs)).collect();
        Ok(Screen { frame, reads })
    }

    pub fn value(&self, spot: Spot) -> Option<u32> {
        self.reads.iter().find(|r| r.spot == spot).and_then(|r| r.value)
    }
}
