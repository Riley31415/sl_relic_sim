// The run loop against a simulated game: screens as classify() would
// report them, clicks mapped back onto the game's buttons.
import assert from 'assert';

import { test } from './harness.mjs';
import { Runner } from '../chrome/lib/runner.js';
import { REF, at, isForbidden } from '../chrome/lib/vision.js';
import { nextTier } from '../chrome/lib/logic.js';

const ANCHOR = { x0: 100, y0: 300, s: 1 };
const pt = (rx, ry) => {
  const [x, y] = at(ANCHOR, rx, ry);
  return { x, y };
};
const BUTTONS = Object.fromEntries(Object.entries(REF.buttons).map(([k, b]) => [k, pt(b.x, b.y)]));
const GOLD = pt(200, 270);
const KEEP = pt(100, 120), REPLACE = pt(300, 120);
const ABANDON = pt(34.5, 232.5); // inside REF.forbidden: only for a wiped attempt
const CONFIRM = pt(135, 102), CANCEL = pt(263, 102);

// the solver's table for the 9-slot levels: 16-18 open with a free glory
// success at 65%, 19 adds a free despair fail and opens at 80%
function boardFor(level) {
  const base = { level, slots: 9, maxSpirit: 10, startGlory: 1, startDespairFail: 0, startTier: 1 };
  if (level === 19) return { ...base, startDespairFail: 1, startTier: 0 };
  if (level >= 16 && level <= 18) return base;
  return { ...base, slots: level < 16 ? 8 : 10 };
}

class FakeGame {
  /**
   * roll(action, state) -> success?; current: the applied memory;
   * readable: whether the rate on the buttons can be read;
   * spLag: looks for which the spirit bar still shows the old value after a
   * training success (the bar animates); cards: whether the Compare cards read
   */
  constructor({ roll, current, screen = 'legacy', level = 18, readable = true, spLag = 0, cards = true }) {
    Object.assign(this, { roll, current, kind: screen, readable, spLag, cards, opening: boardFor(level) });
    this.clicks = [];
    this.lag = 0;
    if (screen === 'board') this.newAttempt();
  }

  newAttempt() {
    const o = this.opening;
    this.st = { gf: o.startGlory, gs: o.startGlory, df: o.startDespairFail, ds: 0, ms: o.slots, sp: o.maxSpirit };
    this.tier = o.startTier;
    this.kind = 'board';
  }

  cells(filled, success) {
    return Array.from({ length: 10 }, (_, i) => (i >= 9 ? 'none' : i >= filled ? 'empty' : i < success ? 'success' : 'fail'));
  }

  memoryOf(g, d, slots = 9) {
    const bar = (s) => Array.from({ length: slots }, (_, i) => (i < s ? 'success' : 'fail'));
    return { glory: { slots, filled: slots, success: g, bar: bar(g) }, despair: { slots, filled: slots, success: d, bar: bar(d) } };
  }

  /** Like the game: a button goes once it cannot be used. */
  present() {
    const { st } = this;
    return { train: st.ms > 0, glory: st.sp > 0 && st.gf < 9, despair: st.sp > 0 && st.df < 9 };
  }

  screen() {
    const st = this.st;
    switch (this.kind) {
      case 'board': {
        let sp = st.sp;
        if (this.lag > 0) {
          this.lag--;
          sp = this.lagSp;
        }
        return {
          kind: 'board', anchor: ANCHOR,
          glory: this.cells(st.gf, st.gs), despair: this.cells(st.df, st.ds),
          leaves: { count: st.ms, contiguous: true }, fill: sp / 10,
          present: this.present(), rates: {}, rateTier: this.readable ? this.tier : null, buttons: BUTTONS,
          abandon: ABANDON,
        };
      }
      case 'complete':
        return { kind: 'complete', anchor: ANCHOR, memory: this.memoryOf(st.gs, st.ds), button: GOLD };
      case 'compare': {
        const read = (m) => (this.cards ? { glory: m.glory, despair: m.despair } : null);
        return {
          kind: 'compare', anchor: ANCHOR, buttons: { keep: KEEP, replace: REPLACE },
          cards: { current: read(this.current), fresh: read({ glory: st.gs, despair: st.ds }) },
        };
      }
      case 'abandon':
        return { kind: 'abandon', anchor: ANCHOR, buttons: { confirm: CONFIRM, cancel: CANCEL } };
      case 'legacy':
        return { kind: 'legacy', anchor: ANCHOR, memory: this.memoryOf(this.current.glory, this.current.despair, 8), button: GOLD };
      default:
        return { kind: 'unknown', anchor: null };
    }
  }

