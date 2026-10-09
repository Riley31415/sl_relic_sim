// The simulated game: the screen the session is on, drawn the way the game
// draws it.  Every button the player can press in the game is a button
// here too (data-press), so the page can point at the one to press and the
// player can say which one they pressed; beside each Attempt button, a
// Success and a Fail (data-outcome) say how it went; every number on it is
// a box (data-field) the player can put right.

import { amplification } from './lib/logic.js';
import { MOVE_NAMES, RELICS, TIER_PERCENT, slotsOf } from './session.js';

/** Each relic's colour, for its tile. */
export const RELIC_COLOURS = [
  '#e0602f', '#59b7e8', '#e23b3b', '#9ccc3a', '#4f8ff0', '#f2c23a',
  '#d8a04a', '#a066e6', '#ef5a4f', '#3a8fe0', '#3fbf7a', '#b98a5a',
];

export const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

/** A memory slot: 'glory' or 'despair' (a success), 'fail' or 'empty'. */
function gem(kind) {
  return `<svg class="gem gem-${kind}" viewBox="0 0 20 26" aria-hidden="true"><path class="gem-body" d="M10 1 19 13 10 25 1 13Z"/><path class="gem-face" d="M10 1 14.5 13 10 25 5.5 13Z"/></svg>`;
}

function leaf(lit) {
  return `<svg class="leaf${lit ? '' : ' leaf-off'}" viewBox="0 0 24 24" aria-hidden="true"><path class="leaf-body" d="M12 22C5 18 3 10 6 3c3 4 9 3 12 0 3 7 1 15-6 19Z"/><path class="leaf-vein" d="M12 21V8"/></svg>`;
}

/** A bar of `slots` slots: `cells` ('success' / 'fail') filled, the rest empty. */
function slotRow(cells, slots, win) {
  const out = [];
  for (let i = 0; i < slots; i++) out.push(gem(cells[i] === 'success' ? win : cells[i] === 'fail' ? 'fail' : 'empty'));
  return `<div class="slots">${out.join('')}</div>`;
}

/** A memory's bars: its successes and the rest failures (the order is not known). */
function memoryCells(n, slots) {
  return Array.from({ length: slots }, (_, i) => (i < n ? 'success' : 'fail'));
}

/**
 * A number box for field `field` of session `s`, marked when a screenshot
 * left it unread.  `wide` for stocks.
 */
function box(s, field, value, [min, max], label, wide = false) {
  const unsure = s.unsure.includes(field);
  return `<input class="sim-num${wide ? ' wide' : ''}${unsure ? ' unsure' : ''}" type="number" inputmode="numeric" step="1" min="${min}" max="${max}"`
    + ` data-field="${field}" value="${value}" aria-label="${esc(label)}" title="${esc(unsure ? `${label}: did not read - check it` : label)}">`;
}

/** The relic's name: a picker, so a screenshot can be put on the right relic. */
function relicName(s, mode) {
  const options = RELICS.map((name, i) => `<option value="${i}"${s.relic === i ? ' selected' : ''}>${esc(name)}</option>`).join('');
  // Smart Leveler has to know which relic it is (in Single Relic mode the name is only a label): while it
  // is not known, a placeholder that cannot be picked - a relic, once named, is never un-named
  const none = mode === 'advisor' && s.relic === null ? '<option value="" selected disabled hidden>Which relic?</option>' : '';
  return `<select class="relic-pick${s.relic === null && mode === 'advisor' ? ' unsure' : ''}" data-set="relic" aria-label="Relic">${none}${options}</select>`;
}

/** Arrows that step field `field`'s box (`value`, from `min` to `max`) up and down one. */
function stepper(field, value, [min, max], label) {
  const arrow = (d, glyph, word, off) => `<button type="button" data-step="${d}" data-step-field="${field}" aria-label="${esc(`${label} ${word}`)}"${off ? ' disabled' : ''}>${glyph}</button>`;
  return `<span class="stepper">${arrow(1, '&#9650;', 'up', value >= max)}${arrow(-1, '&#9660;', 'down', value <= min)}</span>`;
}

