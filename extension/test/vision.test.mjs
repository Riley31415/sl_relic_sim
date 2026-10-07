import assert from 'assert';
import { fileURLToPath } from 'url';

import { readPng, compose } from './png.mjs';
import { test } from './harness.mjs';
import {
  classify, boardState, boardRate, readRate, isForbidden, rel, at, REF,
} from '../chrome/lib/vision.js';

const fixture = readPng(fileURLToPath(new URL('./images/board-l19.png', import.meta.url)));

// level 19: 9 slots, 10 spirit power; the shot is 2 glory successes, the
// free despair failure, 8 mental strength, 9 spirit power, 50%
const WANT = { gf: 2, gs: 2, df: 1, ds: 0, ms: 8, sp: 9 };

// the fixture is a crop; place it in a browser-sized frame like the cloud
// phone page, at the size it was taken and larger and smaller
const FRAMES = [
  { name: 'as captured', width: 1917, height: 915, left: 736, top: 84, scale: 1 },
  { name: 'scaled 1.3x', width: 1917, height: 1000, left: 600, top: 60, scale: 1.3 },
  { name: 'scaled 0.75x', width: 1280, height: 720, left: 450, top: 100, scale: 0.75 },
];

for (const f of FRAMES) {
  test(`reads the level-19 board ${f.name}`, () => {
    const img = compose(fixture, f);
    const screen = classify(img);
    assert.strictEqual(screen.kind, 'board');
    const a = screen.anchor;
    assert.ok(Math.abs(a.s - f.scale) < 0.03, `scale ${a.s}`);
    assert.ok(Math.abs(a.x0 - (f.left + 3 * f.scale)) < 2, `x0 ${a.x0}`);
    assert.ok(Math.abs(a.y0 - (f.top + 400 * f.scale)) < 2, `y0 ${a.y0}`);
    assert.deepStrictEqual(boardState(screen, 9, 10), WANT);
    assert.deepStrictEqual(screen.glory.slice(0, 10), ['success', 'success', ...Array(7).fill('empty'), 'none']);
    assert.deepStrictEqual(screen.despair.slice(0, 10), ['fail', ...Array(8).fill('empty'), 'none']);
  });
}

test('button centres land inside the buttons', () => {
  const f = FRAMES[0];
  const screen = classify(compose(fixture, f));
  // measured by hand on the fixture: train 304-391 x 309-355, glory
  // 306-389 x 428-474, despair 535-580
  const boxes = { train: [304, 391, 309, 355], glory: [306, 389, 428, 474], despair: [306, 389, 535, 580] };
  for (const [name, [x0, x1, y0, y1]] of Object.entries(boxes)) {
    const { x, y } = screen.buttons[name];
    const fx = x - f.left, fy = y - f.top;
    assert.ok(fx > x0 + 10 && fx < x1 - 10 && fy > y0 + 8 && fy < y1 - 8, `${name} at ${fx},${fy}`);
    assert.ok(!isForbidden(screen.anchor, x, y), `${name} is not forbidden`);
  }
});

test('the abandon button is forbidden', () => {
  const f = FRAMES[0];
  const screen = classify(compose(fixture, f));
  // Abandon Inheritance sits at 3-75 x 619-645 in the fixture
  assert.ok(isForbidden(screen.anchor, f.left + 40, f.top + 632));
  const [rx, ry] = rel(screen.anchor, ...at(screen.anchor, 10, 230));
  assert.ok(Math.abs(rx - 10) < 1e-9 && Math.abs(ry - 230) < 1e-9);
});

test('the wrong level is caught', () => {
  const screen = classify(compose(fixture, FRAMES[0]));
  assert.match(boardState(screen, 8, 10).error, /more than 8 slots/);
  assert.match(boardState(screen, 10, 10).error, /fewer than 10 slots/);
});

// video-like noise, deterministic
function noisy(img, amp, seed) {
  let x = seed;
  const rand = () => (x = (x * 1103515245 + 12345) % 2147483648) / 2147483648;
  for (let i = 0; i < img.data.length; i += 4) for (let k = 0; k < 3; k++) img.data[i + k] += Math.round((rand() - 0.5) * 2 * amp);
  return img;
}

