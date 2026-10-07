import assert from 'assert';

import { test } from './harness.mjs';
import { amplification, chooseMemory, hitsObjective, inferOutcome, nextTier, isStartState } from '../chrome/lib/logic.js';

const S = (gf, gs, df, ds, ms, sp) => ({ gf, gs, df, ds, ms, sp });

test('the ladder moves one step and stops at the ends', () => {
  assert.strictEqual(nextTier(0, true), 1);
  assert.strictEqual(nextTier(0, false), 0);
  assert.strictEqual(nextTier(4, true), 4);
  assert.strictEqual(nextTier(4, false), 3);
});

test('amplification is +5 / -2, floored at 0', () => {
  assert.strictEqual(amplification({ glory: 7, despair: 1 }), 33);
  assert.strictEqual(amplification({ glory: 6, despair: 4 }), 22);
  assert.strictEqual(amplification({ glory: 0, despair: 3 }), 0);
});

test('keep or replace', () => {
  const max = { kind: 'max' };
  // the Compare screen in the screenshots: current 7/1 (+33%), new 6/4 (+22%)
  assert.strictEqual(chooseMemory(max, { glory: 7, despair: 1 }, { glory: 6, despair: 4 }), 'keep');
  assert.strictEqual(chooseMemory(max, { glory: 6, despair: 4 }, { glory: 7, despair: 1 }), 'replace');
  assert.strictEqual(chooseMemory(max, { glory: 7, despair: 1 }, { glory: 7, despair: 1 }), 'keep');
  const target = { kind: 'target', glory: 7, despair: 2 };
  // a hit beats a miss with more amplification
  assert.strictEqual(chooseMemory(target, { glory: 9, despair: 3 }, { glory: 7, despair: 2 }), 'replace');
  assert.strictEqual(chooseMemory(target, { glory: 7, despair: 2 }, { glory: 9, despair: 3 }), 'keep');
});

test('glory and despair outcomes', () => {
  const st = S(2, 2, 1, 0, 8, 9);
  assert.deepStrictEqual(inferOutcome(st, S(3, 3, 1, 0, 8, 8), 'glory', 10), { ok: true, success: true });
  assert.deepStrictEqual(inferOutcome(st, S(3, 2, 1, 0, 8, 8), 'glory', 10), { ok: true, success: false });
  assert.deepStrictEqual(inferOutcome(st, S(2, 2, 2, 1, 8, 8), 'despair', 10), { ok: true, success: true });
  assert.strictEqual(inferOutcome(st, S(3, 3, 1, 0, 8, 9), 'glory', 10).ok, false); // no spirit spent
  assert.strictEqual(inferOutcome(st, S(2, 2, 2, 1, 8, 8), 'glory', 10).ok, false); // wrong bar
  assert.strictEqual(inferOutcome(st, S(4, 4, 1, 0, 8, 7), 'glory', 10).ok, false); // two fills
});

test('training outcomes, including the invisible one at full spirit power', () => {
  assert.deepStrictEqual(inferOutcome(S(2, 2, 1, 0, 8, 5), S(2, 2, 1, 0, 7, 7), 'train', 10), { ok: true, success: true });
  assert.deepStrictEqual(inferOutcome(S(2, 2, 1, 0, 8, 9), S(2, 2, 1, 0, 7, 10), 'train', 10), { ok: true, success: true });
  assert.deepStrictEqual(inferOutcome(S(2, 2, 1, 0, 8, 5), S(2, 2, 1, 0, 7, 5), 'train', 10), { ok: true, success: false });
  assert.deepStrictEqual(inferOutcome(S(2, 2, 1, 0, 8, 10), S(2, 2, 1, 0, 7, 10), 'train', 10), { ok: true, success: null });
  assert.strictEqual(inferOutcome(S(2, 2, 1, 0, 8, 5), S(2, 2, 1, 0, 7, 6), 'train', 10).ok, false);
});

test('the opening state', () => {
  const board = { slots: 9, maxSpirit: 10, startGlory: 1, startDespairFail: 1 };
  assert.ok(isStartState(S(1, 1, 1, 0, 9, 10), board));
  assert.ok(!isStartState(S(2, 2, 1, 0, 8, 9), board));
});

test('a target keeps whatever narrows the gap to it, or is better on both bars', () => {
  const target = { kind: 'target', glory: 7, despair: 1 };
  const at = (glory, despair) => ({ glory, despair });
  assert.strictEqual(chooseMemory(target, at(7, 3), at(7, 2)), 'replace'); // a step nearer
  assert.strictEqual(chooseMemory(target, at(7, 3), at(6, 1)), 'replace'); // a glory for two despair: nearer
  assert.strictEqual(chooseMemory(target, at(7, 3), at(6, 2)), 'keep'); // as far off
  assert.strictEqual(chooseMemory(target, at(8, 1), at(9, 1)), 'replace'); // better on both, past the bar
  assert.strictEqual(chooseMemory(target, at(7, 3), at(0, 0)), 'keep'); // a wipe
  assert.strictEqual(chooseMemory({ kind: 'target', glory: 8, despair: null }, at(7, 5), at(8, 9)), 'replace'); // any despair
});

test('above a mark: a memory at the mark already meets it; a plain max never does', () => {
  assert.strictEqual(hitsObjective({ kind: 'max', mark: 46 }, { glory: 10, despair: 2 }), true); // 46%
  assert.strictEqual(hitsObjective({ kind: 'max', mark: 46 }, { glory: 9, despair: 0 }), false); // 45%
  assert.strictEqual(hitsObjective({ kind: 'max' }, { glory: 10, despair: 0 }), false);
});
