// What the site knows of the game, and what it advises.
//
// A session is plain data: the board as far as it is known (the main
// page's level, pity bar, every relic's memory and stock), the relic open,
// the attempt in progress.  Every input makes a new session - a screenshot
// read, a button the player pressed and how it went, a number typed in -
// and the advice is worked out from the session each time it is shown,
// never kept, so the same board always gets the same advice.  The decisions
// are the advisor's, out of its lib/advice.js (the extension's too): the Look Ahead
// plan's roll and keep rule in Smart Leveler mode, the Single Relic strategy
// otherwise, the exact solver's move in every attempt, and the reasons.

import {
  MOVE_NAMES, MOVES, PER_ATTEMPT, applyMove, boardOptions, criteria, explainKeep, finished, isRoll, keepRule, legalMoves, opening, rollOf,
  settledNote, spread,
} from './lib/advice.js';
import { RELICS, amplification, describeObjective, hitsObjective, levelList, pickLevel, strategySettings, TIER_PERCENT } from './lib/logic.js';
import { boardState, levelsFitting } from './lib/vision.js';

export { MOVE_NAMES, MOVES, PER_ATTEMPT, RELICS, TIER_PERCENT, applyMove, opening };
const N = RELICS.length;

const copy = (x) => JSON.parse(JSON.stringify(x));
const known = (x) => x !== null && x !== undefined;
export const fmtMemory = (m) => `${m.glory}/${m.despair} (+${amplification(m)}%)`;
const pct = (tier) => `${TIER_PERCENT[tier]}%`;

/** A session with nothing entered yet: every number at its default. */
export function blank() {
  return {
    fresh: true, // nothing read or entered yet
    view: 'main', // the game's screen: 'main', 'legacy', 'board' or 'compare'
    level: 1, // inheritor level
    pity: 0, // the pity bar, 0..1: read off the levels view, then estimated
    levelUp: false, // the main page showed the Level Up button lit (a level-up earned)
    states: Array.from({ length: N }, () => [0, 0]), // every relic's [glory, despair] (the counts view)
    stock: new Array(N).fill(0), // every relic's stock (the counts view's badges)
    relic: null, // the relic open (Hero's Legacy, an attempt), if known
    memory: { glory: 0, despair: 0 }, // its applied memory
    onHand: 0, // its stock
    attempt: null, // the attempt in progress (see begin)
    result: null, // on the results screen: { current, fresh }
    countsOff: null, // the totals line of a counts view the relics did not add up to, until checked
    unsure: [], // the numbers a screenshot did not read (kept as they were), by field: see FIELDS
    levelFits: null, // the levels an attempt's board fitted, when the level was read off it unsure
  };
}

/**
 * The numbers each screen shows, by field name: what the page lets the
 * player edit there, and what a screenshot can leave unread.  g3 / d3 / n3:
 * relic 3's glory, despair and stock.
 */
const FIELDS = {
  main: (f) => /^([gdn]\d+|level|pity)$/.test(f),
  legacy: (f) => f === 'level' || f === 'onHand',
  board: (f) => f === 'level' || f === 'tier',
  compare: (f) => f === 'level',
};

/** Field `f` read (or entered): sure.  Not read: unsure, until it is. */
function mark(s, f, sure) {
  s.unsure = s.unsure.filter((x) => x !== f);
  if (!sure) s.unsure.push(f);
}

// --------------------------------------------------------------- the board

/** Open relic `i` (null: one not known): its memory and stock as the board has them. */
function open(s, i) {
  s.relic = i;
  s.memory = i !== null ? { glory: s.states[i][0], despair: s.states[i][1] } : { glory: 0, despair: 0 };
  s.onHand = i !== null ? s.stock[i] : 0;
}

/** The open relic's applied memory, on the board too. */
function setMemory(s, m) {
  s.memory = m;
  if (s.relic !== null && s.states) s.states[s.relic] = [m.glory, m.despair];
}

/** The open relic's stock, on the board too. */
function setOnHand(s, n) {
  s.onHand = n;
  if (s.relic !== null) s.stock[s.relic] = n;
}

/** The Look Ahead plan's next step from the board, or null while the board is not all known. */
export function planAdvice(s, settings, solver) {
  if (settings.mode !== 'advisor' || s.level > settings.goal.level) return null;
  try {
    return solver.advise(settings.goal, s.level, s.states, s.stock, s.pity);
  } catch {
    return null;
  }
}

/**
 * How an attempt on the open relic is played and kept, from the board as it
 * stands before it: the plan's roll, in Smart Leveler mode on the relic the
 * plan picks; otherwise the Single Relic strategy, kept when it ranks higher.
 */
