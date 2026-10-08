// The side panel: settings, Start / Stop, and a live view of what the
// autoplayer sees and decides.

import { RelicSolver } from './lib/solver.js';
import { GameTab } from './lib/tab.js';
import { Runner } from './lib/runner.js';
import { Advisor } from './lib/advisor.js';
import { REF, at, classify } from './lib/vision.js';
import { TIER_PERCENT, amplification, show as showState } from './lib/logic.js';

const $ = (id) => document.getElementById(id);
const DEFAULTS = {
  mode: 'advisor', goal: '20+43', level: 18, kind: 'max', glory: 7, despair: 2, closeGlory: 9, closeDespair: 0, mark: '', clickDelay: 1.5,
};
const FIELDS = Object.keys(DEFAULTS);
const STRATEGY_FIELDS = ['kind', 'mark', 'glory', 'despair', 'closeGlory', 'closeDespair'];

const solver = await RelicSolver.load(await (await fetch('relic.wasm')).arrayBuffer());
const stored = await chrome.storage.local.get(['settings']);
chrome.storage.local.remove(['rates', 'escalation']); // kept by older versions
let runner = null;
let tab = null;
let lastFrame = null;
let pendingAsk = null;

// ---------------------------------------------------------------- settings

for (const f of FIELDS) $(f).value = stored.settings?.[f] ?? DEFAULTS[f];
// an older version's "gain toward a mark" is max amplification above that mark
if (stored.settings?.kind === 'reach') $('kind').value = 'max';
// and its weighted glory / despair play is "until a limit" now
if (stored.settings?.kind === 'score') $('kind').value = 'close';
// an older version kept a limit in the target's fields
if (stored.settings?.kind === 'close' && stored.settings.closeGlory == null) {
  $('closeGlory').value = stored.settings.glory ?? DEFAULTS.closeGlory;
  $('closeDespair').value = stored.settings.despair ?? DEFAULTS.closeDespair;
}

/**
 * What the glory, despair and amplification settings can be at the
 * inheritor level set: glory from the free glory successes an attempt opens
 * with up to every slot, despair from none up to every slot that does not
 * open as a failure (from level 19 one does), amplification up to a full
 * glory bar (+5% a slot).  The level's if it exists, else level 20's.
 */
function limits() {
  let board;
  try {
    board = solver.board(Number($('level').value));
  } catch {
    board = solver.board(20);
  }
  return { glory: [board.startGlory, board.slots], despair: [0, board.slots - board.startDespairFail], amp: [0, 5 * board.slots] };
}

/** Set each box's range to the level's, and bring a value outside it back in. */
function applyLimits() {
  const l = limits();
  for (const [id, [lo, hi]] of [['glory', l.glory], ['closeGlory', l.glory], ['despair', l.despair], ['closeDespair', l.despair], ['mark', l.amp]]) {
    const el = $(id);
    el.min = lo;
    el.max = hi;
    if (el.value.trim() === '') continue; // a blank Minimum Useful Amplification: 0
    const v = Number(el.value);
    if (Number.isFinite(v) && (v < lo || v > hi)) el.value = Math.min(hi, Math.max(lo, Math.round(v)));
  }
}

function readSettings() {
  const l = limits();
  const num = (id, lo, hi, name = id) => {
    const v = Number($(id).value);
    if (!Number.isInteger(v) || v < lo || v > hi) throw new Error(`${name} must be a whole number from ${lo} to ${hi}`);
    return v;
  };
  const kind = $('kind').value;
  const objective = { kind };
  if (kind === 'target') {
    Object.assign(objective, { glory: num('glory', ...l.glory, 'Glory Target'), despair: num('despair', ...l.despair, 'Despair Target') });
  }
  if (kind === 'close') {
    Object.assign(objective, { glory: num('closeGlory', ...l.glory, 'Max useful Glory'), despair: num('closeDespair', ...l.despair, 'Min useful Despair') });
  }
  if (kind === 'max' && $('mark').value.trim() !== '') objective.mark = num('mark', ...l.amp, 'Minimum Useful Amplification');
  const clickDelay = Number($('clickDelay').value);
  if (!(clickDelay >= 0.1 && clickDelay <= 10)) throw new Error('click delay must be from 0.1 to 10 seconds');
  const [goalLevel, tier] = $('goal').value.split('+').map(Number);
  return {
    mode: $('mode').value, goal: { level: goalLevel, tier: tier || 0 },
    level: num('level', 1, 20, 'Inheritor level'), objective, clickDelay,
  };
}

