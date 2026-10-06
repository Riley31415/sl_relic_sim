// Reading the Memory Inheritance screens out of a tab screenshot.
//
// Everything is located from one anchor: the orange top border of the
// "Memory of Glory" panel.  Its left end is the origin and its length sets
// the scale, so the game can sit anywhere in the tab at any size.  Every
// other position below is in reference units: pixels of a screenshot where
// that border is 400 px long (measured from test/images/board-l19.png).
//
// An image is { width, height, data } with RGBA bytes, like ImageData.

import { inkBox, trailingNumber, percentNumber, stockNumber, numberGlyphs } from './digits.js';
import { amplification } from './logic.js';
import { findBanner, findBackButton, readMain } from './mainpage.js';

export const REF = {
  borderLen: 400,
  gloryBottom: 100, // the panel's bottom border
  despairTop: 107, // the Memory of Despair panel's top border
  gloryRow: 73.5, // slot centres, glory bar
  despairRow: 180, // slot centres, despair bar
  leafRow: -32, // mental strength leaves
  slotX0: 19.6, // first slot centre
  slotStep: 28.86,
  bgAbove: 16, // plain panel background this far above a slot centre
  greenBottom: -6, // the Mental Training panel's bottom border
  barRows: [-114, -112, -110], // the spirit power bar
  barX: [15, 386],
  buttons: {
    train: { x: 344.5, y: -68, top: -89 },
    glory: { x: 344.5, y: 51, top: 29 },
    despair: { x: 344.5, y: 157.5, top: 136 },
  },
  buttonSpan: [310, 380], // x range of a button's top border
  // the "50%" line on each Attempt button (a button is hidden once it cannot
  // be used, e.g. Mental Training at 0 mental strength)
  rateBoxes: {
    train: { x: [309, 380], y: [-86, -65] },
    glory: { x: [309, 380], y: [33, 54] },
    despair: { x: [309, 380], y: [139.5, 160.5] },
  },
  goldBox: { x: [90, 310], y: [215, 335] }, // Complete Inheritance / Inheritance
  compareBox: { x: [-20, 420], y: [60, 170] }, // Keep / Replace, over a dimmed board
  // Keep and Replace, measured on test/images/compare.png
  compare: { keep: [105.5, 121], replace: [293.5, 121] },
  // the abandon confirmation (test/images/abandon.png) and the button that
  // opens it (board-l19.png, board-wiped.png)
  abandon: { confirm: [135.5, 102.5], cancel: [263.5, 102.5], button: { x: [-3, 72], y: [219, 246] } },
  popupTol: 25, // how far a popup button may sit from where it is expected
  stock: { x: [-20, 30], y: [-23, -4] }, // "127/10" on Hero's Legacy's button, from its centre
  // the two cards above Keep (current memory) and Replace (new memory), in
  // units where the buttons are 188 apart, from the buttons' centre line;
  // measured on test/images/compare*.png
  cards: { spacing: 188, x: [-80, 84], glory: -214, despair: -190, boost: -142, stat: -97, half: 10 },
  // the Abandon Inheritance button: nothing is ever clicked in here
  forbidden: { x: [-30, 85], y: [195, 265] },
};

const MAX_SLOTS = 10; // further right is the attempt buttons

// ------------------------------------------------------------------ pixels

function rgb(img, x, y) {
  const i = (Math.round(y) * img.width + Math.round(x)) * 4;
  return [img.data[i], img.data[i + 1], img.data[i + 2]];
}

function inside(img, x, y) {
  return x >= 0 && y >= 0 && x < img.width && y < img.height;
}

// loose enough for a 1 px border smeared over two rows by scaling; a
// dimmed board is told apart by the border's total brightness (findAnchor)
const isOrange = (r, g, b) => r >= 100 && g >= 0.45 * r && g <= 0.75 * r && b <= 0.3 * r;
const isRedBorder = (r, g, b) => r >= 45 && g <= 45 && b <= 45 && r >= 2.5 * Math.max(g, b, 1);
const isGreenBorder = (r, g, b) => g >= 80 && g - r >= 25 && g - b >= 15;
const isBarFill = (r, g, b) => g >= 90 && g - r >= 25 && g - b >= 60;
const isLeaf = (r, g, b) => g >= 120 && g - b >= 50;
const isGold = (r, g, b) => r >= 190 && g >= 110 && b <= 130 && r - g >= 15 && r - g <= 120;
const isWarm = (r, g, b) => r >= 160 && b <= 110 && r - g >= 25 && r - b >= 80;