export function planFor(s, settings, solver) {
  const a = planAdvice(s, settings, solver);
  if (isRoll(a) && a.relic === s.relic) {
    return { by: 'plan', step: a.step, goal: a.goal, bar: a.bar ?? null, objective: a.objective, roll: rollOf(a), states: copy(s.states) };
  }
  return { by: 'settings', objective: settings.objective, roll: null, states: null };
}

/**
 * The relic a screen seen from the main page is most likely on: the one the
 * plan points at, if its memory is `memory` (or that is not known).
 */
function guessRelic(s, settings, solver, memory = null) {
  const a = planAdvice(s, settings, solver);
  if (!isRoll(a)) return null;
  const [g, d] = s.states[a.relic];
  return !memory || (g === memory.glory && d === memory.despair) ? a.relic : null;
}

/**
 * Relic `i` is the one open: named, if the relic shown was not known (what
 * was read on it belongs to it) or in Single Relic mode (where the name is all
 * it changes), or else another relic, as the board has it.  An attempt in
 * progress goes with it: its 10 relics spent on this one, played and kept
 * as this one is.
 */
function setRelic(s, i, settings, solver) {
  const att = s.attempt;
  if (att) setOnHand(s, s.onHand + att.paid);
  if (s.relic === null || settings.mode !== 'advisor') {
    const { memory, onHand } = s;
    s.relic = i;
    setMemory(s, memory);
    setOnHand(s, onHand);
  } else open(s, i);
  if (att) Object.assign(att, planFor(s, settings, solver), { from: { ...s.memory } }, pay(s));
}

/** An attempt's relics taken off the stock of the relic open: { paid }, how many there were to take. */
function pay(s) {
  const paid = Math.min(PER_ATTEMPT, s.onHand);
  setOnHand(s, s.onHand - paid);
  return { paid };
}

/**
 * Each bar's slots in the order they were filled ('success' or 'fail'), as
 * the game shows them: `bars` if it fits the state (the order seen), else
 * successes first.
 */
export function slotsOf(st, bars = null) {
  const count = (list, v) => list.filter((x) => x === v).length;
  const fits = (list, filled, won) => list && list.length === filled && count(list, 'success') === won;
  const plain = (filled, won) => [...new Array(won).fill('success'), ...new Array(filled - won).fill('fail')];
  return {
    glory: fits(bars?.glory, st.gf, st.gs) ? bars.glory : plain(st.gf, st.gs),
    despair: fits(bars?.despair, st.df, st.ds) ? bars.despair : plain(st.df, st.ds),
  };
}

/** Inheritance: the attempt starts on the open relic, and 10 of it are spent. */
function begin(s, settings, solver) {
  const { st, tier } = opening(solver, s.level);
  s.attempt = { level: s.level, ...planFor(s, settings, solver), from: { ...s.memory }, st, tier, bars: slotsOf(st), ...pay(s) };
  s.result = null;
  s.view = 'board';
}

/**
 * The attempt is over: `applied` is the memory now in place.  The pity bar
 * takes one attempt more only for an attempt seen to its end - completed
 * (kept or replaced) or abandoned; one left behind by a screenshot of
 * another screen part-way through may never have ended, so it adds none.
 */
function end(s, solver, applied, counted = true) {
  const level = s.attempt?.level ?? s.level;
  if (applied) setMemory(s, applied);
  if (counted) s.pity = Math.min(1, s.pity + solver.pityGain(level) / solver.pityNeeded(level + 1));
  s.attempt = null;
  s.result = null;
  s.view = 'legacy';
}

/**
 * Whether the attempt in progress was seen to its end: its bars full, its
 * results shown, or wiped (no move left: it can only have been abandoned).
 */
function completed(s, solver) {
  const att = s.attempt, board = solver.board(att.level);
  return s.view === 'compare' || finished(att.st, board) || !legalMoves(att.st, board).length;
}

/** The memory the attempt plays against changed (a misread put right): its keep rule too. */
function setFrom(s, m) {
  const att = s.attempt;
  if (!att) return;
  att.from = m && { ...m };
  if (att.states && m) att.states[att.roll.relic] = [m.glory, m.despair];
}

/** The solver set up for the attempt in progress. */
function aim(att, solver) {
  solver.configure(att.level, att.objective, att.from);
}

// ------------------------------------------------------------------ inputs

