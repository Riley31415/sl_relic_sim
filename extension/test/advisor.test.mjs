// The advisor against a simulated game: the main page's two views, each
// relic's Hero's Legacy, and attempts that end on the results screen.
import assert from 'assert';

import { test } from '../../advisor/js/test/harness.mjs';
import { Advisor, RELICS } from '../chrome/lib/advisor.js';
import { REF, at } from '../chrome/lib/vision.js';

const ANCHOR = { x0: 100, y0: 300, s: 1 };
const pt = (rx, ry) => {
  const [x, y] = at(ANCHOR, rx, ry);
  return { x, y };
};
const BUTTONS = Object.fromEntries(Object.entries(REF.buttons).map(([k, b]) => [k, pt(b.x, b.y)]));
const GOLD = pt(200, 270);
const KEEP = pt(100, 120), REPLACE = pt(300, 120);
const TILES = RELICS.map((_, i) => ({ x: 1000 + 30 * (i % 3), y: 60 + 30 * Math.floor(i / 3) }));
const TOGGLE = { x: 1500, y: 40 }, BACK = { x: 1600, y: 40 };
const near = (p, x, y) => Math.hypot(p.x - x, p.y - y) < 5;

/** board: 12 [glory, despair]; stock: 12 counts; pity in points of 9000; roll(): the attempt's result */
class Game {
  /**
   * badges: the counts view's stock badges as read (null: unreadable), or none at all;
   * levelUp(game): whether the main page shows the Level Up button
   */
  constructor({ board, stock, level = 18, pity = 0, roll = () => ({ glory: 9, despair: 0 }), screen = 'levels', badges = null, levelUp = () => false }) {
    Object.assign(this, { board: board.map((x) => [...x]), stock: [...stock], level, pity, roll, kind: screen, badges, levelUp });
    this.at = null;
    this.clicks = [];
    this.attempts = 0;
  }

  memoryOf([g, d]) {
    const bar = (s) => Array.from({ length: 9 }, (_, i) => (i < s ? 'success' : 'fail'));
    return { glory: { slots: 9, filled: 9, success: g, bar: bar(g) }, despair: { slots: 9, filled: 9, success: d, bar: bar(d) } };
  }

  screen() {
    const main = { kind: 'main', anchor: null, tiles: TILES, toggle: TOGGLE, banner: { x0: 0, y0: 0, s: 1 }, levelUp: this.levelUp(this) };
    switch (this.kind) {
      case 'levels':
        return { ...main, view: 'levels', level: this.level, pity: this.pity / 9000 };
      case 'counts': {
        const relics = this.board.map(([glory, despair], i) => ({ glory, despair, stock: this.badges ? this.badges(i, this.stock[i]) : null }));
        const totals = { glory: relics.reduce((a, r) => a + r.glory, 0), despair: relics.reduce((a, r) => a + r.despair, 0) };
        return { ...main, view: 'counts', relics, totals, ok: true };
      }
      case 'legacy':
        return { kind: 'legacy', anchor: ANCHOR, memory: this.memoryOf(this.board[this.at]), button: GOLD, stock: this.stock[this.at], back: BACK };
      case 'board': // one move from the end: the last despair slot
        return {
          kind: 'board', anchor: ANCHOR,
          glory: [...Array(9).fill('success'), 'none'], despair: [...Array(8).fill('fail'), 'empty', 'none'],
          leaves: { count: 0, contiguous: true }, fill: 0.1,
          present: { train: false, glory: false, despair: true }, rates: {}, rateTier: 1, buttons: BUTTONS, abandon: null,
        };
      case 'complete':
        return { kind: 'complete', anchor: ANCHOR, memory: this.memoryOf([this.result.glory, this.result.despair]), button: GOLD };
      case 'compare':
        return {
          kind: 'compare', anchor: ANCHOR, buttons: { keep: KEEP, replace: REPLACE },
          cards: { current: { glory: this.board[this.at][0], despair: this.board[this.at][1] }, fresh: this.result },
        };
      default:
        return { kind: 'unknown', anchor: null };
    }
  }