/** Mean colour of the square of half-width `half` around (cx, cy). */
function patch(img, cx, cy, half) {
  let r = 0, g = 0, b = 0, n = 0;
  const h = Math.max(1, Math.round(half));
  for (let y = Math.round(cy) - h; y <= Math.round(cy) + h; y++) {
    for (let x = Math.round(cx) - h; x <= Math.round(cx) + h; x++) {
      if (!inside(img, x, y)) continue;
      const [pr, pg, pb] = rgb(img, x, y);
      r += pr; g += pg; b += pb; n++;
    }
  }
  if (n === 0) return null;
  r /= n; g /= n; b /= n;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  return { r, g, b, max, sat: max > 0 ? (max - min) / max : 0 };
}

/** Share of pixels on row `y` between x0 and x1 that pass `test`. */
function rowCoverage(img, y, x0, x1, test) {
  y = Math.round(y);
  if (y < 0 || y >= img.height) return 0;
  let hit = 0, n = 0;
  for (let x = Math.max(0, Math.round(x0)); x <= Math.min(img.width - 1, Math.round(x1)); x++) {
    const [r, g, b] = rgb(img, x, y);
    if (test(r, g, b)) hit++;
    n++;
  }
  return n ? hit / n : 0;
}

function meanRed(img, y, x0, x1) {
  let sum = 0;
  for (let x = x0; x <= x1; x++) sum += rgb(img, x, y)[0];
  return sum / (x1 - x0 + 1);
}

/** Best coverage over the rows y0..y1. */
function bandCoverage(img, y0, y1, x0, x1, test) {
  let best = 0;
  for (let y = Math.floor(y0); y <= Math.ceil(y1); y++) best = Math.max(best, rowCoverage(img, y, x0, x1, test));
  return best;
}

// ------------------------------------------------------------------ anchor

/** Screen position of a point given in reference units. */
export function at(anchor, rx, ry) {
  return [anchor.x0 + rx * anchor.s, anchor.y0 + ry * anchor.s];
}

/** Reference units of a screen position. */
export function rel(anchor, x, y) {
  return [(x - anchor.x0) / anchor.s, (y - anchor.y0) / anchor.s];
}

/**
 * Find the Memory of Glory panel: an orange border run with a matching one
 * 100 units below it and the red despair border under that.  Returns
 * { x0, y0, s } or null.  Dimmed boards (under a popup) do not match.
 */
export function findAnchor(img) {
  const minRun = 80;
  for (let y = 0; y < img.height; y++) {
    let start = -1, last = -1;
    const flush = () => {
      if (start < 0 || last - start + 1 < minRun) return null;
      // a lit border's red, summed over its row and the brighter neighbour
      // (scaling can smear it over two), is 250+; under a popup's dimming ~90
      const beside = Math.max(y > 0 ? meanRed(img, y - 1, start, last) : 0, y + 1 < img.height ? meanRed(img, y + 1, start, last) : 0);
      if (meanRed(img, y, start, last) + beside < 200) return null;
      const s = (last - start) / REF.borderLen;
      const x0 = start, x1 = last;
      const inner = [x0 + 5 * s, x1 - 5 * s];
      const yb = y + REF.gloryBottom * s;
      if (bandCoverage(img, yb - 3 * s, yb + 3 * s, inner[0], inner[1], isOrange) < 0.7) return null;
      const yd = y + REF.despairTop * s;
      if (bandCoverage(img, yd - 3 * s, yd + 3 * s, inner[0], inner[1], isRedBorder) < 0.6) return null;
      return { x0, y0: y, s };
    };
    for (let x = 0; x < img.width; x++) {
      const [r, g, b] = rgb(img, x, y);
      if (isOrange(r, g, b)) {
        if (start < 0 || x - last > 3) {
          const found = flush();
          if (found) return found;
          start = x;
        }
        last = x;
      }
    }
    const found = flush();
    if (found) return found;
  }
  return null;
}

// ------------------------------------------------------------------- slots

/**
 * One slot (or leaf position): 'success', 'fail', 'empty' or 'none' (plain
 * panel background - past the end of the bar).  A filled slot is a bright
 * crystal: coloured for a success (gold glory, red despair), grey for a fail.
 */
