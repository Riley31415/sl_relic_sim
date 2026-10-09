// The session: the inputs a player makes, and the advice it gives back,
// against the real solver.
import assert from 'assert';
import { readFileSync } from 'fs';

import { test } from '../../advisor/js/test/harness.mjs';
import { RelicSolver } from '../dist/lib/solver.js';
import { singleRelicSettings } from '../dist/lib/advice.js';
import { chooseMemory } from '../dist/lib/logic.js';
import { act, advise, applyMove, blank, opening } from '../dist/session.js';

const solver = await RelicSolver.load(readFileSync(new URL('../dist/relic.wasm', import.meta.url)));

// the level-18 board of the advisor's tests (advisor/js/test/images/main-counts.png)
const BOARD = [[7, 1], [7, 1], [8, 3], [8, 2], [8, 2], [7, 1], [8, 2], [8, 3], [7, 1], [9, 1], [7, 2], [8, 3]];
const STOCK = [207, 115, 135, 92, 109, 28, 156, 105, 36, 180, 4, 137];
const SMART = { mode: 'advisor', goal: { level: 20, tier: 46 } };
const single = (objective) => ({ mode: 'single', goal: SMART.goal, objective });

/** Apply inputs in turn: the session after them. */
function play(s, settings, ...inputs) {
  for (const input of inputs) s = act(s, input, settings, solver).session;
  return s;
}

const counts = (states = BOARD, stock = STOCK) => ({ type: 'read', reading: { kind: 'counts', states, stock, ok: true, totals: { glory: 92, despair: 22 } } });
const levels = (level = 18, pity = 0.5) => ({ type: 'read', reading: { kind: 'levels', level, pity, levelUp: false } });
const legacy = (glory, despair, stock) => ({ type: 'read', reading: { kind: 'legacy', memory: { glory, despair }, slots: 9, stock } });

/** A run of luck a player would take: glory and training succeed, despair fails. */
const lucky = (action) => action !== 'despair';

/** Play the attempt in progress the way the advice says, each move going as `luck` says, to its end. */
function playOut(s, settings, luck = lucky) {
  for (let i = 0; i < 40; i++) {
    const a = advise(s, settings, solver);
    if (a.press === 'complete' || a.press === 'abandon' || s.view !== 'board') return { s, a };
    assert.ok(['glory', 'despair', 'train'].includes(a.press), `pressed ${a.press}`);
    s = play(s, settings, { type: 'move', action: a.press, success: luck(a.press, i) });
  }
  throw new Error('the attempt never ended');
}

test('single relic: Hero\'s Legacy, Inheritance, every move to the end, Keep or Replace', () => {
  const settings = single({ kind: 'max', mark: 46 });
  let s = play(blank(), settings, { type: 'set', level: 19 }, legacy(8, 2, 50));
  let a = advise(s, settings, solver);
  assert.strictEqual(a.press, 'inheritance');
  // the attempt's prospects: every result, and a chance it improves the memory
  const total = a.prospect.wipe + a.prospect.results.reduce((x, r) => x + r.p, 0);
  assert.ok(Math.abs(total - 1) < 1e-9);
  assert.ok(a.prospect.kept > 0 && a.prospect.kept < 1);

  s = play(s, settings, { type: 'press', button: 'inheritance' });
  assert.strictEqual(s.view, 'board');
  assert.strictEqual(s.onHand, 40);
  assert.deepStrictEqual(s.attempt.st, opening(solver, 19).st);
  a = advise(s, settings, solver);
  // the move advised is the solver's, worth what the state is, and no move scores more
  solver.configure(19, settings.objective, { glory: 8, despair: 2 });
  const best = solver.best(s.attempt.st, s.attempt.tier);
  assert.strictEqual(a.press, best);
  const whole = solver.moveValue(s.attempt.st, s.attempt.tier);
  const top = a.moves.find((m) => m.action === best);
  assert.ok(Math.abs(top.value.score - whole.score) < 1e-12);
  for (const m of a.moves) {
    assert.ok(m.value.score <= whole.score + 1e-12);
    // a move's score is its two ways of going, weighed by their chances
    const mixed = m.branches.reduce((x, b) => x + b.p * b.value.score, 0);
    assert.ok(Math.abs(mixed - m.value.score) < 1e-9, `${m.action}: ${mixed} vs ${m.value.score}`);
    assert.ok(m === top ? m.decider === null : typeof m.decider === 'string' || m.decider === null);
  }
  assert.strictEqual(a.criteria[0].label, 'Chance of +46% or more');

  ({ s, a } = playOut(s, settings));
  assert.strictEqual(a.press, 'complete');
  s = play(s, settings, { type: 'press', button: 'complete' });
  assert.strictEqual(s.view, 'compare');
  a = advise(s, settings, solver);
  assert.strictEqual(a.press, chooseMemory(settings.objective, { glory: 8, despair: 2 }, s.result.fresh));
  const applied = a.press === 'keep' ? s.result.current : s.result.fresh;
  s = play(s, settings, { type: 'press', button: a.press });
  assert.strictEqual(s.view, 'legacy');
  assert.deepStrictEqual(s.memory, applied);
  assert.strictEqual(s.attempt, null);
});

