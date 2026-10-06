// The autoplayer: look at the tab, work out the screen, click, check.
//
//   Hero's Legacy --Inheritance--> board --attempts...--> all filled
//     --Complete Inheritance--> Compare results --Keep / Replace--> Hero's Legacy
//
// It only ever clicks the three Attempt buttons, Complete Inheritance,
// Keep / Replace and Inheritance - and Abandon Inheritance then Confirm, but
// only for a wiped attempt (no spirit power or mental strength left, the
// bars unfinished), the one way out of it.  Anything it does not recognise,
// or a click that does not do what it should, stops the run with nothing
// clicked.  Finding a better memory stops it too, once Replace applied it.

import { classify, boardState, boardRate, isForbidden } from './vision.js';
import {
  TIER_PERCENT, amplification, chooseMemory, hitsObjective, inferOutcome, isStartState, moved, nextTier, show,
} from './logic.js';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const TIMING = {
  poll: 250, // between looks while waiting
  settle: 900, // a screen must hold this long to count (the spirit bar animates)
  gap: [1400, 1600], // a random pause between clicks, in this range
};

export class Halt extends Error {}

const MOVE = { glory: 'Glory', despair: 'Despair', train: 'Training' };
const pct = (tier) => `${TIER_PERCENT[tier]}%`;
export const fmtMemory = (m) => `${m.glory} glory, ${m.despair} despair (+${amplification(m)}%)`;
export { sleep, TIMING };

export class Runner {
  /**
   * tab: GameTab; solver: RelicSolver;
   * settings: { level, objective, attempts, clickMethod, clickDelay };
   *   clickDelay, in seconds, spaces clicks a random +/- 0.1 s around it
   * ui: { log(text, level), show(frame, info), askTier(candidates) }
   * timing, see (the screen classifier) and read (the rate reader) are for tests.
   */
  constructor({ tab, solver, settings, ui, timing = TIMING, see = classify, read = boardRate }) {
    const delay = settings.clickDelay;
    if (delay != null) timing = { ...timing, gap: [Math.max(0, delay - 0.1) * 1000, (delay + 0.1) * 1000] };
    Object.assign(this, { tab, solver, settings, ui, timing, see, read });
    this.board = null; // the inheritor level's board, once known
    if (settings.level != null) this.useLevel(settings.level, settings.objective);
    this.running = false;
    this.anchor = null; // the last anchor seen, for the Compare popup
    // the popup looked for: 'compare' right after Complete Inheritance,
    // 'abandon' right after Abandon Inheritance - never otherwise
    this.expect = null;
    this.anywhere = false; // the Compare popup with no screen before it (a run's first look)
    this.tier = null; // the ladder position, while it is known
    this.settled = false; // said this attempt's target is settled
    this.current = null; // the applied memory, from Hero's Legacy
    this.result = null; // the attempt just completed
    this.finished = 0;
    this.lastClick = 0;
    this.lastSeen = null;
  }

  stop() {
    this.running = false;
  }

  /** Play attempts on inheritor `level`'s board, for `objective`. */
  useLevel(level, objective) {
    this.board = this.solver.board(level);
    this.solver.configure(level, objective);
    this.objective = objective;
    this.settled = false;
  }

  /**
   * Once an all-or-nothing target is certain or out of reach, every move
   * scores the same for it, and the solver's tie-breaks play the rest of the
   * attempt: out of reach of a glory/despair target, for the nearest result
   * still possible (7/2 when 7/1 is gone), then for amplification.  Say so,
   * once an attempt.
   */
  settleTarget(st, tier) {
    if (this.settled || !['target', 'reach'].includes(this.objective?.kind)) return;
    const odds = this.solver.expected(st, tier);
    if (odds > 1e-9 && odds < 1 - 1e-9) return;
    this.settled = true;
    const rest = odds < 0.5 && this.objective.kind === 'target'
      ? 'for the result nearest to it still possible, then the most amplification'
      : 'for the most amplification';
    this.ui.log(`The target is ${odds < 0.5 ? 'out of reach' : 'certain'} for this attempt - playing the rest of it ${rest}.`);
  }