/** The inheritor level, top left of every screen, with arrows to step it; `extra` on the right. */
function levelStrip(s, extra = '') {
  return `<div class="sim-strip"><span class="level-box"><label>Inheritor Lv.${box(s, 'level', s.level, [1, 20], 'Inheritor level')}</label>`
    + `${stepper('level', s.level, [1, 20], 'Inheritor level')}</span>${extra}</div>`;
}

/**
 * The phone's screen as HTML, for session `s`.  ctx: { solver, settings,
 * advice (Level Up shows lit only when it is the advice - the game lights
 * it once a level-up is earned - and the advised move's Success and Fail
 * stand out) }.
 */
export function renderPhone(s, ctx) {
  switch (s.view) {
    case 'board': return boardScreen(s, ctx);
    case 'compare': return compareScreen(s, ctx);
    case 'legacy': return legacyScreen(s, ctx);
    default: return mainScreen(s, ctx);
  }
}

function boardScreen(s, { solver, settings, advice }) {
  const att = s.attempt;
  const board = solver.board(att.level);
  const { st, tier } = att;
  const cells = slotsOf(st, att.bars);
  const done = st.gf === board.slots && st.df === board.slots;
  const rate = `${TIER_PERCENT[tier]}%`;
  // the level's rate bonus, as the game prints it under the buttons
  const bonus = (move) => Math.round(100 * (chanceAt(solver, att, move) - TIER_PERCENT[0] / 100));
  const usable = { train: st.ms > 0, glory: st.sp > 0 && st.gf < board.slots, despair: st.sp > 0 && st.df < board.slots };
  const button = (move) => {
    if (done || !usable[move]) return '<div class="attempt-gap"></div>';
    const b = move === 'train' ? 0 : bonus(move);
    return `<div class="attempt-col"><button class="attempt attempt-${move}" data-press="${move}" aria-label="Attempt ${MOVE_NAMES[move]} at ${rate}">`
      + `<span class="rate">${rate}</span><span>Attempt</span></button>`
      + (b ? `<div class="bonus">Success chance: ${b > 0 ? '+' : ''}${b}%</div>` : '') + `${outcomes(move, advice)}</div>`;
  };
  const n = (field, value, max, label) => box(s, field, value, [0, max], label);
  // a bar's successes and fails, as boxes: gs / gx, ds / dx
  const counts = (won, lost, name) => `<span class="panel-counts"><label class="ok">&#10003;${n(`${name[0]}s`, won, board.slots, `${name} successes`)}</label>`
    + `<label class="no">&#10007;${n(`${name[0]}x`, lost, board.slots, `${name} fails`)}</label></span>`;
  const rateBox = `<label>Rate <select class="sim-select${s.unsure.includes('tier') ? ' unsure' : ''}" data-field="tier" aria-label="The rate on the Attempt buttons">`
    + `${TIER_PERCENT.map((p, i) => `<option value="${i}"${tier === i ? ' selected' : ''}>${p}%</option>`).join('')}</select></label>`;
  return `<div class="scr scr-board">
    ${levelStrip(s)}
    <div class="relic-title">${relicName(s, settings.mode)}</div>
    <div class="spirit"><div class="spirit-fill" style="width:${(100 * st.sp) / board.maxSpirit}%"></div>
      <label>Spirit Power ${n('sp', st.sp, board.maxSpirit, 'Spirit power')} / ${board.maxSpirit}</label></div>
    <div class="panel panel-train">
      <div class="panel-text"><div class="panel-title">Mental Training <span class="panel-counts"><label>x${n('ms', st.ms, board.slots, 'Mental strength')}</label></span></div>
        <div class="panel-sub">Spirit Power +2 on Success</div>
        <div class="slots leaves">${Array.from({ length: board.slots }, (_, i) => leaf(i < st.ms)).join('')}</div></div>
      ${button('train')}
    </div>
    <div class="panel panel-glory">
      <div class="panel-text"><div class="panel-title">Memory of Glory ${counts(st.gs, st.gf - st.gs, 'glory')}</div>
        <div class="panel-sub">Unique Effect Amplification <b>+${5 * st.gs}%</b></div>
        ${slotRow(cells.glory, board.slots, 'glory')}</div>
      ${button('glory')}
    </div>
    <div class="panel panel-despair">
      <div class="panel-text"><div class="panel-title">Memory of Despair ${counts(st.ds, st.df - st.ds, 'despair')}</div>
        <div class="panel-sub">Unique Effect Amplification <b>-${2 * st.ds}%</b></div>
        ${slotRow(cells.despair, board.slots, 'despair')}</div>
      ${button('despair')}
    </div>
    <div class="board-foot">
      ${done ? '<button class="btn-gold btn-wide" data-press="complete">Complete Inheritance</button>' : '<button class="btn-abandon" data-press="abandon">Abandon<br>Inheritance</button>'}
      <span class="rate">${rateBox}</span>
    </div>
  </div>`;
}