test('reads through video noise, down to 0.65x', () => {
  for (const scale of [0.65, 0.75, 1, 1.3]) {
    const img = noisy(compose(fixture, { width: 1917, height: 1000, left: 600, top: 60, scale }), 12, 7);
    const screen = classify(img);
    assert.strictEqual(screen.kind, 'board', `scale ${scale}`);
    assert.deepStrictEqual(boardState(screen, 9, 10), WANT, `scale ${scale}`);
  }
});

test('no board, no anchor', () => {
  const blank = compose({ width: 1, height: 1, data: new Uint8ClampedArray([24, 24, 27, 255]) },
    { width: 800, height: 600, left: 0, top: 0, scale: 1 });
  assert.strictEqual(classify(blank).kind, 'unknown');
});

test('a dimmed board (under a popup) is not a board', () => {
  for (const factor of [0.5, 0.6, 0.7]) {
    const dim = compose(fixture, FRAMES[0]);
    for (let i = 0; i < dim.data.length; i += 4) for (let k = 0; k < 3; k++) dim.data[i + k] *= factor;
    assert.strictEqual(classify(dim).kind, 'unknown', `dimmed to ${factor}`);
  }
});

test('reference geometry stays inside the panel', () => {
  for (const b of Object.values(REF.buttons)) assert.ok(b.x > REF.slotX0 + REF.slotStep * 9);
});

// ------------------------------------------------- the other screens, as captured

const shot = (name) => readPng(fileURLToPath(new URL(`./images/${name}.png`, import.meta.url)));
const within = (p, [x0, x1, y0, y1]) => p.x > x0 && p.x < x1 && p.y > y0 && p.y < y1;

test('the full tab: the board', () => {
  const screen = classify(shot('board-full'));
  assert.strictEqual(screen.kind, 'board');
  assert.deepStrictEqual(boardState(screen, 9, 10), WANT);
  // the buttons as they appear in that screenshot
  assert.ok(within(screen.buttons.glory, [1045, 1125, 515, 555]), JSON.stringify(screen.buttons.glory));
});

for (const [name, box] of [['legacy', [150, 300, 655, 725]], ['legacy-full', [865, 1015, 698, 768]]]) {
  test(`Hero's Legacy (${name}): the applied memory and the Inheritance button`, () => {
    const screen = classify(shot(name));
    assert.strictEqual(screen.kind, 'legacy');
    const { glory, despair } = screen.memory;
    // 8 slots from before the 9th unlocked: 7 glory, 1 despair (+33%)
    assert.deepStrictEqual([glory.slots, glory.success, despair.slots, despair.success], [8, 7, 8, 1]);
    assert.ok(within(screen.button, box), JSON.stringify(screen.button));
  });
}

test('Complete: the finished bars and the Complete Inheritance button', () => {
  const screen = classify(shot('complete'));
  assert.strictEqual(screen.kind, 'complete');
  const { glory, despair } = screen.memory;
  assert.deepStrictEqual([glory.filled, glory.success, despair.filled, despair.success], [9, 5, 9, 4]);
  assert.ok(within(screen.button, [145, 285, 690, 742]), JSON.stringify(screen.button));
});

// compare.png is complete.png's view moved by (13, 3); abandon.png is
// board-l19.png's moved by (6, 53) - both found by aligning the backgrounds
const COMPARE_ANCHOR = { x0: 29, y0: 445, s: 0.9975 };
const ABANDON_ANCHOR = { x0: 10, y0: 453, s: 0.9975 };

test('Compare: Keep on the left, Replace on the right', () => {
  const screen = classify(shot('compare'), COMPARE_ANCHOR);
  assert.strictEqual(screen.kind, 'compare');
  assert.ok(within(screen.buttons.keep, [80, 190, 550, 585]), JSON.stringify(screen.buttons.keep));
  assert.ok(within(screen.buttons.replace, [268, 378, 550, 585]), JSON.stringify(screen.buttons.replace));
  // and not at all unless it is being looked for
  assert.strictEqual(classify(shot('compare')).kind, 'unknown');
});

