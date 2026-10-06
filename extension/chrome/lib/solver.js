// The exact solver (src/solver.rs) compiled to WebAssembly by
// extension/solver; see that crate for the exported functions.

const ACTIONS = ['glory', 'despair', 'train', 'done', 'deadend'];
const KINDS = { max: 0, target: 1, reach: 2, score: 3 };
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
    };
  }

  /**
   * Solve for `level` and `objective`: { kind: 'max' } for the most
   * amplification, { kind: 'target', glory, despair } (despair null: any),
   * { kind: 'reach', mark }, or { kind: 'score', wGlory, wDespair };
   * optional wipePenalty.  A no-op if nothing changed (the memo of solved
   * states is kept).
   */
  configure(level, objective) {
    const kind = KINDS[objective.kind];
    if (kind === undefined) throw new Error(`unknown objective ${objective.kind}`);
    const o = objective;
    const a = o.kind === 'target' ? o.glory : o.kind === 'reach' ? o.mark : 0;
    const b = o.kind === 'target' ? (o.despair ?? 255) : 0;
    const [wa, wb] = o.kind === 'score' ? [o.wGlory, o.wDespair] : [0, 0];
    const penalty = Number(o.wipePenalty) || 0;
    const key = [level, kind, a, b, wa, wb, penalty].join();
    if (key === this.key) return;
    if (this.x.configure(level, kind, a, b, wa, wb, penalty) !== 0) throw new Error(`cannot solve level ${level} for ${JSON.stringify(objective)}`);
    this.key = key;
  }

  // ------------------------------------------------------------- advisor

  /** Write every relic's [glory, despair] and stock where the advisor reads them. */
  load(states, stock) {
    if (states.length !== N_RELICS || stock.length !== N_RELICS) throw new Error('the advisor needs all 12 relics');
    const io = new Int32Array(this.x.memory.buffer, this.x.io(), 64);
    states.forEach(([g, d], i) => {
      io[2 * i] = g;
      io[2 * i + 1] = d;
    });
    stock.forEach((n, i) => (io[24 + i] = n));
  }

  /**
   * The look-ahead plan's next step toward `goal` ({ level, tier }; tier 0
   * for none) from the board seen at inheritor `level`: { step, goal } with
   * step 'levelup', 'quest', 'filler', 'summon', 'convert', 'farm' or 'done',
   * and for a roll the relic (0-11, the order of the main page), the
   * objective its attempts are played for, the level that is solved at,
   * and the bar it is rolled toward.
   */
  advise(goal, level, states, stock) {
    this.load(states, stock);
    const code = this.x.advise(goal.level, goal.tier ?? 0, level);
    if (code < 0) throw new Error(`no advice for level ${level} toward ${goal.level}${goal.tier ? '+' + goal.tier : ''}`);
    // a fresh view: the call can grow the module's memory, which detaches
    // every view made before it (they then read as empty)
    const io = new Int32Array(this.x.memory.buffer, this.x.io(), 64);
    const out = { step: STEPS[code], goal: io[40] };
    if (io[41] >= 0) {
      const f = new Float64Array(this.x.memory.buffer, this.x.io_f(), 2);
      out.relic = io[41];
      out.solveLevel = io[43];
      out.objective = io[42] === 1 ? { kind: 'target', glory: io[44], despair: io[45] < 0 ? null : io[45] }
        : io[42] === 2 ? { kind: 'score', wGlory: f[0], wDespair: f[1] }
          : { kind: 'reach', mark: io[44] };
      if (io[46] >= 0) out.bar = { glory: io[46], despair: io[47] < 0 ? null : io[47] };
      out.closer = io[48] === 1;
    }
    return out;
  }

  /** Whether that step keeps `fresh` ({ glory, despair }) on its relic; null if not a roll. */
  keeps(goal, level, states, stock, fresh) {
    this.load(states, stock);
    const k = this.x.keeps(goal.level, goal.tier ?? 0, level, fresh.glory, fresh.despair);
    return k < 0 ? null : k === 1;
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

  /** Expected amplification % ('max') or chance of the objective, from here. */
  expected(st, tier) {
    return this.x.expected_score(...this.args(st, tier));
  }

  /** Chance of finishing without a wipe, from here. */
  finishChance(st, tier) {
    return this.x.finish_chance(...this.args(st, tier));
  }
}