/**
 * The session after an input, and what to log: { session, log }.
 *  { type: 'move', action, success }  a move made, and how it went
 *  { type: 'press', button }          'inheritance', 'complete', 'keep',
 *                                     'replace', 'abandon', 'back', 'levelup'
 *  { type: 'tile', relic }            a relic opened from the main page
 *  { type: 'read', reading }          a screenshot read (screenshot.js)
 *  { type: 'set', ... }               a number put right: level, pity,
 *                                     relic, memory, onHand, tier, st,
 *                                     relicStates: [{ i, glory, despair, stock }]
 * Throws an Error for an input that cannot be.
 */
export function act(prev, input, settings, solver) {
  const s = copy(prev);
  const log = [];
  switch (input.type) {
    case 'move': {
      const att = s.attempt;
      if (s.view !== 'board' || !att) throw new Error('No attempt in progress.');
      const board = solver.board(att.level);
      if (!legalMoves(att.st, board).includes(input.action)) throw new Error(`${MOVE_NAMES[input.action]} cannot be attempted now.`);
      const before = att.tier;
      const bars = slotsOf(att.st, att.bars);
      if (input.action !== 'train') bars[input.action].push(input.success ? 'success' : 'fail');
      Object.assign(att, applyMove(att.st, att.tier, input.action, input.success, board), { bars });
      log.push(`${MOVE_NAMES[input.action]}${before === null ? '' : ` at ${pct(before)}`}: ${input.success ? 'success' : 'fail'}.`);
      if (finished(att.st, board)) log.push(`Attempt finished: ${fmtMemory({ glory: att.st.gs, despair: att.st.ds })}.`);
      break;
    }
    case 'press':
      press(s, input.button, settings, solver, log);
      break;
    case 'tile':
      if (s.attempt) throw new Error('Finish the attempt in progress first.');
      open(s, input.relic);
      s.view = 'legacy';
      log.push(`Opened ${RELICS[input.relic]}.`);
      break;
    case 'read':
      read(s, input.reading, settings, solver, log);
      break;
    case 'set':
      set(s, input, settings, solver);
      break;
    default:
      throw new Error(`unknown input ${input.type}`);
  }
  s.fresh = false;
  return { session: s, log };
}

function press(s, button, settings, solver, log) {
  const att = s.attempt;
  switch (button) {
    case 'inheritance':
      if (s.view !== 'legacy') throw new Error("Inheritance is on Hero's Legacy.");
      begin(s, settings, solver);
      log.push(`Inheritance${s.relic !== null ? ` on ${RELICS[s.relic]}` : ''}: ${describeObjective(s.attempt.objective)}.`);
      return;
    case 'complete': {
      if (!att || !finished(att.st, solver.board(att.level))) throw new Error('The bars are not full yet.');
      const fresh = { glory: att.st.gs, despair: att.st.ds };
      s.result = { current: { ...att.from }, fresh };
      s.view = 'compare';
      log.push('Complete Inheritance.');
      return;
    }
    case 'keep':
    case 'replace': {
      if (s.view !== 'compare' || !s.result) throw new Error('Keep and Replace are on the results screen.');
      const applied = button === 'keep' ? s.result.current : s.result.fresh;
      log.push(`${button === 'keep' ? 'Kept the current memory' : 'Replaced with the new memory'}${applied ? `: ${fmtMemory(applied)}` : ''}.`);
      end(s, solver, applied);
      return;
    }
    case 'abandon':
      if (s.view !== 'board' || !att) throw new Error('No attempt to abandon.');
      log.push(`Abandoned the attempt: the memory stays ${s.memory ? fmtMemory(s.memory) : 'as it was'}.`);
      end(s, solver, null);
      return;
    case 'back':
      if (att) throw new Error('Finish the attempt in progress first.');
      open(s, null);
      s.view = 'main';
      log.push('Back to the main page.');
      return;
    case 'levelup':
      if (s.level >= 20) throw new Error('Level 20 is the top of the requirement table.');
      s.level += 1;
      s.pity = 0; // the bar empties on every level-up
      s.levelUp = false;
      log.push(`Levelled up to ${s.level}.`);
      return;
    default:
      throw new Error(`unknown button ${button}`);
  }
}

/** Every level's board, as the solver has it. */
function levelBoards(solver) {
  return Array.from({ length: 20 }, (_, i) => solver.board(i + 1));
}

/**
 * The inheritor level, off an attempt's screenshot (the game does not show
 * it there): the level set if the board fits it, else the highest it fits -
 * marked to check when other levels fit too.  An attempt in progress is at
 * that level from now on.
 */