  // ------------------------------------------------------------- looking

  async look() {
    if (!this.running) throw new Halt('Stopped.');
    const shot = await this.tab.capture();
    const screen = this.see(shot.img, this.expect ? this.anchor : null, this.anywhere, this.expect ?? 'compare');
    if (screen.anchor) this.anchor = screen.anchor;
    return (this.lastSeen = { ...shot, screen });
  }

  /** The board's state, or { error }. */
  state(screen) {
    if (!this.board) return { error: 'the inheritor level is not known yet' };
    return boardState(screen, this.board.slots, this.board.maxSpirit);
  }

  /** What has to stay the same for a frame to count as settled. */
  signature(frame) {
    const s = frame.screen;
    switch (s.kind) {
      case 'board': {
        const st = this.state(s);
        return JSON.stringify(st.error ? [s.kind, st.error] : [s.kind, st]);
      }
      case 'complete':
      case 'legacy':
        return JSON.stringify([s.kind, s.memory.glory.bar, s.memory.despair.bar, s.stock]);
      case 'main':
        return JSON.stringify([s.kind, s.view, s.level, s.pity && Math.round(s.pity * 100), s.relics, s.totals]);
      default:
        return s.kind;
    }
  }

  /**
   * The first frame passing `test` that stays the same for `timing.settle`
   * (so animations have finished), or null after `ms`.
   */
  async waitFor(test, ms) {
    const deadline = Date.now() + ms;
    let since = null, sig = null;
    while (Date.now() < deadline) {
      const frame = await this.look();
      if (!test(frame)) since = null;
      else {
        const now = this.signature(frame);
        if (since === null || now !== sig) [since, sig] = [Date.now(), now];
        else if (Date.now() - since >= this.timing.settle) return frame;
      }
      await sleep(this.timing.poll);
    }
    return null;
  }

  /** What the last frame showed, for messages. */
  describe(frame = this.lastSeen) {
    if (!frame) return 'nothing yet';
    const s = frame.screen;
    if (s.kind === 'main') return `the main page (${s.view} view)`;
    if (s.kind !== 'board') return s.kind === 'unknown' ? 'a screen I do not recognise' : s.kind;
    const st = this.state(s);
    return st.error ? `a board I could not read (${st.error})` : `a board with ${show(st)}`;
  }

  lost(what) {
    if (this.lastSeen) this.ui.show(this.lastSeen, { note: 'last frame seen' });
    throw new Halt(`Waited for ${what} but it never came; the last thing I saw was ${this.describe()}. Stopped.`);
  }

  // ------------------------------------------------------------ clicking

  /** Click; anywhere near Abandon Inheritance only when `abandon` says so. */
  async click(point, frame, what, { abandon = false } = {}) {
    const { x, y } = point;
    // (before any board is seen there is no anchor - and no Abandon button)
    if (this.anchor && !abandon && isForbidden(this.anchor, x, y)) throw new Halt(`Refusing to click ${what}: too close to Abandon Inheritance.`);
    const [lo, hi] = this.timing.gap;
    const gap = lo + Math.random() * (hi - lo);
    const wait = this.lastClick + gap - Date.now();
    if (wait > 0) await sleep(wait);
    if (!this.running) throw new Halt('Stopped.');
    await this.tab.click(x / frame.ratio, y / frame.ratio, this.settings.clickMethod);
    this.lastClick = Date.now();
  }

  // ------------------------------------------------------------- running

  async run() {
    this.running = true;
    this.ui.log(this.board ? `Started: level ${this.board.level} (${this.board.slots} slots), up to ${this.settings.attempts} attempt(s).`
      : `Started the advisor, up to ${this.settings.attempts} attempt(s).`);
    try {
      this.anywhere = true;
      let frame = await this.waitFor(() => true, 5000);
      this.anywhere = false;
      while (this.running) {
        if (!frame) throw new Halt('The screen would not hold still - stopped.');
        switch (frame.screen.kind) {
          case 'board': frame = await this.onBoard(frame); break;
          case 'complete': frame = await this.onComplete(frame); break;
          case 'compare': frame = await this.onCompare(frame); break;
          case 'legacy': frame = await this.onLegacy(frame); break;
          case 'main': frame = await this.onMain(frame); break;
          default:
            this.ui.show(frame, { note: 'unrecognised screen' });
            throw new Halt('Not on a screen I recognise - stopped without clicking.');
        }
      }
    } catch (err) {
      if (err instanceof Halt) this.ui.log(err.message, 'stop');
      else this.ui.log(`Error: ${err.message}`, 'error');
    } finally {
      this.running = false;
    }
  }

