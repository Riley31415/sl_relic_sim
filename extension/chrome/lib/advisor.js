// The advisor: works the relics the Look Ahead plan picks, from the main
// page, until a level-up, a summon, the goal or the attempt limit.
//
//   main page                the Level Up button showing: stop, for the
//                            player to level up
//   main page, levels view   inheritor level and pity (full: stop, level up)
//     -> counts view         every relic's glory and despair, checked
//                            against the totals line
//     -> each relic once     its stock, off Hero's Legacy ("127/10"), and a
//                            check of its memory against the counts view
//     -> the plan's advice   level up / summon / convert / done: stop;
//                            a roll: that relic's Hero's Legacy, its
//                            attempts played and kept the plan's way, for
//                            as long as the plan keeps picking it
//     -> back to the main page, re-read, and on
//
// Each attempt adds pity; when the bar could be nearly full it goes back to
// read it rather than guess.

import { Runner, Halt, fmtMemory } from './runner.js';
import { RELICS, amplification, chooseMemory, describeObjective } from './logic.js';
import { PER_ATTEMPT, isRoll, keepRule, rollOf } from './advice.js';

export { RELICS, describeObjective };

const PITY_CHECK = 0.98; // estimated this full: go and read the bar
const SAVED_FOR = 60 * 60 * 1000; // an attempt saved longer ago than this is not trusted

export class Advisor extends Runner {
  /**
   * settings.goal: { level, tier } - what the plan works toward;
   * settings.level and .objective - how to finish an attempt already in
   * progress that the advisor did not start (the Single relic settings);
   * ui.saveAttempt(record | null) and ui.loadAttempt() keep the attempt in
   * play, so a stopped run can finish it the way it began.
   */
  constructor(opts) {
    super({ ...opts, settings: { ...opts.settings, level: null } });
    this.goal = opts.settings.goal;
    this.fallback = { level: opts.settings.level, objective: opts.settings.objective };
    this.level = null; // the inheritor level, from the levels view
    this.pity = null; // the pity bar, 0..1: read, then estimated
    this.pityRead = false; // whether `pity` is a reading, not an estimate
    this.states = null; // every relic's [glory, despair], from the counts view
    this.stock = new Array(RELICS.length).fill(null); // each relic's stock: off its badge, or its Hero's Legacy
    this.fromBadge = new Array(RELICS.length).fill(false); // which came off a badge, not yet checked
    this.badges = true; // trusting the badges (until one disagrees with Hero's Legacy)
    this.at = null; // the relic whose Hero's Legacy is showing (or about to)
    this.visiting = false; // there only to read its stock
    this.plan = null; // the advice the attempt in play was rolled under, and its inputs
    this.idle = 0; // decisions since the last attempt (going in circles?)
  }

  // ------------------------------------------------------------ the main page

  async onMain(frame) {
    const s = frame.screen;
    this.at = null;
    await this.ui.saveAttempt?.(null); // on the main page, no attempt is in play
    this.ui.show(frame, { note: `main page, ${s.view} view` });
    if (s.levelUp) {
      throw new Halt(`The Level Up button is showing (the quest or the pity bar is done) - press Level Up in the game yourself, then press Start again.`);
    }
    if (s.view === 'levels') {
      // at the top level the bar shows MAX: there is no pity and nothing to level up to
      const top = s.level >= this.solver.maxLevel();
      if (s.level !== this.level || (!top && (this.pity === null || Math.abs(s.pity - this.pity) > 0.015))) {
        this.ui.log(top ? `Main page: inheritor level ${s.level}, the top - farming the Demon Eye.` : `Main page: inheritor level ${s.level}, pity ${Math.round(s.pity * 100)}%.`);
      }
      this.level = s.level;
      this.pity = top ? null : s.pity;
      this.pityRead = true;
      if (!top && s.pity >= 0.995) throw new Halt(`The pity bar is full - level up to ${s.level + 1} in the game, then press Start again.`);
      return this.toggle(frame, 'counts');
    }
    // the counts view
    if (this.level === null) return this.toggle(frame, 'levels');
    let counts = s;
    if (!counts.ok) {
      const again = await this.waitFor((f) => f.screen.kind === 'main' && f.screen.view === 'counts' && f.screen.ok, 4000);
      if (!again) {
        const read = counts.relics.map((r) => `${r.glory ?? '?'}/${r.despair ?? '?'}`).join(' ');
        throw new Halt(`Could not read every relic's glory and despair so they add up to the totals (${read}; totals ${counts.totals.glory ?? '?'}/${counts.totals.despair ?? '?'}) - stopped.`);
      }
      counts = again.screen;
    }
    this.states = counts.relics.map((r) => [r.glory, r.despair]);
    if (this.badges) {
      counts.relics.forEach((r, i) => {
        if (this.stock[i] === null && r.stock != null) [this.stock[i], this.fromBadge[i]] = [r.stock, true];
      });
    }
    const unknown = RELICS.map((_, i) => i).filter((i) => this.stock[i] === null);
    if (unknown.length) {
      const missing = `${unknown.length} stock${unknown.length === 1 ? '' : 's'} not read off the badges`;
      const it = unknown.length === 1 ? 'it is' : 'they are';
      if (this.stockMatters(unknown)) {
        this.ui.log(`Counts: glory ${counts.totals.glory}, despair ${counts.totals.despair}. ${missing} - reading ${unknown.length === 1 ? 'it' : 'them'} off Hero's Legacy.`);
        return this.openRelic(frame, counts, unknown[0], true);
      }
      this.ui.log(`Counts: glory ${counts.totals.glory}, despair ${counts.totals.despair}. ${missing} - the advice is the same whatever ${it}.`);
    }
    return this.decide(frame, counts);
  }