test('a move entered moves the board and the rate the way the game does', () => {
  const board = solver.board(19);
  const { st, tier } = opening(solver, 19);
  assert.deepStrictEqual(applyMove(st, tier, 'glory', true, board), { st: { ...st, gf: 2, gs: 2, sp: 9 }, tier: tier + 1 });
  assert.deepStrictEqual(applyMove(st, 2, 'despair', false, board), { st: { ...st, df: 2, sp: 9 }, tier: 1 });
  // training at full spirit power: nothing over the cap
  assert.deepStrictEqual(applyMove(st, 0, 'train', true, board), { st: { ...st, ms: 8 }, tier: 1 });
  assert.deepStrictEqual(applyMove({ ...st, sp: 3 }, 4, 'train', true, board).st.sp, 5);
});

test('smart leveler: the plan picks the relic, plays it its way and keeps by its rule', () => {
  let s = play(blank(), SMART, levels(18, 0.5), counts());
  let a = advise(s, SMART, solver);
  const plan = solver.advise(SMART.goal, 18, BOARD, STOCK, 0.5);
  assert.strictEqual(a.press, 'tile');
  assert.strictEqual(a.tile, plan.relic);
  assert.strictEqual(a.plan.quest.kind, 'total');
  assert.deepStrictEqual([a.plan.quest.glory, a.plan.quest.despair], [94, 19]);
  assert.deepStrictEqual(a.plan.totals, [92, 22]);

  s = play(s, SMART, { type: 'tile', relic: plan.relic });
  a = advise(s, SMART, solver);
  assert.strictEqual(a.press, 'inheritance');
  assert.strictEqual(a.prospect.by, 'plan');
  assert.deepStrictEqual(a.prospect.objective, plan.objective);

  s = play(s, SMART, { type: 'press', button: 'inheritance' });
  assert.strictEqual(s.stock[plan.relic], STOCK[plan.relic] - 10);
  assert.deepStrictEqual(s.attempt.objective, plan.objective);
  // lucky but for one glory fail
  ({ s, a } = playOut(s, SMART, (action, i) => (i === 2 ? false : lucky(action))));
  assert.strictEqual(a.press, 'complete');
  s = play(s, SMART, { type: 'press', button: 'complete' });
  a = advise(s, SMART, solver);
  const roll = { horizon: plan.horizon, goal: plan.goal, relic: plan.relic, bar: plan.bar ?? null, farm: false };
  assert.strictEqual(a.press, solver.keepsRoll(roll, BOARD, s.result.fresh) ? 'replace' : 'keep');
  assert.ok(a.compare.rows.length >= 2);
  s = play(s, SMART, { type: 'press', button: a.press });
  // the board moves on: the memory, and one attempt's pity
  assert.deepStrictEqual(s.states[plan.relic], a.press === 'keep' ? BOARD[plan.relic] : [s.memory.glory, s.memory.despair]);
  assert.ok(Math.abs(s.pity - (0.5 + solver.pityGain(18) / solver.pityNeeded(19))) < 1e-12);
});

