// The Hero's Legacy main page: twelve relic tiles in four rows of three,
// shown two ways - the levels view (inheritor level and the pity bar) and
// the counts view (each relic's glory, despair and stock, and the totals).
// The circular-arrows icon top-left switches between them.
//
// Everything is placed from the title banner ("Hero's Legacy", "Memory
// Inheritance"): two gold border rows, the same on every screen.  Positions
// are in units of a 1x capture, from the left end of the banner's bottom
// border (crisp, at 0.917x: 667, 134 in test/images/main-*.png).

import { inkBox, trailingNumber, wholeNumber } from './digits.js';

export const MAIN = {
  // the banner as findBanner sees it (its soft ends included): 183.2 long,
  // its borders 43 apart, its centre at (87.2, -21.3) in the units below
  bannerLen: 183.2,
  bannerGap: 43,
  bannerCentre: [87.2, -21.3],
  // the red square button bottom-left (close, or Hero's Legacy's return):
  // its centre this far from the banner's - it fixes the scale, being far
  redButton: [-178.8, 707.2],
  tiles: { x: [-22.9, 87.2, 198.5], y: [124.3, 243.2, 362.0, 480.9] }, // relic icon centres
  // per tile, from its icon centre: the counts view's lines
  glory: { x: [-5.5, 19.6], y: [-16.4, -1.1] }, // "x 7"
  despair: { x: [-5.5, 19.6], y: [0, 15.3] }, // "x 1"
  badge: { x: [-26, 26], y: [35, 44.7] }, // the count badge: a black box
  stock: { x: [-8, 27], y: [34.5, 46] }, // its number, right of the relic icon, below the gold line
  totals: { glory: { x: [84, 123.2], y: [31.6, 53.4] }, despair: { x: [150.5, 188.7], y: [31.6, 53.4] } },
  level: { x: [9.8, 49.1], y: [566, 587.8] }, // "Lv.18"
  pity: { x: [-49.1, 225.7], y: [607, 619] }, // the bar's track
  toggle: [-82.9, 42.5], // the circular-arrows icon
  back: { at: [-91.6, 688.1], half: 15 }, // Hero's Legacy's red return button
  // the gold Level Up button under the panel, there only once a level-up is
  // earned (quest met or pity full); dark brown otherwise
  levelUp: { x: [50, 125], y: [660, 685] },
};

const rgb = (img, x, y) => {
  const i = (Math.round(y) * img.width + Math.round(x)) * 4;
  return [img.data[i], img.data[i + 1], img.data[i + 2]];
};
const inside = (img, x, y) => x >= 0 && y >= 0 && x < img.width && y < img.height;
// loose, for a 1 px border smeared over two rows by scaling; a dimmed
// (popup-covered) banner is told apart by the border's total brightness
const isBannerGold = (r, g, b) => r >= 100 && g >= 0.5 * r && g <= 0.92 * r && b <= 0.55 * r;

function meanRed(img, y, x0, x1) {
  if (y < 0 || y >= img.height) return 0;
  let sum = 0;
  for (let x = x0; x <= x1; x++) sum += img.data[(y * img.width + x) * 4];
  return sum / (x1 - x0 + 1);
}

/** A border's red over its row and the brighter row beside it: ~250 lit, ~100 dimmed. */
const lit = (img, y, [x0, x1]) => meanRed(img, y, x0, x1) + Math.max(meanRed(img, y - 1, x0, x1), meanRed(img, y + 1, x0, x1));

/** Gold runs at least `min` long on row y: [[start, end], ...]. */
function goldRuns(img, y, min) {
  const runs = [];
  let start = -1, last = -1;
  for (let x = 0; x <= img.width; x++) {
    if (x < img.width && isBannerGold(...rgb(img, x, y))) {
      if (start < 0 || x - last > 3) {
        if (start >= 0 && last - start >= min) runs.push([start, last]);
        start = x;
      }
      last = x;
    }
  }
  if (start >= 0 && last - start >= min) runs.push([start, last]);
  return runs;
}

/**
 * The title banner: a gold run with a matching one 43 units above it.
 * Returns { x0, y0, s } (the bottom border's left end, its row, the scale)
 * or null - also under a popup, whose dimming takes the gold away.
 */