/**
 * The advisor's level and objective for the attempt it starts, copied into
 * the Single relic settings: a run stopped part-way and started again (its own
 * record of the attempt gone or stale) finishes it the same way.
 */
function useSettings({ level, objective: o }) {
  $('level').value = level;
  $('kind').value = o.kind;
  if (o.kind === 'target') {
    $('glory').value = o.glory;
    $('despair').value = o.despair ?? limits().despair[1]; // any despair: as much as the bar can hold
  }
  if (o.kind === 'close') {
    $('closeGlory').value = o.glory;
    $('closeDespair').value = o.despair;
  }
  if (o.kind === 'max') $('mark').value = o.mark ?? '';
  applyLimits();
  saveSettings();
}

function saveSettings() {
  chrome.storage.local.set({ settings: Object.fromEntries(FIELDS.map((f) => [f, $(f).value])) });
  const mode = $('mode').value;
  for (const el of document.querySelectorAll('[data-mode], [data-for]')) {
    el.hidden = (el.dataset.mode && el.dataset.mode !== mode) || (el.dataset.for && !el.dataset.for.split(' ').includes($('kind').value));
  }
  lockStrategy();
}

/** Smart Leveler: the strategy is the plan's (set as each attempt starts), shown but not set. */
function lockStrategy() {
  const plan = $('mode').value === 'advisor';
  for (const id of STRATEGY_FIELDS) {
    $(id).disabled = plan;
    $(id).title = plan ? 'Set by the Look Ahead plan for each attempt' : '';
  }
}
for (const f of FIELDS) $(f).addEventListener('change', saveSettings);
// the ranges follow the inheritor level as it is typed
$('level').addEventListener('input', applyLimits);
$('level').addEventListener('change', () => {
  applyLimits();
  saveSettings();
});
applyLimits();
saveSettings();

// ---------------------------------------------------------------------- ui

function log(text, level = '') {
  const list = $('log');
  // follow new lines only if the log box was already at its end, and scroll
  // only the box - never the panel (scrollIntoView would move both)
  const following = list.scrollHeight - list.scrollTop - list.clientHeight < 8;
  const li = document.createElement('li');
  li.textContent = `${new Date().toLocaleTimeString()} ${text}`;
  li.className = level;
  list.append(li);
  if (following) list.scrollTop = list.scrollHeight;
}

function setStatus(text, cls = '') {
  $('status').textContent = text;
  $('status').className = `pill ${cls}`;
}

function setRunning(on) {
  $('start').disabled = on;
  $('peek').disabled = on;
  $('stop').disabled = !on;
  for (const el of document.querySelectorAll('#settings input, #settings select')) el.disabled = on;
  if (!on) lockStrategy();
  setStatus(on ? 'running' : 'idle', on ? 'running' : '');
}

const MOVES = { glory: 'Attempt Memory of Glory', despair: 'Attempt Memory of Despair', train: 'Mental Training', done: 'Attempt complete', deadend: 'Wiped' };

function show(frame, info = {}) {
  lastFrame = frame;
  const s = frame.screen;
  $('screen').textContent = info.note ? `${s.kind} - ${info.note}` : s.kind;
  $('state').textContent = info.state ? showState(info.state) : '-';
  $('rate').textContent = info.tier != null ? `${TIER_PERCENT[info.tier]}%` : '-';
  $('move').textContent = info.action ? MOVES[info.action] : '-';
  const kind = runner?.objective?.kind ?? 'max';
  const mark = runner?.objective?.mark;
  const worth = kind === 'close' ? `${info.expected?.toFixed(2)} useful steps until the limit`
    : kind === 'target' ? `${(100 * info.expected).toFixed(1)}% to hit the target`
      : maxScore(mark, runner?.current, info.expected);
  $('expected').textContent = info.expected == null ? '-' : `${worth}, ${(100 * info.finish).toFixed(1)}% no wipe`;
  draw(frame, info);
}

/**
 * What 'max' plays for, in words: the chance of the mark (blank: 0), raised to
 * beat the memory in place - or, a 0 mark with nothing to beat, the
 * amplification expected (the engine plays for that then).
 */