  click(x, y) {
    this.clicks.push({ x, y });
    switch (this.kind) {
      case 'levels':
      case 'counts': {
        if (near(TOGGLE, x, y)) this.kind = this.kind === 'levels' ? 'counts' : 'levels';
        const i = TILES.findIndex((t) => near(t, x, y));
        if (i >= 0) [this.at, this.kind] = [i, 'legacy'];
        break;
      }
      case 'legacy':
        if (near(BACK, x, y)) [this.at, this.kind] = [null, 'levels'];
        if (near(GOLD, x, y)) {
          if (this.stock[this.at] < 10) throw new Error('inherited without the stock');
          this.stock[this.at] -= 10;
          this.pity += 180;
          this.attempts++;
          this.kind = 'board';
        }
        break;
      case 'board':
        if (near(BUTTONS.despair, x, y)) [this.result, this.kind] = [this.roll(this.at), 'complete'];
        break;
      case 'complete':
        if (near(GOLD, x, y)) this.kind = 'compare';
        break;
      case 'compare':
        if (near(REPLACE, x, y)) this.board[this.at] = [this.result.glory, this.result.despair];
        if (near(KEEP, x, y) || near(REPLACE, x, y)) this.kind = 'legacy';
        break;
      default:
    }
  }
}

// a level-18 board (the main-counts fixture's) and its stock
const BOARD = [[7, 1], [7, 1], [8, 3], [8, 2], [8, 2], [7, 1], [8, 2], [8, 3], [7, 1], [9, 1], [7, 2], [8, 3]];
const STOCK = [207, 115, 135, 92, 109, 28, 156, 105, 36, 180, 4, 27];

/** A plan: roll `relic` for 8/2, all or nothing, keeping 8+/2-; level up once the board totals `up` despair. */
function plan({ relic = 11, up = 18 } = {}) {
  return {
    calls: [],
    advise(goal, level, states, stock) {
      this.calls.push({ states: states.map((x) => [...x]), stock: [...stock] });
      const despair = states.reduce((a, [, d]) => a + d, 0);
      if (despair <= up) return { step: 'levelup', goal: level + 1 };
      const pick = typeof relic === 'function' ? relic(states, stock) : relic;
      if (pick === null) return { step: 'summon', goal: level + 1 };
      return { step: 'quest', goal: level + 1, relic: pick, solveLevel: level, objective: { kind: 'target', glory: 8, despair: 2 }, bar: { glory: 8, despair: 2 } };
    },
    keepsRoll(roll, states, fresh) {
      return fresh.glory >= 8 && fresh.despair <= 2;
    },
  };
}

function setup(game, { attempts = 20, advice = plan(), saved = null, level = 18, objective = { kind: 'max' }, goal = { level: 20, tier: 46 } } = {}) {
  const logs = [];
  const used = [];
  const store = { attempt: saved };
  const configured = [];
  const solver = {
    board: (level) => ({ level, slots: 9, maxSpirit: 10, startGlory: 1, startDespairFail: 0, startTier: 1 }),
    configure: (level, objective) => configured.push({ level, objective }),
    best: (st) => (st.gf === 9 && st.df === 9 ? 'done' : 'despair'),
    expected: () => 0,
    finishChance: () => 1,
    canFinish: () => true,
    advise: (...a) => advice.advise(...a),
    keepsRoll: (...a) => advice.keepsRoll(...a),
    pityGain: () => 180,
    pityNeeded: () => 9000,
    maxLevel: () => 20,
  };
  const runner = new Advisor({
    tab: { capture: async () => ({ img: null, ratio: 1, png: '' }), click: async (x, y) => game.click(x, y) },
    solver,
    settings: { mode: 'advisor', goal, level, objective, attempts },
    ui: {
      log: (text, lv) => logs.push({ text, level: lv }), show() {}, askTier: async () => null,
      useSettings: (s) => used.push(s),
      saveAttempt: async (record) => (store.attempt = record), loadAttempt: async () => store.attempt,
      raiseGoal: (amp) => ((store.raised = amp), 'Level 20 + Demon Eye 46% amp'),
    },
    timing: { poll: 0, settle: 0, gap: [0, 0], retry: 30 },
    see: (img, lastAnchor, anywhere, popup) => {
      const s = game.screen();
      return s.kind === 'compare' && !(lastAnchor && popup === 'compare') ? { kind: 'unknown', anchor: null } : s;
    },
    read: (screen) => (screen.rateTier == null ? null : { tier: screen.rateTier }),
  });
  const text = () => logs.map((l) => l.text).join('\n');
  return { runner, logs, text, configured, advice, store, used, last: () => logs[logs.length - 1] };
}

