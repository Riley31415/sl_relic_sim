// The page: the simulated game with the button to press marked (and how
// each move went told beside its button), the reasons, screenshots in,
// corrections, settings and a record of what happened.  Everything shown
// comes from the session (session.js) and is drawn again after every input.

import { levelList } from './lib/logic.js';
import { RelicSolver } from './lib/solver.js';
import { MOVES, act, advise, blank } from './session.js';
import { readScreenshot } from './screenshot.js';
import { esc, renderPhone } from './view.js';
import { renderWhy } from './why.js';

const $ = (id) => document.getElementById(id);

/** localStorage, for this browser's own conveniences: it may be missing or refuse. */
const store = {
  get(key, fallback) {
    try {
      const v = localStorage.getItem(key);
      return v ? JSON.parse(v) : fallback;
    } catch {
      return fallback;
    }
  },
  set(key, value) {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      // private mode, or full: the page works without it
    }
  },
};

let solver;
try {
  solver = await RelicSolver.load(await (await fetch('relic.wasm')).arrayBuffer());
} catch (err) {
  $('advice').innerHTML = `<p class="error">The solver would not load (${esc(err.message)}). This page needs a current browser with WebAssembly.</p>`;
  throw err;
}

// ---------------------------------------------------------------- settings

const DEFAULTS = { mode: 'single', goal: '20+43', kind: 'max', mark: '', glory: 7, despair: 2, closeGlory: 9, closeDespair: 0 };
const FIELDS = Object.keys(DEFAULTS);
const saved = store.get('settings', {});
for (const f of FIELDS) $(f).value = saved[f] ?? DEFAULTS[f];

/** The settings as session.js takes them; throws for a value out of range. */
function readSettings() {
  const whole = (id, lo, hi, name) => {
    const v = Number($(id).value);
    if ($(id).value.trim() === '' || !Number.isInteger(v) || v < lo || v > hi) throw new Error(`${name} must be a whole number from ${lo} to ${hi}.`);
    return v;
  };
  const kind = $('kind').value;
  const objective = { kind };
  if (kind === 'target') Object.assign(objective, { glory: whole('glory', 0, 10, 'Glory Target'), despair: whole('despair', 0, 10, 'Despair Target') });
  if (kind === 'close') Object.assign(objective, { glory: whole('closeGlory', 0, 10, 'Max useful Glory'), despair: whole('closeDespair', 0, 10, 'Min useful Despair') });
  if (kind === 'max' && $('mark').value.trim() !== '') objective.mark = whole('mark', 0, 50, 'Minimum Useful Amplification');
  const [level, tier] = $('goal').value.split('+').map(Number);
  return { mode: $('mode').value, goal: { level, tier: tier || 0 }, objective };
}

let settings = (() => {
  try {
    return readSettings();
  } catch {
    for (const f of FIELDS) $(f).value = DEFAULTS[f];
    return readSettings();
  }
})();

function settingsChanged() {
  for (const el of document.querySelectorAll('#settings [data-mode]')) {
    el.hidden = el.dataset.mode !== $('mode').value || (el.dataset.for && el.dataset.for !== $('kind').value);
  }
  try {
    settings = readSettings();
    $('settings-error').hidden = true;
    store.set('settings', Object.fromEntries(FIELDS.map((f) => [f, $(f).value])));
    render();
  } catch (err) {
    $('settings-error').textContent = err.message;
    $('settings-error').hidden = false;
  }
}
$('settings').addEventListener('change', settingsChanged);
// a number setting counts while it is typed, once it is a whole one in range
let settingTimer = null;
$('settings').addEventListener('input', (e) => {
  if (e.target.type !== 'number') return;
  clearTimeout(settingTimer);
  settingTimer = setTimeout(() => {
    try {
      settings = readSettings();
    } catch {
      return; // not yet: leaving the box says why
    }
    $('settings-error').hidden = true;
    store.set('settings', Object.fromEntries(FIELDS.map((f) => [f, $(f).value])));
    render();
  }, 300);
});

// ----------------------------------------------------------------- session

// (v2: every number has a value; a session saved before that is dropped)
let session = store.get('session-v2', null) ?? blank();
let history = store.get('history-v2', []); // earlier sessions, newest last: Undo goes back through them
let log = store.get('log', []); // [{ at, text }], newest first
let opened = null; // the move opened in the reasons
let problem = null; // why the last input could not be taken
try {
  advise(session, settings, solver);
} catch {
  session = blank(); // saved by an older page, or broken: start afresh
  history = [];
}

