// The exact solver (src/solver.rs) compiled to WebAssembly by
// extension/solver; see that crate for the exported functions.

const ACTIONS = ['glory', 'despair', 'train', 'done', 'deadend'];
const MOVES = { glory: 0, despair: 1, train: 2 };
const KINDS = { max: 0, close: 1, target: 2 };
const QUESTS = [null, 'relics', 'each', 'total'];
const STEPS = ['levelup', 'quest', 'filler', 'summon', 'convert', 'farm', 'done'];
const N_RELICS = 12;

export class RelicSolver {
  /** `bytes`: the contents of relic.wasm. */
  static async load(bytes) {
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new RelicSolver(instance.exports);
  }

  constructor(exports) {
    this.x = exports;
    this.key = null;
  }

  /** The board an inheritor level plays on. */
  board(level) {
    const x = this.x;
    const slots = x.cfg_slots(level);
    if (!slots) throw new Error(`inheritor level ${level} does not exist`);
    return {
      level,
      slots,
      maxSpirit: x.cfg_max_spirit(level),
      startGlory: x.cfg_start_glory(level),
      startDespairFail: x.cfg_start_despair_fail(level),
      startTier: x.cfg_start_tier(level),
      // what the level's bonuses add to each success chance (0.1: +10%)
      gloryMod: x.cfg_glory_mod(level),
      despairMod: x.cfg_despair_mod(level),
    };
  }

  /**
   * Solve for `level` and `objective`, played from the memory `from`
   * ({ glory, despair }; null: not known):
   *  { kind: 'max', mark }: maximize amplification above a target (the best
   *    chance of the Minimum Useful Amplification, mark% or more, then the most
   *    amplification - a mark the memory already reaches is raised to beat it;
   *    no mark is a mark of 0: over no memory (or one at +0%), the most
   *    amplification, expected) - the crit relic's;
   *  { kind: 'close', glory, despair, tie }: maximize glory, minimize
   *    despair until a limit (each glory gained up to Max useful Glory and
   *    each despair shed down to Min useful Despair counts), then more glory,
   *    less despair, weighed tie.glory : tie.despair (none: 1 : 1);
   *  { kind: 'target', glory, despair, then }: maximize glory, minimize
   *    despair above a target (the best chance of Glory Target or more and
   *    Despair Target or less together; despair null: any), then below the
   *    limit `then` ({ glory, despair }; none: every slot a glory, no
   *    despair), then more glory, less despair.
   * Optional wipePenalty.  A no-op if nothing changed (the memo of solved
   * states is kept).
   */
  configure(level, objective, from = null) {
    const kind = KINDS[objective.kind];
    if (kind === undefined) throw new Error(`unknown objective ${objective.kind}`);
    const o = objective;
    const bar = o.kind === 'target' || o.kind === 'close';
    const a = bar ? o.glory : o.kind === 'max' ? (o.mark ?? 0) : 0;
    const b = bar ? (o.despair ?? 255) : 0;
    const [c, d] = o.kind === 'target' ? [o.then?.glory ?? 255, o.then?.despair ?? 0]
      : o.kind === 'close' && o.tie ? [o.tie.glory, o.tie.despair] : [0, 0];
    const [fg, fd] = from ? [from.glory, from.despair] : [0, 255];
    const penalty = Number(o.wipePenalty) || 0;
    const key = [level, kind, a, b, c, d, fg, fd, penalty].join();
    if (key === this.key) return;
    if (this.x.configure(level, kind, a, b, c, d, fg, fd, penalty) !== 0) throw new Error(`cannot solve level ${level} for ${JSON.stringify(objective)}`);
    this.key = key;
  }

  // ------------------------------------------------------------- advisor

  /**
   * Write every relic's [glory, despair], stock and how full the pity bar is
   * (0..1, null if not known) where the advisor reads them.
   */
  load(states, stock, pity = null) {
    if (states.length !== N_RELICS || stock.length !== N_RELICS) throw new Error('the advisor needs all 12 relics');
    const io = new Int32Array(this.x.memory.buffer, this.x.io(), 64);
    states.forEach(([g, d], i) => {
      io[2 * i] = g;
      io[2 * i + 1] = d;
    });
    stock.forEach((n, i) => (io[24 + i] = n));
    io[36] = pity == null ? -1 : Math.round(1000 * Math.min(1, Math.max(0, pity)));
  }

  /**
   * The Look Ahead plan's next step toward `goal` ({ level, tier }; tier 0
   * for none) from the board seen at inheritor `level`: { step, goal } with
   * step 'levelup', 'quest', 'filler', 'summon', 'convert', 'farm' or 'done',
   * and for a roll the relic (0-11, the order of the main page), the
   * objective its attempts are played for, the level that is solved at,
   * and the bar it is rolled toward - all from this board alone.
   */
  advise(goal, level, states, stock, pity = null) {
    this.load(states, stock, pity);
    const code = this.x.advise(goal.level, goal.tier ?? 0, level);
    if (code < 0) throw new Error(`no advice for level ${level} toward ${goal.level}${goal.tier ? '+' + goal.tier : ''}`);
    // a fresh view: the call can grow the module's memory, which detaches
    // every view made before it (they then read as empty)
    const io = new Int32Array(this.x.memory.buffer, this.x.io(), 64);
    const out = { step: STEPS[code], goal: io[40] };
    if (io[41] >= 0) {
      out.relic = io[41];
      out.solveLevel = io[43];
      // played from the relic's own memory
      out.objective = io[42] === 1 ? { kind: 'close', glory: io[44], despair: io[45], tie: { glory: io[48], despair: io[49] } }
        : io[42] === 2 ? { kind: 'target', glory: io[44], despair: io[45], then: { glory: io[48], despair: io[49] } }
          : { kind: 'max', mark: io[44] };
      if (io[46] >= 0) out.bar = { glory: io[46], despair: io[47] < 0 ? null : io[47] };
    }
    // the totals every roll at this level closes the gap to, for its keep rule
    out.horizon = io[50] >= 0 ? { glory: io[50], despair: io[51] } : null;
    return out;
  }