test('advisor: reads the main page, every stock, then works the relic the plan picks', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, pity: 4680 });
  // the Crown, if it has 10 on hand: its stock matters until it is read, so every relic is visited
  const advice = plan({ relic: (states, stock) => (stock[11] >= 10 ? 11 : null) });
  const { runner, text, configured, last } = setup(game, { attempts: 2, advice });
  await runner.run();
  assert.match(text(), /Main page: inheritor level 18, pity 52%/);
  // every relic visited for its stock, in order, then Mountain Crown worked
  for (const [i, name] of RELICS.entries()) assert.match(text(), new RegExp(`${name.replace(/'/g, "'")}: ${BOARD[i][0]} glory, ${BOARD[i][1]} despair, ${STOCK[i]} on hand`));
  assert.match(text(), /Advisor: Crown of the Great Mountain \(27 on hand\), for level 19 toward 8\/2 - Maximize glory, minimize despair above a target \(Glory Target 8, Despair Target 2\), keeping a roll that does better/);
  // the attempt is solved for the plan's objective, at the level read
  assert.deepStrictEqual(configured[configured.length - 1], { level: 18, objective: { kind: 'target', glory: 8, despair: 2 } });
  assert.strictEqual(game.attempts, 2);
  assert.match(last().text, /Done: 2 attempt\(s\)/);
});

test('advisor: each attempt it starts sets the Single relic settings to its level and objective', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, pity: 4680 });
  const { runner, used } = setup(game, { attempts: 2 });
  await runner.run();
  assert.deepStrictEqual(used, [
    { level: 18, objective: { kind: 'target', glory: 8, despair: 2 } },
    { level: 18, objective: { kind: 'target', glory: 8, despair: 2 } },
  ]);
});

test('advisor: the plan\'s keep rule decides, and a kept result does not stop it', async () => {
  let n = 0;
  const results = [{ glory: 7, despair: 3 }, { glory: 8, despair: 2 }]; // worse, then the bar
  const game = new Game({ board: BOARD, stock: STOCK, pity: 0, roll: () => results[n++] });
  const { runner, text } = setup(game, { attempts: 2 });
  await runner.run();
  assert.match(text(), /Results: new 7 glory, 3 despair .* - Keep Current Effect/);
  assert.match(text(), /Results: new 8 glory, 2 despair .* - Replace with New Effect/);
  assert.deepStrictEqual(game.board[11], [8, 2]);
  assert.strictEqual(game.attempts, 2);
});

test('advisor: when the plan moves on, back to the main page and re-read', async () => {
  // Mountain Crown until it reaches 8/2, then Oath of Immortality
  const pick = (states) => (states[11][1] > 2 ? 11 : 2);
  const game = new Game({ board: BOARD, stock: STOCK, roll: () => ({ glory: 8, despair: 2 }) });
  const { runner, text } = setup(game, { attempts: 2, advice: plan({ relic: pick }) });
  await runner.run();
  assert.match(text(), /Back to the main page: the plan moves on/);
  assert.match(text(), /Advisor: Oath of Immortality/);
  assert.deepStrictEqual(game.board[11], [8, 2]);
  assert.deepStrictEqual(game.board[2], [8, 2]);
});