test('smart leveler: a relic, once named, cannot be un-named by hand', () => {
  const s = play(blank(), SMART, levels(), counts(), { type: 'tile', relic: 3 });
  assert.throws(() => act(s, { type: 'set', relic: null }, SMART, solver), /The relic must be/);
  assert.strictEqual(play(s, SMART, { type: 'set', relic: 5 }).relic, 5);
});

test('smart leveler: the pity counts an attempt seen to its end, not one a screenshot leaves part-way', () => {
  const plan = solver.advise(SMART.goal, 18, BOARD, STOCK, 0.5);
  const started = () => {
    const s = play(blank(), SMART, levels(), counts(), { type: 'tile', relic: plan.relic }, { type: 'press', button: 'inheritance' });
    return play(s, SMART, { type: 'move', action: advise(s, SMART, solver).press, success: true });
  };
  const one = solver.pityGain(18) / solver.pityNeeded(19);
  const [g, d] = BOARD[plan.relic];
  // part-way, another screen's screenshot: the attempt may never have ended, so no pity
  assert.strictEqual(play(started(), SMART, legacy(g, d, STOCK[plan.relic] - 10)).pity, 0.5);
  assert.strictEqual(play(started(), SMART, counts()).pity, 0.5);
  // abandoned, or completed: one attempt's pity
  assert.ok(Math.abs(play(started(), SMART, { type: 'press', button: 'abandon' }).pity - (0.5 + one)) < 1e-12);
  let { s } = playOut(started(), SMART);
  s = play(s, SMART, { type: 'press', button: 'complete' }, counts());
  assert.ok(Math.abs(s.pity - (0.5 + one)) < 1e-12);
});

test('smart leveler: defaults to start from, numbers that did not read, counts that do not add up', () => {
  // nothing entered: every number at its default, and a prompt to start
  const fresh = blank();
  assert.deepStrictEqual([fresh.level, fresh.pity, fresh.states[0], fresh.stock[0], fresh.memory], [1, 0, [0, 0], 0, { glory: 0, despair: 0 }]);
  let a = advise(fresh, SMART, solver);
  assert.strictEqual(a.headline, 'Add a screenshot to start');
  // a badge that did not read: the stock it had, marked to check, and advice all the same
  let s = play(blank(), SMART, levels(), counts());
  const unread = STOCK.map((n, i) => (i === 3 ? null : n));
  s = play(s, SMART, counts(BOARD, unread));
  a = advise(s, SMART, solver);
  assert.deepStrictEqual([s.stock[3], a.check, a.press], [STOCK[3], ['n3'], 'tile']);
  s = play(s, SMART, { type: 'set', relicStates: [{ i: 3, stock: 90 }] });
  assert.deepStrictEqual([s.stock[3], advise(s, SMART, solver).check], [90, undefined]);
  // one number put right leaves the rest of the relic as it was
  s = play(s, SMART, { type: 'set', relicStates: [{ i: 4, despair: 1 }] });
  assert.deepStrictEqual(s.states[4], [BOARD[4][0], 1]);
  // counts that do not add up to the totals line: checked before anything is advised
  s = play(blank(), SMART, levels(), { type: 'read', reading: { kind: 'counts', states: BOARD, stock: STOCK, ok: false, totals: { glory: 93, despair: 22 } } });
  assert.deepStrictEqual(advise(s, SMART, solver).needs.map((n) => n.what), ['states']);
  s = play(s, SMART, { type: 'set', relicStates: [{ i: 0, glory: 8, despair: 1 }] }); // the misread one, put right
  assert.strictEqual(advise(s, SMART, solver).press, 'tile');
});