  /**
   * Whether a roll keeps `fresh` ({ glory, despair }), judged on the board
   * `states` as it stood when the attempt began.  `roll` is what the attempt
   * was, from its advice: { horizon (the totals it closes the gap to,
   * { glory, despair } or null), goal (the level whose quest it works on),
   * relic, bar ({ glory, despair } or null), farm (a crit relic farm roll) }.
   * Null for a roll that cannot be.
   */
  keepsRoll(roll, states, fresh) {
    this.load(states, new Array(N_RELICS).fill(0));
    const bar = roll.bar ?? { glory: -1, despair: null };
    const h = roll.horizon ?? { glory: -1, despair: -1 };
    const k = this.x.keeps_roll(h.glory, h.despair, roll.goal, roll.relic, bar.glory, bar.despair ?? -1, roll.farm ? 1 : 0, fresh.glory, fresh.despair);
    return k < 0 ? null : k === 1;
  }

  /**
   * What being at inheritor `level` asks for: null (nothing), or
   * { kind: 'relics', relics, glory, despair } (those relics each at the
   * bar), { kind: 'each', glory, despair } (every relic at it) or
   * { kind: 'total', glory, despair } (the totals over every relic) -
   * glory or more, despair or less (null: any).  Undefined past the table.
   */
  quest(level) {
    const code = this.x.quest(level);
    if (code < 0) return undefined;
    if (code === 0) return null;
    const io = new Int32Array(this.x.memory.buffer, this.x.io(), 64);
    const out = { kind: QUESTS[code], glory: io[52], despair: io[53] < 0 ? null : io[53] };
    if (out.kind === 'relics') out.relics = Array.from({ length: N_RELICS }, (_, i) => i).filter((i) => io[54] & (1 << i));
    return out;
  }

  /** Pity points that level up to `goal`, and one attempt's at `level`. */
  pityNeeded(goal) {
    return this.x.pity_needed(goal);
  }

  pityGain(level) {
    return this.x.pity_gain(level);
  }

  args(st, tier) {
    return [st.gf, st.gs, st.df, st.ds, st.ms, st.sp, tier];
  }

  /** 'glory' | 'despair' | 'train' | 'done' | 'deadend' */
  best(st, tier) {
    const code = this.x.best(...this.args(st, tier));
    if (code < 0) throw new Error(`the board cannot be in state ${JSON.stringify(st)} at tier ${tier}`);
    return ACTIONS[code];
  }

  /**
   * What the objective scores from here: the expected amplification % ('max'
   * with no mark), the chance of the mark or target, or the expected steps
   * closed toward the limit ('close').
   */
  expected(st, tier) {
    return this.x.expected_score(...this.args(st, tier));
  }

  /** Chance of finishing without a wipe, from here. */
  finishChance(st, tier) {
    return this.x.finish_chance(...this.args(st, tier));
  }

  /**
   * What the configured objective scores, as the engine settled it: { kind:
   * 'amp' } (the most amplification, expected), { kind: 'mark', mark, least }
   * (the chance of mark% or more; a result kept from least%), { kind:
   * 'target' } or { kind: 'close' }.
   */
  objective() {
    const code = this.x.objective();
    if (code < 0) throw new Error('the solver is not configured');
    if (code !== 1) return { kind: ['amp', 'mark', 'target', 'close'][code] };
    const [mark, least] = new Float64Array(this.x.memory.buffer, this.x.out(), 2);
    return { kind: 'mark', mark, least };
  }

  /** The chance `action` ('glory' | 'despair' | 'train') succeeds at `tier`, the level's bonuses in. */
  chance(action, tier) {
    return this.x.chance(MOVES[action], tier);
  }

  /**
   * What `action` is worth from here with optimal play after it (null:
   * optimal play from here): { finish, score, then, near, last } - P(no
   * wipe), then what best() weighs moves by, in its order: the objective
   * (as expected()), the second objective (a target's `then`), the chance
   * of the nearest result still reachable, and what settles the rest
   * (amplification for 'max', glory less despair otherwise).  Null for a
   * move the state does not allow.
   */
  moveValue(st, tier, action = null) {
    if (this.x.move_value(action === null ? -1 : MOVES[action], ...this.args(st, tier)) < 0) return null;
    // a fresh view: the call can grow the module's memory
    const [finish, score, then, near, last] = new Float64Array(this.x.memory.buffer, this.x.out(), 5);
    return { finish, score, then, near, last };
  }

  /**
   * Every result the attempt can end on: `action` first (null: the best
   * move), optimal play after.  { wipe, results: [{ glory, despair, p }] },
   * results in (glory, despair) order; null for a move the state does not
   * allow.
   */
  outcomes(st, tier, action = null) {
    const n = this.x.outcomes(action === null ? -1 : MOVES[action], ...this.args(st, tier));
    if (n < 0) return null;
    const out = new Float64Array(this.x.memory.buffer, this.x.out(), 1 + 3 * n);
    const results = Array.from({ length: n }, (_, i) => ({ glory: out[1 + 3 * i], despair: out[2 + 3 * i], p: out[3 + 3 * i] }));
    return { wipe: out[0], results };
  }
}