test('advisor: a goal already passed stops it, saying so', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, badges: (i, n) => n });
  const { runner, last } = setup(game, { goal: { level: 12, tier: 0 } });
  await runner.run();
  assert.match(last().text, /Inheritor level 18 is past the goal \(level 12\) - pick a higher goal/);
  assert.strictEqual(game.attempts, 0);
});

test('advisor: a met requirement stops it for the level-up', async () => {
  const game = new Game({ board: BOARD, stock: STOCK });
  const { runner, last } = setup(game, { advice: plan({ up: 22 }) });
  await runner.run();
  assert.match(last().text, /The requirement for level 19 is met - level up in the game/);
  assert.strictEqual(game.attempts, 0);
});

test('advisor: the Level Up button showing stops it at once, on either view', async () => {
  for (const screen of ['levels', 'counts']) {
    const game = new Game({ board: BOARD, stock: STOCK, screen, levelUp: () => true });
    const { runner, last } = setup(game);
    await runner.run();
    assert.match(last().text, /The Level Up button is showing .* press Level Up in the game yourself/, screen);
    assert.strictEqual(game.clicks.length, 0, screen); // Level Up is never pressed for the player
  }
});

test('advisor: an attempt that earns the level-up - back to the main page, and stops on its button', async () => {
  // a quest of 21 despair or less: Mountain Crown's 8/2 (22 -> 21) meets it
  const game = new Game({ board: BOARD, stock: STOCK, roll: () => ({ glory: 8, despair: 2 }),
    levelUp: (g) => g.board.reduce((a, [, d]) => a + d, 0) <= 21 });
  const { runner, text, last } = setup(game, { attempts: 5, advice: plan({ up: 21 }) });
  await runner.run();
  assert.strictEqual(game.attempts, 1);
  assert.match(text(), /Back to the main page/);
  assert.match(last().text, /The Level Up button is showing/);
});

test('advisor: a full pity bar stops it', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, pity: 9000 });
  const { runner, last } = setup(game);
  await runner.run();
  assert.match(last().text, /The pity bar is full - level up to 19/);
  assert.strictEqual(game.clicks.length, 0);
});

test('advisor: the Final Goal reached moves on to the next tier above the Demon Eye, and stops', async () => {
  // level 20, the Demon Eye (relic 1) at 9/1: 43%, the 20 + 43% goal met
  const board = BOARD.map((x, i) => (i === 1 ? [9, 1] : x));
  const game = new Game({ board, stock: STOCK, level: 20, pity: 9000 });
  const done = { calls: [], advise: () => ({ step: 'done', goal: 20 }), keepsRoll: () => false };
  const { runner, last, store } = setup(game, { advice: done, goal: { level: 20, tier: 43 }, level: 20 });
  await runner.run();
  assert.strictEqual(store.raised, 43);
  assert.match(last().text, /The goal is reached \(20 \+ Demon Eye at 43%\)\. The Demon Eye is at 43% - the Final Goal is now Level 20 \+ Demon Eye 46% amp/);
});

test('advisor: at the top level the MAX pity bar is not a level-up - it goes on to the counts', async () => {
  // level 20 shows the bar full, reading MAX: nothing above to level up to
  const game = new Game({ board: BOARD, stock: STOCK, level: 20, pity: 9000 });
  const { runner, text } = setup(game, { attempts: 1 });
  await runner.run();
  assert.match(text(), /Main page: inheritor level 20, the top - farming the Demon Eye/);
  assert.doesNotMatch(text(), /The pity bar is full/);
  assert.ok(game.clicks.length > 0);
});

test('advisor: pity nearly full - goes back to read the bar, and stops when it fills', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, pity: 8640, roll: () => ({ glory: 7, despair: 3 }) }); // 96%
  const { runner, text, last } = setup(game);
  await runner.run();
  assert.strictEqual(game.attempts, 2); // 96% -> 98%: read again -> 100%
  assert.match(text(), /Back to the main page: the pity bar may be nearly full/);
  assert.match(last().text, /The pity bar is full/);
});