export function slotAt(img, anchor, i, row) {
  const rx = REF.slotX0 + REF.slotStep * i;
  const [cx, cy] = at(anchor, rx, row);
  const [bx, by] = at(anchor, rx, row - REF.bgAbove);
  const c = patch(img, cx, cy, 3 * anchor.s);
  const bg = patch(img, bx, by, 2 * anchor.s);
  if (!c || !bg) return 'none';
  if (c.max >= Math.max(95, bg.max * 1.8)) return c.sat >= 0.3 ? 'success' : 'fail';
  if (c.max <= bg.max * 0.55) return 'empty';
  return 'none';
}

function readBar(img, anchor, row, count) {
  const out = [];
  for (let i = 0; i < count; i++) out.push(slotAt(img, anchor, i, row));
  return out;
}

function countLeaves(img, anchor, max) {
  const lit = [];
  for (let i = 0; i < max; i++) {
    const [cx, cy] = at(anchor, REF.slotX0 + REF.slotStep * i, REF.leafRow);
    const p = patch(img, cx, cy, 3 * anchor.s);
    lit.push(!!p && isLeaf(p.r, p.g, p.b));
  }
  const count = lit.filter(Boolean).length;
  const prefix = lit.indexOf(false) === -1 ? lit.length : lit.indexOf(false);
  return { count, contiguous: prefix === count };
}

/** Spirit power bar fill, 0..1 (the median of three rows). */
export function barFill(img, anchor) {
  const [xa] = at(anchor, REF.barX[0], 0);
  const [xb] = at(anchor, REF.barX[1], 0);
  const fills = REF.barRows.map((ry) => {
    const [, y] = at(anchor, 0, ry);
    let right = -1;
    for (let x = Math.round(xa); x <= Math.round(xb); x++) {
      if (!inside(img, x, y)) continue;
      const [r, g, b] = rgb(img, x, y);
      if (isBarFill(r, g, b)) right = x;
    }
    return right < 0 ? 0 : Math.min(1, (right - xa + 1) / (xb - xa + 1));
  });
  fills.sort((a, b) => a - b);
  return fills[1];
}

function hasGreenPanel(img, anchor) {
  const [xa, ya] = at(anchor, 10, REF.greenBottom - 3);
  const [xb, yb] = at(anchor, 390, REF.greenBottom + 3);
  return bandCoverage(img, ya, yb, xa, xb, isGreenBorder) >= 0.6;
}

function buttonPresent(img, anchor, name) {
  const b = REF.buttons[name];
  const [xa, ya] = at(anchor, REF.buttonSpan[0], b.top - 3);
  const [xb, yb] = at(anchor, REF.buttonSpan[1], b.top + 3);
  return bandCoverage(img, ya, yb, xa, xb, (r, g, bl) => Math.max(r, g, bl) >= 140) >= 0.6;
}

// ----------------------------------------------------------------- buttons

/**
 * Column and row extent of the pixels passing `test` in a box: the runs of
 * columns holding at least `minCol` of them, merged across gaps < `gap`.
 */
function columnRuns(img, x0, x1, y0, y1, test, minCol, gap) {
  const runs = [];
  let cur = null;
  for (let x = Math.max(0, Math.round(x0)); x <= Math.min(img.width - 1, Math.round(x1)); x++) {
    let n = 0;
    for (let y = Math.max(0, Math.round(y0)); y <= Math.min(img.height - 1, Math.round(y1)); y++) {
      const [r, g, b] = rgb(img, x, y);
      if (test(r, g, b)) n++;
    }
    if (n >= minCol) {
      if (cur && x - cur.x1 <= gap) cur.x1 = x;
      else runs.push((cur = { x0: x, x1: x }));
    }
  }
  return runs;
}

function rowExtent(img, x0, x1, y0, y1, test, minRow) {
  let top = -1, bottom = -1;
  for (let y = Math.max(0, Math.round(y0)); y <= Math.min(img.height - 1, Math.round(y1)); y++) {
    let n = 0;
    for (let x = Math.round(x0); x <= Math.round(x1); x++) {
      const [r, g, b] = rgb(img, x, y);
      if (test(r, g, b)) n++;
    }
    if (n >= minRow) {
      if (top < 0) top = y;
      bottom = y;
    }
  }
  return top < 0 ? null : { top, bottom };
}

