// The rules between screens: what a roll did, where the chance ladder
// moved, and which memory to keep.  No Chrome, no pixels.

/** The twelve relics, in the order of the main page's tiles and of the engine's RELICS. */
export const RELICS = [
  "Giant's Right Hand", 'Demon Eye of Weakness', 'Oath of Immortality', 'Sacred Tree of Rebirth',
  'Ring of Lightning', 'Golden Star', 'Seal of the Legendary Archer', 'Veil of the Night',
  'Spark of Eternity', "Mermaid's Tear", 'Eye of the Sky', 'Crown of the Great Mountain',
];

/** The shared success-chance ladder, easiest first (solver.rs TIERS). */
export const TIER_PERCENT = [80, 65, 50, 35, 20];

/** Each strategy's name, as the panel shows it. */
export const STRATEGY = {
  max: 'Maximize amplification above a target',
  target: 'Maximize glory, minimize despair above a target',
  close: 'Maximize glory, minimize despair until a limit',
};

/** A strategy's settings, as the panel labels them. */
export function strategySettings(o) {
  switch (o.kind) {
    case 'target':
      return `Glory Target ${o.glory}, Despair Target ${o.despair ?? 'any'}`;
    case 'close':
      return `Max useful Glory ${o.glory}, Min useful Despair ${o.despair}`;
    default:
      return `Minimum Useful Amplification +${o.mark ?? 0}%`;
  }
}

/** How an attempt is played, in the panel's words. */
export function describeObjective(o) {
  let text = `${STRATEGY[o.kind] ?? STRATEGY.max} (${strategySettings(o)})`;
  if (o.kind === 'target' && o.then) {
    // the advisor's: once the target is settled, on toward the totals
    text += `, then until a limit (${strategySettings({ kind: 'close', ...o.then })})`;
  }
  if (o.kind === 'close' && o.tie && (o.tie.glory !== o.tie.despair)) {
    text += `, ties weighed ${o.tie.glory} : ${o.tie.despair}`;
  }
  return text;
}
const WORST_TIER = TIER_PERCENT.length - 1;

/** A success makes the next roll one tier harder, a failure one easier. */
export function nextTier(tier, success) {
  return success ? Math.min(tier + 1, WORST_TIER) : Math.max(tier - 1, 0);
}

/** A memory's Unique Effect Amplification in %: +5 a glory, -2 a despair success. */
export function amplification({ glory, despair }) {
  return Math.max(0, 5 * glory - 2 * despair);
}

/** Whether a memory already meets the objective: a target, or a mark (never, for a plain max). */
export function hitsObjective(objective, memory) {
  switch (objective.kind) {
    case 'target':
      return memory.glory >= objective.glory && (objective.despair == null || memory.despair <= objective.despair);
    case 'close':
      return gapTo(objective, memory) === 0;
    case 'max':
      // a mark of 0 (or blank, the same) is no target to meet
      return objective.mark > 0 && amplification(memory) >= objective.mark;
    default:
      return false;
  }
}

/** The steps a memory is off a target: a glory short or a despair over, one each. */
export function gapTo(objective, { glory, despair }) {
  return Math.max(0, objective.glory - glory) + (objective.despair == null ? 0 : Math.max(0, despair - objective.despair));
}

/**
 * Keep the current memory or replace it with the fresh one: replace if the
 * fresh one ranks higher, as the attempt was played.  A target: its bar met,
 * then more glory less despair.  Closing a gap: nearer it (each glory short
 * or despair over a step), then more glory less despair.  Max amplification:
 * more amplification.  A tie keeps.
 */
export function chooseMemory(objective, current, fresh) {
  const tie = objective.kind === 'close' && objective.tie ? objective.tie : { glory: 1, despair: 1 };
  const steps = (m) => tie.glory * m.glory - tie.despair * m.despair;
  const rank = (m) => {
    switch (objective.kind) {
      case 'target': return [hitsObjective(objective, m) ? 1 : 0, steps(m)];
      case 'close': return [-gapTo(objective, m), steps(m)];
      default: return [amplification(m)];
    }
  };
  const [f, c] = [rank(fresh), rank(current)];
  for (let i = 0; i < f.length; i++) if (f[i] !== c[i]) return f[i] > c[i] ? 'replace' : 'keep';
  return 'keep';
}