function save() {
  store.set('session-v2', session);
  store.set('history-v2', history.slice(-100));
  store.set('log', log.slice(0, 200));
}

function note(lines) {
  const at = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  for (const text of lines) log.unshift({ at, text });
}

/**
 * An input, taken: a new session (the old one kept for Undo), or the reason
 * it cannot be.  `later`: draw the page once the browser has moved focus
 * on (a box changed by Tab), so the box tabbed to keeps it.
 */
let lastKey = null; // the box the last input came from, while it is still being typed in
let future = []; // sessions undone, newest last: Redo goes forward through them; a new input clears them

function dispatch(input, later = false, key = null) {
  try {
    const out = act(session, input, settings, solver);
    // one box typed into, number after number: one step back for Undo
    if (!key || key !== lastKey) {
      history.push(session);
      if (history.length > 100) history.shift();
    }
    lastKey = key;
    future = [];
    session = out.session;
    note(out.log);
    problem = null;
  } catch (err) {
    problem = err.message;
  }
  opened = null;
  save();
  if (later) setTimeout(render);
  else render();
}

function undo() {
  if (!history.length) return;
  lastKey = null;
  future.push(session);
  session = history.pop();
  note(['Undid the last input.']);
  problem = null;
  opened = null;
  save();
  render();
}

/** Every number and the attempt back to the defaults - an input like any other, so Undo brings them back. */
function clearData() {
  if (JSON.stringify(session) === JSON.stringify(blank())) return;
  history.push(session);
  if (history.length > 100) history.shift();
  lastKey = null;
  future = [];
  session = blank();
  note(['Cleared the data.']);
  shotStatus('');
  problem = null;
  opened = null;
  save();
  render();
}

function redo() {
  if (!future.length) return;
  lastKey = null;
  history.push(session);
  session = future.pop();
  note(['Redid the input undone.']);
  problem = null;
  opened = null;
  save();
  render();
}

// ------------------------------------------------------------------ drawing

let advice = null;

function render() {
  try {
    advice = advise(session, settings, solver);
  } catch (err) {
    advice = { press: null, headline: 'Something went wrong', detail: err.message };
  }
  // the box being typed in, kept across the redraw: mid-number the very box (its caret and all), else its new one
  const active = $('screen').contains(document.activeElement) ? document.activeElement : null;
  $('screen').innerHTML = renderPhone(session, { solver, settings, advice });
  const fresh = active?.dataset.field && $('screen').querySelector(`[data-field="${active.dataset.field}"]`);
  if (fresh && active === typing) {
    for (const attr of ['class', 'min', 'max', 'title']) active.setAttribute(attr, fresh.getAttribute(attr) ?? '');
    fresh.replaceWith(active);
    active.focus();
  } else if (fresh) {
    fresh.focus();
    if (fresh.type === 'number') fresh.select();
  }
  pointAt();
  showPlanStrategy();
  renderAdvice();
  $('why').innerHTML = renderWhy(advice, opened);
  renderLog();
  $('undo').disabled = !history.length;
  $('redo').disabled = !future.length;
  $('clear').disabled = JSON.stringify(session) === JSON.stringify(blank());
}

/**
 * Smart Leveler: the strategy the plan plays the attempt in view for (or would
 * start one with), in the settings' own boxes, which cannot be changed. With no
 * attempt in view, the plan has not picked one yet.
 */
function showPlanStrategy() {
  const o = advice.objective ?? advice.prospect?.objective ?? advice.plan?.objective ?? session.attempt?.objective ?? null;
  $('plan-kind').value = o?.kind ?? '';
  const shown = (label, value) => `<label>${label} <input type="number" value="${value ?? ''}" placeholder="${value === null ? 'any' : ''}" disabled></label>`;
  $('plan-numbers').innerHTML = !o ? ''
    : o.kind === 'max' ? `<label>Minimum Useful Amplification <span class="unit"><input type="number" value="${o.mark ?? ''}" disabled>%</span></label>`
      : o.kind === 'target' ? shown('Glory Target', o.glory) + shown('Despair Target', o.despair)
        : shown('Max useful Glory', o.glory) + shown('Min useful Despair', o.despair);
}

/** The game button the advice points at, on the simulated screen. */
function target() {
  if (!advice.press) return null;
  if (advice.press === 'tile') return $('screen').querySelector(`.tile[data-tile="${advice.tile}"]`);
  return $('screen').querySelector(`[data-press="${advice.press}"]`);
}