test('advisor: nothing affordable - stops for a summon', async () => {
  const game = new Game({ board: BOARD, stock: STOCK });
  const { runner, last } = setup(game, { advice: plan({ relic: () => null }) });
  await runner.run();
  assert.match(last().text, /summon more relics/);
});

test('advisor: a relic whose Hero\'s Legacy disagrees with the main page stops it', async () => {
  const game = new Game({ board: BOARD, stock: STOCK });
  const screen = game.screen.bind(game);
  game.screen = () => {
    const s = screen();
    if (s.kind === 'legacy' && game.at === 3) s.memory = game.memoryOf([5, 5]);
    return s;
  };
  const { runner, last } = setup(game);
  await runner.run();
  assert.match(last().text, /Sacred Tree of Rebirth's Hero's Legacy shows 5 glory, 5 despair .* but the main page said 8\/2/);
});

test('advisor: started on Hero\'s Legacy, it goes to the main page first', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, screen: 'legacy' });
  game.at = 4;
  const { runner, text } = setup(game, { attempts: 1 });
  await runner.run();
  assert.match(text(), /Main page: inheritor level 18/);
  assert.strictEqual(game.attempts, 1);
});

test('advisor: a stock that will not read is asked for, not a stop', async () => {
  const game = new Game({ board: BOARD, stock: STOCK });
  const screen = game.screen.bind(game);
  game.screen = () => {
    const s = screen();
    if (s.kind === 'legacy' && game.at === 5) s.stock = null; // Golden Star's
    return s;
  };
  const { runner, text } = setup(game, { attempts: 1 });
  const asked = [];
  runner.ui.askNumber = async (q) => {
    asked.push(q);
    return 28;
  };
  await runner.run();
  assert.deepStrictEqual(asked, ['How many Golden Star are on hand? (the number before "/10")']);
  assert.match(text(), /Golden Star: 7 glory, 1 despair, 28 on hand/);
  assert.strictEqual(game.attempts, 1);
});

test('advisor: stock off the badges - no start-up visits', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, badges: (i, n) => n });
  const { runner, text } = setup(game, { attempts: 1 });
  await runner.run();
  assert.doesNotMatch(text(), /^ {2}.*on hand\.$/m); // no visit lines
  assert.doesNotMatch(text(), /not read off the badges/);
  assert.match(text(), /Advisor: Crown of the Great Mountain \(27 on hand\)/);
  assert.strictEqual(game.attempts, 1);
});

test('advisor: an unreadable badge the advice turns on - only that relic is visited', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, badges: (i, n) => (i === 4 ? null : n) });
  // the Ring of Lightning is rolled if it has 10 on hand, else the Crown
  const advice = plan({ relic: (states, stock) => (stock[4] >= 10 ? 4 : 11) });
  const { runner, text } = setup(game, { attempts: 1, advice });
  await runner.run();
  assert.match(text(), /1 stock not read off the badges - reading it off Hero's Legacy/);
  assert.match(text(), /Ring of Lightning: 8 glory, 2 despair, 109 on hand/);
  assert.strictEqual(text().match(/^ {2}.*on hand\.$/gm).length, 1);
});

test('advisor: an unreadable badge the advice does not turn on is not visited', async () => {
  // the plan rolls the Crown (27 on hand, read) whatever the Ring of Lightning has
  const game = new Game({ board: BOARD, stock: STOCK, badges: (i, n) => (i === 4 ? null : n) });
  const { runner, text } = setup(game, { attempts: 1 });
  await runner.run();
  assert.match(text(), /1 stock not read off the badges - the advice is the same whatever it is\./);
  assert.doesNotMatch(text(), /^ {2}.*on hand\.$/m); // no visit lines
  assert.match(text(), /Advisor: Crown of the Great Mountain \(27 on hand\)/);
  assert.strictEqual(game.attempts, 1);
});