  click(x, y) {
    this.clicks.push({ x, y, at: Date.now() });
    const near = (p) => Math.hypot(p.x - x, p.y - y) < 5;
    const st = this.st;
    // (the game shows Abandon Inheritance throughout an attempt)
    const abandon = this.kind === 'board' && near(ABANDON);
    if (isForbidden(ANCHOR, x, y) && !abandon) throw new Error('clicked a forbidden spot');
    if (abandon) this.kind = 'abandon';
    else if (this.kind === 'abandon') {
      if (near(CONFIRM)) this.kind = 'legacy'; // the applied memory stays
      else if (near(CANCEL)) this.kind = 'board';
    } else if (this.kind === 'board') {
      const action = Object.keys(BUTTONS).find((k) => near(BUTTONS[k]));
      const ok = this.roll(action, { ...st, tier: this.tier });
      if (action === 'train') {
        st.ms--;
        if (ok) {
          [this.lag, this.lagSp] = [this.spLag, st.sp];
          st.sp = Math.min(10, st.sp + 2);
        }
      } else {
        const [f, s] = action === 'glory' ? ['gf', 'gs'] : ['df', 'ds'];
        st[f]++;
        st.sp--;
        if (ok) st[s]++;
      }
      this.tier = nextTier(this.tier, ok);
      if (st.gf === 9 && st.df === 9) this.kind = 'complete';
    } else if (this.kind === 'complete' && near(GOLD)) this.kind = 'compare';
    else if (this.kind === 'compare' && (near(KEEP) || near(REPLACE))) {
      if (near(REPLACE)) this.current = { glory: st.gs, despair: st.ds };
      this.kind = 'legacy';
    } else if (this.kind === 'legacy' && near(GOLD)) this.newAttempt();
  }
}

// a plain policy: train when out of fuel (or full of it on a bad rate), else fill glory, then despair
function fakeSolver(game) {
  return {
    board: boardFor,
    configure() {},
    seen: [],
    best(st, tier) {
      this.seen.push({ tier, truth: game.tier });
      if (st.gf === 9 && st.df === 9) return 'done';
      if ((st.sp === 0 || (tier >= 3 && st.sp === 10)) && st.ms > 0) return 'train';
      if (st.sp === 0) return 'deadend';
      return st.gf < 9 ? 'glory' : 'despair';
    },
    expected: () => 0,
    finishChance: () => 1,
  };
}

