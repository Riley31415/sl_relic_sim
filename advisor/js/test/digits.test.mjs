import assert from 'assert';
import { fileURLToPath } from 'url';

import { readPng } from './png.mjs';
import { test } from './harness.mjs';
import { inkBox, glyphs, readDigit } from '../digits.js';

const shot = (name) => readPng(fileURLToPath(new URL(`./images/${name}.png`, import.meta.url)));

// every digit the Compare captures show, in their three text sizes:
// [capture, box x0 x1 y0 y1, the digits its tall glyphs read, '.' for non-digits]
const LINES = [
  ['compare', [60, 230, 340, 365], '.....7'], // Glory x 7 (gem, G-l-o-r-y, x)
  ['compare-full', [763, 915, 460, 480], '396..'], // 396% and the red arrow
  ['compare-full', [590, 742, 460, 480], '402'], // 402%
  ['compare', [50, 222, 458, 482], '931'],
  ['compare', [238, 410, 458, 482], '854'],
  ['compare', [238, 410, 412, 436], '.22'], // +22%
  ['compare-full', [590, 742, 418, 438], '.34'], // +34%
];

test('every digit 0-9 reads right in the captures', () => {
  const seen = new Set();
  for (const [name, box, want] of LINES) {
    const gs = glyphs(inkBox(shot(name), ...box)).filter((g) => g.h >= 9);
    const tall = name === 'compare' && want.endsWith('7') ? gs.slice(-1) : gs;
    const digits = want.replace(/\./g, '');
    const read = tall.map(readDigit).filter((d) => d !== null).join('');
    assert.ok(read.includes(digits), `${name} ${box}: read ${read}, want ${digits}`);
    for (const d of digits) seen.add(d);
  }
  assert.strictEqual([...seen].sort().join(''), '0123456789');
});

test('not a digit: unsure', () => {
  assert.strictEqual(readDigit({ w: 10, h: 10, ink: new Array(100).fill(1) }), null); // a block
  assert.strictEqual(readDigit({ w: 3, h: 4, ink: new Array(12).fill(1) }), null); // too small
});
