// Reading numbers off the game's pixel font: the counts and boosts on the
// Compare Inheritance Results cards, the main page's counts and level, and
// the stock on Hero's Legacy.
//
// A digit is matched against labelled examples cut from every number in
// test/images, at the size captured and rescaled 0.8x-1.3x
// (chrome/lib/glyphdata.js, made by test/make-glyphs.mjs): its ink on a 7x10
// grid, its width/height and its middle.  It reads only when one digit is clearly
// nearest - otherwise null, unsure, never a guess.  The callers then check
// the numbers against each other (totals, boosts, "/10").

import { GLYPHS } from './glyphdata.js';

const T = 0.45; // ink at or above this is a stroke

/**
 * A box of the image as ink 0..1: the brightest channel, stretched between
 * the box's median (the background) and its 99th percentile (the text), so
 * gold, red and cyan text all read alike.
 */
export function inkBox(img, x0, x1, y0, y1) {
  x0 = Math.max(0, Math.round(x0));
  y0 = Math.max(0, Math.round(y0));
  x1 = Math.min(img.width, Math.round(x1));
  y1 = Math.min(img.height, Math.round(y1));
  const w = x1 - x0, h = y1 - y0;
  if (w <= 0 || h <= 0) return null;
  const m = new Float32Array(w * h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = ((y0 + y) * img.width + x0 + x) * 4;
      m[y * w + x] = Math.max(img.data[i], img.data[i + 1], img.data[i + 2]);
    }
  }
  const sorted = Array.from(m).sort((a, b) => a - b);
  const lo = sorted[Math.floor(sorted.length * 0.5)], hi = sorted[Math.floor(sorted.length * 0.99)];
  if (hi - lo < 40) return null; // no text in it
  const ink = Array.from(m, (v) => Math.min(1, Math.max(0, (v - lo) / (hi - lo))));
  // a row inked nearly the whole width is a border caught at the box's edge
  // (a plate's frame), never text - it would join every glyph into one
  for (let y = 0; y < h; y++) {
    let n = 0;
    for (let x = 0; x < w; x++) if (ink[y * w + x] >= T) n++;
    if (n >= 0.85 * w) for (let x = 0; x < w; x++) ink[y * w + x] = 0;
  }
  return { w, h, ink };
}


/** The glyphs of a text box, left to right: 8-connected strokes, merged when they overlap. */
export function glyphs(box) {
  const { w, h, ink } = box;
  const seen = new Uint8Array(w * h), found = [];
  for (let i = 0; i < w * h; i++) {
    if (seen[i] || ink[i] < T) continue;
    let x0 = w, x1 = -1, y0 = h, y1 = -1, n = 0;
    const stack = [i];
    seen[i] = 1;
    while (stack.length) {
      const j = stack.pop(), x = j % w, y = (j - x) / w;
      n++;
      x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y);
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const nx = x + dx, ny = y + dy, k = ny * w + nx;
          if (nx >= 0 && ny >= 0 && nx < w && ny < h && !seen[k] && ink[k] >= T) {
            seen[k] = 1;
            stack.push(k);
          }
        }
      }
    }
    if (n >= 3) found.push({ x0, x1, y0, y1 });
  }
  found.sort((a, b) => a.x0 - b.x0);
  const merged = [];
  for (const g of found) {
    const p = merged[merged.length - 1];
    if (p && g.x0 <= p.x1 - Math.min(p.x1 - p.x0, g.x1 - g.x0) * 0.5) {
      Object.assign(p, { x0: Math.min(p.x0, g.x0), x1: Math.max(p.x1, g.x1), y0: Math.min(p.y0, g.y0), y1: Math.max(p.y1, g.y1) });
    } else merged.push({ ...g });
  }
  return merged.flatMap((g) => split(box, g, 3));
}

/** A glyph box cut out of the text box, trimmed to its strokes. */
function cut(box, x0, x1, y0, y1) {
  const on = (x, y) => box.ink[y * box.w + x] >= T;
  const col = (x) => { for (let y = y0; y <= y1; y++) if (on(x, y)) return true; return false; };
  const row = (y) => { for (let x = x0; x <= x1; x++) if (on(x, y)) return true; return false; };
  while (x0 < x1 && !col(x0)) x0++;
  while (x1 > x0 && !col(x1)) x1--;
  while (y0 < y1 && !row(y0)) y0++;
  while (y1 > y0 && !row(y1)) y1--;
  const gi = [];
  for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) gi.push(box.ink[y * box.w + x]);
  return { x0, x1, y0, y1, w: x1 - x0 + 1, h: y1 - y0 + 1, ink: gi };
}