  /** The rate read off the buttons, if it is one of `candidates`; else null. */
  readTier(screen, candidates = [0, 1, 2, 3, 4]) {
    const read = this.read(screen);
    return read && candidates.includes(read.tier) ? read.tier : null;
  }

  /**
   * An opening board (nothing spent yet) that does not open the way the
   * configured level does: the level setting is wrong.
   */
  checkOpening(st) {
    const b = this.board;
    if (st.ms !== b.slots || st.sp !== b.maxSpirit || isStartState(st, b)) return;
    const fits = [];
    for (let level = 1; level <= 28; level++) {
      const o = this.solver.board(level);
      if (o.slots === b.slots && o.maxSpirit === b.maxSpirit && o.startGlory === st.gf && o.startDespairFail === st.df) fits.push(level);
    }
    const opens = `${st.gf} free glory success${st.gf === 1 ? '' : 'es'} and ${st.df} free despair fail${st.df === 1 ? '' : 's'}`;
    const levels = fits.length ? `inheritor level ${fits.join(', ').replace(/, (\d+)$/, ' or $1')}` : 'no level I know';
    throw new Halt(`This attempt opens with ${opens} on ${b.slots} slots - that is ${levels}, `
      + `but the level is set to ${b.level}. Fix the level and press Start (it picks up mid-attempt).`);
  }

  /** Where the ladder stands: read off the buttons, else tracked, else asked. */
  async resolveTier(screen, st) {
    if (this.tier === null && isStartState(st, this.board)) this.tier = this.board.startTier;
    const read = this.readTier(screen);
    if (read !== null) {
      if (this.tier === null) this.ui.log(`Picking up an attempt in progress: ${show(st)}, rate ${pct(read)}.`);
      return (this.tier = read);
    }
    if (this.tier !== null) return this.tier;
    this.ui.log('Could not read the rate on the buttons - which is it?', 'ask');
    return (this.tier = await this.ask([0, 1, 2, 3, 4]));
  }

  async ask(candidates) {
    const tier = await this.ui.askTier(candidates);
    if (tier === null || !this.running) throw new Halt('Stopped.');
    return tier;
  }

  async onMain(frame) {
    this.ui.show(frame, { note: `main page, ${frame.screen.view} view` });
    if (frame.screen.levelUp) throw new Halt('On the main page with Level Up showing - press it in the game yourself.');
    throw new Halt('On the main page - pick a relic to work on, or switch the mode to Advisor.');
  }

