//! Numbers in the game's pixel font, read off a screenshot by template.
//!
//! A field is sampled at the size of the relic cells' font, so a line of text
//! is `ROWS` samples high, and turned into ink (0 background, 1 text) for its
//! text colour.  A line is read by dynamic programming: every column is blank
//! or part of a glyph, and every glyph is the closest learned template.

use image::RgbImage;

/// Rows in a line of text: the cell font's 9-10 rows and a margin.
pub const ROWS: usize = 11;

/// Below this spread between background and text a field holds no text.
const MIN_CONTRAST: f64 = 40.0;
/// Cost of a column left blank, per unit of ink in it.
const BLANK: f32 = 1.0;
/// Cost of each glyph, so one glyph is not read as two.
const PER_GLYPH: f32 = 1.5;

/// The text colours on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    /// stock counts
    White,
    /// glory counts, over relic art
    Gold,
    /// the requirement's glory, beside white text
    Orange,
    /// despair counts and the requirement's despair
    Red,
}

impl Color {
    /// How strongly a pixel shows text of this colour.
    fn value(self, [r, g, b]: [f64; 3]) -> f64 {
        match self {
            Color::White => r.min(g).min(b),
            Color::Gold => r.min(g),
            Color::Orange => r.min(g) - b,
            Color::Red => r - g.max(b),
        }
    }
}

/// Mean colour of the `size`-wide square centred on pixel position (x, y),
/// each pixel weighted by how much of it the square covers.
pub fn sample(img: &RgbImage, x: f64, y: f64, size: f64) -> [f64; 3] {
    let half = size.max(1.0) / 2.0;
    let (x0, x1, y0, y1) = (x + 0.5 - half, x + 0.5 + half, y + 0.5 - half, y + 0.5 + half);
    let (w, h) = (i64::from(img.width()), i64::from(img.height()));
    let mut sum = [0.0; 3];
    let mut weight = 0.0;
    for py in (y0.floor() as i64).max(0)..(y1.ceil() as i64).min(h) {
        let wy = y1.min(py as f64 + 1.0) - y0.max(py as f64);
        for px in (x0.floor() as i64).max(0)..(x1.ceil() as i64).min(w) {
            let wt = wy * (x1.min(px as f64 + 1.0) - x0.max(px as f64));
            if wt <= 0.0 {
                continue;
            }
            let p = img.get_pixel(px as u32, py as u32).0;
            for c in 0..3 {
                sum[c] += wt * f64::from(p[c]);
            }
            weight += wt;
        }
    }
    if weight == 0.0 { [0.0; 3] } else { sum.map(|v| v / weight) }
}

/// A grid of ink in [0, 1], row by row.
#[derive(Clone, Debug, PartialEq)]
pub struct Ink {
    pub w: usize,
    pub h: usize,
    pub ink: Vec<f32>,
}

impl Ink {
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.ink[y * self.w + x]
    }

    /// Ink summed down each column.
    pub fn columns(&self) -> Vec<f32> {
        (0..self.w).map(|x| (0..self.h).map(|y| self.at(x, y)).sum()).collect()
    }

    /// Columns `x0..x1` on their own.
    pub fn slice(&self, x0: usize, x1: usize) -> Ink {
        let w = x1 - x0;
        let ink =
            (0..self.h).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| self.at(x, y)).collect();
        Ink { w, h: self.h, ink }
    }

    /// Light to dark, one character a sample, for --dump.
    pub fn art(&self) -> String {
        const SHADES: [char; 5] = [' ', '.', ':', 'o', '#'];
        let mut out = String::new();
        for y in 0..self.h {
            for x in 0..self.w {
                out.push(SHADES[((self.at(x, y) * 4.0).round() as usize).min(4)]);
            }
            out.push('\n');
        }
        out
    }
}

/// Sample a field as ink of `color`: `w` x `h` samples `step` pixels apart,
/// the first centred on pixel position (x, y).  None if it shows no text.
pub fn field(img: &RgbImage, (x, y): (f64, f64), w: usize, h: usize, step: f64, color: Color) -> Option<Ink> {
    let values: Vec<f64> = (0..h)
        .flat_map(|j| (0..w).map(move |i| (i, j)))
        .map(|(i, j)| color.value(sample(img, x + i as f64 * step, y + j as f64 * step, step)))
        .collect();
    // background at the median (text covers far less than half a field),
    // text at the 98th percentile (more than 2% of any field is text)
    let mut sorted = values.clone();
    sorted.sort_by(f64::total_cmp);
    let lo = sorted[sorted.len() / 2];
    let hi = sorted[(sorted.len() * 98 / 100).min(sorted.len() - 1)];
    if hi - lo < MIN_CONTRAST {
        return None;
    }
    let ink = values.iter().map(|v| ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32).collect();
    Some(Ink { w, h, ink })
}