/**
 * Glyphs that blur into their neighbours ("22", "3%", or "135" joined along
 * a shared bottom row) come out much wider than tall.  The font is
 * proportional (a 1 is narrow) and open digits (3, 5, 7) have faint sides
 * as faint as a seam, so no rule finds the cuts: every way of cutting it
 * into 2-4 pieces of a digit's width is tried, keeping the best for each
 * reading.  The best reading wins only if its pieces match closely and no
 * other reading comes near - a wrong cut can read too ("127" as "20") -
 * otherwise it is cut evenly, and reads as unsure.
 */
export const SPLIT = { close: 0.17, margin: 0.02 };

function split(box, g, depth, force = false) {
  const whole = cut(box, g.x0, g.x1, g.y0, g.y1);
  if (depth === 0 || (!force && whole.w <= 1.15 * whole.h)) return [whole];
  const h = whole.h, minW = Math.max(2, Math.round(0.25 * h)), maxW = Math.round(0.95 * h);
  const scores = new Map();
  const piece = (x0, x1) => {
    const key = x0 * 10000 + x1;
    if (!scores.has(key)) {
      const glyph = cut(box, x0, x1, g.y0, g.y1);
      scores.set(key, { glyph, r: scoreDigit(glyph) });
    }
    return scores.get(key);
  };
  const readings = new Map(); // digits -> the best cut giving them
  const walk = (start, pieces, cost, digits) => {
    const rest = whole.x1 - start + 1;
    if (rest < minW) return;
    if (rest <= maxW && pieces.length >= 1) {
      const p = piece(start, whole.x1);
      if (p.r) {
        const all = [...pieces, p.glyph], text = digits + p.r.digit, mean = (cost + p.r.d) / all.length;
        if (!readings.has(text) || mean < readings.get(text).mean) readings.set(text, { mean, pieces: all });
      }
    }
    if (pieces.length >= 3) return;
    for (let w = minW; w <= maxW && start + w <= whole.x1 - minW + 1; w++) {
      const p = piece(start, start + w - 1);
      if (p.r) walk(start + w, [...pieces, p.glyph], cost + p.r.d, digits + p.r.digit);
    }
  };
  walk(whole.x0, [], 0, '');
  const ranked = [...readings.values()].sort((p, q) => p.mean - q.mean);
  if (ranked.length && ranked[0].mean <= SPLIT.close && (ranked.length === 1 || ranked[1].mean - ranked[0].mean >= SPLIT.margin)) {
    return ranked[0].pieces;
  }
  // no clear reading: cut evenly, for the readers to call unsure
  const parts = Math.max(2, Math.round(whole.w / (0.65 * h)));
  const out = [];
  for (let i = 0; i < parts; i++) {
    const x0 = whole.x0 + Math.round((i * whole.w) / parts), x1 = whole.x0 + Math.round(((i + 1) * whole.w) / parts) - 1;
    if (x1 > x0) out.push(cut(box, x0, x1, g.y0, g.y1));
  }
  return out;
}

const GW = 7, GH = 10;

/** A glyph's ink, area-averaged onto a 7x10 grid over its bounding box. */
export function gridOf(g) {
  const out = new Array(GW * GH).fill(0);
  const SUB = 3;
  for (let gy = 0; gy < GH; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      let sum = 0;
      for (let sy = 0; sy < SUB; sy++) {
        for (let sx = 0; sx < SUB; sx++) {
          const x = Math.min(g.w - 1, Math.floor(((gx + (sx + 0.5) / SUB) / GW) * g.w));
          const y = Math.min(g.h - 1, Math.floor(((gy + (sy + 0.5) / SUB) / GH) * g.h));
          sum += g.ink[y * g.w + x];
        }
      }
      out[gy * GW + gx] = sum / (SUB * SUB);
    }
  }
  return out;
}