function readLevel(s, r, solver, log) {
  const boards = levelBoards(solver);
  let fits = r.kind === 'board' ? levelsFitting(r.screen, boards) : boards.filter((b) => b.slots === r.slots).map((b) => b.level);
  // a level that is itself a guess is not confirmed by fitting: the two guesses narrow each other
  const guessed = s.unsure.includes('level');
  if (guessed && s.levelFits) fits = fits.filter((l) => s.levelFits.includes(l)).length ? fits.filter((l) => s.levelFits.includes(l)) : fits;
  const pick = pickLevel(fits, boards, guessed ? null : s.level);
  if (!pick) throw new Error('That attempt fits no inheritor level I know.');
  if (pick.level !== s.level) {
    log.push(`Inheritor level ${pick.level}, read off the attempt${pick.sure ? '' : ` (it fits ${levelList(fits)})`}.`);
    s.level = pick.level;
    s.levelUp = false;
    if (s.attempt) s.attempt.level = pick.level;
  }
  mark(s, 'level', pick.sure);
  s.levelFits = pick.sure ? null : fits;
}

/**
 * A memory shown with `slots` slots a bar: it was inherited at a level with
 * that many, and levels only go up, so the inheritor level has at least
 * that many.  A level set below that is raised to the highest level with
 * that many (a guess: marked to check).
 */
function atLeast(s, slots, solver, log) {
  if (!slots || solver.board(s.level).slots >= slots) return;
  const fits = levelBoards(solver).filter((b) => b.slots === slots).map((b) => b.level);
  if (!fits.length) return;
  s.level = Math.max(...fits);
  s.levelUp = false;
  mark(s, 'level', false);
  s.levelFits = fits;
  log.push(`That memory has ${slots} slots a bar: the inheritor level is at least ${fits[0]}, set to ${s.level} - check it.`);
}

