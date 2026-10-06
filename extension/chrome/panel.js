// The side panel: settings, Start / Stop, and a live view of what the
// autoplayer sees and decides.

import { RelicSolver } from './lib/solver.js';
import { GameTab } from './lib/tab.js';
import { Runner } from './lib/runner.js';
import { Advisor } from './lib/advisor.js';
import { REF, at, classify } from './lib/vision.js';
import { TIER_PERCENT, show as showState } from './lib/logic.js';

const $ = (id) => document.getElementById(id);
const DEFAULTS = { mode: 'advisor', goal: '20+46', level: 18, kind: 'max', glory: 7, despair: 2, mark: 30, attempts: 1, wipe: 0, clickDelay: 1.5, clickMethod: 'mouse' };
const FIELDS = Object.keys(DEFAULTS);

const solver = await RelicSolver.load(await (await fetch('relic.wasm')).arrayBuffer());
const stored = await chrome.storage.local.get(['settings']);
chrome.storage.local.remove('rates'); // samples from an older version
let runner = null;
let tab = null;
let lastFrame = null;
let pendingAsk = null;

// ---------------------------------------------------------------- settings

for (const f of FIELDS) $(f).value = stored.settings?.[f] ?? DEFAULTS[f];

function readSettings() {
  const num = (id, lo, hi) => {
    const v = Number($(id).value);
    if (!Number.isInteger(v) || v < lo || v > hi) throw new Error(`${id} must be a whole number from ${lo} to ${hi}`);
    return v;
  };
  const kind = $('kind').value;
  const objective = { kind, wipePenalty: Math.max(0, Number($('wipe').value) || 0) };
  if (kind === 'target') Object.assign(objective, { glory: num('glory', 0, 10), despair: num('despair', 0, 10) });
  if (kind === 'reach') objective.mark = num('mark', 0, 50);
  const clickDelay = Number($('clickDelay').value);
  if (!(clickDelay >= 0.1 && clickDelay <= 10)) throw new Error('click delay must be from 0.1 to 10 seconds');
  const [goalLevel, tier] = $('goal').value.split('+').map(Number);
  return {
    mode: $('mode').value, goal: { level: goalLevel, tier: tier || 0 },
    level: num('level', 1, 28), objective, attempts: num('attempts', 1, 999),
    clickMethod: $('clickMethod').value, clickDelay,
  };
}

function saveSettings() {
  chrome.storage.local.set({ settings: Object.fromEntries(FIELDS.map((f) => [f, $(f).value])) });
  const mode = $('mode').value;
  for (const el of document.querySelectorAll('[data-mode], [data-for]')) {
    el.hidden = (el.dataset.mode && el.dataset.mode !== mode) || (el.dataset.for && el.dataset.for !== $('kind').value);
  }
}
for (const f of FIELDS) $(f).addEventListener('change', saveSettings);
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
  const worth = kind === 'max' ? `+${info.expected?.toFixed(1)}% amplification`
    : kind === 'score' ? `score ${info.expected?.toFixed(2)}` : `${(100 * info.expected).toFixed(1)}% to hit`;
  $('expected').textContent = info.expected == null ? '-' : `${worth}, ${(100 * info.finish).toFixed(1)}% no wipe`;
  draw(frame, info);
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
  log, show, askTier, askNumber,
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
    tab = new GameTab(active.id);
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
  const a = document.createElement('a');
  a.href = `data:image/png;base64,${frame.png}`;
  a.download = `relic-${frame.screen.kind}-${Date.now()}.png`;
  a.click();
  log(`Saved the ${frame.screen.kind === 'unknown' ? 'current' : frame.screen.kind} screen.`);
};

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