function setup({ game, attempts = 3, level = 18, answers = [], clickDelay, timing = { poll: 0, settle: 0, gap: [0, 0] } }) {
  const logs = [];
  const asked = [];
  const solver = fakeSolver(game);
  const runner = new Runner({
    tab: { capture: async () => ({ img: null, ratio: 1, png: '' }), click: async (x, y) => game.click(x, y) },
    solver,
    settings: { level, objective: { kind: 'max' }, attempts, clickMethod: 'mouse', clickDelay },
    ui: {
      log: (text, level) => logs.push({ text, level }),
      show() {},
      askTier: async (c) => {
        asked.push(c);
        return answers.shift() ?? null;
      },
    },
    timing,
    // like classify(): a popup is only found when looked for - Compare at
    // the last anchor (or anywhere, if its cards both read), the abandon
    // confirmation only after Abandon Inheritance
    see: (img, lastAnchor, anywhere, popup) => {
      const screen = game.screen();
      const unknown = { kind: 'unknown', anchor: null };
      if (screen.kind === 'compare') {
        const sought = lastAnchor && popup === 'compare';
        return sought || (anywhere && screen.cards.current && screen.cards.fresh) ? screen : unknown;
      }
      if (screen.kind === 'abandon') return lastAnchor && popup === 'abandon' ? screen : unknown;
      return screen;
    },
    read: (screen) => (screen.rateTier == null ? null : { tier: screen.rateTier }),
  });
  const text = () => logs.map((l) => l.text).join('\n');
  return { runner, logs, asked, solver, text, last: () => logs[logs.length - 1] };
}

/** Every decision was made at the rate the game really had. */
function rightRates(solver) {
  const wrong = solver.seen.filter((s) => s.tier !== s.truth);
  assert.deepStrictEqual(wrong, [], 'decisions made at the wrong rate');
}

// glory always succeeds, despair always fails, training always succeeds
const lucky = (action) => action !== 'despair';
// glory always fails, despair always succeeds
const unlucky = (action) => action === 'train';

test('a better memory is applied and the run stops', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 3, despair: 4 } });
  const { runner, last, solver } = setup({ game });
  await runner.run();
  assert.match(last().text, /Found a better memory and applied it: 9 glory, 0 despair/);
  assert.deepStrictEqual(game.current, { glory: 9, despair: 0 });
  assert.strictEqual(game.kind, 'legacy');
  assert.strictEqual(runner.finished, 1);
  rightRates(solver);
});

test('a worse memory is kept, and the attempt limit stops the run', async () => {
  const game = new FakeGame({ roll: unlucky, current: { glory: 8, despair: 0 } });
  const { runner, last } = setup({ game, attempts: 2 });
  await runner.run();
  assert.match(last().text, /Done: 2 attempt\(s\), current memory 8 glory, 0 despair/);
  assert.deepStrictEqual(game.current, { glory: 8, despair: 0 });
});

test('the log: one line a move, in words', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 0 }, screen: 'board' });
  const { runner, text } = setup({ game, attempts: 1 });
  await runner.run();
  assert.match(text(), /^Glory at 65%: success {2}-> {2}glory 2✓ 0✗ · despair 0✓ 0✗ · spirit 9 · mental 9, next 50%$/m);
  assert.doesNotMatch(text(), /click:|tracking/);
});

test('the wrong level is caught on the opening board', async () => {
  // a level-18 game with the panel set to 19 (no free despair fail)
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, level: 18 });
  const { runner, last } = setup({ game, level: 19 });
  await runner.run();
  assert.match(last().text, /opens with 1 free glory success and 0 free despair fails on 9 slots - that is inheritor level 16, 17 or 18, but the level is set to 19/);
  assert.strictEqual(game.clicks.length, 1); // Inheritance, then nothing
});

test('a level-19 game is fine at level 19', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, level: 19 });
  const { runner, last, solver } = setup({ game, level: 19 });
  await runner.run();
  assert.match(last().text, /Found a better memory/);
  rightRates(solver);
});

test('the rate comes off the buttons when tracking cannot know it', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, screen: 'board' });
  game.st = { gf: 3, gs: 3, df: 1, ds: 0, ms: 9, sp: 8 };
  game.tier = 2;
  const { runner, asked, text, solver } = setup({ game, attempts: 1 });
  await runner.run();
  assert.deepStrictEqual(asked, []);
  assert.match(text(), /Picking up an attempt in progress: glory 3✓ 0✗ · despair 0✓ 1✗ · spirit 8 · mental 9, rate 50%/);
  rightRates(solver);
});