test('the abandon confirmation is never taken for Compare', () => {
  assert.strictEqual(classify(shot('abandon')).kind, 'unknown');
  // even when the Compare popup is being looked for, at the board's anchor
  assert.strictEqual(classify(shot('abandon'), ABANDON_ANCHOR).kind, 'unknown');
  for (let dx = -20; dx <= 20; dx += 5) {
    for (let dy = -20; dy <= 20; dy += 5) {
      const a = { ...ABANDON_ANCHOR, x0: ABANDON_ANCHOR.x0 + dx, y0: ABANDON_ANCHOR.y0 + dy };
      assert.strictEqual(classify(shot('abandon'), a).kind, 'unknown', `anchor moved ${dx},${dy}`);
    }
  }
});

// ------------------------------------------------------- reading the rate

test('no mental strength left: the training button is gone, the board still reads', () => {
  const screen = classify(shot('board-no-training'));
  assert.strictEqual(screen.kind, 'board');
  assert.deepStrictEqual(screen.present, { train: false, glory: true, despair: true });
  // a narrower tab (the side panel open): 0.92x
  assert.ok(Math.abs(screen.anchor.s - 0.92) < 0.02, `scale ${screen.anchor.s}`);
  assert.deepStrictEqual(boardState(screen, 9, 10), { gf: 5, gs: 5, df: 4, ds: 0, ms: 0, sp: 9 });
});

test('the rate is read off every button: 50% and 65%', () => {
  assert.deepStrictEqual(boardRate(classify(shot('board-l19'))), { tier: 2, read: { train: 2, glory: 2, despair: 2 } });
  assert.deepStrictEqual(boardRate(classify(shot('board-full'))), { tier: 2, read: { train: 2, glory: 2, despair: 2 } });
  assert.deepStrictEqual(boardRate(classify(shot('board-no-training'))), { tier: 1, read: { glory: 1, despair: 1 } });
});

/** A region of an image. */
function crop(img, x0, y0, w, h) {
  const data = new Uint8ClampedArray(w * h * 4);
  for (let y = 0; y < h; y++) data.set(img.data.subarray(((y0 + y) * img.width + x0) * 4, ((y0 + y) * img.width + x0 + w) * 4), y * w * 4);
  return { width: w, height: h, data };
}

test('board and rate read right from 0.8x to 1.5x, through noise - never a wrong rate', () => {
  const cases = [
    { img: fixture, base: 1, tier: 2, st: WANT },
    // the phone area of a 0.92x capture, so rescaling it resamples twice
    { img: crop(shot('board-no-training'), 535, 0, 465, 847), base: 0.92, tier: 1, st: { gf: 5, gs: 5, df: 4, ds: 0, ms: 0, sp: 9 } },
  ];
  for (const { img, base, tier, st } of cases) {
    for (const size of [0.8, 0.9, 1, 1.1, 1.2, 1.3, 1.5]) {
      const scale = size / base;
      for (const [amp, seed] of [[0, 1], [10, 2]]) {
        const frame = noisy(compose(img, { width: Math.ceil(img.width * scale) + 40, height: Math.ceil(img.height * scale) + 40, left: 20, top: 20, scale }), amp, seed);
        const screen = classify(frame);
        const where = `tier ${tier} board at ${size}x, noise ${amp}`;
        assert.strictEqual(screen.kind, 'board', where);
        assert.deepStrictEqual(boardState(screen, 9, 10), st, where);
        for (const [button, feature] of Object.entries(screen.rates)) {
          const read = readRate(feature);
          assert.ok(read === null || read === tier, `${where}: ${button} read ${read}`);
        }
        assert.strictEqual(boardRate(screen)?.tier, tier, where);
      }
    }
  }
});

test('text too small to read is unsure, not wrong', () => {
  const frame = compose(fixture, { width: 400, height: 560, left: 20, top: 20, scale: 0.7 });
  const screen = classify(frame);
  assert.strictEqual(screen.kind, 'board');
  for (const feature of Object.values(screen.rates)) assert.ok([null, 2].includes(readRate(feature)));
});