  /** " (n on hand)" for relic `i`, once its stock is read. */
  onHand(i) {
    return this.stock[i] === null ? '' : ` (${this.stock[i]} on hand)`;
  }

  /** Switch the main page to `view` with the circular-arrows icon. */
  async toggle(frame, view) {
    return this.clickFor(frame, (s) => s.toggle, `the ${view} view toggle`,
      (f) => f.screen.kind === 'main' && f.screen.view === view, `the ${view} view`);
  }

  /** Click relic `i`'s tile; Hero's Legacy should follow. */
  async openRelic(frame, screen, i, visiting) {
    this.at = i;
    this.visiting = visiting;
    return this.clickFor(frame, (s) => s.tiles?.[i], RELICS[i], (f) => f.screen.kind === 'legacy', `${RELICS[i]}'s Hero's Legacy`);
  }

  /**
   * The plan's next step from what the main page and the visits showed.  A
   * stock still unread is one the advice does not turn on (see stockMatters).
   */
  advise(stock = this.stock.map((n) => n ?? 0)) {
    // the pity matters to a plan that rolls spare relics only once they can fill the bar
    return this.solver.advise(this.goal, this.level, this.states, stock, this.pity);
  }

  /**
   * Whether the plan's advice turns on the stocks not read yet (relics
   * `unknown`): each tried with none on hand and with plenty, every way
   * round.  The same advice every time and the visits to read them are
   * skipped - at the top level, with the Demon Eye's stock read and 10 or
   * more of it, the rest never matter.
   */
  stockMatters(unknown) {
    if (unknown.length > 4) return true;
    let seen = null;
    for (let mask = 0; mask < 1 << unknown.length; mask++) {
      const stock = this.stock.map((n) => n ?? 0);
      unknown.forEach((i, k) => (stock[i] = mask & (1 << k) ? 9999 : 0));
      let advice;
      try {
        advice = JSON.stringify(this.advise(stock));
      } catch {
        return true;
      }
      if (seen !== null && advice !== seen) return true;
      seen = advice;
    }
    return false;
  }