/** Whether a board read differs from `prev` in anything an action changes. */
export function moved(prev, next) {
  return prev.gf !== next.gf || prev.df !== next.df || prev.ms !== next.ms;
}

/**
 * What one action did, judged from the board before and after it.
 * { ok: false, why } if the change is not one that action can make;
 * otherwise { ok: true, success } with success null when a mental training
 * at full spirit power left no trace (success and failure look the same).
 */
export function inferOutcome(prev, next, action, maxSpirit) {
  const same = (...keys) => keys.every((k) => prev[k] === next[k]);
  const bad = (why) => ({ ok: false, why: `${why} after ${action}: ${show(prev)} -> ${show(next)}` });
  switch (action) {
    case 'glory':
    case 'despair': {
      const [f, s] = action === 'glory' ? ['gf', 'gs'] : ['df', 'ds'];
      const others = action === 'glory' ? ['df', 'ds'] : ['gf', 'gs'];
      if (next[f] !== prev[f] + 1) return bad(`expected one more ${action} slot`);
      if (next[s] !== prev[s] && next[s] !== prev[s] + 1) return bad(`${action} successes jumped`);
      if (!same(...others, 'ms')) return bad('other counters changed');
      if (next.sp !== prev.sp - 1) return bad('spirit power should drop by 1');
      return { ok: true, success: next[s] === prev[s] + 1 };
    }
    case 'train': {
      if (next.ms !== prev.ms - 1) return bad('expected one less mental strength');
      if (!same('gf', 'gs', 'df', 'ds')) return bad('a bar changed');
      const gained = Math.min(prev.sp + 2, maxSpirit);
      if (next.sp !== prev.sp && next.sp !== gained) return bad('spirit power moved oddly');
      if (next.sp > prev.sp) return { ok: true, success: true };
      return { ok: true, success: prev.sp === maxSpirit ? null : false };
    }
    default:
      return bad('unknown action');
  }
}

/** Whether two levels' boards play the same once an attempt is under way: slots, spirit power and rate bonuses. */
function playsAlike(a, b) {
  return a.slots === b.slots && a.maxSpirit === b.maxSpirit
    && Math.abs(a.gloryMod - b.gloryMod) < 1e-9 && Math.abs(a.despairMod - b.despairMod) < 1e-9;
}

/**
 * The level to play a board at, of the levels it fits (`fits`, see
 * vision.js levelsFitting; `boards` the solver's board for each level):
 * `current` if it fits, else the highest that fits.  { level, sure, alike }
 * - sure: no other level fits; alike: every level that fits plays the same
 * from here (18 and 19 do, part-way) - or null if none fits.
 */
export function pickLevel(fits, boards, current) {
  if (!fits.length) return null;
  if (fits.includes(current)) return { level: current, sure: true, alike: true };
  const of = (level) => boards.find((b) => b.level === level);
  const level = Math.max(...fits);
  return { level, sure: fits.length === 1, alike: fits.every((l) => playsAlike(of(l), of(level))) };
}

/** Levels as people write them: "16-19", or "12, 14 or 15". */
export function levelList(levels) {
  const run = levels.every((l, i) => i === 0 || l === levels[i - 1] + 1);
  if (levels.length > 2 && run) return `${levels[0]}-${levels[levels.length - 1]}`;
  return levels.join(', ').replace(/, (\d+)$/, ' or $1');
}

/** A board for people: successes and fails per bar, spirit power, mental strength. */
export function show(st) {
  return `glory ${st.gs}✓ ${st.gf - st.gs}✗ · despair ${st.ds}✓ ${st.df - st.ds}✗ · spirit ${st.sp} · mental ${st.ms}`;
}

/** The state every attempt opens on. */
export function isStartState(st, board) {
  return st.gf === board.startGlory && st.gs === board.startGlory
    && st.df === board.startDespairFail && st.ds === 0
    && st.ms === board.slots && st.sp === board.maxSpirit;
}