/** The arrow onto the button to press. */
function pointAt() {
  const arrow = $('arrow');
  const el = target();
  if (!el) {
    arrow.setAttribute('hidden', '');
    return;
  }
  el.classList.add('target');
  // from the left onto an Attempt button, and where a box sits right above the button; else from above
  const side = MOVES.includes(advice.press) || advice.press === 'inheritance' || advice.press === 'levelup' ? 'left' : 'above';
  const p = $('phone').getBoundingClientRect(), t = el.getBoundingClientRect();
  arrow.removeAttribute('hidden');
  arrow.setAttribute('class', `arrow arrow-${side}`);
  // smaller on a relic tile, and onto its top edge: the tile above keeps its numbers in view
  const tile = advice.press === 'tile';
  const size = tile ? 28 : 40;
  arrow.style.width = arrow.style.height = `${size}px`;
  if (side === 'left') {
    arrow.style.left = `${t.left - p.left - size - 6}px`;
    arrow.style.top = `${t.top - p.top + t.height / 2 - size / 2}px`;
  } else {
    arrow.style.left = `${t.left - p.left + t.width / 2 - size / 2}px`;
    arrow.style.top = `${Math.max(0, t.top - p.top - size + (tile ? 12 : -2))}px`;
  }
}
// the phone's layout moves the button: follow it
new ResizeObserver(() => advice && pointAt()).observe($('phone'));
document.fonts?.ready.then(() => advice && pointAt());


/**
 * The advice is the arrow on the game screen; a step with no button to point
 * at (Summon, Done, open the relic) is a popup over the game - red for a stop.
 * Under the import, only what is about the numbers: an input refused, a
 * number to check, which relic it is.
 */
const popup = $('advice-pop'); // held here: on the main page it is moved into the screen, which is redrawn
function renderAdvice() {
  const a = advice;
  const say = !a.press && !a.start && !a.needs;
  popup.hidden = !say;
  popup.classList.toggle('advice-stop', !!a.stop);
  popup.innerHTML = say ? `<b>${esc(a.headline)}</b>${a.detail ? `<p>${esc(a.detail)}</p>` : ''}` : '';
  // on the main page, in the middle of the twelve relic tiles; elsewhere the middle of the game
  ($('screen').querySelector('.tiles') ?? $('phone')).append(popup);
  let html = '';
  if (problem) html += `<p class="error" role="alert">${esc(problem)}</p>`;
  if (a.levelFits) {
    html += `<p class="warn">The level is not shown on this screen: it fits ${levelList(a.levelFits)}, set to ${session.level}. Check the orange box.</p>`;
  }
  const unread = (a.check ?? []).filter((f) => !(f === 'level' && a.levelFits));
  if (unread.length) {
    const one = unread.length === 1;
    html += `<p class="warn">Check the orange ${one ? 'box' : 'boxes'}: ${one ? 'it' : 'they'} did not read.</p>`;
  }
  for (const n of a.needs ?? []) {
    const ok = n.what === 'states' ? ' Put them right on the game screen, or: <button type="button" data-counts-ok>They are right</button>' : '';
    html += `<div class="need"><span>${esc(n.text)}</span>${ok}</div>`;
  }
  $('advice').innerHTML = html;
}

// the game's layout follows the window: the arrow with it
window.addEventListener('resize', pointAt);

$('advice').addEventListener('click', (e) => {
  if (e.target.closest('[data-counts-ok]')) dispatch({ type: 'set', countsChecked: true });
});

$('undo').onclick = undo;
$('redo').onclick = redo;
$('clear').onclick = clearData;

