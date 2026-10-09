// How an attempt is advised, and why - the part the extension and the
// website share.  No Chrome, no pixels, no page: a board in, the options on
// it, which one is advised, and the numbers that make it so.
//
// An attempt here is plain data: { level, objective, from (the memory it
// plays against), roll and states (the plan's keep rule, in Smart Leveler
// mode: see rollOf), st (the solver's state), tier }.

import { amplification, chooseMemory, gapTo, hitsObjective, nextTier, strategySettings, TIER_PERCENT } from './logic.js';

export const PER_ATTEMPT = 10;
export const MOVES = ['glory', 'despair', 'train'];
export const MOVE_NAMES = { glory: 'Memory of Glory', despair: 'Memory of Despair', train: 'Mental Training' };

/** The plan's steps that roll a relic. */
const ROLLS = ['quest', 'filler', 'farm'];

/** Whether the plan's advice `a` rolls a relic. */
export const isRoll = (a) => !!a && ROLLS.includes(a.step);

/**
 * What a roll the plan advises is, for its Keep or Replace (see keepRule):
 * the totals it closes the gap to, the level whose quest it works on, the
 * relic, the bar it is rolled toward, and whether it farms the crit relic.
 */
export function rollOf(a) {
  return { horizon: a.horizon ?? null, goal: a.goal, relic: a.relic, bar: a.bar ?? null, farm: a.step === 'farm' };
}

// ------------------------------------------------------------------ the rules

/** The state every attempt at `level` opens on, and its rate. */
export function opening(solver, level) {
  const b = solver.board(level);
  return { st: { gf: b.startGlory, gs: b.startGlory, df: b.startDespairFail, ds: 0, ms: b.slots, sp: b.maxSpirit }, tier: b.startTier };
}

/**
 * Single Relic's settings for a relic whose Hero's Legacy shows `memory`, at
 * inheritor `level`: only the ones the memory already meets, each moved just
 * past it, since only a better memory is kept.  `now` holds all three
 * strategies' settings ({ mark (null: blank, 0), glory, despair, closeGlory,
 * closeDespair }); what comes back is the ones to change, with a line saying
 * why (empty: nothing changes).
 *  - Minimum Useful Amplification: the memory's amplification + 1%, while
 *    the board has room above it (5% a slot).
 *  - Glory Target / Despair Target: one glory or one despair better than the
 *    memory, whichever an attempt at this level is likelier to reach (a
 *    glory on a tie: it is worth more).
 *  - Max useful Glory / Min useful Despair: every slot a glory, no despair.
 */
export function singleRelicSettings(solver, level, memory, now) {
  const board = solver.board(level);
  const { glory: g, despair: d } = memory;
  const amp = amplification(memory);
  const out = {};
  const why = [];
  if ((now.mark ?? 0) <= amp && amp < 5 * board.slots) {
    out.mark = amp + 1;
    why.push(`Minimum Useful Amplification ${amp + 1}%`);
  }
  if (hitsObjective({ kind: 'target', glory: now.glory, despair: now.despair }, memory)) {
    const ways = [];
    if (g < board.slots) ways.push({ glory: Math.max(g + 1, board.startGlory), despair: Math.min(d, board.slots - board.startDespairFail) });
    if (d > 0) ways.push({ glory: Math.max(g, board.startGlory), despair: Math.min(d - 1, board.slots - board.startDespairFail) });
    if (ways.length) {
      const { st, tier } = opening(solver, level);
      const chance = (t) => {
        solver.configure(level, { kind: 'target', ...t }, memory);
        return solver.expected(st, tier);
      };
      const best = ways.length === 1 || chance(ways[0]) >= chance(ways[1]) ? ways[0] : ways[1];
      Object.assign(out, best);
      why.push(`Glory Target ${best.glory}, Despair Target ${best.despair}`);
    }
  }
  if (hitsObjective({ kind: 'close', glory: now.closeGlory, despair: now.closeDespair }, memory) && (g < board.slots || d > 0)) {
    Object.assign(out, { closeGlory: board.slots, closeDespair: 0 });
    why.push(`Max useful Glory ${board.slots}, Min useful Despair 0`);
  }
  out.said = why.length ? `The current memory (${g} glory, ${d} despair, +${amp}%) already met them: set ${why.join('; ')}.` : '';
  return out;
}