/**
 * The ink in the middle of a grid (rows 4-5, columns 2-4): an 8's crossbar,
 * a 0's hole - the difference blur blunts most, so weighed on its own.
 */
function middle(grid) {
  let sum = 0;
  for (let y = 4; y <= 5; y++) for (let x = 2; x <= 4; x++) sum += grid[y * GW + x];
  return sum / 6;
}

let examples = null;
let lines = GLYPHS;
/** Match against other examples (lines as in glyphdata.js) - for tests and tools. */
export function useExamples(list) {
  lines = list;
  examples = null;
}
const known = () => (examples ??= lines.map((line) => {
  const g = Array.from(line.slice(4), (c) => Number(c) / 9);
  return { d: Number(line[0]), a: Number(line.slice(1, 4)) / 100, g, m: middle(g) };
}));

/** How far a glyph's grid and shape are from an example's. */
function distance(grid, aspect, mid, ex) {
  let d = 0;
  for (let i = 0; i < grid.length; i++) d += Math.abs(grid[i] - ex.g[i]);
  return d / grid.length + 0.3 * Math.abs(aspect - ex.a) + 0.3 * Math.abs(mid - ex.m);
}

export const MATCH = { far: 0.2, margin: 0.02 }; // nearest must be this close, and this much nearer than any other digit

/** One digit glyph as { digit, d } (its distance), or null when no digit is clearly nearest. */
function scoreDigit(g) {
  if (!g || g.h < 6 || g.w < 1 || g.w > g.h) return null;
  const grid = gridOf(g), aspect = g.w / g.h, mid = middle(grid);
  const best = new Array(10).fill(Infinity);
  for (const ex of known()) best[ex.d] = Math.min(best[ex.d], distance(grid, aspect, mid, ex));
  const order = best.map((d, digit) => ({ d, digit })).sort((x, y) => x.d - y.d);
  if (order[0].d > MATCH.far || order[1].d - order[0].d < MATCH.margin) return null;
  return order[0];
}

/** One digit glyph as 0-9, or null when no digit is clearly nearest. */
export function readDigit(g) {
  return scoreDigit(g)?.digit ?? null;
}

/** Digits to a number; null if any is unreadable or there are none. */
function toNumber(gs) {
  if (!gs.length) return null;
  let n = 0;
  for (const g of gs) {
    const d = readDigit(g);
    if (d === null) return null;
    n = 10 * n + d;
  }
  return n;
}

/**
 * The glyphs of a number in a line, by where the number sits:
 *  'trailing' - ending the line, as "Glory x 8": the trailing glyphs at full
 *               digit height (the "x" before them is shorter)
 *  'whole'    - the whole box is the number (a count badge)
 *  'percent'  - as "+34%": the full-height glyphs after the sign, before
 *               the % (shorter pieces, or one glyph as wide as it is tall)
 *  'stock'    - as "127/10" at scale `s`: the digits up to the slash, or
 *               if the slash runs into them, the text short of the fixed
 *               "/10" at its right end (20 units)
 * At most `max` of them; null if there are none.
 */
