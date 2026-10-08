// A screenshot, read: which screen of the game it shows and what is on it,
// in the shape session.js takes.
//
// The screen reader is the advisor's (lib/vision.js), as the extension's is.  It places
// everything from the game's own borders, so any size works, but its digit
// examples come from cloud-phone captures at about 1x, and a phone's own
// screenshot is 2-3x that.  So a screen found larger than 1.4x is read again
// shrunk to about 1x, and a number the two reads disagree on is left
// unread (for the player to type in) rather than guessed.
//
// An image is { width, height, data } with RGBA bytes, like ImageData.

import { REF, at, boardRate, classify } from './lib/vision.js';

/** `img` shrunk by `k` (under 1): each pixel the mean of those it covers. */
export function shrink(img, k) {
  const width = Math.max(1, Math.round(img.width * k)), height = Math.max(1, Math.round(img.height * k));
  const data = new Uint8ClampedArray(width * height * 4);
  const sx = img.width / width, sy = img.height / height;
  for (let y = 0; y < height; y++) {
    const y0 = Math.floor(y * sy), y1 = Math.max(y0 + 1, Math.floor((y + 1) * sy));
    for (let x = 0; x < width; x++) {
      const x0 = Math.floor(x * sx), x1 = Math.max(x0 + 1, Math.floor((x + 1) * sx));
      let r = 0, g = 0, b = 0;
      for (let yy = y0; yy < y1; yy++) {
        for (let xx = x0; xx < x1; xx++) {
          const i = (yy * img.width + xx) * 4;
          r += img.data[i];
          g += img.data[i + 1];
          b += img.data[i + 2];
        }
      }
      const n = (x1 - x0) * (y1 - y0), o = (y * width + x) * 4;
      data[o] = r / n;
      data[o + 1] = g / n;
      data[o + 2] = b / n;
      data[o + 3] = 255;
    }
  }
  return { width, height, data };
}

/** How large the game is in a screen read: 1 is the size the reader was tuned on. */
function scaleOf(screen) {
  return screen.anchor?.s ?? screen.banner?.s ?? null;
}

/** The value two reads agree on: either alone if the other did not read, none if they differ. */
function agree(a, b) {
  if (a === null || a === undefined) return b ?? null;
  if (b === null || b === undefined) return a;
  return a === b ? a : null;
}

/**
 * Read a screenshot: { kind, ... } with kind
 *  'board'    an attempt in progress (or finished: every slot filled) -
 *             `screen`, the slots as vision.js read them (boardState turns
 *             them into a state once the level is known), and `tier`, the
 *             rate on the Attempt buttons (null if it would not read)
 *  'legacy'   Hero's Legacy: `memory` { glory, despair } and `stock`
 *  'compare'  the results screen: `current` and `fresh` (null: unreadable)
 *  'levels'   the main page's levels view: `level`, `pity` (0..1), `levelUp`
 *  'counts'   its counts view: `states` (12 [glory, despair], null where
 *             one would not read), `ok` (they add up to the totals line),
 *             `totals`, `stock` (12, null where a badge would not read)
 *  'unknown'  none of these
 * and `marks`: where it read what, for showing over the screenshot.
 */
export function readScreenshot(img) {
  let screen = classify(img, null, true);
  let k = 1;
  if (screen.kind === 'unknown') {
    // a Compare popup bigger than the reader looks for, say: try it smaller
    for (const f of [1 / 2, 1 / 3]) {
      if (img.width * f < 300) break;
      const small = classify(shrink(img, f), null, true);
      if (small.kind !== 'unknown') {
        [screen, k] = [small, f];
        break;
      }
    }
  }
  const s = scaleOf(screen);
  let check = null;
  if (s && s > 1.4) {
    const again = classify(shrink(img, k / s), null, true);
    if (again.kind === screen.kind && again.view === screen.view) check = again;
  }
  return { ...contents(screen, check), marks: marks(screen, 1 / k) };
}