test('smart leveler: a full pity bar or a met requirement is a level-up', () => {
  let s = play(blank(), SMART, levels(18, 1), counts());
  assert.strictEqual(advise(s, SMART, solver).press, 'levelup');
  // the game's own Level Up button, lit
  const lit = { type: 'read', reading: { kind: 'levels', level: 18, pity: 0.4, levelUp: true } };
  assert.strictEqual(advise(play(blank(), SMART, lit, counts()), SMART, solver).press, 'tile'); // the counts view after it showed it unlit
  assert.strictEqual(advise(play(blank(), SMART, counts(), lit), SMART, solver).press, 'levelup');
  s = play(s, SMART, { type: 'press', button: 'levelup' });
  assert.deepStrictEqual([s.level, s.pity], [19, 0]);
  const met = BOARD.map((x) => [...x]);
  met[9] = [9, 0];
  met[10] = [9, 0];
  met[2] = [8, 1];
  assert.strictEqual(advise(play(blank(), SMART, levels(18, 0), counts(met)), SMART, solver).press, 'levelup');
  // nothing on hand: summon
  const none = advise(play(blank(), SMART, levels(18, 0), counts(BOARD, new Array(12).fill(0))), SMART, solver);
  assert.strictEqual(none.press, null);
  assert.strictEqual(none.headline, 'Summon');
});

test('an attempt nothing useful can come of is abandoned, and so is a wipe', () => {
  const settings = single({ kind: 'max' });
  // a 9/0 memory (+45%) needs every glory slot: two glory fails and it is out of reach
  let s = play(blank(), settings, { type: 'set', level: 19 }, legacy(9, 0, 30), { type: 'press', button: 'inheritance' });
  s = play(s, settings, { type: 'set', st: { gf: 3, gs: 1, df: 1, ds: 0, ms: 9, sp: 8 } });
  let a = advise(s, settings, solver);
  assert.strictEqual(a.press, 'abandon');
  assert.match(a.detail, /would not replace/);
  s = play(s, settings, { type: 'press', button: 'abandon' });
  assert.deepStrictEqual([s.view, s.memory, s.onHand], ['legacy', { glory: 9, despair: 0 }, 20]);
  // no spirit power, no mental strength, the bars unfinished
  s = play(blank(), single({ kind: 'max', mark: 40 }), { type: 'set', level: 19 }, legacy(7, 1, 30), { type: 'press', button: 'inheritance' },
    { type: 'set', st: { gf: 6, gs: 5, df: 5, ds: 1, ms: 0, sp: 0 } });
  a = advise(s, single({ kind: 'max', mark: 40 }), solver);
  assert.strictEqual(a.press, 'abandon');
  assert.match(a.detail, /Out of spirit power/);
  // moves left, but five slots and three spirit power with no mental strength: it can only wipe
  s = play(blank(), single({ kind: 'max', mark: 40 }), { type: 'set', level: 19 }, legacy(7, 1, 30), { type: 'press', button: 'inheritance' },
    { type: 'set', st: { gf: 6, gs: 6, df: 7, ds: 2, ms: 0, sp: 3 } });
  a = advise(s, single({ kind: 'max', mark: 40 }), solver);
  assert.strictEqual(a.press, 'abandon');
  assert.ok(a.abandon.doomed && !a.abandon.wiped);
  assert.match(a.detail, /can only wipe/);
  // one mental strength more and a training could still cover it: played on
  s = play(s, single({ kind: 'max', mark: 40 }), { type: 'set', st: { gf: 6, gs: 6, df: 7, ds: 2, ms: 1, sp: 3 } });
  assert.notStrictEqual(advise(s, single({ kind: 'max', mark: 40 }), solver).press, 'abandon');
});