  async onBoard(frame) {
    const { screen } = frame;
    if (!this.board) throw new Halt("An attempt is in progress but the inheritor level is not known yet - finish it, then start the advisor from the main page or Hero's Legacy.");
    const st = this.state(screen);
    if (st.error) throw new Halt(st.error);
    this.checkOpening(st);
    const tier = await this.resolveTier(screen, st);

    const useless = this.hopeless(st);
    if (useless) return this.abandonAttempt(frame, st, useless);
    this.settleTarget(st, tier);
    const action = this.solver.best(st, tier);
    const info = {
      state: st, tier, action,
      expected: this.solver.expected(st, tier),
      finish: this.solver.finishChance(st, tier),
    };
    this.ui.show(frame, info);
    if (action === 'done') {
      // both bars are full: Complete Inheritance should replace the buttons
      return (await this.waitFor((f) => f.screen.kind === 'complete', 5000)) ?? this.lost('the Complete Inheritance button');
    }
    if (action === 'deadend') return this.abandonAttempt(frame, st);
    if (!screen.present[action]) throw new Halt(`The ${MOVE[action]} Attempt button is not showing - stopped.`);

    const lastFill = 2 * this.board.slots - 1 === st.gf + st.df && action !== 'train';
    const changed = (f) => {
      if (f.screen.kind === 'complete') return true;
      if (f.screen.kind !== 'board') return false;
      const now = this.state(f.screen);
      return !now.error && moved(st, now);
    };
    await this.click(screen.buttons[action], frame, action);
    let next = await this.waitFor(changed, 9000);
    if (!next) {
      // no response in 9 s: if the board is exactly as it was, the click
      // missed - try once more (a late response on top would show as two
      // fills and stop the run)
      const again = (await this.waitFor(() => true, 3000)) ?? this.lost('the board');
      if (again.screen.kind !== 'board') return again;
      const now = this.state(again.screen);
      if (now.error) return again;
      if (moved(st, now)) next = again;
      else {
        this.ui.log(`The ${MOVE[action]} click did not register - trying once more.`, 'warn');
        if (!again.screen.present[action]) throw new Halt(`The ${MOVE[action]} Attempt button is not showing - stopped.`);
        await this.click(again.screen.buttons[action], again, action);
        next = (await this.waitFor(changed, 9000)) ?? this.lost('a response to the click');
      }
    }

    if (next.screen.kind === 'complete') {
      if (!lastFill) throw new Halt('The attempt ended early - stopped to be safe.');
      this.tier = null;
      return next;
    }
    return this.judge(st, tier, action, next);
  }

  /** Work out how a move went and where the rate is now; log it. */
  async judge(st, tier, action, next) {
    let after, outcome, read;
    for (let tries = 0; ; tries++) {
      after = this.state(next.screen);
      if (after.error) throw new Halt(after.error);
      outcome = inferOutcome(st, after, action, this.board.maxSpirit);
      if (!outcome.ok) throw new Halt(`That is not what the click should do (${outcome.why}).`);
      read = this.readTier(next.screen);
      const expected = outcome.success === null ? null : nextTier(tier, outcome.success);
      // the buttons and the board disagree: the board may still be catching up
      if (read === null || expected === null || read === expected || tries === 1) break;
      next = (await this.waitFor((f) => f.screen.kind === 'board', 4000)) ?? this.lost('the board');
    }

    let success = outcome.success;
    if (success === null) {
      // a training at full spirit power: only the rate shows how it went
      const options = [nextTier(tier, true), nextTier(tier, false)];
      this.tier = this.readTier(next.screen, options);
      if (this.tier === null) {
        this.ui.log(`Training at full spirit power and the rate would not read - is it now ${options.map(pct).join(' or ')}?`, 'ask');
        this.tier = await this.ask(options);
      }
      success = this.tier === options[0];
    } else {
      const expected = nextTier(tier, success);
      this.tier = read ?? expected;
      if (read !== null && read !== expected) {
        this.ui.log(`The buttons show ${pct(read)}, not the ${pct(expected)} this ${success ? 'success' : 'fail'} should give - going by the buttons.`, 'warn');
      }
    }
    this.ui.log(`${MOVE[action]} at ${pct(tier)}: ${success ? 'success' : 'fail'}  ->  ${show(after)}, next ${pct(this.tier)}`);
    return next;
  }

  async onComplete(frame) {
    this.settled = false;
    const { glory, despair } = frame.screen.memory;
    if (glory.slots !== this.board.slots || despair.slots !== this.board.slots
      || glory.filled !== glory.slots || despair.filled !== despair.slots) {
      throw new Halt(`Complete screen, but the bars read ${glory.filled}/${glory.slots} and ${despair.filled}/${despair.slots} - stopped.`);
    }
    this.result = { glory: glory.success, despair: despair.success };
    this.ui.show(frame, { note: `attempt finished: ${fmtMemory(this.result)}` });
    this.ui.log(`Attempt finished: ${fmtMemory(this.result)}. Clicking Complete Inheritance.`, 'good');
    await this.click(frame.screen.button, frame, 'Complete Inheritance');
    this.expect = 'compare';
    const next = (await this.waitFor((f) => f.screen.kind === 'compare' || f.screen.kind === 'legacy', 10000))
      ?? this.lost('the results screen');
    if (next.screen.kind === 'legacy') {
      // no memory to compare with: the result was applied as it is
      this.expect = null;
      this.finished++;
      this.result = null;
    }
    return next;
  }