/** The moves a board allows: an Attempt with spirit power and a slot left, training with mental strength. */
export function legalMoves(st, board) {
  return MOVES.filter((m) => (m === 'train' ? st.ms > 0 : st.sp > 0 && st[m === 'glory' ? 'gf' : 'df'] < board.slots));
}

/** A move and how it went: the board and rate after it. */
export function applyMove(st, tier, action, success, board) {
  const next = { ...st };
  if (action === 'train') {
    next.ms -= 1;
    if (success) next.sp = Math.min(board.maxSpirit, st.sp + 2);
  } else {
    const [filled, won] = action === 'glory' ? ['gf', 'gs'] : ['df', 'ds'];
    next[filled] += 1;
    if (success) next[won] += 1;
    next.sp -= 1;
  }
  return { st: next, tier: tier === null ? null : nextTier(tier, success) };
}

/** Both bars full: the attempt is over. */
export const finished = (st, board) => st.gf === board.slots && st.df === board.slots;

/**
 * Whether a fresh result would be kept over the memory the attempt plays
 * against: the plan's keep rule for its roll, else the strategy's.  A
 * function of the result: true, false, or null when that cannot be told
 * (no memory known).
 */
export function keepRule(att, solver) {
  if (att.roll) return (fresh) => solver.keepsRoll(att.roll, att.states, fresh);
  if (!att.from) return () => null;
  return (fresh) => chooseMemory(att.objective, att.from, fresh) === 'replace';
}

/**
 * Whether no result the attempt can still end on would be kept (while the
 * bars can still be filled, every count of successes in the slots left can
 * still happen; `keeps` as keepRule): the best result still possible, to say
 * so, or null - also when it cannot be told.
 */
export function nothingToKeep(st, board, keeps) {
  for (let g = st.gs + board.slots - st.gf; g >= st.gs; g--) {
    for (let d = st.ds; d <= st.ds + board.slots - st.df; d++) {
      if (keeps({ glory: g, despair: d }) !== false) return null;
    }
  }
  return { glory: st.gs + board.slots - st.gf, despair: st.ds };
}

// ---------------------------------------------------------------- the options

/**
 * The numbers the solver weighs moves by, in its order, as an attempt's
 * objective names them: [{ key, label, kind: 'chance' | 'steps' | 'amp' }].
 * `kept` on the first: it is the chance a result is kept, and nothing else
 * (a mark that is the memory + 1: beating it is being kept).  What a
 * 'max' attempt plays for is the engine's to say (solver.objective()), not
 * worked out again here - so a score is never shown as what it is not.
 */
export function criteria(att, solver) {
  const o = att.objective, from = att.from;
  if (o.kind === 'max') {
    solver.configure(att.level, o, from);
    const engine = solver.objective();
    // no mark to speak of (blank or 0, nothing to beat): the most amplification, expected
    if (engine.kind === 'amp') return [{ key: 'score', kind: 'amp', label: 'Expected amplification' }];
    const { mark } = engine;
    const beat = from ? amplification(from) + 1 : 0;
    return [
      {
        key: 'score', kind: 'chance', kept: mark === beat,
        label: (o.mark ?? 0) >= beat ? `Chance of +${mark}% or more` : `Chance of improving the memory (+${mark}% or more)`,
      },
      { key: 'near', kind: 'chance', label: 'Chance of the best result still in reach' },
      { key: 'last', kind: 'amp', label: 'Expected amplification' },
    ];
  }
  if (o.kind === 'target') {
    const then = o.then && o.then.glory < 255 ? `${o.then.glory}/${o.then.despair}` : 'a full glory bar, no despair';
    return [
      { key: 'score', kind: 'chance', label: `Chance of ${o.glory}+ glory${o.despair == null ? '' : ` and ${o.despair} or less despair`}` },
      { key: 'then', kind: 'steps', label: `Steps closed toward ${then}` },
      { key: 'near', kind: 'chance', label: 'Chance of the nearest result still in reach' },
      { key: 'last', kind: 'steps', label: 'Expected glory - despair' },
    ];
  }
  const gap = from ? gapTo(o, from) : null;
  const tie = o.tie && o.tie.glory !== o.tie.despair ? `${o.tie.glory} x glory - ${o.tie.despair} x despair` : 'glory - despair';
  return [
    { key: 'score', kind: 'steps', label: `Steps closed toward ${o.glory}/${o.despair}${gap !== null ? ` (of ${gap})` : ''}` },
    { key: 'last', kind: 'steps', label: `Expected ${tie}` },
  ];
}