/**
 * How a move went, told beside its Attempt button: Success and Fail, each
 * saying where it leads (the advice's own moves know).  The advised move's
 * stand out.
 */
function outcomes(move, advice) {
  const advised = advice?.press === move;
  const branches = advice?.moves?.find((m) => m.action === move)?.branches ?? [];
  const leads = (b) => (!b ? '' : `: the rate goes to ${TIER_PERCENT[b.tier]}%, then ${b.then === 'complete' ? 'Complete Inheritance'
    : b.then === 'deadend' ? 'a wipe' : MOVE_NAMES[b.then]}`);
  const one = (ok, b) => `<button class="oc ${ok ? 'oc-ok' : 'oc-bad'}" data-outcome="${ok ? 1 : 0}" data-move="${move}"`
    + ` title="${esc(`${MOVE_NAMES[move]} ${ok ? 'succeeded' : 'failed'}${leads(b)}`)}">${ok ? '&#10003;' : '&#10007;'}<span> ${ok ? 'Success' : 'Fail'}</span></button>`;
  return `<div class="outcome${advised ? ' advised' : ''}" role="group" aria-label="How ${MOVE_NAMES[move]} went">${one(true, branches[0])}${one(false, branches[1])}</div>`;
}

/** The success chance of `move` at the opening tier, bonuses in: what the buttons' footnote is worked out from. */
function chanceAt(solver, att, move) {
  solver.configure(att.level, att.objective, att.from);
  return solver.chance(move, 0);
}

function legacyScreen(s, { solver, settings }) {
  const slots = solver.board(s.level).slots;
  const m = s.memory;
  const step = (d) => (s.relic === null ? '' : ` data-press="tile" data-tile="${(s.relic + d + RELICS.length) % RELICS.length}"`);
  const count = (field, v, label) => `<span class="panel-counts level-box"><label>x${box(s, field, v, [0, slots], label)}</label>${stepper(field, v, [0, slots], label)}</span>`;
  return `<div class="scr scr-legacy">
    ${levelStrip(s)}
    <div class="relic-title legacy-title"><button class="chevron" aria-label="Previous relic"${step(-1)}>&#9664;</button>${relicName(s, settings.mode)}<button class="chevron" aria-label="Next relic"${step(1)}>&#9654;</button></div>
    <div class="relic-icon" style="--c:${s.relic !== null ? RELIC_COLOURS[s.relic] : '#6f747a'}">${gem('icon')}</div>
    <div class="panel panel-glory"><div class="panel-text"><div class="panel-title">Memory of Glory ${count('mg', m.glory, 'Memory glory')}</div>
      <div class="panel-sub">Unique Effect Amplification <b>+${5 * m.glory}%</b></div>${slotRow(memoryCells(m.glory, slots), slots, 'glory')}</div></div>
    <div class="panel panel-despair"><div class="panel-text"><div class="panel-title">Memory of Despair ${count('md', m.despair, 'Memory despair')}</div>
      <div class="panel-sub">Unique Effect Amplification <b>-${2 * m.despair}%</b></div>${slotRow(memoryCells(m.despair, slots), slots, 'despair')}</div></div>
    <div class="legacy-foot">
      <label class="stock-chip">${box(s, 'onHand', s.onHand, [0, 99999], 'On hand', true)}/10</label>
      <button class="btn-gold btn-inherit" data-press="inheritance">Inheritance</button>
    </div>
    <button class="btn-red btn-back" data-press="back" aria-label="Back to the main page">&#8617;</button>
  </div>`;
}