export function findBanner(img) {
  const rows = [];
  for (let y = 0; y < img.height; y++) {
    for (const run of goldRuns(img, y, 90)) rows.push({ y, run });
  }
  for (const bottom of rows) {
    const s = (bottom.run[1] - bottom.run[0]) / MAIN.bannerLen;
    const gap = MAIN.bannerGap * s;
    const top = rows.find((t) => Math.abs(bottom.y - t.y - gap) <= 3 * s
      && Math.abs(t.run[0] - bottom.run[0]) <= 8 * s && Math.abs(t.run[1] - bottom.run[1]) <= 8 * s);
    if (top && lit(img, bottom.y, bottom.run) >= 200 && lit(img, top.y, top.run) >= 200) {
      // placed by its centre, which the ends' softness does not move, and
      // scaled by the red button far below it (the banner's length alone is
      // about 1% out, ~6 px by the bottom tiles)
      const cx = (bottom.run[0] + bottom.run[1] + top.run[0] + top.run[1]) / 4, cy = (top.y + bottom.y) / 2;
      const red = redButton(img, cx + MAIN.redButton[0] * s, cy + MAIN.redButton[1] * s, s);
      if (!red) continue;
      const fine = (red.y - cy) / MAIN.redButton[1];
      if (Math.abs(fine / s - 1) > 0.08) continue;
      return { x0: cx - MAIN.bannerCentre[0] * fine, y0: cy - MAIN.bannerCentre[1] * fine, s: fine };
    }
  }
  return null;
}

/** The red square button near (x, y): the centre of its red's bounding box. */
function redButton(img, x, y, s) {
  const reach = 45 * s;
  let x0 = Infinity, x1 = -1, y0 = Infinity, y1 = -1, n = 0;
  for (let yy = Math.round(y - reach); yy <= Math.round(y + reach); yy++) {
    for (let xx = Math.round(x - reach); xx <= Math.round(x + reach); xx++) {
      if (!inside(img, xx, yy)) continue;
      const [r, g, b] = rgb(img, xx, yy);
      if (r >= 180 && g <= 90 && b <= 90) {
        n++;
        x0 = Math.min(x0, xx); x1 = Math.max(x1, xx); y0 = Math.min(y0, yy); y1 = Math.max(y1, yy);
      }
    }
  }
  const side = 36 * s;
  if (n < 0.3 * side * side || x1 - x0 > 1.4 * side || y1 - y0 > 1.4 * side || y1 - y0 < 0.6 * side) return null;
  return { x: (x0 + x1) / 2, y: (y0 + y1) / 2 };
}

const at = (b, rx, ry) => [b.x0 + rx * b.s, b.y0 + ry * b.s];

function box(img, b, cx, cy, spec) {
  const [x0, y0] = at(b, cx + spec.x[0], cy + spec.y[0]);
  const [x1, y1] = at(b, cx + spec.x[1], cy + spec.y[1]);
  return inkBox(img, x0, x1, y0, y1);
}

/** The twelve tiles' centres, in the order of the solver's relics. */
export function tileCentres(b) {
  const out = [];
  for (const ty of MAIN.tiles.y) for (const tx of MAIN.tiles.x) out.push({ x: at(b, tx, ty)[0], y: at(b, tx, ty)[1], rx: tx, ry: ty });
  return out;
}

/** Whether the counts view is up: a black count badge under (nearly) every tile. */
function countsShown(img, b) {
  let badges = 0;
  for (const t of tileCentres(b)) {
    const [x0, y0] = at(b, t.rx + MAIN.badge.x[0], t.ry + MAIN.badge.y[0]);
    const [x1, y1] = at(b, t.rx + MAIN.badge.x[1], t.ry + MAIN.badge.y[1]);
    let dark = 0, c = 0;
    for (let y = Math.round(y0); y <= Math.round(y1); y++) {
      for (let x = Math.round(x0); x <= Math.round(x1); x++) {
        if (!inside(img, x, y)) continue;
        c++;
        if (Math.max(...rgb(img, x, y)) <= 35) dark++;
      }
    }
    if (c && dark >= 0.5 * c) badges++;
  }
  return badges >= 10;
}