/** A screenshot's reading, merged in: the screen it shows is where the game is now. */
function read(s, r, settings, solver, log) {
  switch (r.kind) {
    case 'levels':
      if (s.attempt) end(s, solver, null);
      if (r.level !== null) s.level = r.level;
      mark(s, 'level', r.level !== null);
      s.levelFits = null;
      s.pity = r.pity;
      mark(s, 'pity', true);
      s.levelUp = r.levelUp;
      open(s, null);
      s.view = 'main';
      log.push(`Read the levels view: inheritor level ${r.level ?? 'unread'}, pity ${Math.round(100 * r.pity)}%${r.levelUp ? ', Level Up showing' : ''}.`);
      return;
    case 'counts': {
      if (s.attempt) end(s, solver, null, completed(s, solver));
      // what would not read stays as it was, marked to be checked
      r.states.forEach((x, i) => {
        if (x) s.states[i] = [...x];
        mark(s, `g${i}`, !!x);
        mark(s, `d${i}`, !!x);
      });
      r.stock.forEach((n, i) => {
        if (known(n)) s.stock[i] = n;
        mark(s, `n${i}`, known(n));
      });
      s.countsOff = r.ok ? null : r.totals;
      s.levelUp = r.levelUp;
      open(s, null);
      s.view = 'main';
      const missing = r.stock.filter((n) => !known(n)).length;
      log.push(`Read the counts view: totals ${r.totals.glory ?? '?'} glory, ${r.totals.despair ?? '?'} despair${r.ok ? '' : " - the relics' counts do not add up to the totals, check them"}`
        + `${missing ? `; ${missing} stock badge${missing === 1 ? '' : 's'} would not read` : ''}.`);
      return;
    }
    case 'legacy': {
      atLeast(s, r.slots, solver, log);
      // the relic: the one open, or the one the plan points at, if its memory fits
      if (s.view === 'main' && s.relic === null) open(s, guessRelic(s, settings, solver, r.memory));
      if (s.attempt) {
        // the attempt ended in the game: what it left is in place now (its pity only if it was seen to its end)
        log.push(`The attempt ended in the game: ${fmtMemory(r.memory)} in place.`);
        end(s, solver, r.memory, completed(s, solver));
      } else if (s.relic !== null && s.memory && (s.memory.glory !== r.memory.glory || s.memory.despair !== r.memory.despair)) {
        // not the memory the relic open has: another relic, or a misread board - which, the player says
        log.push(`Hero's Legacy shows ${fmtMemory(r.memory)}, but ${RELICS[s.relic]} has ${fmtMemory(s.memory)} - which relic is this?`);
        s.relic = null;
      }
      setMemory(s, r.memory);
      if (known(r.stock)) setOnHand(s, r.stock);
      mark(s, 'onHand', known(r.stock));
      s.view = 'legacy';
      log.push(`Read Hero's Legacy${s.relic !== null ? ` (${RELICS[s.relic]})` : ''}: ${fmtMemory(r.memory)}, ${known(r.stock) ? `${r.stock} on hand` : 'stock unread'}.`);
      return;
    }
    case 'board':
    case 'complete': {
      readLevel(s, r, solver, log);
      const board = solver.board(s.level);
      let st, tier;
      if (r.kind === 'board') {
        st = boardState(r.screen, board.slots, board.maxSpirit);
        if (st.error) throw new Error(`${st.error[0].toUpperCase()}${st.error.slice(1)}.`);
        tier = r.tier;
      } else {
        if (!r.full) throw new Error('The Complete Inheritance screen shows unfilled slots.');
        st = { gf: board.slots, gs: r.memory.glory, df: board.slots, ds: r.memory.despair, ms: 0, sp: 0 };
        tier = null;
      }
      if (s.view === 'compare' && r.kind === 'complete') s.view = 'board'; // the same attempt, a step back
      if (s.view === 'compare') {
        // a new attempt: the last one was kept or replaced in the game, as advised
        const keeps = s.result.current ? keepRule(s.attempt, solver)(s.result.fresh) : true;
        log.push(`The last attempt was ${keeps ? 'replaced' : 'kept'} in the game, as advised.`);
        end(s, solver, keeps ? s.result.fresh : s.result.current);
      }
      if (s.view !== 'board') {
        // started from the main page: the relic the plan points at, if any
        if (s.view === 'main') open(s, guessRelic(s, settings, solver));
        begin(s, settings, solver);
        log.push(`An attempt${s.relic !== null ? ` on ${RELICS[s.relic]}` : ''}: ${describeObjective(s.attempt.objective)}.`);
      }
      const att = s.attempt;
      const start = opening(solver, att.level);
      // the rate: read, else the opening's, else carried on from the moves entered if the board is
      // where they left it - and if none of those, the last known, to be checked
      const same = JSON.stringify(st) === JSON.stringify(att.st), opens = JSON.stringify(st) === JSON.stringify(start.st);
      att.tier = tier ?? (opens ? start.tier : att.tier);
      mark(s, 'tier', tier !== null || opens || same || r.kind === 'complete');
      att.st = st;
      // the slots in the order the screenshot shows them
      const filled = (cells) => cells.filter((v) => v === 'success' || v === 'fail');
      att.bars = r.kind === 'board' ? slotsOf(st, { glory: filled(r.screen.glory), despair: filled(r.screen.despair) }) : slotsOf(st, att.bars);
      log.push(`Read the board: glory ${st.gs}/${st.gf}, despair ${st.ds}/${st.df}, spirit ${st.sp}, mental ${st.ms}, ${tier !== null ? pct(tier) : 'rate unread'}.`);
      return;
    }
    case 'compare': {
      if (!r.current || !r.fresh) throw new Error('Could not read both memory cards on that screenshot.');
      if (!s.attempt) {
        // an attempt not seen: the one the cards tell of, on the relic open or the plan's
        if (s.view === 'main') open(s, guessRelic(s, settings, solver, r.current));
        setMemory(s, r.current);
        begin(s, settings, solver);
      }
      if (s.attempt.from && (s.attempt.from.glory !== r.current.glory || s.attempt.from.despair !== r.current.despair)) {
        log.push(`The current memory card reads ${fmtMemory(r.current)}, not the ${fmtMemory(s.attempt.from)} known - going by the card.`);
      }
      setMemory(s, r.current);
      setFrom(s, r.current);
      const board = solver.board(s.attempt.level);
      s.attempt.st = { gf: board.slots, gs: r.fresh.glory, df: board.slots, ds: r.fresh.despair, ms: 0, sp: 0 };
      s.result = { current: r.current, fresh: r.fresh };
      s.view = 'compare';
      log.push(`Read the results: new ${fmtMemory(r.fresh)}, current ${fmtMemory(r.current)}.`);
      return;
    }
    default:
      throw new Error("That screenshot is not a screen I know: the main page (either view), Hero's Legacy, an attempt, or the results.");
  }
}