test('screenshots mid-way: a board, the results, Hero\'s Legacy after', () => {
  const settings = single({ kind: 'close', glory: 9, despair: 0 });
  let s = play(blank(), settings, { type: 'set', level: 19 }, legacy(7, 3, 60));
  // the level-19 board of test/images/board-l19.png, read at 50%
  const cells = (n, v, slots = 9) => [...Array(n).fill(v[0]), ...Array(slots - n).fill(v[1]), 'none'];
  const screen = {
    glory: cells(2, ['success', 'empty']), despair: cells(1, ['fail', 'empty']),
    leaves: { count: 8, contiguous: true }, fill: 0.9, present: { train: true, glory: true, despair: true },
  };
  s = play(s, settings, { type: 'read', reading: { kind: 'board', screen, tier: 2 } });
  assert.deepStrictEqual(s.attempt.st, { gf: 2, gs: 2, df: 1, ds: 0, ms: 8, sp: 9 });
  assert.strictEqual(s.onHand, 50);
  assert.ok(['glory', 'despair', 'train'].includes(advise(s, settings, solver).press));
  // the rate unread, off the opening and off the moves entered: the last known, marked to check
  s = play(s, settings, { type: 'read', reading: { kind: 'board', screen: { ...screen, leaves: { count: 7, contiguous: true } }, tier: null } });
  let a = advise(s, settings, solver);
  assert.deepStrictEqual([s.attempt.tier, a.check], [2, ['tier']]);
  assert.ok(a.press);
  s = play(s, settings, { type: 'set', tier: 1 });
  assert.deepStrictEqual([s.attempt.tier, advise(s, settings, solver).check], [1, undefined]);
  // a board at another level than the one set: the level read off it, the attempt with it - marked to check
  const other = play(s, settings, { type: 'read', reading: { kind: 'board', screen: { ...screen, glory: cells(2, ['success', 'empty'], 8), despair: cells(1, ['fail', 'empty'], 8) }, tier: 2 } });
  assert.deepStrictEqual([other.level, other.attempt.level, other.levelFits, advise(other, settings, solver).check], [15, 15, [12, 13, 14, 15], ['level']]);
  // the results screen: its cards
  s = play(s, settings, { type: 'read', reading: { kind: 'compare', current: { glory: 7, despair: 3 }, fresh: { glory: 8, despair: 2 } } });
  a = advise(s, settings, solver);
  assert.strictEqual(a.press, 'replace');
  assert.deepStrictEqual(a.compare.rows[0], { label: 'Steps off 9/0', current: '5', fresh: '3', better: 'fresh' });
  // Hero's Legacy: the attempt is over, the memory as the game has it
  s = play(s, settings, legacy(8, 2, 50));
  assert.deepStrictEqual([s.view, s.attempt, s.memory], ['legacy', null, { glory: 8, despair: 2 }]);
});

test('a relic never inherited: Replace, with a word that the game may have done it already', () => {
  const settings = single({ kind: 'max' });
  let s = play(blank(), settings, { type: 'set', level: 19 }, legacy(0, 0, 30), { type: 'press', button: 'inheritance' },
    { type: 'set', st: { gf: 9, gs: 6, df: 9, ds: 2, ms: 0, sp: 1 } }, { type: 'press', button: 'complete' });
  const a = advise(s, settings, solver);
  assert.strictEqual(a.press, 'replace');
  assert.match(a.detail, /no memory yet/);
  s = play(s, settings, { type: 'press', button: 'replace' });
  assert.deepStrictEqual([s.view, s.memory], ['legacy', { glory: 6, despair: 2 }]);
});

test('the cards on the results screen put right: the keep rule goes by them', () => {
  const settings = single({ kind: 'close', glory: 9, despair: 0 });
  let s = play(blank(), settings, { type: 'set', level: 19 }, legacy(7, 3, 60),
    { type: 'read', reading: { kind: 'compare', current: { glory: 7, despair: 3 }, fresh: { glory: 8, despair: 2 } } });
  assert.strictEqual(advise(s, settings, solver).press, 'replace');
  s = play(s, settings, { type: 'set', fresh: { glory: 6, despair: 2 } });
  assert.deepStrictEqual([s.result.fresh, s.attempt.st.gs, advise(s, settings, solver).press], [{ glory: 6, despair: 2 }, 6, 'keep']);
  s = play(s, settings, { type: 'set', memory: { glory: 5, despair: 3 } });
  assert.deepStrictEqual([s.result.current, advise(s, settings, solver).press], [{ glory: 5, despair: 3 }, 'replace']);
});

