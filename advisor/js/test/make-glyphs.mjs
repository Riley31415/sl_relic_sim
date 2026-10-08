// node test/make-glyphs.mjs - rebuild glyphdata.js, the labelled
// digit examples readDigit matches against.
//
// Every number shown in the fixtures, read where the extension reads it,
// at the size captured and rescaled 0.8x-1.3x (the browser's resampling is
// what changes a glyph most).  A line whose glyph count does not match its
// label is skipped, so a bad cut never teaches a wrong digit.
import { writeFileSync } from 'fs';
import { fileURLToPath } from 'url';

import { readPng, compose } from './png.mjs';
import { inkBox, glyphs, gridOf, numberGlyphs } from '../digits.js';
import { findBanner, tileCentres, MAIN } from '../mainpage.js';
import { classify, readCards, findCompareAnywhere, REF } from '../vision.js';

const fixture = (name) => readPng(fileURLToPath(new URL(`./images/${name}.png`, import.meta.url)));
const crop = (img, x0, y0, w, h) => {
  const d = new Uint8ClampedArray(w * h * 4);
  for (let y = 0; y < h; y++) d.set(img.data.subarray(((y0 + y) * img.width + x0) * 4, ((y0 + y) * img.width + x0 + w) * 4), y * w * 4);
  return { width: w, height: h, data: d };
};

// [fixture, crop of the phone (or null), scale captured, reader, labels]
// a counts view: each relic's glory and despair, its badge, the totals
const MAIN_COUNTS = {
  counts: ['71', '71', '83', '82', '82', '71', '82', '83', '71', '91', '72', '83'],
  badges: ['207', '115', '135', '92', '109', '28', '156', '105', '36', '180', '4', '27'],
  totals: ['92', '22'],
};
const LEVELUP_COUNTS = {
  counts: ['71', '71', '82', '82', '82', '71', '82', '81', '71', '91', '72', '83'],
  badges: ['137', '115', '125', '92', '109', '28', '136', '5', '36', '160', '4', '7'],
  totals: ['92', '19'],
};
const HERO = ['giant', 'demon', 'oath', 'tree', 'ring', 'star', 'seal', 'veil', 'spark', 'mermaid', 'sky', 'crown'];
const SOURCES = [
  ['main-counts', [535, 0, 465, 847], 0.917, 'main', MAIN_COUNTS],
  ['main-levels', [535, 0, 465, 847], 0.917, 'level', '18'],
  ['main-levelup-counts', [535, 0, 465, 847], 0.917, 'main', LEVELUP_COUNTS],
  ['main-levelup-levels', [535, 0, 465, 847], 0.917, 'level', '18'],
  ['compare', null, 1, 'cards', { current: [7, 1, 33, 931], fresh: [6, 4, 22, 854] }],
  ['compare-full', [535, 0, 465, 847], 0.92, 'cards', { current: [8, 3, 34, 402], fresh: [8, 4, 32, 396] }],
  ['legacy-crown', [535, 0, 465, 847], 0.917, 'stock', '127'],
  ['legacy-full', [710, 0, 501, 915], 1, 'stock', '195'],
  ['legacy', null, 1, 'stock', '195'],
  ['legacy-giant', [535, 0, 465, 847], 0.917, 'stock', '207'],
  ['legacy-oath', [535, 0, 465, 847], 0.917, 'stock', '135'],
  ['legacy-star', [535, 0, 465, 847], 0.917, 'stock', '28'],
  ['legacy-seal', [535, 0, 465, 847], 0.917, 'stock', '156'],
  ['legacy-mermaid', [535, 0, 465, 847], 0.917, 'stock', '180'],
  ['legacy-sky', [535, 0, 465, 847], 0.917, 'stock', '4'],
  ['legacy-giant-167', [535, 0, 465, 847], 0.917, 'stock', '167'],
  ['legacy-seal-116', [488, 0, 465, 847], 0.917, 'stock', '116'], // the 6 run into the slash
  ['legacy-seal-86-sd', [488, 0, 465, 847], 0.917, 'stock', '86'], // the stream in SD: blurrier
  // every relic's Hero's Legacy (hero-NN-*.png, NN the relic's place on the main page)
  ...['207', '115', '135', '92', '109', '28', '156', '105', '36', '180', '4', '27'].map((stock, i) =>
    [`hero-${String(i).padStart(2, '0')}-${HERO[i]}`, [535, 0, 465, 847], 0.917, 'stock', stock]),
  ['board-l19', null, 1, 'rate', '50'],
  ['board-full', [710, 0, 501, 915], 1, 'rate', '50'],
  ['board-no-training', [535, 0, 465, 847], 0.92, 'rate', '65'],
];

const samples = [];
const add = (gs, label) => {
  if (!gs || gs.length !== label.length) return 0;
  gs.forEach((g, i) => samples.push({ d: Number(label[i]), g: gridOf(g), a: g.w / g.h }));
  return gs.length;
};