test("advisor: a badge that disagrees with Hero's Legacy - every stock is re-read", async () => {
  // Crown's badge misread as 77: the plan picks it, its Hero's Legacy says 27
  const game = new Game({ board: BOARD, stock: STOCK, badges: (i, n) => (i === 11 ? 77 : n) });
  const { runner, text } = setup(game, { attempts: 1 });
  await runner.run();
  assert.match(text(), /Crown of the Great Mountain's badge read 77, but its Hero's Legacy says 27 - re-reading every stock off Hero's Legacy/);
  assert.match(text(), /11 stocks not read off the badges/);
  assert.match(text(), /Advisor: Crown of the Great Mountain \(27 on hand\)/);
  assert.strictEqual(game.attempts, 1);
});

test('advisor: an attempt it started and was stopped in is finished the way it began', async () => {
  // stopped mid-attempt on Oath of Immortality, rolled for 8/2
  const game = new Game({ board: BOARD, stock: STOCK, screen: 'board', roll: () => ({ glory: 7, despair: 1 }) });
  game.at = 2;
  const saved = {
    at: Date.now(), goal: { level: 20, tier: 46 }, level: 18, relic: 2, objective: { kind: 'target', glory: 8, despair: 2 }, roll: { horizon: { glory: 94, despair: 19 }, goal: 19, relic: 2, bar: { glory: 8, despair: 2 }, farm: false },
    states: BOARD, stock: STOCK, pity: 0.52,
  };
  const { runner, text, configured, store } = setup(game, { attempts: 1, saved });
  await runner.run();
  assert.match(text(), /Finishing the Oath of Immortality attempt the advisor started: Maximize glory, minimize despair above a target \(Glory Target 8, Despair Target 2\)/);
  assert.deepStrictEqual(configured[0], { level: 18, objective: { kind: 'target', glory: 8, despair: 2 } });
  // the plan's keep rule (8+/2- in this test's plan): 7/1 is not kept
  assert.match(text(), /Results: new 7 glory, 1 despair .* - Keep Current Effect/);
  assert.match(text(), /Done: 1 attempt\(s\)/);
  assert.strictEqual(store.attempt, null);
});

test('advisor: an attempt it did not start is finished by the Single relic settings, then on to the main page', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, screen: 'board', roll: () => ({ glory: 9, despair: 0 }) });
  game.at = 5;
  const { runner, text, configured } = setup(game, { attempts: 2, level: 18, objective: { kind: 'max' } });
  await runner.run();
  assert.match(text(), /Finishing the attempt in progress by the Single relic settings: level 18, Maximize amplification above a target \(Minimum Useful Amplification \+0%\)/);
  assert.deepStrictEqual(configured[0], { level: 18, objective: { kind: 'max' } });
  assert.match(text(), /Results: new 9 glory, 0 despair .* - Replace with New Effect/);
  // then the advisor proper: the main page, the plan, its next attempt
  assert.match(text(), /Main page: inheritor level 18/);
  assert.strictEqual(game.attempts, 1); // the second attempt was the advisor's own
  assert.match(text(), /Done: 2 attempt\(s\)/);
});

test('advisor: a stale saved attempt is not trusted', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, screen: 'board' });
  game.at = 2;
  const saved = { at: Date.now() - 2 * 60 * 60 * 1000, goal: { level: 20, tier: 46 }, level: 18, relic: 2, objective: { kind: 'target', glory: 8, despair: 2 }, roll: { horizon: { glory: 94, despair: 19 }, goal: 19, relic: 2, bar: { glory: 8, despair: 2 }, farm: false }, states: BOARD, stock: STOCK, pity: 0.5 };
  const { runner, text } = setup(game, { attempts: 1, saved });
  await runner.run();
  assert.match(text(), /by the Single relic settings/);
});