/** Which number set `best` above `other`: the first that differs, as the solver compares them (null: a dead heat). */
export function decider(crit, best, other) {
  for (const c of crit) {
    if (Math.abs(best[c.key] - other[c.key]) > 1e-12) return c.key;
  }
  return null;
}

/** A results spread (solver.outcomes), each result marked with whether it is kept: and the totals of that. */
export function spread(out, keeps) {
  const results = out.results.map((r) => ({ ...r, amp: amplification(r), kept: keeps(r) }));
  const sum = (f) => results.reduce((a, r) => a + (f(r) ? r.p : 0), 0);
  return {
    wipe: out.wipe, results,
    kept: results.some((r) => r.kept === null) ? null : sum((r) => r.kept),
    amp: results.reduce((a, r) => a + r.p * r.amp, 0),
  };
}

/** One move: its chance, what it is worth, what each way it can go leads to, and every result it can end on. */
export function moveOption(solver, att, board, action, keeps) {
  const chance = solver.chance(action, att.tier);
  const value = solver.moveValue(att.st, att.tier, action);
  const branches = [true, false].map((success) => {
    const next = applyMove(att.st, att.tier, action, success, board);
    const then = finished(next.st, board) ? 'complete' : solver.best(next.st, next.tier);
    return { success, p: success ? chance : 1 - chance, ...next, then, value: solver.moveValue(next.st, next.tier) };
  });
  return { action, chance, value, branches, ...spread(solver.outcomes(att.st, att.tier, action), keeps) };
}

/**
 * The options on a board in play: every move it allows (moveOption) and
 * Abandon Inheritance, and the one advised - the solver's move, or Abandon
 * when the attempt is wiped (no move left), doomed (moves left, but the
 * spirit power and mental strength cannot fill the bars: it can only wipe)
 * or no result it can still end on would be kept (`useless`: the best of
 * those).  Each move not advised says what put it behind (`decider`, a
 * criteria key).  Bars not yet full.
 */
export function boardOptions(solver, att) {
  const board = solver.board(att.level);
  solver.configure(att.level, att.objective, att.from);
  const keeps = keepRule(att, solver);
  const crit = criteria(att, solver);
  const moves = legalMoves(att.st, board).map((action) => moveOption(solver, att, board, action, keeps));
  const wiped = !moves.length;
  const doomed = !wiped && !solver.canFinish(att.st, att.tier);
  const useless = wiped || doomed ? null : nothingToKeep(att.st, board, keeps);
  let best = 'abandon';
  if (!wiped && !doomed && !useless) {
    best = solver.best(att.st, att.tier);
    const top = moves.find((m) => m.action === best);
    for (const m of moves) m.decider = m === top ? null : decider(crit, top.value, m.value);
  }
  return { moves, criteria: crit, best, abandon: { wiped, doomed, useless } };
}

/**
 * Said once a target or mark is out of reach part-way, and play turns to
 * what comes after it; null while it is in reach.
 */