function set(s, input, settings, solver) {
  const int = (v, lo, hi, what) => {
    if (!Number.isInteger(v) || v < lo || v > hi) throw new Error(`${what} must be a whole number from ${lo} to ${hi}.`);
    return v;
  };
  if ('level' in input) {
    const was = s.level;
    s.level = int(input.level, 1, 20, 'The inheritor level');
    mark(s, 'level', true);
    s.levelFits = null;
    if (was !== s.level) s.levelUp = false;
    const att = s.attempt;
    if (att && was !== s.level) {
      const untouched = JSON.stringify(att.st) === JSON.stringify(opening(solver, att.level).st);
      const b = solver.board(s.level), st = att.st;
      if (!untouched && (st.gf > b.slots || st.df > b.slots || st.ms > b.slots || st.sp > b.maxSpirit)) {
        throw new Error(`The attempt in progress does not fit level ${s.level} (${b.slots} slots, ${b.maxSpirit} spirit power): it has gone further than that board can.`);
      }
      att.level = s.level;
      if (untouched) Object.assign(att, opening(solver, s.level));
    }
  }
  if ('countsChecked' in input) s.countsOff = null;
  if ('pity' in input) {
    s.pity = int(input.pity, 0, 100, 'The pity bar') / 100;
    mark(s, 'pity', true);
  }
  // a relic is named, never un-named: only a screenshot that does not match it can make it unknown again
  if ('relic' in input) setRelic(s, int(input.relic, 0, N - 1, 'The relic'), settings, solver);
  if ('memory' in input) {
    const { slots } = solver.board(s.level);
    int(input.memory.glory, 0, slots, `Glory (level ${s.level} has ${slots} slots)`);
    int(input.memory.despair, 0, slots, `Despair (level ${s.level} has ${slots} slots)`);
    setMemory(s, input.memory);
    setFrom(s, input.memory);
    if (s.result) s.result.current = input.memory;
  }
  if ('onHand' in input) {
    setOnHand(s, int(input.onHand, 0, 99999, 'The stock'));
    mark(s, 'onHand', true);
  }
  // any of a relic's glory, despair and stock
  for (const { i, glory, despair, stock } of input.relicStates ?? []) {
    int(i, 0, N - 1, 'The relic');
    for (const [k, v, f] of [[0, glory, 'g'], [1, despair, 'd']]) {
      if (v === undefined) continue;
      s.states[i][k] = int(v, 0, 10, k ? 'Despair' : 'Glory');
      mark(s, `${f}${i}`, true);
    }
    if (s.relic === i) s.memory = { glory: s.states[i][0], despair: s.states[i][1] };
    const sum = (k) => s.states.reduce((a, x) => a + x[k], 0);
    if (s.countsOff && sum(0) === s.countsOff.glory && sum(1) === s.countsOff.despair) s.countsOff = null;
    if (stock !== undefined) {
      s.stock[i] = int(stock, 0, 99999, 'The stock');
      mark(s, `n${i}`, true);
      if (s.relic === i) s.onHand = s.stock[i];
    }
  }
  if ('tier' in input && s.attempt) {
    s.attempt.tier = int(input.tier, 0, 4, 'The rate');
    mark(s, 'tier', true);
  }
  if ('fresh' in input && s.result) {
    // the new memory card, put right: the attempt's result
    const board = solver.board(s.attempt.level);
    s.result.fresh = { glory: int(input.fresh.glory, 0, board.slots, 'Glory'), despair: int(input.fresh.despair, 0, board.slots, 'Despair') };
    Object.assign(s.attempt.st, { gs: s.result.fresh.glory, ds: s.result.fresh.despair });
  }
  if ('st' in input && s.attempt) {
    const board = solver.board(s.attempt.level);
    const st = input.st;
    for (const [k, hi] of [['gf', board.slots], ['df', board.slots], ['ms', board.slots], ['sp', board.maxSpirit]]) int(st[k], 0, hi, k);
    if (!(st.gs >= 0 && st.gs <= st.gf && st.ds >= 0 && st.ds <= st.df)) throw new Error('More successes than filled slots.');
    s.attempt.st = { ...st };
  }
}

// ------------------------------------------------------------------ advice

/**
 * What to press next and why: {
 *   press: the game button the player should press ('glory', 'despair',
 *     'train', 'complete', 'abandon', 'inheritance', 'back', 'tile',
 *     'keep', 'replace', 'levelup') or null; tile: for 'tile', the relic;
 *   headline, detail: in words;
 *   needs: what has to be settled first ([{ what, text }], what: 'states'
 *     (counts that do not add up) or 'relic' (which relic is shown));
 *   check: the fields on the screen shown that did not read, to check;
 *   levelFits: the levels an attempt's board fitted, when the level is one of them unsure;
 *   stop: true when there is nothing to press (done, summon, ...);
 *   moves | prospect | compare | plan: the why, for the view shown
 * }
 */
export function advise(s, settings, solver) {
  // an attempt played by the Single Relic strategy follows the strategy as it is set now
  if (s.attempt?.by === 'settings') s = { ...s, attempt: { ...s.attempt, objective: settings.objective } };
  const a = s.view === 'board' ? adviseBoard(s, settings, solver)
    : s.view === 'compare' ? adviseCompare(s, solver)
      : s.view === 'legacy' ? adviseLegacy(s, settings, solver) : adviseMain(s, settings, solver);
  const check = s.unsure.filter(FIELDS[s.view] ?? FIELDS.main);
  if (check.length) a.check = check;
  if (check.includes('level') && s.levelFits) a.levelFits = s.levelFits;
  return a;
}