function compareScreen(s, { solver }) {
  const { current, fresh } = s.result;
  const slots = solver.board(s.attempt.level).slots;
  // cg / cd: the current memory card, fg / fd: the new one
  const card = (title, m, f) => `<div class="mcard"><div class="mcard-title">&lt; ${title} &gt;</div>
    <label class="mcard-g">${gem('glory')} Glory x ${box(s, `${f}g`, m.glory, [0, slots], `${title} glory`)}</label>
    <label class="mcard-d">${gem('despair')} Despair x ${box(s, `${f}d`, m.despair, [0, slots], `${title} despair`)}</label>
    <div class="mcard-label">Final Boost Value</div><div class="mcard-value">+${amplification(m)}%</div></div>`;
  return `<div class="scr scr-compare">
    ${levelStrip(s)}
    <div class="popup">
      <div class="popup-title">Compare Inheritance Results</div>
      <div class="cards">${card('Current Memory', current, 'c')}${card('New Memory', fresh, 'f')}</div>
      <div class="popup-note">Select the Memory to apply.</div>
      <div class="popup-buttons"><button class="btn-orange" data-press="keep">Keep Current<br>Effect</button><button class="btn-gold" data-press="replace">Replace with<br>New Effect</button></div>
    </div>
  </div>`;
}

function mainScreen(s, { solver, advice }) {
  const tiles = RELICS.map((name, i) => {
    const [g, d] = s.states[i];
    // a tile is a button in the game; here it holds boxes too, so it is one by role
    return `<div class="tile" role="button" tabindex="0" data-press="tile" data-tile="${i}" aria-label="Open ${esc(name)}">
      <span class="tile-name">${esc(name)}</span>
      <span class="tile-body"><span class="tile-icon" style="--c:${RELIC_COLOURS[i]}">${gem('icon')}</span>
        <span class="tile-counts"><label class="tc-g">&#9670;x${box(s, `g${i}`, g, [0, 10], `${name} glory`)}</label><label class="tc-d">&#9670;x${box(s, `d${i}`, d, [0, 10], `${name} despair`)}</label></span></span>
      <label class="tile-stock">Count ${box(s, `n${i}`, s.stock[i], [0, 99999], `${name} count`, true)}</label>
    </div>`;
  }).join('');
  const totals = s.states.reduce(([g, d], [a, b]) => [g + a, d + b], [0, 0]);
  // Level Up lights as the game's does: the requirement met (the game showed it lit) or the pity bar full
  const lit = advice?.press === 'levelup' || s.levelUp || (s.pity >= 0.995 && s.level < solver.maxLevel());
  return `<div class="scr scr-main">
    <div class="main-head">${levelStrip(s)}<div class="totals">Total <span class="tc-g">${gem('glory')} x ${totals[0]}</span> <span class="tc-d">${gem('despair')} x ${totals[1]}</span></div><span></span></div>
    <div class="tiles">${tiles}</div>
    <div class="quest">${questText(solver.quest(s.level + 1))}</div>
    <div class="inheritor">
      <div class="pity"><div class="pity-fill" style="width:${100 * s.pity}%"></div><label>Pity&nbsp;${box(s, 'pity', Math.round(100 * s.pity), [0, 100], 'Pity bar %')}%</label></div>
    </div>
    <div class="main-foot"><button class="${lit ? 'btn-gold' : 'btn-dim'}" data-press="levelup">Level Up</button></div>
  </div>`;
}

/** The requirement box, as the counts view words it. */
function questText(q) {
  if (q === undefined) return 'Max level reached!';
  if (q === null) return 'Unknown quest?';
  const despair = q.despair === null ? '' : ` and <span class="tc-d">'Despair' ${q.despair}</span> or less`;
  if (q.kind === 'total') return `All Relics Total<br><span class="tc-g">'Glory' ${q.glory}</span> or more${despair}`;
  const who = q.kind === 'each' ? 'Every relic' : q.relics.map((i) => esc(RELICS[i])).join(', ');
  return `${who}<br><span class="tc-g">'Glory' ${q.glory}</span> or more${despair}`;
}
