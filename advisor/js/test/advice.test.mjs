// The advice the extension and the website share: what an attempt's options
// are worth is the solver's, so these are the rules around it.
import assert from 'assert';

import { test } from './harness.mjs';
import { applyMove, criteria, isRoll, keepRule, legalMoves, nothingToKeep, rollOf } from '../advice.js';

const BOARD = { slots: 9, maxSpirit: 10 };

test('a mark that is the memory + 1 is just the chance of a kept result; one above it is its own number', () => {
  const memory = { glory: 7, despair: 1 }; // +33%
  // the engine's word on what is played for: a mark of 0 over nothing to beat is no mark; any other, raised to beat the memory
  const engine = (mark, from) => {
    const beat = from ? 5 * from.glory - 2 * from.despair + 1 : 0;
    return { configure() {}, objective: () => (!mark && beat <= 1 ? { kind: 'amp' } : { kind: 'mark', mark: Math.max(mark ?? 0, beat), least: beat }) };
  };
  const at = (mark, from) => criteria({ objective: { kind: 'max', mark }, from }, engine(mark, from));
  const kept = (mark) => at(mark, memory)[0].kept;
  assert.deepStrictEqual([kept(34), kept(20), kept(46)], [true, true, false]); // 20: raised to beat the memory
  // blank is a mark of 0: raised to beat a memory, like any other mark under it
  assert.deepStrictEqual([kept(undefined), kept(0)], [true, true]);
  // and over nothing to beat (no memory, or +0%): no mark at all - the expected amplification, and nothing else
  const amp = [{ key: 'score', kind: 'amp', label: 'Expected amplification' }];
  assert.deepStrictEqual(at(undefined, null), amp);
  assert.deepStrictEqual(at(0, { glory: 0, despair: 0 }), amp);
});

test('nothing left to keep: the best result still possible, when none would be kept', () => {
  const st = { gf: 3, gs: 1, df: 1, ds: 0, ms: 9, sp: 8 };
  const beats = (memory) => (fresh) => 5 * fresh.glory - 2 * fresh.despair > 5 * memory.glory - 2 * memory.despair;
  // a 9/0 memory (+45%) after two glory fails: at best 7/0, +35%
  assert.deepStrictEqual(nothingToKeep(st, BOARD, beats({ glory: 9, despair: 0 })), { glory: 7, despair: 0 });
  assert.strictEqual(nothingToKeep(st, BOARD, beats({ glory: 5, despair: 0 })), null);
  // a result whose keeping is not known counts as one that might be kept
  assert.strictEqual(nothingToKeep(st, BOARD, () => null), null);
});

test('the rules of a move, the keep rule, and the plan\'s rolls', () => {
  const st = { gf: 8, gs: 5, df: 9, ds: 2, ms: 0, sp: 1 };
  assert.deepStrictEqual(legalMoves(st, BOARD), ['glory']);
  assert.deepStrictEqual(applyMove(st, 2, 'glory', true, BOARD), { st: { ...st, gf: 9, gs: 6, sp: 0 }, tier: 3 });
  const keeps = keepRule({ objective: { kind: 'max' }, from: { glory: 7, despair: 1 } }, null);
  assert.deepStrictEqual([keeps({ glory: 8, despair: 1 }), keeps({ glory: 7, despair: 1 })], [true, false]);
  assert.strictEqual(keepRule({ objective: { kind: 'max' }, from: null }, null)({ glory: 9, despair: 0 }), null);
  const a = { step: 'quest', goal: 19, relic: 11, bar: { glory: 8, despair: 2 }, horizon: { glory: 96, despair: 16 } };
  assert.ok(isRoll(a) && !isRoll({ step: 'summon' }) && !isRoll(null));
  assert.deepStrictEqual(rollOf(a), { horizon: { glory: 96, despair: 16 }, goal: 19, relic: 11, bar: { glory: 8, despair: 2 }, farm: false });
});
