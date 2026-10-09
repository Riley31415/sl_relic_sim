// Why the advice is what it is, in numbers: every move the board allows
// with what each way it can go leads to and every result it can end on;
// what an attempt about to start can come to; the keep rule's reasons.

import { describeObjective } from './lib/logic.js';
import { MOVE_NAMES, TIER_PERCENT, fmtMemory } from './session.js';
import { esc } from './view.js';

const pct = (p, d = 1) => `${(100 * p).toFixed(d)}%`;

/** A criterion's number, to `d` places. */
function fmt(kind, v, d) {
  if (kind === 'chance') return pct(v, d);
  if (kind === 'amp') return `+${v.toFixed(d)}%`;
  return v.toFixed(d + 1);
}

/** The places `fmt` needs to tell a `kind` number `best` from each of `others`: 1 to 4. */
function places(kind, best, others) {
  const gap = Math.min(...others.map((v) => Math.abs(best - v)).filter((v) => v > 1e-12), Infinity);
  const unit = { chance: 100, amp: 1, steps: 10 }[kind]; // fmt's last place, in d = 0 units
  for (let d = 1; d < 4; d++) if (gap * unit * 10 ** d >= 1) return d;
  return 4;
}

/** The reasons for the advice, as HTML. `open`: the move whose details are shown. */
export function renderWhy(advice, open) {
  if (advice.moves) return movesWhy(advice, open);
  if (advice.prospect) return prospectWhy(advice.prospect);
  if (advice.compare) return compareWhy(advice);
  return ''; // nothing to weigh up: no panel
}

function playedFor(objective, from, by) {
  const whose = by === 'plan' ? "the plan's roll, " : '';
  return `<p class="played">Played for ${whose}<b>${esc(describeObjective(objective))}</b>${from ? `, against the memory in place, <b>${fmtMemory(from)}</b>` : ''}.</p>`;
}

/**
 * The options on a board: every move it allows and Abandon Inheritance, the
 * advised one first, then the rest as the solver ranks them; under the list,
 * the one picked (`open`, else the advised) opened up.
 */
function movesWhy(a, open) {
  const crit = a.criteria;
  const main = crit[0];
  const advisedMove = a.moves.find((m) => m.action === a.best);
  const ranked = [...a.moves].sort((x, y) => (x === advisedMove ? -1 : y === advisedMove ? 1 : y.value.score - x.value.score));
  const scores = a.moves.map((m) => m.value.score);
  const d = advisedMove ? places(main.kind, advisedMove.value.score, scores.filter((v) => v !== advisedMove.value.score)) : 1;
  const shown = (open === 'abandon' && a.best === 'abandon') || a.moves.some((m) => m.action === open) ? open : a.best;
  const row = (action, name, cells) => {
    const advised = action === a.best;
    return `<tr class="move-row${advised ? ' best' : ''}${action === shown ? ' open' : ''}" data-expand="${action}" tabindex="0" aria-pressed="${action === shown}">
      <td class="move-name">${advised ? '<span class="star" title="advised">&#9733;</span>' : ''}${name}</td>${cells.map((c) => `<td>${c}</td>`).join('')}</tr>`;
  };
  // the objective's own number, unless it is just the chance a result is kept: then Kept says it
  // Improvement Chance (the result replaces the memory) beside the objective's own number - Target
  // Chance for a mark or target - unless that is just the improvement chance again
  const own = !main.kept;
  const ownHead = main.kind === 'chance' ? 'Target Chance' : main.label;
  const moveRows = ranked.map((m) => row(m.action, MOVE_NAMES[m.action], [
    pct(m.chance, 0), main.kept ? fmt(main.kind, m.value.score, d) : m.kept === null ? '-' : pct(m.kept),
    ...(own ? [fmt(main.kind, m.value.score, d)] : []), pct(1 - m.value.finish),
  ]));
  // Abandon Inheritance is offered only when it is the advice: it ends the attempt with nothing
  const rows = a.best === 'abandon' ? [row('abandon', 'Abandon Inheritance', ['-', '0.0%', ...(own ? [fmt(main.kind, 0, d)] : []), '-']), ...moveRows] : moveRows;
  const ties = crit.length > 1 ? `, then ${crit.slice(1).map((c) => esc(c.label.toLowerCase())).join(', then ')}` : '';
  const memory = a.from ? ` Memory in place: <b>${fmtMemory(a.from)}</b>.` : '';
  const picked = shown === 'abandon' ? abandonDetail(a) : moveDetail(a, a.moves.find((m) => m.action === shown), advisedMove, d);
  const title = a.best === 'abandon' ? 'Abandon Inheritance' : MOVE_NAMES[a.best];
  return `<h2>Why ${title}?</h2>
    <p class="muted">Ranked by <b>${esc(main.label.toLowerCase())}</b>${ties}.${memory}</p>
    <div class="table-wrap"><table class="moves">
      <thead><tr><th>Option</th><th>Succeeds</th><th title="Chance the result replaces the memory in place">Improvement Chance</th>${own ? `<th title="${esc(main.label)}">${esc(ownHead)}</th>` : ''}<th>Wipe Chance</th></tr></thead>
      <tbody>${rows.join('')}</tbody>
    </table></div>
    <div class="option-detail"><h3>${shown === 'abandon' ? 'Abandon Inheritance' : MOVE_NAMES[shown]}</h3>${picked}</div>`;
}