/** The pity bar's fill, 0..1: the right end of its gold. */
function pityFill(img, b) {
  const [xa, ya] = at(b, MAIN.pity.x[0], MAIN.pity.y[0]);
  const [xb, yb] = at(b, MAIN.pity.x[1], MAIN.pity.y[1]);
  let right = -1, rows = 0;
  for (let y = Math.round(ya); y <= Math.round(yb); y++) {
    for (let x = Math.round(xa); x <= Math.round(xb); x++) {
      if (!inside(img, x, y)) continue;
      const [r, g, bl] = rgb(img, x, y);
      if (r >= 200 && g >= 140 && g <= 230 && bl <= 120) right = Math.max(right, x);
    }
    rows++;
  }
  if (!rows) return null;
  return right < 0 ? 0 : Math.min(1, (right - xa + 1) / (xb - xa + 1));
}

/** Whether the gold Level Up button shows: most of its middle bright gold. */
function levelUpShown(img, b) {
  let gold = 0, c = 0;
  for (let ry = MAIN.levelUp.y[0]; ry <= MAIN.levelUp.y[1]; ry++) {
    for (let rx = MAIN.levelUp.x[0]; rx <= MAIN.levelUp.x[1]; rx++) {
      const [x, y] = at(b, rx, ry);
      if (!inside(img, x, y)) continue;
      c++;
      const [r, g, bl] = rgb(img, x, y);
      if (r >= 190 && g >= 130 && bl <= 130) gold++;
    }
  }
  return c > 0 && gold >= 0.4 * c;
}

/** Hero's Legacy's red return button, as { x, y }, or null. */
export function findBackButton(img, b) {
  const [cx, cy] = at(b, ...MAIN.back.at);
  const h = MAIN.back.half * b.s;
  let n = 0, c = 0;
  for (let y = Math.round(cy - h); y <= Math.round(cy + h); y++) {
    for (let x = Math.round(cx - h); x <= Math.round(cx + h); x++) {
      if (!inside(img, x, y)) continue;
      c++;
      const [r, g, bl] = rgb(img, x, y);
      if (r >= 180 && g <= 90 && bl <= 90) n++;
    }
  }
  return c && n >= 0.3 * c ? { x: cx, y: cy } : null;
}

/**
 * The main page, if it is showing: { view: 'levels', level, pity } or
 * { view: 'counts', relics, totals, ok }, both with the banner, the tiles'
 * centres, the view toggle and `levelUp` (the Level Up button showing).  `relics` is every tile's { glory, despair,
 * stock } (null where a number would not read); `ok` says the glory and
 * despair all read and add up to the totals line - the check on all of them.
 */
export function readMain(img, banner = findBanner(img)) {
  const b = banner;
  if (!b) return null;
  const tiles = tileCentres(b);
  const [tx, ty] = at(b, ...MAIN.toggle);
  const common = { banner: b, tiles, toggle: { x: tx, y: ty }, levelUp: levelUpShown(img, b) };
  if (countsShown(img, b)) {
    // stock off the badge: nothing on the page checks it, so null unless
    // every digit reads clearly (and the advisor checks it on Hero's Legacy)
    const relics = tiles.map((t) => ({
      glory: trailingNumber(box(img, b, t.rx, t.ry, MAIN.glory)),
      despair: trailingNumber(box(img, b, t.rx, t.ry, MAIN.despair)),
      stock: wholeNumber(box(img, b, t.rx, t.ry, MAIN.stock), 4),
    }));
    const totals = {
      glory: trailingNumber(box(img, b, 0, 0, MAIN.totals.glory), 3),
      despair: trailingNumber(box(img, b, 0, 0, MAIN.totals.despair), 3),
    };
    const sum = (k) => relics.reduce((a, r) => (a === null || r[k] === null ? null : a + r[k]), 0);
    const ok = relics.every((r) => r.glory !== null && r.despair !== null)
      && totals.glory !== null && sum('glory') === totals.glory && sum('despair') === totals.despair;
    return { view: 'counts', relics, totals, ok, ...common };
  }
  const level = trailingNumber(box(img, b, 0, 0, MAIN.level));
  const pity = pityFill(img, b);
  if (level === null || level < 1 || level > 30 || pity === null) return null;
  return { view: 'levels', level, pity, ...common };
}
