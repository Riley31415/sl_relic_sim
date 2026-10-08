// Reading screenshots the way a player adds them: cloud-phone captures, and
// phone screenshots 2.5-3.3x their size.
import assert from 'assert';
import { fileURLToPath } from 'url';

import { test } from '../../advisor/js/test/harness.mjs';
import { readPng, compose } from '../../advisor/js/test/png.mjs';
import { readScreenshot, shrink } from '../dist/screenshot.js';

const image = (name) => readPng(fileURLToPath(new URL(`../../advisor/js/test/images/${name}`, import.meta.url)));
const scaled = (img, scale) => compose(img, { width: Math.round(img.width * scale), height: Math.round(img.height * scale), left: 0, top: 0, scale });

/** The phone's screen out of a cloud-phone browser capture. */
function phone(img) {
  const [x0, y0, w, h] = [535, 44, 425, 756];
  const data = new Uint8ClampedArray(w * h * 4);
  for (let y = 0; y < h; y++) data.set(img.data.subarray(((y0 + y) * img.width + x0) * 4, ((y0 + y) * img.width + x0 + w) * 4), y * w * 4);
  return { width: w, height: h, data };
}

for (const scale of [1, 2.5, 3.3]) {
  test(`a board at ${scale}x: its slots and its rate`, () => {
    const r = readScreenshot(scaled(image('board-l19.png'), scale));
    assert.strictEqual(r.kind, 'board');
    assert.strictEqual(r.tier, 2);
    assert.deepStrictEqual(r.screen.leaves, { count: 8, contiguous: true });
    assert.ok(r.marks.crop && r.marks.dots.length > 20);
  });

  test(`Hero's Legacy at ${scale}x: the memory and the stock`, () => {
    const r = readScreenshot(scaled(image('legacy.png'), scale));
    assert.deepStrictEqual([r.kind, r.memory, r.stock], ['legacy', { glory: 7, despair: 1 }, 195]);
  });

  test(`the results at ${scale}x: both cards`, () => {
    const r = readScreenshot(scaled(image('compare.png'), scale));
    assert.deepStrictEqual([r.kind, r.current, r.fresh], ['compare', { glory: 7, despair: 1 }, { glory: 6, despair: 4 }]);
  });

  test(`the main page at ${scale}x: the level and pity, every relic's counts`, () => {
    const lv = readScreenshot(scaled(phone(image('main-levels.png')), scale));
    assert.deepStrictEqual([lv.kind, lv.level], ['levels', 18]);
    assert.ok(Math.abs(lv.pity - 0.52) < 0.01);
    const c = readScreenshot(scaled(phone(image('main-counts.png')), scale));
    assert.strictEqual(c.kind, 'counts');
    assert.ok(c.ok);
    assert.deepStrictEqual(c.states, [[7, 1], [7, 1], [8, 3], [8, 2], [8, 2], [7, 1], [8, 2], [8, 3], [7, 1], [9, 1], [7, 2], [8, 3]]);
    // a badge reads right or not at all - never a wrong number
    const truth = [207, 115, 135, 92, 109, 28, 156, 105, 36, 180, 4, 27];
    c.stock.forEach((n, i) => assert.ok(n === null || n === truth[i], `badge ${i}: ${n}`));
  });
}

test('a badge the two reads disagree on is left for the player', () => {
  // at 2.5x this capture's Oath of Immortality badge reads 1125 alone; the 1x read says 125
  const c = readScreenshot(scaled(phone(image('main-levelup-counts.png')), 2.5));
  assert.strictEqual(c.kind, 'counts');
  const truth = [137, 115, 125, 92, 109, 28, 136, 5, 36, 160, 4, 7];
  c.stock.forEach((n, i) => assert.ok(n === null || n === truth[i], `badge ${i}: ${n}`));
});

test('shrinking keeps a flat colour flat and averages the rest', () => {
  const img = { width: 4, height: 2, data: new Uint8ClampedArray(32) };
  for (let i = 0; i < 8; i++) img.data.set(i % 2 ? [200, 100, 0, 255] : [0, 100, 200, 255], i * 4);
  const half = shrink(img, 0.5);
  assert.deepStrictEqual([half.width, half.height], [2, 1]);
  assert.deepStrictEqual([...half.data.subarray(0, 4)], [100, 100, 100, 255]);
});