/** The gold button under the panels (Complete Inheritance, or Inheritance). */
export function findGoldButton(img, anchor) {
  const s = anchor.s;
  const [xa, ya] = at(anchor, REF.goldBox.x[0], REF.goldBox.y[0]);
  const [xb, yb] = at(anchor, REF.goldBox.x[1], REF.goldBox.y[1]);
  const runs = columnRuns(img, xa, xb, ya, yb, isGold, Math.max(3, 4 * s), 6 * s)
    .filter((r) => r.x1 - r.x0 >= 60 * s);
  if (runs.length !== 1) return null;
  const run = runs[0];
  const rows = rowExtent(img, run.x0, run.x1, ya, yb, isGold, 0.25 * (run.x1 - run.x0));
  if (!rows) return null;
  const w = run.x1 - run.x0, h = rows.bottom - rows.top;
  if (w > 230 * s || h < 15 * s || h > 100 * s) return null;
  return { x: (run.x0 + run.x1) / 2, y: (rows.top + rows.bottom) / 2, w, h };
}

/**
 * The Compare Inheritance Results popup's two buttons, found over the
 * (dimmed) board whose anchor is known from the screen before: two warm
 * buttons side by side, Keep Current Effect on the left of the panel's
 * centre and Replace with New Effect on the right.
 */
/**
 * A popup's two buttons side by side over the (dimmed) board whose anchor
 * is known from the screen before it: two warm buttons near `spec.left`
 * and `spec.right` (reference units), each of the colour its range of
 * green/red allows.  Returns [left, right] or null.
 */
function findPopupButtons(img, anchor, spec) {
  const s = anchor.s;
  const [xa, ya] = at(anchor, REF.compareBox.x[0], REF.compareBox.y[0]);
  const [xb, yb] = at(anchor, REF.compareBox.x[1], REF.compareBox.y[1]);
  const minCol = Math.max(4, 0.12 * (yb - ya));
  const runs = columnRuns(img, xa, xb, ya, yb, isWarm, minCol, 6 * s).filter((r) => r.x1 - r.x0 >= 60 * s);
  if (runs.length !== 2) return null;
  const boxes = runs.map((run) => {
    const rows = rowExtent(img, run.x0, run.x1, ya, yb, isWarm, 0.3 * (run.x1 - run.x0));
    if (!rows) return null;
    return { x: (run.x0 + run.x1) / 2, y: (rows.top + rows.bottom) / 2, w: run.x1 - run.x0, h: rows.bottom - rows.top };
  });
  const [left, right] = boxes;
  if (!left || !right) return null;
  if (Math.abs(left.y - right.y) > 15 * s) return null;
  for (const b of boxes) if (b.h < 15 * s || b.h > 90 * s || b.w > 200 * s) return null;
  const ratio = left.w / right.w;
  if (ratio < 0.6 || ratio > 1.6) return null;
  for (const [b, [rx, ry]] of [[left, spec.left], [right, spec.right]]) {
    const [ex, ey] = at(anchor, rx, ry);
    if (Math.abs(b.x - ex) > REF.popupTol * s || Math.abs(b.y - ey) > REF.popupTol * s) return null;
  }
  const lh = warmHue(img, left), rh = warmHue(img, right);
  if (lh < spec.leftHue[0] || lh > spec.leftHue[1] || rh < spec.rightHue[0] || rh > spec.rightHue[1]) return null;
  return [left, right];
}

/**
 * Compare Inheritance Results: Keep Current Effect (orange, green/red
 * ~0.45) on the left, Replace with New Effect (gold, ~0.70) on the right.
 */
export function findCompareButtons(img, anchor) {
  const found = findPopupButtons(img, anchor, {
    left: REF.compare.keep, right: REF.compare.replace, leftHue: [0.3, 0.55], rightHue: [0.6, 0.85],
  });
  return found && { keep: found[0], replace: found[1] };
}

/**
 * The abandon confirmation: Confirm (gold, ~0.63) on the left, Cancel
 * (red, ~0.01) on the right - where and in what colours Compare has none.
 */
export function findAbandonButtons(img, anchor) {
  const found = findPopupButtons(img, anchor, {
    left: REF.abandon.confirm, right: REF.abandon.cancel, leftHue: [0.55, 0.85], rightHue: [0, 0.25],
  });
  return found && { confirm: found[0], cancel: found[1] };
}

const isTan = (r, g, b) => r >= 120 && g >= 90 && b >= 50 && r - b >= 40 && r - b <= 130 && r >= g && g >= b;