test('no text, no reading', () => {
  assert.strictEqual(readRate(null), null);
  assert.strictEqual(readRate({ w: 40, h: 20, s: 1, ink: new Array(800).fill(0) }), null);
});

// ------------------------------------------------- results cards, wipes

test('Compare: both cards read, each checked against its boost', () => {
  for (const [name, anchor, current, fresh] of [
    ['compare', COMPARE_ANCHOR, { glory: 7, despair: 1 }, { glory: 6, despair: 4 }],
    ['compare-full', { x0: 555, y0: 430, s: 0.92 }, { glory: 8, despair: 3 }, { glory: 8, despair: 4 }],
  ]) {
    const img = shot(name);
    for (const screen of [classify(img, anchor), classify(img, null, true)]) {
      assert.strictEqual(screen.kind, 'compare', name);
      assert.deepStrictEqual(screen.cards.current, current, name);
      assert.deepStrictEqual(screen.cards.fresh, fresh, name);
    }
  }
});

test('found anywhere, Compare still never matches the abandon popup or a board', () => {
  assert.strictEqual(classify(shot('abandon'), null, true).kind, 'unknown');
  for (const name of ['board-l19', 'legacy', 'complete', 'board-wiped']) assert.notStrictEqual(classify(shot(name), null, true).kind, 'compare');
});

test('a wiped board: no Attempt buttons, Abandon Inheritance found', () => {
  const screen = classify(shot('board-wiped'));
  assert.strictEqual(screen.kind, 'board');
  assert.deepStrictEqual(screen.present, { train: false, glory: false, despair: false });
  assert.deepStrictEqual(boardState(screen, 9, 10), { gf: 9, gs: 7, df: 8, ds: 2, ms: 0, sp: 0 });
  // the button spans about 562-629 x 649-672 in that capture
  assert.ok(within(screen.abandon, [570, 620, 652, 670]), JSON.stringify(screen.abandon));
  assert.ok(isForbidden(screen.anchor, screen.abandon.x, screen.abandon.y)); // only the wipe flow may click it
});

test('the abandon confirmation: found only when looked for, Confirm on the left', () => {
  const screen = classify(shot('abandon'), ABANDON_ANCHOR, false, 'abandon');
  assert.strictEqual(screen.kind, 'abandon');
  // Confirm spans about 98-193 x 535-576 in that capture
  assert.ok(within(screen.buttons.confirm, [110, 180, 540, 570]), JSON.stringify(screen.buttons.confirm));
  assert.strictEqual(classify(shot('abandon'), ABANDON_ANCHOR).kind, 'unknown');
  assert.strictEqual(classify(shot('compare'), COMPARE_ANCHOR, false, 'abandon').kind, 'unknown');
});

test('Hero\'s Legacy on another relic (legacy-crown)', () => {
  const screen = classify(shot('legacy-crown'));
  assert.strictEqual(screen.kind, 'legacy');
  assert.deepStrictEqual([screen.memory.glory.success, screen.memory.despair.success], [8, 3]);
});

// ------------------------------------------------------- the main page, stock

const MAIN_BOARD = [[7, 1], [7, 1], [8, 3], [8, 2], [8, 2], [7, 1], [8, 2], [8, 3], [7, 1], [9, 1], [7, 2], [8, 3]];

test('the main page, levels view: level 18, pity 52%, where to click', () => {
  const screen = classify(shot('main-levels'));
  assert.strictEqual(screen.kind, 'main');
  assert.strictEqual(screen.view, 'levels');
  assert.strictEqual(screen.level, 18);
  assert.ok(Math.abs(screen.pity - 0.52) < 0.01, `pity ${screen.pity}`);
  // Giant's Right Hand's icon spans about 620-672 x 222-273; Crown 823-875 x 549-601
  assert.ok(within(screen.tiles[0], [630, 662, 232, 264]), JSON.stringify(screen.tiles[0]));
  assert.ok(within(screen.tiles[11], [833, 865, 559, 591]), JSON.stringify(screen.tiles[11]));
  assert.ok(within(screen.toggle, [585, 597, 167, 179]), JSON.stringify(screen.toggle));
});