test('a target: the move table names its numbers, and every move scored on them', () => {
  const settings = single({ kind: 'target', glory: 8, despair: 2 });
  const s = play(blank(), settings, { type: 'set', level: 18 }, legacy(7, 3, 30), { type: 'press', button: 'inheritance' });
  const a = advise(s, settings, solver);
  assert.deepStrictEqual(a.criteria.map((c) => c.key), ['score', 'then', 'near', 'last']);
  assert.strictEqual(a.criteria[0].label, 'Chance of 8+ glory and 2 or less despair');
  for (const m of a.moves) {
    const sum = m.wipe + m.results.reduce((x, r) => x + r.p, 0);
    assert.ok(Math.abs(sum - 1) < 1e-9);
    // the chance of the target, summed off the move's results, is its score
    const hit = m.results.filter((r) => r.glory >= 8 && r.despair <= 2).reduce((x, r) => x + r.p, 0);
    assert.ok(Math.abs(hit - m.value.score) < 1e-9, `${m.action}`);
  }
});

test('a relic picked: named when it was not known, switched to as the board has it, the attempt going with it', () => {
  let s = play(blank(), SMART, levels(18, 0.5), counts());
  const plan = solver.advise(SMART.goal, 18, BOARD, STOCK, 0.5);
  // a board straight from the main page: the plan's relic, its 10 spent
  const cells = (n, v) => [...Array(n).fill(v[0]), ...Array(9 - n).fill(v[1]), 'none'];
  const screen = { glory: cells(1, ['success', 'empty']), despair: cells(0, ['fail', 'empty']), leaves: { count: 9, contiguous: true }, fill: 1, present: { train: true, glory: true, despair: true } };
  s = play(s, SMART, { type: 'read', reading: { kind: 'board', screen, tier: 1 } });
  assert.deepStrictEqual([s.relic, s.stock[plan.relic], s.attempt.by], [plan.relic, STOCK[plan.relic] - 10, 'plan']);
  // it was Golden Star: the 10 come back, Golden Star pays, and it plays against Golden Star's memory
  s = play(s, SMART, { type: 'set', relic: 5 });
  assert.deepStrictEqual([s.relic, s.stock[plan.relic], s.stock[5], s.attempt.from], [5, STOCK[plan.relic], STOCK[5] - 10, { glory: 7, despair: 1 }]);
  assert.deepStrictEqual(s.attempt.st, { gf: 1, gs: 1, df: 0, ds: 0, ms: 9, sp: 10 }); // the board as it was

  // Hero's Legacy that does not fit the relic open: which relic, asked; named, it takes what was read
  s = play(blank(), SMART, levels(18, 0.5), counts(), { type: 'tile', relic: 2 }, legacy(9, 1, 40));
  assert.strictEqual(s.relic, null);
  assert.deepStrictEqual(advise(s, SMART, solver).needs.map((n) => n.what), ['relic']);
  s = play(s, SMART, { type: 'set', relic: 9 });
  assert.deepStrictEqual([s.states[9], s.stock[9], s.states[2], s.stock[2]], [[9, 1], 40, BOARD[2], STOCK[2]]);
  // switched to another relic: its own numbers
  s = play(s, SMART, { type: 'set', relic: 0 });
  assert.deepStrictEqual([s.memory, s.onHand], [{ glory: 7, despair: 1 }, STOCK[0]]);
});