/** The Abandon Inheritance button under the panels, as { x, y }, or null. */
export function findAbandonButton(img, anchor) {
  const box = REF.abandon.button;
  const [xa, ya] = at(anchor, box.x[0], box.y[0]);
  const [xb, yb] = at(anchor, box.x[1], box.y[1]);
  let n = 0, total = 0;
  for (let y = Math.round(ya); y <= Math.round(yb); y++) {
    for (let x = Math.round(xa); x <= Math.round(xb); x++) {
      if (!inside(img, x, y)) continue;
      total++;
      if (isTan(...rgb(img, x, y))) n++;
    }
  }
  if (!total || n < 0.1 * total) return null;
  return { x: (xa + xb) / 2, y: (ya + yb) / 2 };
}

/** Mean green/red of the warm pixels in a found button. */
function warmHue(img, b) {
  let sum = 0, n = 0;
  for (let y = Math.round(b.y - b.h / 2); y <= Math.round(b.y + b.h / 2); y++) {
    for (let x = Math.round(b.x - b.w / 2); x <= Math.round(b.x + b.w / 2); x++) {
      if (!inside(img, x, y)) continue;
      const [r, g, bl] = rgb(img, x, y);
      if (isWarm(r, g, bl)) {
        sum += g / r;
        n++;
      }
    }
  }
  return n ? sum / n : -1;
}

/**
 * Read both Compare cards: { current, fresh }, each { glory, despair } or
 * null.  A card counts only if its numbers agree with each other - the
 * Final Boost Value it shows must be 5 x glory - 2 x despair - so a misread
 * digit makes the card unreadable rather than wrong.  `seen` has every raw
 * reading, for the log.
 */
export function readCards(img, buttons, withLines = false) {
  const C = REF.cards;
  const u = (buttons.replace.x - buttons.keep.x) / C.spacing;
  const midY = (buttons.keep.y + buttons.replace.y) / 2;
  const line = (cx, row) => inkBox(img, cx + C.x[0] * u, cx + C.x[1] * u, midY + (row - C.half) * u, midY + (row + C.half) * u);
  const card = (cx) => {
    const glory = trailingNumber(line(cx, C.glory));
    const despair = trailingNumber(line(cx, C.despair));
    const boost = percentNumber(line(cx, C.boost));
    const ok = glory !== null && despair !== null && boost !== null && amplification({ glory, despair }) === boost;
    return { glory, despair, boost, ok };
  };
  const current = card(buttons.keep.x), fresh = card(buttons.replace.x);
  const pick = (c) => (c.ok ? { glory: c.glory, despair: c.despair } : null);
  const out = { current: pick(current), fresh: pick(fresh), seen: { current, fresh } };
  if (withLines) {
    // the glyphs themselves, for test/make-glyphs.mjs
    const glyphsOf = (cx) => ({
      glory: numberGlyphs(line(cx, C.glory), 'trailing', 2),
      despair: numberGlyphs(line(cx, C.despair), 'trailing', 2),
      boost: numberGlyphs(line(cx, C.boost), 'percent', 2),
    });
    out.lines = { current: glyphsOf(buttons.keep.x), fresh: glyphsOf(buttons.replace.x) };
  }
  return out;
}

/**
 * The Compare popup with no screen before it to go by: two warm buttons
 * side by side on a row, checked by `findCompareButtons` at the anchor
 * they imply.
 */
export function findCompareAnywhere(img) {
  const [kx, ky] = REF.compare.keep;
  for (let y = 0; y < img.height; y += 2) {
    const runs = [];
    let start = -1, last = -1;
    for (let x = 0; x <= img.width; x++) {
      const warm = x < img.width && isWarm(...rgb(img, x, y));
      if (warm) {
        if (start < 0 || x - last > 3) {
          if (start >= 0) runs.push([start, last]);
          start = x;
        }
        last = x;
      }
    }
    if (start >= 0) runs.push([start, last]);
    const long = runs.filter(([a, b]) => b - a >= 50);
    for (let i = 0; i + 1 < long.length; i++) {
      const left = (long[i][0] + long[i][1]) / 2, right = (long[i + 1][0] + long[i + 1][1]) / 2;
      const s = (right - left) / REF.cards.spacing;
      if (s < 0.5 || s > 3) continue;
      const guess = { x0: left - kx * s, y0: y - ky * s, s };
      const first = findCompareButtons(img, guess);
      if (!first) continue;
      // centre the anchor on the buttons found, and check again
      const anchor = { x0: first.keep.x - kx * s, y0: first.keep.y - ky * s, s };
      const buttons = findCompareButtons(img, anchor);
      if (buttons) return { anchor, buttons };
    }
  }
  return null;
}