function maxScore(mark, current, expected) {
  const beat = current ? amplification(current) + 1 : 0;
  if (!mark && beat <= 1) return `+${expected?.toFixed(1)}% amplification expected`;
  return `${(100 * expected).toFixed(1)}% to reach +${Math.max(mark ?? 0, beat)}%`;
}

/** The game around the panels, with what was read marked on it. */
function draw(frame, info) {
  const canvas = $('view');
  const ctx = canvas.getContext('2d');
  const src = new OffscreenCanvas(frame.img.width, frame.img.height);
  src.getContext('2d').putImageData(frame.img, 0, 0);
  const a = frame.screen.anchor;
  const main = frame.screen.kind === 'main' ? frame.screen : null;
  let [x0, y0, x1, y1] = [0, 0, frame.img.width, frame.img.height];
  if (a) {
    [x0, y0] = at(a, -15, -150);
    [x1, y1] = at(a, 415, 345);
  } else if (main) {
    // the phone's screen around the main page
    const b = main.banner;
    [x0, y0, x1, y1] = [b.x0 - 140 * b.s, b.y0 - 150 * b.s, b.x0 + 330 * b.s, b.y0 + 720 * b.s];
  }
  const k = canvas.width / (x1 - x0);
  canvas.height = Math.round((y1 - y0) * k);
  ctx.drawImage(src, x0, y0, x1 - x0, y1 - y0, 0, 0, canvas.width, canvas.height);
  const dot = (x, y, colour, r = 3) => {
    ctx.beginPath();
    ctx.arc((x - x0) * k, (y - y0) * k, r, 0, 2 * Math.PI);
    ctx.strokeStyle = colour;
    ctx.lineWidth = 2;
    ctx.stroke();
  };
  if (main) {
    main.tiles.forEach((t, i) => {
      const r = main.relics?.[i];
      dot(t.x, t.y, r && r.glory !== null && r.despair !== null ? '#7CFC00' : main.relics ? '#ff4040' : '#6cb6ff', 10);
    });
    dot(main.toggle.x, main.toggle.y, '#ffd700', 8);
  }
  if (!a) return;
  const colours = { success: '#7CFC00', fail: '#ff4040', empty: '#6cb6ff', none: '#666' };
  const s = frame.screen;
  const bars = s.kind === 'board' ? [[s.glory, REF.gloryRow], [s.despair, REF.despairRow]]
    : s.memory ? [[s.memory.glory.bar, REF.gloryRow], [s.memory.despair.bar, REF.despairRow]] : [];
  for (const [cells, row] of bars) {
    cells.forEach((v, i) => dot(...at(a, REF.slotX0 + REF.slotStep * i, row), colours[v]));
  }
  if (s.kind === 'board') {
    for (let i = 0; i < s.leaves.count; i++) dot(...at(a, REF.slotX0 + REF.slotStep * i, REF.leafRow), '#7CFC00');
    const [bx] = at(a, REF.barX[0] + s.fill * (REF.barX[1] - REF.barX[0]), 0);
    dot(bx, at(a, 0, REF.barRows[1])[1], '#fff', 4);
    const target = info.action && s.buttons[info.action];
    if (target) dot(target.x, target.y, '#ffd700', 12);
  }
  if (s.button) dot(s.button.x, s.button.y, '#ffd700', 12);
  if (s.buttons?.keep) for (const b of [s.buttons.keep, s.buttons.replace]) dot(b.x, b.y, '#ffd700', 12);
}

function askTier(candidates) {
  $('ask').hidden = false;
  $('askText').textContent = candidates.length === 5
    ? 'Which rate is on the Attempt buttons right now?'
    : 'Training at full spirit power - which rate do the buttons show now?';
  const box = $('askButtons');
  box.replaceChildren();
  return new Promise((resolve) => {
    pendingAsk = resolve;
    for (const tier of candidates) {
      const b = document.createElement('button');
      b.textContent = `${TIER_PERCENT[tier]}%`;
      b.onclick = () => finishAsk(tier);
      box.append(b);
    }
  });
}

/** Which of the inheritor levels a board fits it is, or null if you press Stop. */
function askLevel(levels) {
  $('ask').hidden = false;
  $('askText').textContent = 'The board fits more than one inheritor level - which is yours?';
  const box = $('askButtons');
  box.replaceChildren();
  return new Promise((resolve) => {
    pendingAsk = resolve;
    for (const level of levels) {
      const b = document.createElement('button');
      b.textContent = `Level ${level}`;
      b.onclick = () => finishAsk(level);
      box.append(b);
    }
  });
}