  async onCompare(frame) {
    // the two cards, checked against what this run saw for itself
    const cards = frame.screen.cards ?? {};
    for (const [mine, read, name] of [[this.current, cards.current, 'current'], [this.result, cards.fresh, 'new']]) {
      if (mine && read && (mine.glory !== read.glory || mine.despair !== read.despair)) {
        this.ui.show(frame, { note: 'compare' });
        throw new Halt(`The ${name} memory card reads ${fmtMemory(read)}, but I saw ${fmtMemory(mine)} - choose Keep or Replace yourself.`);
      }
    }
    const current = cards.current ?? this.current, fresh = cards.fresh ?? this.result;
    if (!current || !fresh) {
      this.ui.show(frame, { note: 'compare' });
      throw new Halt(`Could not read the ${current ? 'new' : 'current'} memory card - choose Keep or Replace yourself.`);
    }
    const choice = this.choose(current, fresh);
    this.ui.show(frame, { note: `${choice}: new ${fmtMemory(fresh)} vs current ${fmtMemory(current)}` });
    this.ui.log(`Results: new ${fmtMemory(fresh)} vs current ${fmtMemory(current)} - ${choice === 'keep' ? 'Keep Current Effect' : 'Replace with New Effect'}.`);
    await this.click(frame.screen.buttons[choice], frame, choice);
    const next = (await this.waitFor((f) => f.screen.kind === 'legacy', 10000)) ?? this.lost('Hero\'s Legacy');
    this.expect = null;
    const want = choice === 'keep' ? current : fresh;
    const got = { glory: next.screen.memory.glory.success, despair: next.screen.memory.despair.success };
    if (got.glory !== want.glory || got.despair !== want.despair) {
      throw new Halt(`After ${choice}, the applied memory reads ${fmtMemory(got)}, expected ${fmtMemory(want)}. Check it!`);
    }
    this.finished++;
    this.result = null;
    this.current = want;
    this.afterAttempt(choice, want);
    return next;
  }

  /** Keep the current memory or replace it with the fresh one. */
  choose(current, fresh) {
    return chooseMemory(this.objective, current, fresh);
  }

  /** After an attempt's memory is applied (or, with choice null, abandoned). */
  afterAttempt(choice, applied) {
    if (choice === 'replace') throw new Halt(`Found a better memory and applied it: ${fmtMemory(applied)}. Stopped.`);
  }

  /**
   * Whether no result this attempt can still reach would be kept: then why
   * play it out?  Every roll succeeds with a chance strictly between 0 and
   * 1, so every count of successes in the slots left is reachable; each is
   * put to the Keep/Replace rule against the applied memory.  A reason to
   * give, or null - also when that memory is not known.
   */
  hopeless(st) {
    const current = this.current;
    if (!current) return null;
    const left = { glory: this.board.slots - st.gf, despair: this.board.slots - st.df };
    // the best corner first: most glory, least despair - usually the answer
    for (let g = st.gs + left.glory; g >= st.gs; g--) {
      for (let d = st.ds; d <= st.ds + left.despair; d++) {
        if (this.choose(current, { glory: g, despair: d }) === 'replace') return null;
      }
    }
    const best = { glory: st.gs + left.glory, despair: st.ds };
    return `even the best result still possible (${fmtMemory(best)}) would not replace ${fmtMemory(current)}`;
  }