export function settledNote(solver, att) {
  const o = att.objective;
  if (o.kind === 'max' && o.mark != null) {
    const board = solver.board(att.level);
    const best = amplification({ glory: att.st.gs + board.slots - att.st.gf, despair: att.st.ds });
    return best < o.mark ? `The +${o.mark}% Minimum Useful Amplification is out of reach this attempt - aiming for +${best}% now, the highest still possible.` : null;
  }
  if (o.kind !== 'target' && o.kind !== 'close') return null;
  solver.configure(att.level, o, att.from);
  if (solver.expected(att.st, att.tier) > 1e-9) return null;
  return o.kind === 'target'
    ? `The target (${strategySettings(o)}) is out of reach this attempt - playing the rest of it ${o.then ? `until a limit (${strategySettings({ kind: 'close', ...o.then })})` : 'for more glory, less despair'}.`
    : `Nothing more toward the limit (${strategySettings(o)}) can be gained this attempt - playing the rest of it for more glory, less despair.`;
}

// ---------------------------------------------------------------- keep or replace

const barText = (bar) => `${bar.glory}${bar.despair === null ? '' : `/${bar.despair}`}`;

/**
 * The keep rule's reasons for `current` against `fresh`, ranked as it
 * ranks them: { rows: [{ label, current, fresh, better: 'current' |
 * 'fresh' | null }], rule }.
 */
export function explainKeep(solver, att, current, fresh) {
  const rows = [];
  const row = (label, c, f, cmp) => rows.push({ label, current: c, fresh: f, better: cmp > 0 ? 'fresh' : cmp < 0 ? 'current' : null });
  const o = att.objective;
  const weighed = (tie) => (m) => tie.glory * m.glory - tie.despair * m.despair;
  if (att.roll) {
    const quest = solver.quest(att.roll.goal);
    const bar = att.roll.bar;
    // the relic's bar counts only where the level's requirement names it
    if (bar && (quest?.kind === 'each' || (quest?.kind === 'relics' && quest.relics.includes(att.roll.relic)))) {
      const meets = (m) => m.glory >= bar.glory && (bar.despair === null || m.despair <= bar.despair);
      row(`Meets the bar ${barText(bar)}`, meets(current) ? 'yes' : 'no', meets(fresh) ? 'yes' : 'no', meets(fresh) - meets(current));
    }
    const h = att.roll.horizon;
    if (h) {
      const [g, d] = att.states.reduce(([a, b], [x, y]) => [a + x, b + y], [0, 0]);
      const off = (m) => Math.max(0, h.glory - (g - current.glory + m.glory)) + Math.max(0, (d - current.despair + m.despair) - h.despair);
      const text = (m) => `${g - current.glory + m.glory}/${d - current.despair + m.despair}, ${off(m)} off`;
      row(`Board totals vs ${h.glory}/${h.despair}`, text(current), text(fresh), off(current) - off(fresh));
    }
    const tie = o.kind === 'close' && o.tie ? o.tie : { glory: 1, despair: 1 };
    const w = weighed(tie);
    row(tie.glory === tie.despair ? 'Glory - despair' : `${tie.glory} x glory - ${tie.despair} x despair`, String(w(current)), String(w(fresh)), w(fresh) - w(current));
    return { rows, rule: 'The plan keeps a result that ranks higher on these, in this order (a wipe never).' };
  }
  if (o.kind === 'target') {
    const hit = (m) => hitsObjective(o, m);
    const w = weighed({ glory: 1, despair: 1 });
    row(`Meets ${strategySettings(o)}`, hit(current) ? 'yes' : 'no', hit(fresh) ? 'yes' : 'no', hit(fresh) - hit(current));
    row('Glory - despair', String(w(current)), String(w(fresh)), w(fresh) - w(current));
  } else if (o.kind === 'close') {
    const w = weighed(o.tie ?? { glory: 1, despair: 1 });
    row(`Steps off ${o.glory}/${o.despair}`, String(gapTo(o, current)), String(gapTo(o, fresh)), gapTo(o, current) - gapTo(o, fresh));
    row('Glory - despair', String(w(current)), String(w(fresh)), w(fresh) - w(current));
  } else {
    row('Amplification', `+${amplification(current)}%`, `+${amplification(fresh)}%`, amplification(fresh) - amplification(current));
  }
  return { rows, rule: 'Kept when it ranks higher on these, in this order; a tie keeps the current memory.' };
}

/** The rate as the buttons show it. */
export const ratePct = (tier) => `${TIER_PERCENT[tier]}%`;