// the simulated screen: a button there is one the player pressed in the game
$('screen').addEventListener('click', (e) => {
  // an arrow beside a box: its number one up or down, within the box's range
  const step = e.target.closest('[data-step]');
  if (step) {
    const field = step.dataset.stepField, el = $('screen').querySelector(`input[data-field="${field}"]`);
    const value = Math.min(Number(el.max), Math.max(Number(el.min), Number(el.value) + Number(step.dataset.step)));
    return dispatch(fieldInput(field, String(value)));
  }
  if (e.target.closest('input, select, label')) return; // a box, not the button around it
  const how = e.target.closest('[data-outcome]');
  if (how) return dispatch({ type: 'move', action: how.dataset.move, success: how.dataset.outcome === '1' });
  const b = e.target.closest('[data-press]');
  if (!b) return;
  const press = b.dataset.press;
  if (MOVES.includes(press)) {
    // an Attempt button: how it went is told beside it; here, where it leads
    opened = press;
    $('why').innerHTML = renderWhy(advice, opened);
    return;
  }
  if (press === 'tile') return dispatch({ type: 'tile', relic: Number(b.dataset.tile) });
  dispatch({ type: 'press', button: press });
});
$('screen').addEventListener('keydown', (e) => {
  // a relic tile, by keyboard
  const tile = e.target.closest('[role=button][data-press]');
  if (tile && e.target === tile && (e.key === 'Enter' || e.key === ' ')) {
    e.preventDefault();
    e.stopPropagation();
    tile.click();
  }
});
$('screen').addEventListener('change', (e) => {
  const el = e.target;
  clearTimeout(typingTimer);
  typing = null;
  if (el.dataset.set === 'relic') dispatch({ type: 'set', relic: Number(el.value) });
  else if (el.dataset.field) dispatch(fieldInput(el.dataset.field, el.value), true, el.dataset.field);
});

/** A whole number within a box's range: what is taken while it is still being typed. */
const fits = (el) => el.value.trim() !== '' && Number.isInteger(Number(el.value)) && Number(el.value) >= Number(el.min) && Number(el.value) <= Number(el.max);

// a number being typed counts as soon as it is a whole one in range, so the
// advice (and its arrow) follows it; a half-typed one waits for the box to be left
let typing = null, typingTimer = null;
$('screen').addEventListener('input', (e) => {
  const el = e.target;
  if (el.type !== 'number' || !el.dataset.field) return;
  clearTimeout(typingTimer);
  typingTimer = setTimeout(() => {
    if (!fits(el) || document.activeElement !== el) return;
    typing = el;
    dispatch(fieldInput(el.dataset.field, el.value), false, el.dataset.field);
    typing = null;
  }, 300);
});

/** A number box on the game screen, changed: the correction it makes. */
function fieldInput(field, raw) {
  const v = raw.trim() === '' ? NaN : Number(raw);
  const s = session;
  const relic = /^([gdn])(\d+)$/.exec(field);
  if (relic) return { type: 'set', relicStates: [{ i: Number(relic[2]), [{ g: 'glory', d: 'despair', n: 'stock' }[relic[1]]]: v }] };
  switch (field) {
    case 'level': return { type: 'set', level: v };
    case 'pity': return { type: 'set', pity: v };
    case 'onHand': return { type: 'set', onHand: v };
    case 'tier': return { type: 'set', tier: v };
    case 'mg': return { type: 'set', memory: { ...s.memory, glory: v } };
    case 'md': return { type: 'set', memory: { ...s.memory, despair: v } };
    case 'cg': return { type: 'set', memory: { ...s.result.current, glory: v } };
    case 'cd': return { type: 'set', memory: { ...s.result.current, despair: v } };
    case 'fg': return { type: 'set', fresh: { ...s.result.fresh, glory: v } };
    case 'fd': return { type: 'set', fresh: { ...s.result.fresh, despair: v } };
    default: {
      // the attempt's board: successes (gs, ds) and fails (gx, dx) a bar, spirit power, mental strength
      const st = s.attempt.st;
      const n = { gs: st.gs, gx: st.gf - st.gs, ds: st.ds, dx: st.df - st.ds, sp: st.sp, ms: st.ms, [field]: v };
      return { type: 'set', st: { gs: n.gs, gf: n.gs + n.gx, ds: n.ds, df: n.ds + n.dx, sp: n.sp, ms: n.ms } };
    }
  }
}

// the reasons: a move opened up
$('why').addEventListener('click', (e) => {
  const row = e.target.closest('[data-expand]');
  if (!row) return;
  opened = row.dataset.expand;
  $('why').innerHTML = renderWhy(advice, opened);
});
$('why').addEventListener('keydown', (e) => {
  const row = e.target.closest('[data-expand]');
  if (row && (e.key === 'Enter' || e.key === ' ')) {
    e.preventDefault();
    opened = row.dataset.expand;
    $('why').innerHTML = renderWhy(advice, opened);
    $('why').querySelector(`[data-expand="${opened}"]`)?.focus();
  }
});

// Ctrl+Z (Cmd+Z): Undo; Ctrl+Y or Ctrl+Shift+Z: Redo - outside a box, where they undo and redo the typing instead
document.addEventListener('keydown', (e) => {
  if (!(e.ctrlKey || e.metaKey) || (e.target instanceof Element && e.target.closest('input, select, textarea'))) return;
  const key = e.key.toLowerCase();
  if (key === 'z' && !e.shiftKey) undo();
  else if (key === 'y' || (key === 'z' && e.shiftKey)) redo();
  else return;
  e.preventDefault();
});