/** The inheritor level a run read off the board: the setting follows it. */
function levelSeen(level) {
  $('level').value = level;
  applyLimits();
  saveSettings();
}

/** A whole number from you, or null if you press Stop. */
function askNumber(question) {
  $('ask').hidden = false;
  $('askText').textContent = question;
  const box = $('askButtons');
  box.replaceChildren();
  const input = document.createElement('input');
  input.type = 'number';
  input.min = '0';
  const ok = document.createElement('button');
  ok.textContent = 'OK';
  box.append(input, ok);
  input.focus();
  return new Promise((resolve) => {
    pendingAsk = resolve;
    const submit = () => {
      const n = Number(input.value);
      if (input.value !== '' && Number.isInteger(n) && n >= 0) finishAsk(n);
    };
    ok.onclick = submit;
    // (a block body: an on-handler that returns false cancels the keystroke)
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') submit();
    });
  });
}

function finishAsk(tier) {
  $('ask').hidden = true;
  const resolve = pendingAsk;
  pendingAsk = null;
  resolve?.(tier);
}

const ui = {
  log, show, askTier, askLevel, askNumber, useSettings, levelSeen,
  // the frame a run stopped on, unrecognised: kept for a look
  saveFrame: (frame) => (frame?.png ? download(frame) : null),
  // the attempt the advisor has in play, so a stopped run can finish it
  saveAttempt: (record) => (record ? chrome.storage.local.set({ attempt: record }) : chrome.storage.local.remove('attempt')),
  loadAttempt: async () => (await chrome.storage.local.get('attempt')).attempt ?? null,
};

// ----------------------------------------------------------------- actions

async function gameTab() {
  const [active] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!active) throw new Error('No active tab.');
  if (!tab || tab.target.tabId !== active.id) {
    await tab?.detach();
    tab = new GameTab(active.id, (text) => log(text, 'warn'));
  }
  await tab.attach();
  return tab;
}

function makeRunner(gt) {
  const settings = readSettings();
  return settings.mode === 'advisor' ? new Advisor({ tab: gt, solver, settings, ui }) : new Runner({ tab: gt, solver, settings, ui });
}

$('start').onclick = async () => {
  try {
    runner = makeRunner(await gameTab());
  } catch (err) {
    log(err.message, 'error');
    return;
  }
  setRunning(true);
  await runner.run();
  setRunning(false);
  setStatus('stopped', 'stopped');
  await tab?.detach();
};

$('stop').onclick = () => {
  runner?.stop();
  finishAsk(null);
  log('Stopping...', 'stop');
};

$('peek').onclick = async () => {
  try {
    runner = makeRunner(await gameTab());
    const info = await runner.peek();
    log(`Read: ${info.action ? `${MOVES[info.action]} at ${TIER_PERCENT[info.tier]}%` : info.note}`);
  } catch (err) {
    log(err.message, 'error');
  } finally {
    await tab?.detach();
  }
};

// a fresh screenshot of the tab as it is now (during a run: the frame
// the run last looked at, which is just as fresh)
$('save').onclick = async () => {
  let frame = lastFrame;
  if (!runner?.running) {
    try {
      const gt = await gameTab();
      const shot = await gt.capture();
      frame = { ...shot, screen: classify(shot.img, null, true) };
      show(frame, { note: 'saved frame' });
    } catch (err) {
      return log(err.message, 'error');
    } finally {
      await tab?.detach();
    }
  }
  if (!frame) return log('Nothing captured yet.', 'warn');
  download(frame);
  log(`Saved the ${frame.screen.kind === 'unknown' ? 'current' : frame.screen.kind} screen.`);
};

/** A frame to the Downloads folder, as relic-<screen>-<time>.png; its name. */
function download(frame) {
  const name = `relic-${frame.screen.kind}-${Date.now()}.png`;
  const a = document.createElement('a');
  a.href = `data:image/png;base64,${frame.png}`;
  a.download = name;
  a.click();
  return name;
}

chrome.debugger.onDetach.addListener((source, reason) => {
  if (tab && source.tabId === tab.target.tabId && tab.attached) {
    tab.attached = false;
    if (runner?.running) {
      runner.stop();
      finishAsk(null);
      log(`Lost the tab (${reason}) - stopped.`, 'stop');
    }
  }
});

setStatus('idle');
log('Ready. Press Read screen to check what I see, then Start.');