test('advisor: a resumed attempt that can no longer be kept is abandoned at once', async () => {
  // resumed on Mermaid's Tear (9/1), the plan's keep rule 8+/2-: one glory
  // slot left, 6 glory successes - nothing reachable would be kept
  const game = new Game({ board: BOARD, stock: STOCK, screen: 'board' });
  game.at = 9;
  // Abandon Inheritance on the board, and its confirmation
  const ABANDON = { x: 120, y: 520 }, CONFIRM = { x: 300, y: 300 };
  const screen = game.screen.bind(game);
  game.screen = () => {
    if (game.kind === 'confirm') return { kind: 'abandon', anchor: ANCHOR, buttons: { confirm: CONFIRM, cancel: { x: 400, y: 300 } } };
    const s = screen();
    if (s.kind === 'board') {
      s.glory = [...Array(6).fill('success'), 'fail', 'fail', 'empty', 'none'];
      s.despair = ['success', 'fail', 'fail', ...Array(6).fill('empty'), 'none'];
      Object.assign(s, { leaves: { count: 8, contiguous: true }, fill: 0, present: { train: true, glory: false, despair: false }, abandon: ABANDON });
    }
    return s;
  };
  const click = game.click.bind(game);
  game.click = (x, y) => {
    if (game.kind === 'board' && near(ABANDON, x, y)) game.kind = 'confirm';
    else if (game.kind === 'confirm' && near(CONFIRM, x, y)) game.kind = 'legacy';
    else click(x, y);
  };
  const saved = {
    at: Date.now(), goal: { level: 20, tier: 46 }, level: 18, relic: 9, objective: { kind: 'target', glory: 8, despair: 2 }, roll: { horizon: { glory: 94, despair: 19 }, goal: 19, relic: 9, bar: { glory: 8, despair: 2 }, farm: false },
    states: BOARD, stock: STOCK, pity: 0.5,
  };
  const { runner, text } = setup(game, { attempts: 1, saved, advice: plan({ relic: 9 }) });
  runner.solver.keepsRoll = (roll, states, fresh) => fresh.glory >= 8 && fresh.despair <= 2;
  await runner.run();
  assert.match(text(), /Nothing useful left in this attempt: even the best result still possible \(7 glory, 1 despair/);
  assert.match(text(), /Abandoned - the applied memory is unchanged \(9 glory, 1 despair/);
});

test('advisor: a pity filler says so, loudly, when it starts', async () => {
  const filler = {
    advise: (goal, level) => ({ step: 'filler', goal: level + 1, relic: 0, solveLevel: level, objective: { kind: 'close', glory: 9, despair: 0 }, bar: { glory: 4, despair: 2 }, closer: false }),
    keepsRoll: () => false,
  };
  const game = new Game({ board: BOARD, stock: STOCK, badges: (i, n) => n, roll: () => ({ glory: 6, despair: 3 }) });
  const { runner, logs } = setup(game, { attempts: 1, advice: filler });
  await runner.run();
  const line = logs.find((l) => /^PITY FILLER:/.test(l.text));
  assert.ok(line, logs.map((l) => l.text).join('\n'));
  assert.match(line.text, /nothing the level 19 work needs has 10 on hand, so Giant's Right Hand \(207 on hand\) is rolled just for the pity \(\+180 an attempt\)/);
  assert.strictEqual(line.level, 'warn');
  assert.ok(logs.some((l) => /Clicking Inheritance \(attempt 1 of 1; PITY FILLER, /.test(l.text)));
});

test('advisor: a relic tile click that does not land is clicked again', async () => {
  const game = new Game({ board: BOARD, stock: STOCK, pity: 4680, badges: (i, n) => n });
  const click = game.click.bind(game);
  let dropped = false;
  game.click = (x, y) => {
    if (!dropped && TILES.some((t) => near(t, x, y))) {
      dropped = true; // the main page does not take it
      game.clicks.push({ x, y });
      return;
    }
    click(x, y);
  };
  const { runner, text } = setup(game, { attempts: 1 });
  await runner.run();
  assert.match(text(), /Crown of the Great Mountain did not respond in \d+ s - clicking it again \(2 of 3\)/);
  assert.strictEqual(game.attempts, 1);
});