function renderLog() {
  $('log').innerHTML = log.slice(0, 60).map((l) => `<li><time>${esc(l.at)}</time> ${esc(l.text)}</li>`).join('') || '<li class="muted">Nothing yet.</li>';
}

// ------------------------------------------------------------- screenshots

/** An image file's pixels. */
async function pixels(file) {
  const bitmap = await createImageBitmap(file);
  const canvas = document.createElement('canvas');
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  ctx.drawImage(bitmap, 0, 0);
  return { canvas, img: ctx.getImageData(0, 0, bitmap.width, bitmap.height) };
}

const SCREENS = {
  board: 'an attempt in progress', complete: 'Complete Inheritance', legacy: "Hero's Legacy", compare: 'the results',
  levels: 'the main page (levels view)', counts: 'the main page (counts view)', unknown: 'a screen I do not recognise',
};

async function readFiles(files) {
  for (const file of files) {
    if (!file.type.startsWith('image/')) continue;
    shotStatus(`Reading ${file.name || 'the pasted image'}...`);
    await new Promise((r) => setTimeout(r, 30)); // let that show before the work
    try {
      const { canvas, img } = await pixels(file);
      const reading = readScreenshot(img);
      showShot(canvas, reading);
      shotStatus(`: ${SCREENS[reading.kind]}.`, file.name || 'Pasted image');
      if (reading.kind === 'unknown') {
        problem = "That screenshot is not a screen I know. Try the main page, a relic's Hero's Legacy, an attempt, Complete Inheritance or the results, with the whole game in view.";
        render();
        continue;
      }
      if (reading.kind === 'counts' && $('mode').value !== 'advisor') {
        // every relic's glory and despair: the whole board, which is what Smart Leveler works from
        $('mode').value = 'advisor';
        settingsChanged();
        note(['Switched to Smart Leveler: the counts view shows the whole board.']);
      }
      dispatch({ type: 'read', reading });
    } catch (err) {
      shotStatus(`Could not read that image (${err.message}).`);
    }
  }
}

/** The screenshot, cut to the game, with a dot on everything read (green read, red not). */
function showShot(source, reading) {
  const view = $('shot-view');
  const [x0, y0, x1, y1] = reading.marks.crop ?? [0, 0, source.width, source.height];
  const w = Math.max(1, x1 - x0), h = Math.max(1, y1 - y0);
  const k = Math.min(1, 360 / w, 440 / h);
  view.width = Math.round(w * k);
  view.height = Math.round(h * k);
  const ctx = view.getContext('2d');
  ctx.drawImage(source, x0, y0, w, h, 0, 0, view.width, view.height);
  ctx.lineWidth = 2;
  for (const d of reading.marks.dots) {
    ctx.beginPath();
    ctx.arc((d.x - x0) * k, (d.y - y0) * k, Math.max(2, d.r * k), 0, 2 * Math.PI);
    ctx.strokeStyle = d.colour;
    ctx.stroke();
  }
}

/** The line under the upload box: `text`, after the screenshot's name (hover it to see what was read) if one was read. */
function shotStatus(text, name = null) {
  $('shot-status').hidden = !text;
  $('shot-name').hidden = name === null;
  $('shot-file').textContent = name ?? '';
  $('shot-text').textContent = text;
}

$('files').addEventListener('change', (e) => {
  readFiles([...e.target.files]);
  e.target.value = '';
});
document.addEventListener('paste', (e) => {
  const files = [...(e.clipboardData?.items ?? [])].filter((i) => i.type.startsWith('image/')).map((i) => i.getAsFile()).filter(Boolean);
  if (files.length) {
    e.preventDefault();
    readFiles(files);
  }
});
let dragging = 0;
document.addEventListener('dragenter', (e) => {
  if (![...(e.dataTransfer?.types ?? [])].includes('Files')) return;
  dragging++;
  $('overlay').hidden = false;
});
document.addEventListener('dragleave', () => {
  dragging = Math.max(0, dragging - 1);
  if (!dragging) $('overlay').hidden = true;
});
document.addEventListener('dragover', (e) => e.preventDefault());
document.addEventListener('drop', (e) => {
  e.preventDefault();
  dragging = 0;
  $('overlay').hidden = true;
  readFiles([...(e.dataTransfer?.files ?? [])]);
});

settingsChanged();