  async decide(frame, screen) {
    if (this.level > this.goal.level) {
      throw new Halt(`Inheritor level ${this.level} is past the goal (level ${this.goal.level}) - pick a higher goal, then press Start again.`);
    }
    if (++this.idle > 3) throw new Halt('The advisor keeps going back and forth without an attempt - stopped.');
    const a = this.advise();
    const goal = `level ${a.goal}`;
    switch (a.step) {
      case 'levelup':
        throw new Halt(`The requirement for level ${a.goal} is met - level up in the game, then press Start again.`);
      case 'summon':
        throw new Halt(`Nothing the plan needs is affordable (10 of a relic per attempt) - summon more relics, then press Start again.`);
      case 'convert':
        throw new Halt('The Demon Eye of Weakness has fewer than 10 on hand - convert other relics into it (Relic Conversion, 10 of a type for 7), then press Start again.');
      case 'done': {
        const reached = `The goal is reached (${this.goal.level}${this.goal.tier ? ` + Demon Eye at ${this.goal.tier}%` : ''}).`;
        if (!this.goal.tier) throw new Halt(reached);
        // the Final Goal moves on to the next tier above what the Demon Eye has now
        const [g, d] = this.states[RELICS.indexOf('Demon Eye of Weakness')];
        const amp = amplification({ glory: g, despair: d });
        const next = this.ui.raiseGoal?.(amp);
        throw new Halt(`${reached} The Demon Eye is at ${amp}%${next ? ` - the Final Goal is now ${next}; press Start to farm on.` : ', the highest tier.'}`);
      }
      default:
    }
    if (a.relic == null || !a.objective) throw new Halt(`The plan said ${a.step} but gave no relic or way to play it - stopped.`);
    if (a.step === 'filler') {
      this.ui.log(`PITY FILLER: nothing the ${goal} work needs has ${PER_ATTEMPT} on hand, so ${RELICS[a.relic]}${this.onHand(a.relic)} is rolled just for the pity `
        + `(+${this.solver.pityGain(this.level)} an attempt) - its memory changes only if a result improves it. Summon to give the plan something better.`, 'warn');
      return this.openRelic(frame, screen, a.relic, false);
    }
    const why = a.step === 'farm' ? 'farming the crit relic' : `for ${goal}`;
    const bar = a.bar ? ` toward ${a.bar.glory}${a.bar.despair === null ? '' : `/${a.bar.despair}`}` : '';
    this.ui.log(`Advisor: ${RELICS[a.relic]}${this.onHand(a.relic)}, ${why}${bar} - ${describeObjective(a.objective)}, keeping a roll that does better.`, 'good');
    return this.openRelic(frame, screen, a.relic, false);
  }

  // ------------------------------------------------------------ a relic

  async onLegacy(frame) {
    this.expect = null;
    const s = frame.screen;
    const memory = { glory: s.memory.glory.success, despair: s.memory.despair.success };
    this.current = memory;
    this.ui.show(frame, { note: this.at === null ? 'Hero\'s Legacy' : RELICS[this.at] });
    if (this.at === null || !this.states) return this.back(frame); // which relic? read the main page first

    const known = this.states[this.at];
    if (memory.glory !== known[0] || memory.despair !== known[1]) {
      throw new Halt(`${RELICS[this.at]}'s Hero's Legacy shows ${fmtMemory(memory)}, but the main page said ${known[0]}/${known[1]} - stopped (wrong tile?).`);
    }
    let stock = s.stock;
    if (stock === null) {
      const again = await this.waitFor((f) => f.screen.kind === 'legacy' && f.screen.stock !== null, 4000);
      stock = again?.screen.stock ?? null;
    }
    if (stock === null) {
      // a digit this text has not shown before: ask, rather than stop
      this.ui.log(`Could not read ${RELICS[this.at]}'s stock on its Inheritance button - how many are on hand?`, 'ask');
      stock = this.ui.askNumber ? await this.ui.askNumber(`How many ${RELICS[this.at]} are on hand? (the number before "/10")`) : null;
      if (stock === null || !this.running) throw new Halt(`No stock for ${RELICS[this.at]} - stopped.`);
    }
    if (this.fromBadge[this.at] && this.stock[this.at] !== stock) {
      // a badge misread: none of them can be trusted this run
      this.ui.log(`${RELICS[this.at]}'s badge read ${this.stock[this.at]}, but its Hero's Legacy says ${stock} - re-reading every stock off Hero's Legacy.`, 'warn');
      this.badges = false;
      this.fromBadge.forEach((b, i) => {
        if (b) this.stock[i] = null;
      });
      this.fromBadge.fill(false);
      this.stock[this.at] = stock;
      return this.back(frame);
    }
    this.stock[this.at] = stock;
    this.fromBadge[this.at] = false;

    if (this.visiting) {
      this.ui.log(`  ${RELICS[this.at]}: ${known[0]} glory, ${known[1]} despair, ${stock} on hand.`);
      return this.back(frame);
    }

    // working this relic: another attempt, while the plan still picks it
    if (this.finished >= this.attempts) {
      throw new Halt(`Done: ${this.finished} attempt(s). ${RELICS[this.at]}: ${fmtMemory(memory)}, ${stock} on hand.`);
    }
    if (!this.pityRead && this.pity >= PITY_CHECK) return this.back(frame, 'the pity bar may be nearly full - reading it');
    const a = this.advise();
    if (!isRoll(a) || a.relic !== this.at) return this.back(frame, 'the plan moves on');
    if (stock < PER_ATTEMPT) return this.back(frame, `only ${stock} on hand`);

    this.useLevel(this.level, a.objective);
    this.ui.useSettings?.({ level: this.level, objective: a.objective });
    // what this roll is, for its Keep or Replace: the keep rule needs nothing else
    const roll = rollOf(a);
    this.plan = { advice: a, roll, states: this.states.map((x) => [...x]), stock: [...this.stock] };
    await this.ui.saveAttempt?.({
      at: Date.now(), goal: this.goal, level: this.level, relic: this.at, objective: a.objective, roll,
      states: this.plan.states, stock: this.plan.stock, pity: this.pity,
    });
    const filler = a.step === 'filler' ? 'PITY FILLER, ' : '';
    this.ui.log(`${RELICS[this.at]}: ${fmtMemory(memory)}. Clicking Inheritance (attempt ${this.ordinal()}; ${filler}${describeObjective(a.objective)}).`);
    const next = await this.clickFor(frame, (sc) => sc.button, 'Inheritance', (f) => f.screen.kind === 'board', 'the attempt to start');
    this.tier = null;
    return next;
  }