export function numberGlyphs(box, mode, max, s = 1) {
  if (!box) return null;
  const gs = glyphs(box);
  if (!gs.length) return null;
  if (mode === 'trailing') {
    // from the right, while the glyphs read as digits (at small sizes the
    // "x" before the number is nearly as tall as it)
    const H = gs[gs.length - 1].h;
    const out = [];
    for (let i = gs.length - 1; i >= 0 && out.length < max; i--) {
      const g = gs[i];
      if (g.h < 0.88 * H) break;
      if (out.length && out[0].x0 - g.x1 > 0.6 * H) break;
      if (out.length && readDigit(g) === null) break;
      out.unshift(g);
    }
    return out;
  }
  if (mode === 'whole') {
    // the box holds only the number: every full-height glyph is a digit
    const H = Math.max(...gs.map((g) => g.h));
    const out = pairs(box, gs.filter((x) => x.h >= 0.7 * H));
    return out.length <= max ? out : null;
  }
  if (mode === 'percent') {
    const H = Math.max(...gs.map((g) => g.h));
    let i = 0;
    while (i < gs.length && gs[i].h < 0.88 * H) i++; // the + sign
    const out = [];
    for (; i < gs.length && out.length < max; i++) {
      const g = gs[i];
      if (g.h < 0.88 * H || g.w > 0.9 * g.h) break; // the % sign
      if (out.length && g.x0 - out[out.length - 1].x1 > 0.6 * H) break;
      out.push(g);
    }
    return out;
  }
  // stock: the digits from the left up to a clean slash (taller than they
  // are, and narrow) - or, when the slash is run into them, the text cut
  // short of the "/10" by its width
  const lead = [];
  for (const g of gs) {
    if (readDigit(g) === null) break;
    lead.push(g);
  }
  const slash = gs[lead.length];
  if (lead.length && lead.length <= max && slash && slash.h >= 1.15 * Math.max(...lead.map((g) => g.h)) && slash.w <= 0.6 * slash.h) {
    return lead;
  }
  let left = -1, right = -1;
  for (const g of gs) {
    if (left < 0 || g.x0 < left) left = g.x0;
    right = Math.max(right, g.x1);
  }
  const end = Math.round(right - 20 * s);
  if (end <= left) return null;
  const part = { w: end - left, h: box.h, ink: [] };
  for (let y = 0; y < box.h; y++) for (let x = left; x < end; x++) part.ink.push(box.ink[y * box.w + x]);
  const ps = glyphs(part);
  const H = Math.max(0, ...ps.map((g) => g.h));
  const out = pairs(part, ps.filter((g) => g.h >= 0.7 * H));
  return out.length <= max ? out : null;
}

/**
 * Glyphs, with any that will not read but are wider than a 1 tried as two
 * digits run together - "11" sharing one base row is narrower than it is
 * tall, so the usual splitting never sees it.  Kept only if every piece reads.
 */
function pairs(box, gs) {
  const out = [];
  for (const g of gs) {
    if (readDigit(g) === null && g.w >= 0.6 * g.h) {
      const pieces = split(box, g, 1, true);
      if (pieces.length >= 2 && pieces.every((p) => readDigit(p) !== null)) {
        out.push(...pieces);
        continue;
      }
    }
    out.push(g);
  }
  return out;
}

/** The number ending a line ("Glory x 8", "Despair x 10"), up to `maxDigits`. */
export function trailingNumber(box, maxDigits = 2) {
  return toNumber(numberGlyphs(box, 'trailing', maxDigits) ?? []);
}

/** A box holding only a number (a count badge): null unless every glyph reads. */
export function wholeNumber(box, maxDigits = 4) {
  return toNumber(numberGlyphs(box, 'whole', maxDigits) ?? []);
}

/** The number in a "+34%" line. */
export function percentNumber(box) {
  return toNumber(numberGlyphs(box, 'percent', 2) ?? []);
}

/**
 * The stock on Hero's Legacy's Inheritance button ("127/10") at scale `s`,
 * accepted only when the text ends in the 0 of that "/10".
 */
export function stockNumber(box, s) {
  if (!box) return null;
  const all = glyphs(box);
  if (!all.length || !endsInTen(all[all.length - 1])) return null;
  return toNumber(numberGlyphs(box, 'stock', 4, s) ?? []);
}

/**
 * The end of "/10": a 0, or the 1 and 0 run together - cut in two, a narrow
 * 1 on the left (too thin to match well) and a 0 that reads on the right.
 */
function endsInTen(g) {
  const d = readDigit(g);
  if (d === 0) return true;
  if (d !== null || g.w < 0.9 * g.h) return false;
  // the faintest column near the middle, then each side trimmed and read
  let at = -1, least = Infinity;
  for (let x = Math.round(0.3 * g.w); x <= Math.round(0.6 * g.w); x++) {
    let sum = 0;
    for (let y = 0; y < g.h; y++) sum += g.ink[y * g.w + x];
    if (sum < least) [least, at] = [sum, x];
  }
  const side = (x0, x1) => cut({ w: g.w, h: g.h, ink: g.ink }, x0, x1, 0, g.h - 1);
  const one = side(0, at - 1);
  return one.w <= 0.55 * one.h && one.h >= 0.8 * g.h && readDigit(side(at + 1, g.w - 1)) === 0;
}