/** Whether a click at screen (x, y) would land on Abandon Inheritance. */
export function isForbidden(anchor, x, y) {
  const [rx, ry] = rel(anchor, x, y);
  const f = REF.forbidden;
  return rx >= f.x[0] && rx <= f.x[1] && ry >= f.y[0] && ry <= f.y[1];
}

// -------------------------------------------------------------------- rate

/**
 * The "NN%" text on one Attempt button: the grey levels of its box as ink
 * (0 button, 1 text), at the size it was captured; `readRate` reads it.
 */
export function rateFeature(img, anchor, button = 'train') {
  const box = REF.rateBoxes[button];
  const [xa, ya] = at(anchor, box.x[0], box.y[0]);
  const [xb, yb] = at(anchor, box.x[1], box.y[1]);
  const X0 = Math.round(xa), Y0 = Math.round(ya);
  const w = Math.round(xb) - X0 + 1, h = Math.round(yb) - Y0 + 1;
  const ink = new Array(w * h).fill(0);
  let total = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!inside(img, X0 + x, Y0 + y)) continue;
      const [r, g, b] = rgb(img, X0 + x, Y0 + y);
      const v = Math.min(1, Math.max(0, ((r + g + b) / 3 - 60) / 120));
      ink[y * w + x] = Math.round(v * 100) / 100;
      total += v;
    }
  }
  if (total < 0.03 * w * h) return null; // no text: not the rate
  return { w, h, s: anchor.s, ink };
}

/**
 * Read "80%", "65%", "50%", "35%" or "20%" off a rate crop by the shape of
 * its digits, returning the ladder tier (0-4) or null when unsure.
 *
 * The five rates differ in a few features that survive blur and scaling:
 *  - the second digit: "0" has a hole, "5" none - {80, 50, 20} vs {65, 35}
 *  - the first digit: 8 and 6 have holes, 5, 3 and 2 none
 *  - 5 against 2: 5's upper stroke is on the left, 2's on the right
 * The text is cut at the faintest column between glyphs (blur can make
 * neighbours touch and close a false hole between them), and read at three
 * ink thresholds that must agree.  Measured on test/images: right from
 * 0.8x to 1.5x; smaller text reads as unsure (null), never as a wrong rate.
 */
export function readRate(feature) {
  if (!feature) return null;
  let max = 0;
  for (const v of feature.ink) max = Math.max(max, v);
  if (max < 0.3) return null;
  // higher thresholds thin the strokes until counters leak open; these hold
  const reads = [0.3, 0.4, 0.5].map((k) => readRateAt(feature, k * max));
  const sure = reads.filter((r) => r !== null);
  return sure.length >= 2 && sure.every((r) => r === sure[0]) ? sure[0] : null;
}