test('unreadable buttons mid-attempt: ask once, then track', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, screen: 'board', readable: false });
  game.st = { gf: 3, gs: 3, df: 1, ds: 0, ms: 9, sp: 8 };
  game.tier = 2;
  const { runner, asked, solver } = setup({ game, answers: [2], attempts: 1 });
  await runner.run();
  assert.deepStrictEqual(asked, [[0, 1, 2, 3, 4]]);
  rightRates(solver);
});

test('training at full spirit power is settled by the buttons', async () => {
  const game = new FakeGame({ roll: (a) => a !== 'despair', current: { glory: 9, despair: 9 }, screen: 'board' });
  game.st = { gf: 4, gs: 4, df: 1, ds: 0, ms: 8, sp: 10 }; // a training has refilled it
  game.tier = 3;
  const { runner, asked, text, solver } = setup({ game, attempts: 1 });
  await runner.run();
  assert.deepStrictEqual(asked, []);
  assert.match(text(), /^Training at 35%: success .*next 20%$/m);
  rightRates(solver);
});

test('training at full spirit power with unreadable buttons asks between the two rates', async () => {
  const game = new FakeGame({ roll: () => false, current: { glory: 9, despair: 9 }, screen: 'board', readable: false });
  game.st = { gf: 4, gs: 4, df: 1, ds: 0, ms: 8, sp: 10 }; // a training has refilled it
  game.tier = 3;
  const { runner, asked } = setup({ game, answers: [3, 2], attempts: 1 });
  await runner.run();
  assert.deepStrictEqual(asked[0], [0, 1, 2, 3, 4]);
  assert.deepStrictEqual(asked[1], [4, 2]);
});

test('a lagging spirit bar is waited out (the 4:42:31 bug)', async () => {
  // unreadable buttons, so only the board tells how a training went
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 0 }, screen: 'board', readable: false, spLag: 3 });
  game.st = { gf: 4, gs: 4, df: 0, ds: 0, ms: 9, sp: 3 };
  game.tier = 0;
  const { runner, solver, text } = setup({ game, answers: [0], attempts: 1, timing: { poll: 5, settle: 40, gap: [0, 0] } });
  await runner.run();
  assert.match(text(), /Training at .*: success/);
  rightRates(solver);
});

test('a lagging spirit bar is caught by the buttons even without the wait', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 0 }, screen: 'board', spLag: 2 });
  game.st = { gf: 4, gs: 4, df: 0, ds: 0, ms: 9, sp: 3 };
  game.tier = 0;
  const { runner, solver, text } = setup({ game, attempts: 1 });
  await runner.run();
  assert.doesNotMatch(text(), /Training at .*: fail/);
  rightRates(solver);
});

test('an unknown screen stops the run with no clicks', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, screen: 'unknown' });
  const { runner, last } = setup({ game });
  await runner.run();
  assert.match(last().text, /Not on a screen I recognise/);
  assert.strictEqual(game.clicks.length, 0);
});

test('a popup mid-attempt stops the run, saying what it saw', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, screen: 'board' });
  const click = game.click.bind(game);
  game.click = (x, y) => {
    click(x, y);
    if (game.clicks.length === 3) game.kind = 'unknown'; // e.g. the abandon confirmation
  };
  const { runner, last } = setup({ game });
  await runner.run();
  assert.match(last().text, /last thing I saw was a screen I do not recognise|Not on a screen I recognise/);
  assert.strictEqual(game.clicks.length, 3);
});

test('a two-button popup mid-attempt is not answered', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, screen: 'board' });
  const click = game.click.bind(game);
  game.click = (x, y) => {
    click(x, y);
    if (game.clicks.length === 2) game.kind = 'compare';
  };
  const { runner, last } = setup({ game });
  runner.current = { glory: 0, despair: 0 };
  runner.result = { glory: 9, despair: 0 };
  await runner.run();
  assert.strictEqual(game.clicks.length, 2);
  assert.match(last().text, /never came|Not on a screen I recognise/);
});