const thenText = (b) => (b.then === 'complete' ? 'the bars are full'
  : b.then === 'deadend' ? 'a wipe (no spirit power or mental strength left)'
    : `${MOVE_NAMES[b.then]} next, at ${TIER_PERCENT[b.tier]}%`);

/** One move opened up: why it ranks where it does, its two ways of going, and its results. */
function moveDetail(a, m, best, d) {
  const crit = a.criteria, main = crit[0];
  let verdict;
  if (a.best === 'abandon') verdict = '<p class="verdict">Nothing it leads to would be kept.</p>';
  else if (m === best) verdict = '';
  else if (m.decider === 'score') {
    verdict = `<p class="verdict">${fmt(main.kind, m.value.score, d)} against ${fmt(main.kind, best.value.score, d)} for ${MOVE_NAMES[best.action]}.</p>`;
  } else if (m.decider) {
    const c = crit.find((x) => x.key === m.decider);
    const dd = places(c.kind, best.value[c.key], [m.value[c.key]]);
    verdict = `<p class="verdict">Tied, then ${esc(c.label.toLowerCase())}: ${fmt(c.kind, m.value[c.key], dd)} against ${fmt(c.kind, best.value[c.key], dd)}.</p>`;
  } else verdict = `<p class="verdict">Tied with ${MOVE_NAMES[best.action]} on everything.</p>`;
  const branch = (b) => `<div class="branch ${b.success ? 'branch-ok' : 'branch-bad'}">
      <span class="branch-how">${b.success ? '&#10003; Success' : '&#10007; Fail'} <b>${pct(b.p, 0)}</b></span>
      <span class="branch-then">&rarr; ${thenText(b)}</span>
      <span class="branch-value">${fmt(main.kind, b.value.score, d)}</span></div>`;
  const [ok, bad] = m.branches;
  return `${verdict}<div class="branches">${branch(ok)}${branch(bad)}</div>`;
}

/**
 * Abandon Inheritance opened up (offered only when it is the advice): its
 * one result - the memory as it is, the relics spent and the pity counted
 * either way - and why.
 */
function abandonDetail(a) {
  const why = a.abandon.wiped ? 'No move left.'
    : a.abandon.doomed ? 'The bars can no longer all be filled: it can only wipe.'
      : `Nothing still reachable would be kept (${esc(a.abandon.useless)}).`;
  return `<p class="verdict">${why} The memory stays ${a.from ? fmtMemory(a.from) : 'as it is'}; the pity still counts.</p>`;
}