/// The `ROWS`-high band of a field its text sits in, centred on the text's
/// ink.  None if the field is too short or empty.
pub fn line(field: &Ink) -> Option<Ink> {
    if field.h < ROWS {
        return None;
    }
    let rows: Vec<f32> = (0..field.h).map(|y| (0..field.w).map(|x| field.at(x, y)).sum()).collect();
    let window = |t: usize| rows[t..t + ROWS].iter().sum::<f32>();
    let best = (0..=field.h - ROWS).max_by(|&a, &b| window(a).total_cmp(&window(b)))?;
    let mass = window(best);
    if mass <= 0.0 {
        return None;
    }
    let centre = (best..best + ROWS).map(|y| y as f32 * rows[y]).sum::<f32>() / mass;
    let top = (centre - (ROWS - 1) as f32 / 2.0).round().clamp(0.0, (field.h - ROWS) as f32) as usize;
    let ink = field.ink[top * field.w..(top + ROWS) * field.w].to_vec();
    Some(Ink { w: field.w, h: ROWS, ink })
}

/// Columns of a line with no more than this share of its densest column's
/// ink count as gaps.
const FAINT: f32 = 0.15;

/// Runs of inked columns, as [start, end), split wherever at least `gap`
/// faint columns in a row fall between them.
pub fn words(line: &Ink, gap: usize) -> Vec<(usize, usize)> {
    let cols = line.columns();
    let max = cols.iter().copied().fold(0.0, f32::max);
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (x, &c) in cols.iter().enumerate() {
        if c <= FAINT * max {
            continue;
        }
        match out.last_mut() {
            Some((_, end)) if x - *end < gap => *end = x + 1,
            _ => out.push((x, x + 1)),
        }
    }
    out
}

/// One glyph as the screen draws it at the cell font's size.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub ch: char,
    pub ink: Ink,
}

/// How wide a glyph usually is between the valleys around it.
fn usual_width(ch: char) -> f32 {
    match ch {
        '1' => 3.0,
        'x' => 6.0,
        _ => 5.5,
    }
}

/// Cut a line known to read `text` into its glyphs: its ink split at the
/// faintest columns that leave each glyph near its usual width, every
/// glyph trimmed of faint columns.  For learning templates.
pub fn cut(line: &Ink, text: &str) -> Option<Vec<Glyph>> {
    let chars: Vec<char> = text.chars().collect();
    let cols = line.columns();
    let max = cols.iter().copied().fold(0.0, f32::max);
    let first = cols.iter().position(|&c| c > FAINT * max)?;
    let last = cols.iter().rposition(|&c| c > FAINT * max)?;
    // cut columns c_1 < ... < c_{n-1}; glyph k runs between its cuts
    let n = chars.len();
    let mut best: Option<(f32, Vec<usize>)> = None;
    let mut cuts = Vec::with_capacity(n);
    search_cuts(&cols, &chars, first, last + 1, &mut cuts, 0.0, &mut best);
    let (_, cuts) = best?;
    let bounds: Vec<(usize, usize)> = (0..n)
        .map(|k| {
            let start = if k == 0 { first } else { cuts[k - 1] + 1 };
            let end = if k == n - 1 { last + 1 } else { cuts[k] };
            (start, end)
        })
        .collect();
    bounds
        .into_iter()
        .zip(chars)
        .map(|((mut a, mut b), ch)| {
            while b - a > 1 && cols[a] <= FAINT * max {
                a += 1;
            }
            while b - a > 1 && cols[b - 1] <= FAINT * max {
                b -= 1;
            }
            Some(Glyph { ch, ink: line.slice(a, b) })
        })
        .collect()
}

fn search_cuts(
    cols: &[f32],
    chars: &[char],
    start: usize,
    end: usize,
    cuts: &mut Vec<usize>,
    cost: f32,
    best: &mut Option<(f32, Vec<usize>)>,
) {
    let k = cuts.len();
    let from = if k == 0 { start } else { cuts[k - 1] + 1 };
    let shape = |width: usize, ch: char| 0.3 * (width as f32 - usual_width(ch)).powi(2);
    if k == chars.len() - 1 {
        let total = cost + shape(end.saturating_sub(from), chars[k]);
        if end >= from + 2 && best.as_ref().is_none_or(|(b, _)| total < *b) {
            *best = Some((total, cuts.clone()));
        }
        return;
    }
    for c in from + 2..end.saturating_sub(2) {
        cuts.push(c);
        search_cuts(cols, chars, start, end, cuts, cost + cols[c] + shape(c - from, chars[k]), best);
        cuts.pop();
    }
}