function readRateAt({ w, h, ink }, t) {
  const on0 = (x, y) => ink[y * w + x] >= t;
  // the text's rows: at least 2 lit pixels, and not a button border line
  const rowOk = [];
  for (let y = 0; y < h; y++) {
    let n = 0;
    for (let x = 0; x < w; x++) if (on0(x, y)) n++;
    rowOk.push(n >= 2 && n <= 0.7 * w);
  }
  // the longest run of text rows (gaps of one row allowed)
  let best = null, start = -1, gap = 0;
  for (let y = 0; y <= h; y++) {
    if (y < h && rowOk[y]) {
      if (start < 0) start = y;
      gap = 0;
    } else if (start >= 0 && (y === h || ++gap > 1)) {
      const end = y - (y === h ? 1 : gap);
      if (!best || end - start > best[1] - best[0]) best = [start, end];
      start = -1;
      gap = 0;
    }
  }
  if (!best) return null;
  const [top, bottom] = best;
  let left = w, right = -1;
  for (let x = 0; x < w; x++) {
    let n = 0;
    for (let y = top; y <= bottom; y++) if (on0(x, y)) n++;
    if (n >= 2) {
      left = Math.min(left, x);
      right = Math.max(right, x);
    }
  }
  const tw = right - left + 1, th = bottom - top + 1;
  // under ~9 px tall (a game shown below ~0.8x) the counters blur shut
  if (th < 9 || tw > 6 * th || th > tw) return null;

  // cut between the glyphs at the faintest column of each expected gap
  const faintest = (lo, hi) => {
    let at = -1, least = Infinity;
    for (let x = Math.round(lo * tw); x <= Math.round(hi * tw); x++) {
      let sum = 0;
      for (let y = top; y <= bottom; y++) sum += ink[y * w + left + x];
      if (sum < least) [least, at] = [sum, x];
    }
    return at;
  };
  const cut1 = faintest(0.18, 0.36), cut2 = faintest(0.5, 0.68);

  // background reachable from outside the text, 4-connected, the cuts open
  const W = tw + 2, H = th + 2;
  const on = (x, y) => x >= 1 && y >= 1 && x <= tw && y <= th && x - 1 !== cut1 && x - 1 !== cut2
    && on0(left + x - 1, top + y - 1);
  const seen = new Uint8Array(W * H);
  const flood = (sx, sy) => {
    const stack = [[sx, sy]];
    let count = 0, sumX = 0;
    seen[sy * W + sx] = 1;
    while (stack.length) {
      const [x, y] = stack.pop();
      count++;
      sumX += x - 1;
      for (const [nx, ny] of [[x + 1, y], [x - 1, y], [x, y + 1], [x, y - 1]]) {
        if (nx < 0 || ny < 0 || nx >= W || ny >= H || seen[ny * W + nx] || on(nx, ny)) continue;
        seen[ny * W + nx] = 1;
        stack.push([nx, ny]);
      }
    }
    return { count, cx: sumX / count };
  };
  flood(0, 0);
  const holes = [0, 0, 0]; // first digit, second digit, % sign
  for (let y = 1; y <= th; y++) {
    for (let x = 1; x <= tw; x++) {
      if (seen[y * W + x] || on(x, y)) continue;
      const { count, cx } = flood(x, y);
      if (count < 2) continue; // a speck, not a counter
      holes[cx < cut1 ? 0 : cx < cut2 ? 1 : 2]++;
    }
  }

  const [first, second] = holes;
  if (second > 0) {
    if (first > 0) return 0; // 80%
    // 50% or 20%: is the first digit's upper stroke on its left or right?
    const part = 0.4 * cut1;
    let l = 0, r = 0;
    for (let y = top + Math.round(0.2 * th); y <= top + Math.round(0.45 * th); y++) {
      for (let x = 0; x < cut1; x++) {
        const v = ink[y * w + left + x];
        if (x < part) l += v;
        else if (x >= cut1 - part) r += v;
      }
    }
    if (l > 1.3 * r) return 2; // 50%
    if (r > 1.3 * l) return 4; // 20%
    return null;
  }
  return first > 0 ? 1 : 3; // 65% : 35%
}

/**
 * The rate shown on the board's buttons, if every button that can be read
 * agrees: { tier, read } with the per-button readings, or null.
 */
export function boardRate(screen) {
  const read = {};
  for (const [button, feature] of Object.entries(screen.rates ?? {})) read[button] = readRate(feature);
  const tiers = new Set(Object.values(read).filter((t) => t !== null));
  return tiers.size === 1 ? { tier: [...tiers][0], read } : null;
}

// ----------------------------------------------------------------- screens

function buttonCentres(anchor) {
  const out = {};
  for (const [name, b] of Object.entries(REF.buttons)) {
    const [x, y] = at(anchor, b.x, b.y);
    out[name] = { x, y };
  }
  return out;
}

/** The two bars of a finished memory: slots counted until the background. */
function readMemory(img, anchor) {
  const tally = (row) => {
    const bar = readBar(img, anchor, row, MAX_SLOTS);
    const end = bar.indexOf('none');
    const used = end < 0 ? bar : bar.slice(0, end);
    return { slots: used.length, filled: used.filter((v) => v !== 'empty').length, success: used.filter((v) => v === 'success').length, bar: used };
  };
  return { glory: tally(REF.gloryRow), despair: tally(REF.despairRow) };
}

/**
 * Work out which screen is showing.
 *
 *  board    - an attempt in progress: the three panels and at least one
 *             Attempt button (Mental Training's goes at 0 mental strength),
 *             or none and only Abandon Inheritance (a wiped attempt)
 *  complete - every slot filled, Complete Inheritance below the panels
 *  legacy   - Hero's Legacy (the start screen): the current memory and the
 *             Inheritance button
 *  compare  - Compare Inheritance Results, with both cards read: found at
 *             `lastAnchor` (the screen before it - the popup hides the
 *             panels), or, if `anywhere`, wherever its cards both check out
 *  main     - the main page (see mainpage.js): levels or counts view
 *  abandon  - the abandon confirmation, looked for only when `popup` is
 *             'abandon', at `lastAnchor` (after clicking Abandon Inheritance)
 *  unknown  - anything else, including the abandon confirmation
 */