function needs(list) {
  return { press: null, needs: list, headline: 'A few things to fill in', detail: list.map((n) => n.text).join(' ') };
}

function adviseMain(s, settings, solver) {
  if (s.fresh) {
    return {
      press: null, start: true, headline: 'Add a screenshot to start',
      detail: "Paste or drop a screenshot of the relic's Hero's Legacy, or the game's main page in both views for Smart Leveler: the levels view (inheritor level and pity bar) and the counts view (the circular-arrows icon: every relic's glory, despair and stock). Or type the numbers into the game screen below.",
    };
  }
  if (settings.mode !== 'advisor') {
    return { press: null, headline: 'Open the relic to roll', detail: "Tap its tile, or add a screenshot of its Hero's Legacy." };
  }
  const need = [];
  if (s.countsOff) {
    need.push({ what: 'states', text: `The relics' glory and despair do not add up to the Total line (${s.countsOff.glory ?? '?'} glory, ${s.countsOff.despair ?? '?'} despair): check them, then confirm.` });
  }
  if (need.length) return needs(need);
  const goal = settings.goal;
  if (s.level > goal.level) {
    return { press: null, stop: true, headline: 'Past the goal', detail: `Inheritor level ${s.level} is past the goal (level ${goal.level}) - pick a higher goal.` };
  }
  if (s.level < goal.level && (s.levelUp || s.pity >= 0.995)) {
    const why = s.levelUp ? 'The game shows Level Up lit (the requirement is met or the pity bar is full)' : 'The pity bar is full';
    return { press: 'levelup', headline: 'Level Up', detail: `${why}: level up to ${s.level + 1}.` };
  }
  const a = planAdvice(s, settings, solver);
  if (!a) return { press: null, stop: true, headline: 'No advice', detail: `The plan has no step for level ${s.level} toward ${goal.level} from this board.` };
  const plan = explainPlan(s, a, solver);
  switch (a.step) {
    case 'levelup':
      return { press: 'levelup', plan, headline: 'Level Up', detail: `The board meets level ${a.goal}'s requirement: level up.` };
    case 'summon':
      return { press: null, stop: true, plan, headline: 'Summon', detail: `Nothing the plan needs has ${PER_ATTEMPT} on hand. Summon, then add a counts view screenshot.` };
    case 'convert':
      return { press: null, stop: true, plan, headline: 'Convert into Demon Eyes', detail: 'Convert the other relics into Demon Eye of Weakness, then add a counts view screenshot.' };
    case 'done':
      return { press: null, stop: true, plan, headline: 'Goal reached', detail: `Level ${goal.level}${goal.tier ? ` with Demon Eye at ${goal.tier}%` : ''}: nothing more to do.` };
    default:
      return {
        press: 'tile', tile: a.relic, plan,
        headline: `Open ${RELICS[a.relic]}`,
        detail: a.step === 'filler'
          ? `A pity filler: nothing the level ${a.goal} work needs has ${PER_ATTEMPT} on hand, so ${RELICS[a.relic]} is rolled for the pity (+${solver.pityGain(s.level)} an attempt). Summon to give the plan something better.`
          : a.step === 'farm' ? `Farming the crit relic for +${a.objective.mark}%.` : `Rolled for level ${a.goal}${a.bar ? ` toward ${barText(a.bar)}` : ''}.`,
      };
  }
}

const barText = (bar) => `${bar.glory}${bar.despair === null ? '' : `/${bar.despair}`}`;

/** The plan's step in numbers: the requirement, the board's totals, the relic and how it is rolled. */
function explainPlan(s, a, solver) {
  const totals = s.states.reduce(([g, d], [rg, rd]) => [g + rg, d + rd], [0, 0]);
  return {
    step: a.step, goal: a.goal, relic: a.relic ?? null, bar: a.bar ?? null, objective: a.objective ?? null,
    horizon: a.horizon, quest: a.goal > s.level ? solver.quest(a.goal) : null, totals,
  };
}