  /** Hero's Legacy's return button, to the main page. */
  async back(frame, why) {
    if (why) this.ui.log(`Back to the main page: ${why}.`);
    if (!frame.screen.back) throw new Halt("Cannot find Hero's Legacy's return button - stopped.");
    this.at = null;
    return this.clickFor(frame, (s) => s.back, 'back', (f) => f.screen.kind === 'main', 'the main page');
  }

  // ------------------------------------------------------------ an attempt in progress

  /**
   * Started part-way through an attempt: finish it the way the advisor began
   * it (saved when it clicked Inheritance), or else by the Single relic
   * settings, then carry on from the main page.
   */
  async resume() {
    if (this.board) return;
    const saved = await this.ui.loadAttempt?.();
    const fresh = saved?.roll && 'horizon' in saved.roll && Date.now() - saved.at < SAVED_FOR && saved.goal?.level === this.goal.level && (saved.goal.tier ?? 0) === (this.goal.tier ?? 0);
    if (fresh) {
      Object.assign(this, { level: saved.level, at: saved.relic, states: saved.states, stock: [...saved.stock], pity: saved.pity });
      // the memory to beat: what lets a hopeless attempt be abandoned at once
      const [glory, despair] = saved.states[saved.relic];
      this.current = { glory, despair };
      this.plan = { roll: saved.roll, states: saved.states, stock: saved.stock };
      this.useLevel(saved.level, saved.objective);
      this.ui.log(`Finishing the ${RELICS[saved.relic]} attempt the advisor started: ${describeObjective(saved.objective)}, kept the plan's way.`);
      return;
    }
    const { level, objective } = this.fallback;
    if (level == null) throw new Halt("An attempt is in progress, and there are no Single relic settings to finish it by - set the level, or finish it yourself.");
    this.useLevel(level, objective);
    this.ui.log(`Finishing the attempt in progress by the Single relic settings: level ${level}, ${describeObjective(objective)}.`);
  }

  async onBoard(frame) {
    await this.resume();
    return super.onBoard(frame);
  }

  async onComplete(frame) {
    await this.resume();
    return super.onComplete(frame);
  }

  async onCompare(frame) {
    await this.resume();
    return super.onCompare(frame);
  }

  // ------------------------------------------------------------ an attempt's end

  /** Keep or replace as the plan does: its keep rule, for the board it rolled under. */
  choose(current, fresh) {
    const p = this.plan;
    if (!p) return chooseMemory(this.objective, current, fresh); // an attempt it did not start
    const keepsNew = keepRule({ roll: p.roll, states: p.states }, this.solver)(fresh);
    if (keepsNew === null) throw new Halt('No keep rule for this attempt - choose Keep or Replace yourself.');
    return keepsNew ? 'replace' : 'keep';
  }

  /**
   * The board moves on: the relic's memory, and the pity one attempt adds -
   * an abandoned one too (the game credits pity either way).
   */
  afterAttempt(choice, applied) {
    if (this.at !== null && this.states) this.states[this.at] = [applied.glory, applied.despair];
    if (this.pity !== null) this.pity += this.solver.pityGain(this.level) / this.solver.pityNeeded(this.level + 1);
    this.pityRead = false;
    this.plan = null;
    this.idle = 0;
    this.ui.saveAttempt?.(null);
    if (this.level === null) this.board = null; // finished by the Single relic settings: read the level next
  }
}