  /**
   * Abandon Inheritance, then Confirm: for a wiped attempt (no spirit power
   * or mental strength left, the bars unfinished, no Attempt buttons), or
   * one no reachable result of which would be kept (`useless`, the reason).
   * The applied memory stays as it was.
   */
  async abandonAttempt(frame, st, useless = null) {
    this.settled = false;
    const { screen } = frame;
    if (!useless && Object.values(screen.present).some(Boolean)) {
      throw new Halt(`The board reads as wiped (${show(st)}) but an Attempt button is showing - stopped.`);
    }
    if (!screen.abandon) throw new Halt(`Abandoning this attempt (${show(st)}) but I cannot find Abandon Inheritance - stopped.`);
    this.ui.log(useless ? `Nothing useful left in this attempt: ${useless} - abandoning it.`
      : `Out of spirit power and mental strength with the bars unfinished (${show(st)}) - abandoning the attempt.`, 'warn');
    await this.click(screen.abandon, frame, 'Abandon Inheritance', { abandon: true });
    this.expect = 'abandon';
    const popup = (await this.waitFor((f) => f.screen.kind === 'abandon', 6000)) ?? this.lost('the abandon confirmation');
    await this.click(popup.screen.buttons.confirm, popup, 'Confirm');
    const next = (await this.waitFor((f) => f.screen.kind === 'legacy', 10000)) ?? this.lost('Hero\'s Legacy');
    this.expect = null;
    const got = { glory: next.screen.memory.glory.success, despair: next.screen.memory.despair.success };
    if (this.current && (got.glory !== this.current.glory || got.despair !== this.current.despair)) {
      throw new Halt(`After abandoning, the applied memory reads ${fmtMemory(got)}, but it was ${fmtMemory(this.current)}. Check it!`);
    }
    this.finished++;
    this.tier = null;
    this.result = null;
    this.ui.log(`Abandoned - the applied memory is unchanged (${fmtMemory(got)}).`);
    this.afterAttempt(null, got);
    return next;
  }

  async onLegacy(frame) {
    this.expect = null;
    const m = frame.screen.memory;
    this.current = { glory: m.glory.success, despair: m.despair.success };
    this.ui.show(frame, { note: `current memory: ${fmtMemory(this.current)}` });
    if (this.finished >= this.settings.attempts) {
      throw new Halt(`Done: ${this.finished} attempt(s), current memory ${fmtMemory(this.current)}.`);
    }
    if (hitsObjective(this.objective, this.current)) {
      throw new Halt(`The current memory (${fmtMemory(this.current)}) already meets the objective.`);
    }
    this.ui.log(`Current memory: ${fmtMemory(this.current)}. Clicking Inheritance (attempt ${this.finished + 1} of ${this.settings.attempts}).`);
    await this.click(frame.screen.button, frame, 'Inheritance');
    // any board: onBoard checks it opens the way this level should
    const next = (await this.waitFor((f) => f.screen.kind === 'board', 10000)) ?? this.lost('the attempt to start (out of relics?)');
    this.tier = null;
    return next;
  }

  /** One look, no clicks: the screen and, on a board, the move it would make. */
  async peek() {
    this.running = true;
    this.anywhere = true;
    try {
      const frame = await this.waitFor(() => true, 4000);
      if (!frame) return { note: 'the screen would not hold still' };
      const { screen } = frame;
      if (screen.kind !== 'board') {
        const m = screen.memory;
        const note = m ? `${screen.kind}: ${fmtMemory({ glory: m.glory.success, despair: m.despair.success })}` : screen.kind;
        this.ui.show(frame, { note });
        return { note };
      }
      const st = this.state(screen);
      if (st.error) {
        this.ui.show(frame, { note: st.error });
        return { note: st.error };
      }
      try {
        this.checkOpening(st);
      } catch (err) {
        this.ui.show(frame, { state: st, note: err.message });
        return { note: err.message };
      }
      const tier = this.readTier(screen) ?? (isStartState(st, this.board) ? this.board.startTier : null);
      if (tier === null) {
        this.ui.show(frame, { state: st, note: 'could not read the rate on the buttons' });
        return { note: 'rate unreadable' };
      }
      const info = {
        state: st, tier, action: this.solver.best(st, tier),
        expected: this.solver.expected(st, tier), finish: this.solver.finishChance(st, tier),
      };
      this.ui.show(frame, info);
      return info;
    } finally {
      this.running = false;
      this.anywhere = false;
    }
  }
}