/// Mismatch between columns `x..x + w` of `line` and `glyph` stretched to
/// `w` columns, at the best of three vertical shifts.
fn mismatch(line: &Ink, x: usize, w: usize, glyph: &Ink) -> f32 {
    let column = |gx: f32, y: usize| -> f32 {
        // glyph ink at fractional column gx, linearly interpolated
        let a = gx.floor() as usize;
        let t = gx - a as f32;
        let b = (a + 1).min(glyph.w - 1);
        glyph.at(a.min(glyph.w - 1), y) * (1.0 - t) + glyph.at(b, y) * t
    };
    let scale = if w > 1 { (glyph.w - 1) as f32 / (w - 1) as f32 } else { 0.0 };
    let mut best = f32::INFINITY;
    for dy in [-1i32, 0, 1] {
        let mut d = 0.0;
        for y in 0..line.h {
            let gy = y as i32 - dy;
            for i in 0..w {
                let g = if (0..glyph.h as i32).contains(&gy) {
                    column(i as f32 * scale, gy as usize)
                } else {
                    0.0
                };
                d += (line.at(x + i, y) - g).powi(2);
            }
        }
        best = best.min(d);
    }
    best
}

/// A line read as text.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub text: String,
    /// total cost of the reading
    pub cost: f32,
    /// the share of the line's ink the reading leaves unexplained: comparable
    /// between samplings of the same text at different sizes
    pub fit: f32,
    /// the smallest lead any glyph's character had over the next-best
    /// character for the same columns: small means a doubtful reading
    pub margin: f32,
}

/// Read a line with the glyphs whose characters are in `alphabet`.
pub fn decode(line: &Ink, glyphs: &[Glyph], alphabet: &str) -> Option<Reading> {
    let glyphs: Vec<&Glyph> = glyphs.iter().filter(|g| alphabet.contains(g.ch)).collect();
    let cols: Vec<f32> = (0..line.w).map(|x| (0..line.h).map(|y| line.at(x, y).powi(2)).sum()).collect();
    let w = line.w;
    // cost[x]: the cheapest reading of columns 0..x, and how it got there
    let mut cost = vec![f32::INFINITY; w + 1];
    let mut back: Vec<Option<(usize, Option<char>)>> = vec![None; w + 1];
    cost[0] = 0.0;
    for x in 0..w {
        if cost[x].is_infinite() {
            continue;
        }
        let blank = cost[x] + BLANK * cols[x];
        if blank < cost[x + 1] {
            cost[x + 1] = blank;
            back[x + 1] = Some((x, None));
        }
        for g in &glyphs {
            for gw in g.ink.w.saturating_sub(1).max(2)..=g.ink.w + 1 {
                if x + gw > w {
                    break;
                }
                let c = cost[x] + PER_GLYPH + mismatch(line, x, gw, &g.ink);
                if c < cost[x + gw] {
                    cost[x + gw] = c;
                    back[x + gw] = Some((x, Some(g.ch)));
                }
            }
        }
    }
    let mut text = Vec::new();
    let mut margin = f32::INFINITY;
    let mut x = w;
    while x > 0 {
        let (from, ch) = back[x]?;
        if let Some(ch) = ch {
            text.push(ch);
            margin = margin.min(lead(line, from, x - from, &glyphs, ch));
        }
        x = from;
    }
    text.reverse();
    let energy: f32 = cols.iter().sum();
    let fit = (cost[w] - PER_GLYPH * text.len() as f32) / energy.max(1e-6);
    Some(Reading { text: text.into_iter().collect(), cost: cost[w], fit, margin })
}

/// How much better `ch` fits columns `x..x + w` than any other character.
fn lead(line: &Ink, x: usize, w: usize, glyphs: &[&Glyph], ch: char) -> f32 {
    let (mut mine, mut other) = (f32::INFINITY, f32::INFINITY);
    for g in glyphs {
        let d = mismatch(line, x, w, &g.ink);
        if g.ch == ch {
            mine = mine.min(d);
        } else {
            other = other.min(d);
        }
    }
    other - mine
}