test('the inheritor level, read off an attempt: kept if it fits, else the highest that does, to check', () => {
  const settings = single({ kind: 'max' });
  const cells = (n, v) => [...Array(n).fill(v[0]), ...Array(9 - n).fill(v[1]), 'none'];
  // part-way through a 9-slot attempt (test/images/board-l19.png): levels 16-19 all fit
  const screen = { glory: cells(2, ['success', 'empty']), despair: cells(1, ['fail', 'empty']), leaves: { count: 8, contiguous: true }, fill: 0.9, present: { train: true, glory: true, despair: true } };
  const board = { type: 'read', reading: { kind: 'board', screen, tier: 2 } };
  let s = play(blank(), settings, legacy(7, 1, 40), board);
  let a = advise(s, settings, solver);
  assert.deepStrictEqual([s.level, a.check, a.levelFits], [19, ['level'], [16, 17, 18, 19]]);
  assert.ok(['glory', 'despair', 'train'].includes(a.press));
  // put right: sure from then on
  s = play(s, settings, { type: 'set', level: 17 });
  a = advise(s, settings, solver);
  assert.deepStrictEqual([s.level, s.attempt.level, a.check, a.levelFits], [17, 17, undefined, undefined]);
  // a level the board fits is kept as it is
  s = play(blank(), settings, { type: 'set', level: 16 }, legacy(7, 1, 40), board);
  assert.deepStrictEqual([s.level, advise(s, settings, solver).check], [16, undefined]);
  // an opening board tells it exactly: level 19's free despair fail
  const opening = { glory: cells(1, ['success', 'empty']), despair: cells(1, ['fail', 'empty']), leaves: { count: 9, contiguous: true }, fill: 1, present: { train: true, glory: true, despair: true } };
  s = play(blank(), settings, legacy(7, 1, 40), { type: 'read', reading: { kind: 'board', screen: opening, tier: 0 } });
  assert.deepStrictEqual([s.level, advise(s, settings, solver).check], [19, undefined]);
});

test('a state the game cannot be in is refused, or the level raised to fit it', () => {
  const settings = single({ kind: 'max' });
  // lowering the level under an attempt that has filled more slots than it has: refused, nothing changed
  let s = play(blank(), settings, { type: 'set', level: 19 }, legacy(7, 1, 50), { type: 'press', button: 'inheritance' },
    { type: 'set', st: { gf: 9, gs: 5, df: 3, ds: 1, ms: 4, sp: 3 } });
  assert.throws(() => act(s, { type: 'set', level: 12 }, settings, solver), /does not fit level 12/);
  // a memory more than the level's bars hold: refused
  s = play(blank(), settings, { type: 'set', level: 5 });
  assert.throws(() => act(s, { type: 'set', memory: { glory: 7, despair: 1 } }, settings, solver), /level 5 has 6 slots/);
  // Hero's Legacy showing 8-slot bars at level 1: the level is at least 12, set to 15 and marked to check
  s = play(blank(), settings, { type: 'read', reading: { kind: 'legacy', memory: { glory: 7, despair: 1 }, slots: 8, stock: 50 } });
  const a = advise(s, settings, solver);
  assert.deepStrictEqual([s.level, a.check, a.levelFits], [15, ['level'], [12, 13, 14, 15]]);
  // fewer slots than the level has: the level stands (the slots were expanded since)
  s = play(blank(), settings, { type: 'set', level: 19 }, { type: 'read', reading: { kind: 'legacy', memory: { glory: 7, despair: 1 }, slots: 8, stock: 50 } });
  assert.deepStrictEqual([s.level, advise(s, settings, solver).check], [19, undefined]);
});

