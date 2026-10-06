// The rules between screens: what a roll did, where the chance ladder
// moved, and which memory to keep.  No Chrome, no pixels.

/** The shared success-chance ladder, easiest first (solver.rs TIERS). */
export const TIER_PERCENT = [80, 65, 50, 35, 20];
const WORST_TIER = TIER_PERCENT.length - 1;

/** A success makes the next roll one tier harder, a failure one easier. */
export function nextTier(tier, success) {
  return success ? Math.min(tier + 1, WORST_TIER) : Math.max(tier - 1, 0);
}

/** A memory's Unique Effect Amplification in %: +5 a glory, -2 a despair success. */
export function amplification({ glory, despair }) {
  return Math.max(0, 5 * glory - 2 * despair);
}

/** Whether a memory hits an all-or-nothing objective (never, for 'max'). */
export function hitsObjective(objective, memory) {
  switch (objective.kind) {
    case 'target':
      return memory.glory >= objective.glory && memory.despair <= objective.despair;
    case 'reach':
      return amplification(memory) >= objective.mark;
    default:
      return false;
  }
}

/**
 * Keep the current memory or replace it with the fresh one: a hit on the
 * objective beats a miss, then more amplification wins; a tie keeps.
 */
export function chooseMemory(objective, current, fresh) {
  const hc = hitsObjective(objective, current), hf = hitsObjective(objective, fresh);
  if (hc !== hf) return hf ? 'replace' : 'keep';
  return amplification(fresh) > amplification(current) ? 'replace' : 'keep';
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