test('mid-attempt start: the cards say what the current memory is', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 3, despair: 4 }, screen: 'board' });
  const { runner, last, text } = setup({ game });
  await runner.run();
  assert.match(text(), /Results: new 9 glory, 0 despair \(\+45%\) vs current 3 glory, 4 despair \(\+7%\) - Replace with New Effect/);
  assert.match(last().text, /Found a better memory and applied it: 9 glory, 0 despair/);
  assert.deepStrictEqual(game.current, { glory: 9, despair: 0 });
});

test('mid-attempt start with unreadable cards: the results screen is left to you', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, screen: 'board', cards: false });
  const { runner, last } = setup({ game });
  await runner.run();
  assert.match(last().text, /Could not read the current memory card - choose Keep or Replace yourself/);
  assert.strictEqual(game.kind, 'compare');
});

test('starting on the results screen: the cards decide', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 8, despair: 0 }, screen: 'board' });
  game.st = { gf: 9, gs: 6, df: 9, ds: 3, ms: 0, sp: 0 };
  game.kind = 'compare';
  const { runner, text } = setup({ game, attempts: 1 });
  await runner.run();
  assert.match(text(), /Results: new 6 glory, 3 despair \(\+24%\) vs current 8 glory, 0 despair \(\+40%\) - Keep Current Effect/);
  assert.match(text(), /Done: 1 attempt\(s\)/);
  assert.deepStrictEqual(game.current, { glory: 8, despair: 0 });
});

test('a card that disagrees with what the run saw stops it', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 3, despair: 4 } });
  const screen = game.screen.bind(game);
  game.screen = () => {
    const s = screen();
    if (s.kind === 'compare') s.cards.current = { glory: 5, despair: 4 }; // misread
    return s;
  };
  const { runner, last } = setup({ game });
  await runner.run();
  assert.match(last().text, /The current memory card reads 5 glory, 4 despair \(\+17%\), but I saw 3 glory, 4 despair/);
  assert.strictEqual(game.kind, 'compare');
});

test('a wiped attempt is abandoned and confirmed, and the run goes on', async () => {
  // the next attempt comes out worse, so it is kept and the limit stops the run
  const game = new FakeGame({ roll: unlucky, current: { glory: 8, despair: 0 }, screen: 'board' });
  game.st = { gf: 9, gs: 7, df: 8, ds: 2, ms: 0, sp: 0 }; // board-wiped.png
  const { runner, text, last } = setup({ game, attempts: 2 });
  await runner.run();
  assert.match(text(), /Out of spirit power and mental strength with the bars unfinished \(glory 7✓ 2✗ · despair 2✓ 6✗ · spirit 0 · mental 0\) - abandoning the attempt/);
  assert.match(text(), /Abandoned - the applied memory is unchanged \(8 glory, 0 despair/);
  assert.match(text(), /Clicking Inheritance \(attempt 2 of 2\)/);
  assert.match(last().text, /Done: 2 attempt\(s\)/);
  assert.ok(game.clicks.some((c) => Math.hypot(c.x - CONFIRM.x, c.y - CONFIRM.y) < 5));
});

test('a board that reads wiped but still shows a button is not abandoned', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 8, despair: 0 }, screen: 'board' });
  game.st = { gf: 9, gs: 7, df: 8, ds: 2, ms: 0, sp: 0 };
  game.present = () => ({ train: false, glory: false, despair: true }); // e.g. spirit misread as 0
  const { runner, last } = setup({ game });
  await runner.run();
  assert.match(last().text, /reads as wiped .* but an Attempt button is showing - stopped/);
  assert.strictEqual(game.clicks.length, 0);
});