test('Single Relic: the settings a memory already meets move just past it, and only those', () => {
  const DEFAULT = { mark: null, glory: 7, despair: 2, closeGlory: 9, closeDespair: 0 };
  // 8/1 at level 18 (+38%): past the blank mark and the 7/2 target, short of the 9/0 limit
  const set = singleRelicSettings(solver, 18, { glory: 8, despair: 1 }, DEFAULT);
  assert.strictEqual(set.mark, 39);
  // one glory or one despair better, whichever an attempt is likelier to reach
  const chance = (t) => {
    solver.configure(18, { kind: 'target', ...t }, { glory: 8, despair: 1 });
    return solver.expected(opening(solver, 18).st, opening(solver, 18).tier);
  };
  const [up, down] = [chance({ glory: 9, despair: 1 }), chance({ glory: 8, despair: 0 })];
  assert.deepStrictEqual([set.glory, set.despair], up >= down ? [9, 1] : [8, 0]);
  assert.ok(!('closeGlory' in set) && !('closeDespair' in set));
  assert.match(set.said, /already met them: set Minimum Useful Amplification 39%; Glory Target \d, Despair Target \d\./);
  // a memory short of every setting changes none of them
  assert.deepStrictEqual(singleRelicSettings(solver, 18, { glory: 3, despair: 4 }, { ...DEFAULT, mark: 20 }), { said: '' });
  // nothing better than 9/0 on nine slots: the settings stay
  assert.deepStrictEqual(singleRelicSettings(solver, 18, { glory: 9, despair: 0 }, { ...DEFAULT, mark: 45, closeDespair: 0 }), { said: '' });
  // a limit met: every slot a glory, no despair (level 20: ten slots)
  const top = singleRelicSettings(solver, 20, { glory: 8, despair: 1 }, { mark: 50, glory: 10, despair: 0, closeGlory: 8, closeDespair: 1 });
  assert.deepStrictEqual({ ...top, said: undefined }, { closeGlory: 10, closeDespair: 0, said: undefined });
});

test('a blank Minimum Useful Amplification is a mark of 0, and nothing more', () => {
  const at = (settings, glory, despair) => play(blank(), settings, { type: 'set', level: 19 }, legacy(glory, despair, 50), { type: 'press', button: 'inheritance' },
    { type: 'set', st: { gf: 4, gs: 3, df: 2, ds: 0, ms: 8, sp: 6 }, tier: 2 });
  const blankMark = single({ kind: 'max' }), zero = single({ kind: 'max', mark: 0 });
  // blank and 0 are the same, memory or not
  for (const [g, d] of [[0, 0], [8, 1]]) {
    const a = advise(at(blankMark, g, d), blankMark, solver), b = advise(at(zero, g, d), zero, solver);
    assert.deepStrictEqual([a.press, a.criteria, a.moves.map((m) => m.value)], [b.press, b.criteria, b.moves.map((m) => m.value)]);
  }
  // nothing to beat (+0%): no mark at all - the most amplification, expected
  assert.strictEqual(advise(at(blankMark, 0, 0), blankMark, solver).criteria[0].label, 'Expected amplification');
  // a memory to beat: raised to beat it, as any mark under it is
  const over = advise(at(blankMark, 8, 1), blankMark, solver).criteria[0];
  assert.deepStrictEqual([over.label, over.kept], ['Chance of improving the memory (+39% or more)', true]);
  // the settings stay as they were set: nothing is filled in
  assert.strictEqual(blankMark.objective.mark, undefined);
  // and every option is offered
  assert.strictEqual(advise(at(blankMark, 8, 1), blankMark, solver).moves.length, 3);
});

test('a score is shown as what the solver played for: a chance is a chance, an amplification an amplification', () => {
  // the 2743% bug: the engine played for amplification while the label said a chance
  for (const mark of [undefined, 0, 1, 34, 46]) {
    for (const [g, d] of [[0, 0], [1, 2], [7, 1]]) {
      const settings = single({ kind: 'max', ...(mark === undefined ? {} : { mark }) });
      const s = play(blank(), settings, { type: 'set', level: 19 }, legacy(g, d, 50), { type: 'press', button: 'inheritance' },
        { type: 'set', st: { gf: 4, gs: 3, df: 2, ds: 1, ms: 8, sp: 6 }, tier: 4 });
      const a = advise(s, settings, solver);
      const main = a.criteria[0];
      for (const m of a.moves) {
        const v = m.value.score;
        if (main.kind === 'chance') assert.ok(v >= 0 && v <= 1 + 1e-9, `mark ${mark}, ${g}/${d}: ${main.label} ${v}`);
        else assert.ok(main.kind === 'amp' && v >= 0 && v <= 50, `mark ${mark}, ${g}/${d}: ${main.label} ${v}`);
      }
    }
  }
});