/** Every result a spread can end on, likeliest first, marked kept or not. */
function results(spread, title) {
  const list = [...spread.results].sort((x, y) => y.p - x.p);
  const top = list.filter((r) => r.p >= 0.002).slice(0, 14);
  const rest = list.slice(top.length).reduce((x, r) => x + r.p, 0);
  const max = Math.max(...list.map((r) => r.p), spread.wipe, 1e-9);
  const row = (label, amp, p, kept) => `<div class="res${kept === true ? ' kept' : ''}">
      <span class="res-gd">${label}</span><span class="res-amp">${amp}</span>
      <span class="res-bar"><i style="width:${(100 * p) / max}%"></i></span><span class="res-p">${pct(p)}</span>
      <span class="res-k">${kept === true ? 'kept' : ''}</span></div>`;
  const rows = top.map((r) => row(`<span class="tc-g">${r.glory}</span> / <span class="tc-d">${r.despair}</span>`, `+${r.amp}%`, r.p, r.kept));
  if (rest > 0.0005) rows.push(row('<span class="muted">others</span>', '', rest, null));
  if (spread.wipe > 0.0005) rows.push(row('<span class="tc-d">wipe</span>', '', spread.wipe, false));
  const stats = `<div class="res-stats"><span>Improvement Chance <b>${spread.kept === null ? '-' : pct(spread.kept)}</b></span><span>Expected amplification <b>+${spread.amp.toFixed(1)}%</b></span><span>Wipe Chance <b>${pct(spread.wipe)}</b></span></div>`;
  return `<div class="results"><div class="results-title">${esc(title)}: glory / despair, amplification, chance</div>${stats}${rows.join('')}</div>`;
}

function prospectWhy(p) {
  const main = p.criteria[0];
  const unreachable = main.kind === 'chance' && p.score === 0 && p.criteria.length > 1
    ? `<p class="muted">Nothing an attempt can end on reaches that at this level, so it is played for ${p.criteria.slice(1).map((c) => `<i>${esc(c.label.toLowerCase())}</i>`).join(', then ')}.</p>` : '';
  return `<h2>What this attempt can come to</h2>
    ${playedFor(p.objective, p.from, p.by)}${unreachable}
    <div class="stats">
      <div class="stat"><b>${p.kept === null ? '-' : pct(p.kept)}</b><span>chance it improves the memory</span></div>
      <div class="stat"><b>${fmt(main.kind, p.score, 1)}</b><span>${esc(main.label.toLowerCase())}</span></div>
      <div class="stat"><b>${pct(p.wipe)}</b><span>chance of a wipe</span></div>
    </div>
    ${results(p, 'Every result an attempt from here can end on, played as advised')}`;
}

function compareWhy(a) {
  const { rows, rule } = a.compare;
  const decisive = rows.find((r) => r.better);
  const cell = (r, side) => `<td class="${r.better === side ? 'better' : ''}">${esc(r[side])}</td>`;
  const choice = a.press === 'complete' ? a.next : a.press;
  const title = a.press === 'complete' ? `Next, on the results screen: ${choice === 'keep' ? 'Keep' : 'Replace'}` : `Why ${choice === 'keep' ? 'Keep' : 'Replace'}?`;
  return `<h2>${title}</h2>
    <div class="table-wrap"><table class="keep">
      <thead><tr><th></th><th>Current</th><th>New</th></tr></thead>
      <tbody>${rows.map((r) => `<tr class="${r === decisive ? 'decisive' : ''}"><th>${esc(r.label)}</th>${cell(r, 'current')}${cell(r, 'fresh')}</tr>`).join('')}</tbody>
    </table></div>
    <p class="muted">${esc(rule)}${decisive ? ` Decided on <b>${esc(decisive.label.toLowerCase())}</b>.` : ' They tie on every count: the current memory stays.'}</p>`;
}