test('the abandon confirmation is never answered unless the run opened it', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, screen: 'board' });
  const click = game.click.bind(game);
  game.click = (x, y) => {
    click(x, y);
    if (game.clicks.length === 2) game.kind = 'abandon'; // you pressed Abandon mid-run
  };
  const { runner, last } = setup({ game });
  await runner.run();
  assert.strictEqual(game.clicks.length, 2);
  assert.strictEqual(game.kind, 'abandon');
  assert.match(last().text, /never came|Not on a screen I recognise/);
});

test('clicks are spaced by the random gap', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 3, despair: 4 } });
  const { runner } = setup({ game, attempts: 1, timing: { poll: 0, settle: 0, gap: [40, 60] } });
  await runner.run();
  assert.ok(game.clicks.length > 5);
  for (let i = 1; i < game.clicks.length; i++) {
    const dt = game.clicks[i].at - game.clicks[i - 1].at;
    assert.ok(dt >= 38, `gap ${dt} ms`);
  }
});

test('a wrong change after a click stops the run', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 0, despair: 0 }, screen: 'board' });
  const click = game.click.bind(game);
  game.click = (x, y) => {
    click(x, y);
    game.st.ms--; // something else moved too
  };
  const { runner, last } = setup({ game });
  await runner.run();
  assert.match(last().text, /not what the click should do/);
  assert.strictEqual(game.clicks.length, 1);
});

test('every wipe is abandoned, up to the attempt limit', async () => {
  // training always fails: every attempt runs dry
  const game = new FakeGame({ roll: (a) => a === 'glory', current: { glory: 0, despair: 0 } });
  const { runner, last, text } = setup({ game, attempts: 3 });
  await runner.run();
  assert.strictEqual(text().match(/abandoning the attempt/g).length, 3);
  assert.match(last().text, /Done: 3 attempt\(s\)/);
});

test('the training button can be gone at 0 mental strength', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 0 }, screen: 'board' });
  game.st = { gf: 5, gs: 5, df: 4, ds: 0, ms: 0, sp: 9 };
  game.tier = 1;
  const { runner, text } = setup({ game, attempts: 1 });
  await runner.run();
  assert.match(text(), /Attempt finished: 9 glory/);
});

/** Clicks reach the game `delay` ms late (the first `count` of them). */
function laggy(game, delay, count) {
  const click = game.click.bind(game);
  const queue = [];
  let n = 0;
  game.click = (x, y) => {
    if (n++ < count) queue.push({ x, y, due: Date.now() + delay });
    else click(x, y);
  };
  const see = game.screen.bind(game);
  game.screen = () => {
    while (queue.length && Date.now() >= queue[0].due) {
      const { x, y } = queue.shift();
      click(x, y);
    }
    return see();
  };
}

function quickWaits(runner) {
  const waitFor = runner.waitFor.bind(runner);
  runner.waitFor = (test, ms) => waitFor(test, ms === 9000 ? 60 : ms);
  runner.timing = { poll: 5, settle: 0, gap: [0, 0] };
}

test('a missed click is tried once more', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 0 }, screen: 'board' });
  laggy(game, Infinity, 1); // the first click never arrives
  const { runner, text } = setup({ game, attempts: 1 });
  quickWaits(runner);
  await runner.run();
  assert.match(text(), /click did not register - trying once more/);
  assert.match(text(), /^Glory at 65%: success/m);
  assert.match(text(), /Attempt finished/);
});

test('a response that lands after the retry stops the run', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 0 }, screen: 'board' });
  laggy(game, 100, 2); // both the click and its retry land, late
  const { runner, last } = setup({ game, attempts: 1 });
  quickWaits(runner);
  await runner.run();
  assert.match(last().text, /not what the click should do|never came/);
});