export function classify(img, lastAnchor = null, anywhere = false, popup = 'compare') {
  const anchor = findAnchor(img);
  if (!anchor) {
    if (lastAnchor && popup === 'abandon') {
      const buttons = findAbandonButtons(img, lastAnchor);
      if (buttons) return { kind: 'abandon', anchor: lastAnchor, buttons };
    } else if (lastAnchor) {
      const buttons = findCompareButtons(img, lastAnchor);
      if (buttons) return { kind: 'compare', anchor: lastAnchor, buttons, cards: readCards(img, buttons) };
    }
    if (anywhere) {
      // with no screen before it, only a popup whose two cards both read
      // and check out is taken for Compare
      const found = findCompareAnywhere(img);
      const cards = found && readCards(img, found.buttons);
      if (cards?.current && cards.fresh) return { kind: 'compare', anchor: found.anchor, buttons: found.buttons, cards };
    }
    const main = readMain(img);
    if (main) return { kind: 'main', anchor: null, ...main };
    return { kind: 'unknown', anchor: null };
  }
  const green = hasGreenPanel(img, anchor);
  const present = Object.fromEntries(Object.keys(REF.buttons).map((name) => [name, buttonPresent(img, anchor, name)]));
  const shown = Object.keys(present).filter((name) => present[name]);
  // a board, or one with no Attempt buttons left (out of spirit power and
  // mental strength), where only Abandon Inheritance is showing
  const abandon = green ? findAbandonButton(img, anchor) : null;
  if (green && (shown.length > 0 || (abandon && !findGoldButton(img, anchor)))) {
    return {
      kind: 'board',
      anchor,
      abandon,
      glory: readBar(img, anchor, REF.gloryRow, MAX_SLOTS),
      despair: readBar(img, anchor, REF.despairRow, MAX_SLOTS),
      leaves: countLeaves(img, anchor, MAX_SLOTS),
      fill: barFill(img, anchor),
      rates: Object.fromEntries(shown.map((name) => [name, rateFeature(img, anchor, name)])),
      present,
      buttons: buttonCentres(anchor),
    };
  }
  if (shown.length === 0) {
    const gold = findGoldButton(img, anchor);
    if (gold && green) return { kind: 'complete', anchor, memory: readMemory(img, anchor), button: gold };
    if (gold) {
      // Hero's Legacy: its stock ("127/10" on the button) and the way back
      const S = REF.stock, s = anchor.s;
      const stock = stockNumber(inkBox(img, gold.x + S.x[0] * s, gold.x + S.x[1] * s, gold.y + S.y[0] * s, gold.y + S.y[1] * s), s);
      const banner = findBanner(img);
      return { kind: 'legacy', anchor, memory: readMemory(img, anchor), button: gold, stock, back: banner && findBackButton(img, banner) };
    }
  }
  return { kind: 'unknown', anchor };
}

/**
 * The solver's state from a board screen, for a board of `slots` slots and
 * `maxSpirit` spirit power, or { error } when the screen does not fit it.
 */
export function boardState(screen, slots, maxSpirit) {
  const bar = (name) => {
    const cells = screen[name];
    const head = cells.slice(0, slots);
    if (head.includes('none')) return { error: `${name} bar shows fewer than ${slots} slots - check the inheritor level` };
    if (slots < MAX_SLOTS && cells[slots] !== 'none') {
      return { error: `${name} bar shows more than ${slots} slots - check the inheritor level` };
    }
    const filled = head.filter((v) => v !== 'empty').length;
    if (head.slice(0, filled).includes('empty')) return { error: `${name} bar has a gap: ${head.join(' ')}` };
    return { filled, success: head.filter((v) => v === 'success').length };
  };
  const g = bar('glory');
  if (g.error) return g;
  const d = bar('despair');
  if (d.error) return d;
  if (!screen.leaves.contiguous) return { error: 'mental strength leaves are not in a row' };
  const ms = screen.leaves.count;
  if (ms > slots) return { error: `${ms} mental strength leaves on a ${slots}-slot board` };
  if ((ms > 0) !== screen.present.train) {
    return { error: `${ms} mental strength leaves but the Mental Training button is ${screen.present.train ? 'showing' : 'gone'}` };
  }
  return { gf: g.filled, gs: g.success, df: d.filled, ds: d.success, ms, sp: Math.round(screen.fill * maxSpirit) };
}