function adviseLegacy(s, settings, solver) {
  if (settings.mode === 'advisor' && s.relic === null) return needs([{ what: 'relic', text: 'Which relic is this? Pick it in its name on the game screen.' }]);
  if (settings.mode === 'advisor') {
    const a = planAdvice(s, settings, solver);
    if (!isRoll(a) || a.relic !== s.relic) {
      const next = !a ? 'the board is not all known' : a.relic != null ? `the plan rolls ${RELICS[a.relic]} next` : `the plan's next step is ${a.step}`;
      return { press: 'back', headline: 'Back to the main page', detail: `Not this relic: ${next}.` };
    }
    if (s.onHand < PER_ATTEMPT) {
      return { press: 'back', headline: 'Back to the main page', detail: `Only ${s.onHand} on hand - an attempt takes ${PER_ATTEMPT}.` };
    }
  } else {
    if (hitsObjective(settings.objective, s.memory)) {
      return { press: null, stop: true, headline: 'Done', detail: `The memory ${fmtMemory(s.memory)} already meets the strategy's target (${strategySettings(settings.objective)}).` };
    }
    if (s.onHand < PER_ATTEMPT) {
      return { press: null, stop: true, headline: 'Out of relics', detail: `Only ${s.onHand} on hand - an attempt takes ${PER_ATTEMPT}.` };
    }
  }
  const prospect = prospectOf(s, settings, solver);
  return {
    press: 'inheritance', prospect,
    headline: 'Inheritance',
    detail: `Play it for ${describeObjective(prospect.objective)}.`,
  };
}

/** What an attempt from here would come to: its results, and the chance one is kept. */
function prospectOf(s, settings, solver) {
  const plan = planFor(s, settings, solver);
  const att = { level: s.level, ...plan, from: s.memory };
  aim(att, solver);
  const { st, tier } = opening(solver, s.level);
  return { by: plan.by, step: plan.step ?? null, objective: plan.objective, from: s.memory, ...spread(solver.outcomes(st, tier), keepRule(att, solver)), score: solver.expected(st, tier), criteria: criteria(att, solver) };
}

function adviseBoard(s, settings, solver) {
  const att = s.attempt;
  const board = solver.board(att.level);
  aim(att, solver);
  if (finished(att.st, board)) {
    const fresh = { glory: att.st.gs, despair: att.st.ds };
    const next = att.from ? keepRule(att, solver)(fresh) : null;
    return {
      press: 'complete', headline: 'Complete Inheritance', detail: `The bars are full: ${fmtMemory(fresh)}.`,
      // what the results screen will say, ahead of it
      compare: next === null ? null : explainKeep(solver, att, att.from, fresh), next: next === null ? null : next ? 'replace' : 'keep',
    };
  }
  // the options: every move the board allows, and Abandon Inheritance - each with where it leads
  const { moves, criteria: crit, best, abandon } = boardOptions(solver, att);
  const useless = abandon.useless && `even the best result still possible (${fmtMemory(abandon.useless)}) would not replace ${fmtMemory(att.from)}`;
  const base = { moves, abandon: { wiped: abandon.wiped, doomed: abandon.doomed, useless }, criteria: crit, best, objective: att.objective, by: att.by, from: att.from };
  if (abandon.wiped) {
    return { ...base, press: 'abandon', headline: 'Abandon Inheritance', detail: 'Out of spirit power and mental strength with the bars unfinished: abandon it (the memory stays as it was; the pity still counts).' };
  }
  if (abandon.doomed) {
    return { ...base, press: 'abandon', headline: 'Abandon Inheritance', detail: 'The spirit power and mental strength left cannot fill the bars, so this attempt can only wipe: abandon it now (the memory stays as it was; the pity still counts).' };
  }
  if (useless) {
    return { ...base, press: 'abandon', headline: 'Abandon Inheritance', detail: `Nothing useful is left in this attempt: ${useless}. Abandon it - the relics are spent either way, and the pity still counts.` };
  }
  const note = settledNote(solver, att);
  return {
    ...base, press: best,
    headline: `${MOVE_NAMES[best]} at ${pct(att.tier)}`,
    detail: `${note ? `${note} ` : ''}Played for ${describeObjective(att.objective)}.`,
  };
}

function adviseCompare(s, solver) {
  const att = s.attempt;
  const { current, fresh } = s.result;
  const keeps = keepRule(att, solver)(fresh);
  const choice = keeps ? 'replace' : 'keep';
  // a relic never inherited: the game skips this screen and applies the result itself
  const none = current.glory === 0 && current.despair === 0
    ? " (If the game went straight back to Hero's Legacy, the relic had no memory yet and the new one is already in place: press Done.)" : '';
  return {
    press: choice,
    headline: choice === 'keep' ? 'Keep Current Effect' : 'Replace with New Effect',
    detail: `${keeps ? `The new ${fmtMemory(fresh)} ranks above ${fmtMemory(current)}.` : `The new ${fmtMemory(fresh)} does not rank above ${fmtMemory(current)}.`}${none}`,
    compare: explainKeep(solver, att, current, fresh),
  };
}