test('the click delay setting spaces clicks +/- 0.1 s around it', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 0 }, screen: 'board' });
  game.st = { gf: 8, gs: 8, df: 8, ds: 0, ms: 5, sp: 5 }; // two moves left
  const { runner } = setup({ game, attempts: 1, clickDelay: 0.15 });
  assert.deepStrictEqual(runner.timing.gap.map(Math.round), [50, 250]);
  await runner.run();
  assert.ok(game.clicks.length >= 2);
  for (let i = 1; i < game.clicks.length; i++) {
    const dt = game.clicks[i].at - game.clicks[i - 1].at;
    assert.ok(dt >= 48 && dt <= 400, `gap ${dt} ms`);
  }
  // and without the setting, the 1.4-1.6 s default
  const plain = new Runner({ tab: {}, solver: fakeSolver(game), settings: { level: 18, objective: { kind: 'max' } }, ui: {} });
  assert.deepStrictEqual(plain.timing.gap, [1400, 1600]);
});

test('a settled target is said once, and left to the solver', async () => {
  // one glory slot left; the box can no longer be hit (the solver's
  // tie-break then plays for amplification - tests/solver.rs)
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 9 }, screen: 'board' });
  game.st = { gf: 7, gs: 4, df: 9, ds: 3, ms: 3, sp: 4 };
  game.tier = 3;
  const configured = [];
  const solver = {
    board: boardFor,
    configure: (level, objective) => configured.push(objective.kind),
    best: (st) => (st.gf === 9 && st.df === 9 ? 'done' : 'glory'),
    expected: () => 0,
    finishChance: () => 1,
  };
  const logs = [];
  const runner = new Runner({
    tab: { capture: async () => ({ img: null, ratio: 1, png: '' }), click: async (x, y) => game.click(x, y) },
    solver,
    settings: { level: 18, objective: { kind: 'target', glory: 8, despair: 2 }, attempts: 1, clickMethod: 'mouse' },
    ui: { log: (text) => logs.push(text), show() {}, askTier: async () => null },
    timing: { poll: 0, settle: 0, gap: [0, 0] },
    see: () => game.screen(),
    read: (screen) => (screen.rateTier == null ? null : { tier: screen.rateTier }),
  });
  await runner.run();
  const said = logs.filter((l) => /The target is out of reach for this attempt - playing the rest of it for the result nearest to it still possible/.test(l));
  assert.strictEqual(said.length, 1); // two moves played, said once
  assert.deepStrictEqual(configured, ['target']); // never reconfigured
});

test('an attempt nothing useful can come of is abandoned at once', async () => {
  // the applied memory is already 9/0 (+45%): no result can replace it
  const game = new FakeGame({ roll: lucky, current: { glory: 9, despair: 0 } });
  const { runner, text } = setup({ game, attempts: 1 });
  await runner.run();
  assert.match(text(), /Nothing useful left in this attempt: even the best result still possible \(9 glory, 0 despair \(\+45%\)\) would not replace 9 glory, 0 despair \(\+45%\) - abandoning it/);
  assert.match(text(), /Abandoned - the applied memory is unchanged/);
  assert.match(text(), /Done: 1 attempt\(s\)/);
  assert.strictEqual(game.clicks.length, 3); // Inheritance, Abandon, Confirm
});

test('an attempt that turns hopeless part-way is abandoned then', async () => {
  // current 8/0 (+40%); every glory roll fails: once even a full glory bar
  // from here cannot pass +40%, it stops playing
  const game = new FakeGame({ roll: unlucky, current: { glory: 8, despair: 0 } });
  const { runner, text } = setup({ game, attempts: 1 });
  await runner.run();
  assert.match(text(), /^Glory at .*: fail/m); // it played a while
  assert.match(text(), /Nothing useful left in this attempt/);
  assert.ok(game.st.gf < 9, `abandoned with glory ${game.st.gf}/9 filled`);
});

test('while a better result is still possible, the attempt is played', async () => {
  const game = new FakeGame({ roll: lucky, current: { glory: 8, despair: 1 } });
  const { runner, text } = setup({ game, attempts: 1 });
  await runner.run();
  assert.doesNotMatch(text(), /Nothing useful left/);
  assert.match(text(), /Found a better memory and applied it: 9 glory, 0 despair/);
});