function harvest(img, kind, labels) {
  let n = 0;
  if (kind === 'main' || kind === 'level') {
    const b = findBanner(img);
    if (!b) return 0;
    const at = (rx, ry) => [b.x0 + rx * b.s, b.y0 + ry * b.s];
    const line = (cx, cy, spec) => {
      const [x0, y0] = at(cx + spec.x[0], cy + spec.y[0]);
      const [x1, y1] = at(cx + spec.x[1], cy + spec.y[1]);
      return inkBox(img, x0, x1, y0, y1);
    };
    if (kind === 'level') return add(numberGlyphs(line(0, 0, MAIN.level), 'trailing', 2), labels);
    tileCentres(b).forEach((t, i) => {
      n += add(numberGlyphs(line(t.rx, t.ry, MAIN.glory), 'trailing', 2), labels.counts[i][0]);
      n += add(numberGlyphs(line(t.rx, t.ry, MAIN.despair), 'trailing', 2), labels.counts[i][1]);
    });
    tileCentres(b).forEach((t, i) => {
      n += add(numberGlyphs(line(t.rx, t.ry, MAIN.stock), 'whole', 4), labels.badges[i]);
    });
    n += add(numberGlyphs(line(0, 0, MAIN.totals.glory), 'trailing', 3), labels.totals[0]);
    n += add(numberGlyphs(line(0, 0, MAIN.totals.despair), 'trailing', 3), labels.totals[1]);
    return n;
  }
  if (kind === 'cards') {
    // found by its buttons alone: recognising it needs the cards read
    const found = findCompareAnywhere(img);
    if (!found) return 0;
    const { buttons } = found;
    const lines = readCards(img, buttons, true).lines;
    const C = REF.cards, u = (buttons.replace.x - buttons.keep.x) / C.spacing, midY = (buttons.keep.y + buttons.replace.y) / 2;
    for (const card of ['current', 'fresh']) {
      const [g, d, boost, stat] = labels[card];
      const cx = buttons[card === 'current' ? 'keep' : 'replace'].x;
      // the stat line under the boost ("402%", "931"): more digits to learn from
      const statLine = inkBox(img, cx + C.x[0] * u, cx + C.x[1] * u, midY + (C.stat - C.half) * u, midY + (C.stat + C.half) * u);
      n += add(lines[card].glory, String(g)) + add(lines[card].despair, String(d)) + add(lines[card].boost, String(boost))
        + add(numberGlyphs(statLine, 'percent', 3), String(stat));
    }
    return n;
  }
  const s = classify(img, null, true);
  if (kind === 'stock') {
    if (s.kind !== 'legacy') return 0;
    const k = s.anchor.s, b = s.button;
    const box = inkBox(img, b.x + REF.stock.x[0] * k, b.x + REF.stock.x[1] * k, b.y + REF.stock.y[0] * k, b.y + REF.stock.y[1] * k);
    // the number, and the 0 that ends "/10"
    const all = box ? glyphs(box) : [];
    // (a last glyph as wide as tall is the "10" run together: not a 0 to learn)
    const last = all[all.length - 1];
    return add(numberGlyphs(box, 'stock', 4, k), labels) + (last && last.w < 0.9 * last.h ? add([last], '0') : 0);
  }
  if (kind === 'rate') {
    // "50%" / "65%" on every Attempt button showing
    if (s.kind !== 'board') return 0;
    for (const [button, shown] of Object.entries(s.present)) {
      if (!shown) continue;
      const r = REF.rateBoxes[button], a = s.anchor;
      n += add(numberGlyphs(inkBox(img, a.x0 + r.x[0] * a.s, a.x0 + r.x[1] * a.s, a.y0 + r.y[0] * a.s, a.y0 + r.y[1] * a.s), 'percent', 2), labels);
    }
    return n;
  }
  return n;
}

for (const [name, box, base, kind, labels] of SOURCES) {
  let img = fixture(name);
  if (box) img = crop(img, ...box);
  const got = [];
  // the sizes the game shows at, and the capture's own (exactly as taken)
  for (const size of [...new Set([0.8, 0.85, 0.9, base, 0.95, 1, 1.05, 1.1, 1.2, 1.3])]) {
    const scale = size / base;
    const frame = Math.abs(scale - 1) < 0.01 ? img
      : compose(img, { width: Math.ceil(img.width * scale) + 40, height: Math.ceil(img.height * scale) + 40, left: 20, top: 20, scale });
    got.push(`${size}:${harvest(frame, kind, labels)}`);
  }
  console.log(name.padEnd(14), got.join(' '));
}

const counts = Array(10).fill(0);
for (const x of samples) counts[x.d]++;
console.log('examples per digit', counts.join(' '));
const line = (x) => `${x.d}${Math.round(x.a * 100).toString().padStart(3, '0')}${x.g.map((v) => Math.min(9, Math.round(v * 9))).join('')}`;
writeFileSync(fileURLToPath(new URL('../glyphdata.js', import.meta.url)),
  `// Generated by test/make-glyphs.mjs from test/images - do not edit.\n`
  + `// Each: the digit, its width/height x 100 (3 places), then its ${samples[0].g.length}-cell ink grid 0-9.\n`
  + `export const GLYPHS = [\n${samples.map((x) => `  '${line(x)}',`).join('\n')}\n];\n`);
console.log(`${samples.length} examples written`);