function contents(screen, check) {
  const memoryOf = (m) => ({ glory: m.glory.success, despair: m.despair.success });
  switch (screen.kind) {
    case 'board':
      return {
        kind: 'board',
        screen: { glory: screen.glory, despair: screen.despair, leaves: screen.leaves, fill: screen.fill, present: screen.present },
        tier: agree(boardRate(screen)?.tier, check && boardRate(check)?.tier),
      };
    case 'complete':
      // the bars of a finished attempt: what a board with every slot filled shows
      return { kind: 'complete', memory: memoryOf(screen.memory), slots: screen.memory.glory.slots, full: screen.memory.glory.filled === screen.memory.glory.slots && screen.memory.despair.filled === screen.memory.despair.slots };
    case 'legacy':
      return { kind: 'legacy', memory: memoryOf(screen.memory), slots: screen.memory.glory.slots, stock: agree(screen.stock, check?.stock) };
    case 'compare': {
      const cards = screen.cards.current && screen.cards.fresh ? screen.cards : check?.cards ?? screen.cards;
      return { kind: 'compare', current: cards.current, fresh: cards.fresh };
    }
    case 'main':
      if (screen.view === 'levels') return { kind: 'levels', level: agree(screen.level, check?.level), pity: screen.pity, levelUp: screen.levelUp };
      return counts(screen, check);
    default:
      return { kind: 'unknown' };
  }
}

/** The counts view: the read whose counts add up, and each badge as both reads agree. */
function counts(screen, check) {
  const best = screen.ok || !check?.ok ? screen : check;
  return {
    kind: 'counts',
    states: best.relics.map((r) => (r.glory !== null && r.despair !== null ? [r.glory, r.despair] : null)),
    ok: best.ok,
    totals: best.totals,
    stock: screen.relics.map((r, i) => agree(r.stock, check?.relics[i].stock)),
    levelUp: screen.levelUp,
  };
}

const COLOURS = { success: '#7cfc00', fail: '#ff4040', empty: '#6cb6ff', none: '#666' };

/**
 * Where the screen was read, in the screenshot's own pixels (a read made
 * on a copy shrunk to 1/`back` scaled back): { crop: [x0, y0, x1, y1] the
 * game, dots: [{ x, y, r, colour }] }.
 */
function marks(screen, back) {
  const dots = [];
  const dot = ([x, y], colour, r = 3) => dots.push({ x: x * back, y: y * back, r: r * back, colour });
  const a = screen.anchor;
  let crop = null;
  if (screen.kind === 'main') {
    const b = screen.banner;
    crop = [b.x0 - 140 * b.s, b.y0 - 150 * b.s, b.x0 + 330 * b.s, b.y0 + 720 * b.s];
    screen.tiles.forEach((t, i) => {
      const r = screen.relics?.[i];
      dot([t.x, t.y], r && r.glory !== null && r.despair !== null ? COLOURS.success : screen.relics ? COLOURS.fail : COLOURS.empty, 10 * b.s);
    });
  } else if (a) {
    crop = [...at(a, -15, -150), ...at(a, 415, 345)];
    const bars = screen.kind === 'board' ? [[screen.glory, REF.gloryRow], [screen.despair, REF.despairRow]]
      : screen.memory ? [[screen.memory.glory.bar, REF.gloryRow], [screen.memory.despair.bar, REF.despairRow]] : [];
    for (const [cells, row] of bars) cells.forEach((v, i) => dot(at(a, REF.slotX0 + REF.slotStep * i, row), COLOURS[v], 3 * a.s));
    if (screen.kind === 'board') {
      for (let i = 0; i < screen.leaves.count; i++) dot(at(a, REF.slotX0 + REF.slotStep * i, REF.leafRow), COLOURS.success, 3 * a.s);
      dot([at(a, REF.barX[0] + screen.fill * (REF.barX[1] - REF.barX[0]), 0)[0], at(a, 0, REF.barRows[1])[1]], '#fff', 4 * a.s);
      for (const [name, shown] of Object.entries(screen.present)) if (shown) dot([screen.buttons[name].x, screen.buttons[name].y], '#ffd700', 12 * a.s);
    }
    if (screen.button) dot([screen.button.x, screen.button.y], '#ffd700', 12 * a.s);
    if (screen.buttons?.keep) for (const b of [screen.buttons.keep, screen.buttons.replace]) dot([b.x, b.y], '#ffd700', 12 * a.s);
  }
  return { crop: crop && crop.map((v) => v * back), dots };
}