test('the main page, counts view: every relic, checked against the totals', () => {
  const screen = classify(shot('main-counts'));
  assert.strictEqual(screen.view, 'counts');
  assert.deepStrictEqual(screen.relics.map((r) => [r.glory, r.despair]), MAIN_BOARD);
  assert.deepStrictEqual(screen.relics.map((r) => r.stock), [207, 115, 135, 92, 109, 28, 156, 105, 36, 180, 4, 27]);
  assert.deepStrictEqual(screen.totals, { glory: 92, despair: 22 });
  assert.ok(screen.ok);
});

test('the Level Up button: seen on both views once a level-up is earned, never otherwise', () => {
  for (const name of ['main-levels', 'main-counts']) assert.strictEqual(classify(shot(name)).levelUp, false, name);
  const levels = classify(shot('main-levelup-levels'));
  assert.strictEqual(levels.kind, 'main');
  assert.strictEqual(levels.view, 'levels');
  assert.strictEqual(levels.levelUp, true);
  assert.strictEqual(levels.level, 18);
  assert.ok(levels.pity > 0.99, `pity ${levels.pity}`);
  const counts = classify(shot('main-levelup-counts'));
  assert.strictEqual(counts.view, 'counts');
  assert.strictEqual(counts.levelUp, true);
  // the quest text in the panel below does not get in the way of the counts
  assert.deepStrictEqual(counts.relics.map((r) => [r.glory, r.despair]),
    [[7, 1], [7, 1], [8, 2], [8, 2], [8, 2], [7, 1], [8, 2], [8, 1], [7, 1], [9, 1], [7, 2], [8, 3]]);
  assert.deepStrictEqual(counts.totals, { glory: 92, despair: 19 });
  assert.ok(counts.ok);
});

test('Hero\'s Legacy: the stock on its button, and the way back', () => {
  for (const [name, stock] of [['legacy-crown', 127], ['legacy-full', 195], ['legacy', 195]]) {
    const screen = classify(shot(name));
    assert.strictEqual(screen.stock, stock, name);
    assert.ok(screen.back, name);
  }
  // the red return button spans 565-601 x 746-780 in legacy-crown
  assert.ok(within(classify(shot('legacy-crown')).back, [575, 591, 755, 771]));
});

test('Hero\'s Legacy: every stock read, digits run together or not', () => {
  // "207/10" with the 10's 1 and 0 joined; "135/" and "156/" joined along the bottom row
  for (const [name, stock, memory] of [
    ['legacy-giant', 207, [7, 1]], ['legacy-oath', 135, [8, 3]], ['legacy-star', 28, [7, 1]],
    ['legacy-seal', 156, [8, 2]], ['legacy-mermaid', 180, [9, 1]], ['legacy-sky', 4, [7, 2]],
    ['legacy-giant-167', 167, [7, 1]], ['legacy-seal-116', 116, [8, 2]],
    ['legacy-seal-86-sd', 86, [8, 2]],
  ]) {
    const screen = classify(shot(name));
    assert.strictEqual(screen.kind, 'legacy', name);
    assert.strictEqual(screen.stock, stock, name);
    assert.deepStrictEqual([screen.memory.glory.success, screen.memory.despair.success], memory, name);
  }
});

test('every relic\'s Hero\'s Legacy: its stock and memory, as the main page has them', () => {
  const HERO = ['giant', 'demon', 'oath', 'tree', 'ring', 'star', 'seal', 'veil', 'spark', 'mermaid', 'sky', 'crown'];
  const STOCK = [207, 115, 135, 92, 109, 28, 156, 105, 36, 180, 4, 27];
  HERO.forEach((name, i) => {
    const screen = classify(shot(`hero-${String(i).padStart(2, '0')}-${name}`));
    assert.strictEqual(screen.kind, 'legacy', name);
    assert.strictEqual(screen.stock, STOCK[i], name);
    assert.deepStrictEqual([screen.memory.glory.success, screen.memory.despair.success], MAIN_BOARD[i], name);
    assert.ok(screen.back, name);
  });
});
