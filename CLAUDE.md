# CLAUDE.md

Guidelines for working in this repo: how it is layered, where each kind of
change belongs, and the rules the code has settled on.

## The layers

```
engine/     computes          the exact solver, the Look Ahead plan, the Monte Carlo behind the cost tables
advisor/    advises           the engine's results, turned into what to do from what a front end sees
extension/  plays (Chrome)    reads the cloud-phone tab and clicks
website/    shows (Pages)     reads screenshots, draws the game, the player clicks
```

Each layer only uses the ones above it. The front ends (extension, website)
never re-implement a decision: if both need it, or it is a decision at all, it
goes in `advisor/`. The front ends hold only what is specific to them.

## Where things get implemented

| Change | Goes in |
| --- | --- |
| Game rules, the solver's objectives, plans, costs | `engine/src/` (`solver.rs`; `levels/` for levelling and the plan) |
| Exposing an engine result to JavaScript | `advisor/wasm/src/lib.rs` (flat C ABI, numbers only), then its wrapper `advisor/js/solver.js` |
| Advice: the options on a board, which is advised, keep or replace, abandoning, the reasons | `advisor/js/advice.js` |
| Rules and words shared by both front ends: relic names, the rate ladder, strategy names and descriptions, the keep rule, level fitting | `advisor/js/logic.js` |
| Reading screenshots (any screen, any size) | `advisor/js/vision.js`, `mainpage.js`, `digits.js` (`glyphdata.js` is generated) |
| The command-line advisor | `advisor/src/` (Rust) |
| Chrome only: capture, clicking, the run loop, the Smart Leveler runner, the panel | `extension/chrome/` (`lib/runner.js`, `lib/advisor.js`, `lib/tab.js`, `panel.*`) |
| The site's state machine (inputs in, session out) | `website/src/session.js` (no DOM, tested in Node) |
| The site's screenshot pipeline (shrink, cross-check, marks) | `website/src/screenshot.js` |
| Drawing the game; the "why" panel; the page wiring | `website/src/view.js`, `why.js`, `app.js` |

`advisor/js` is the one copy of the shared library:

- The extension can only load files from inside its own folder, so
  `extension/build.mjs` (and `extension/test/run.mjs`) copy `advisor/js/*.js`
  into `extension/chrome/lib/` and `relic.wasm` into `extension/chrome/`.
  Those copies are gitignored. **Edit `advisor/js`, never the copies.**
- The website publishes `advisor/js` as `lib/`. `website/files.mjs` maps every
  published path to its source, and both `build.mjs` (to `dist/`) and
  `serve.mjs` (live) use it. Nothing is copied into `website/`.

## Rules for decisions

- **Every decision comes from the visible board.** The game shows everything:
  the board, the stock and the pity. No hidden state, no remembered
  escalation, no options that change a decision outside the board. A front end
  may keep what the board showed (the session, the attempt in progress) but
  never decides from anything else. One allowed snapshot: an attempt's keep
  rule is judged on the board as it stood when the attempt began, as the
  engine's Monte Carlo does.
- **Never trade diamond cost for simplicity.** If a stateful or simpler rule
  measures cheaper or easier, find what it reacts to on the board and express
  that. Measure over several seeds before claiming a win.
- **Don't copy the engine's rules into JavaScript.** Ask the engine through a
  wasm export. For example, what a score means comes from
  `solver.objective()`, so a score is never labelled as something it isn't.
  A copied rule drifts, and that's how expected amplification once showed up
  as a "2743% chance".
- **Same board, same advice.** Advice is worked out fresh each time it is
  shown (`advise(session, …)`), never stored.
- **Abandoning:** an abandoned attempt still earns its pity, and the 10 relics
  are spent when it starts either way. Abandon only when the attempt is wiped
  or nothing still reachable would be kept; it is offered only when advised.
- **Minimum Useful Amplification:** blank = 0, nothing more. Nothing fills
  the box in.
  - Like any mark the memory already reaches, a 0 is raised to beat the memory
    in place (`Strategy::above`), since only a better result is kept.
  - With no memory, or one at +0%, a 0 mark is no mark at all: the engine plays
    for the most expected amplification (`Strategy::above(0, None)` is
    `Strategy::default()`). It is never scored as the chance of +0% or more,
    which is only the chance of not wiping.
  - A 0 mark is never "met": it doesn't stop a run.
- **The inheritor level is read, not assumed.** The main page's levels view
  shows it. An attempt's board narrows it down (`levelsFitting`: slots, the
  spirit fill, the free first slots, an untouched opening). Keep the level set
  if it fits. Otherwise take the highest that fits and say so: the site marks
  it to check, the extension asks unless the levels play alike.
- **Never guess a number.** A number that reads ambiguously stays unread or
  keeps its old value, marked for the player to check. It is never filled in
  with a guess.

## Front-end conventions

- **Wording follows the extension's panel:** "Single Relic", "Smart Leveler",
  "Final Goal", the three strategy names, "Minimum Useful Amplification",
  "Glory Target", "Max useful Glory". The plan is the "Look Ahead plan". Shared
  wording lives in `logic.js` (`STRATEGY`, `strategySettings`,
  `describeObjective`).
- **Website:**
  - One centered column. Every number on the drawn game is an editable box with
    a default (0, level 1), never "?".
  - The page only advises; the player tells it what happened. Success/Fail sit
    beside each Attempt button, and other steps are a tap on the drawn button.
  - Hotkeys are Ctrl+Z (undo), Ctrl+Y (redo) and Ctrl+V (paste a screenshot),
    nothing else.
  - Edits apply as they are typed, so the advice and its arrow always match
    what is on screen.
  - The minimum and the advice re-check on load and after every input.
- **Extension:** clicks only what it recognises, halts rather than guesses,
  and asks the player (`ui.askTier`, `askLevel`, `askNumber`) when a reading
  cannot be settled.

## Code style

- Comments are plain prose about the game and the decision, saying what and
  why, and they read like the code around them. Match the density of the file
  you are in.
- JavaScript: ES modules, no dependencies, no bundler. Two spaces, single
  quotes, semicolons. Browser code stays free of Node APIs. Anything shared
  stays free of Chrome and DOM APIs too.
- Rust: `rustfmt` (`max_width = 110`). The wasm crate exports numbers only;
  exchange areas (`IO`, `OUT`) are read through fresh typed-array views after
  every call, because a call can grow the module's memory.

## Build and test

```
cd engine   && cargo test --release                  # the engine (slow parts are the Monte Carlo)
cd advisor  && cargo test --release                  # the command-line advisor
cd advisor  && node build.mjs                        # wasm/ -> js/relic.wasm (needs rustup target wasm32-unknown-unknown)
cd advisor/wasm && cargo test --release              # the wasm exports
cd advisor/js   && node test/run.mjs                 # the shared library: screen reader, rules, advice
cd advisor/js   && node test/browser.mjs             # the solver and screen reader in headless Chrome
cd extension && node build.mjs && node test/run.mjs  # copy the library in; the run loop and the Smart Leveler
cd website   && node build.mjs && node test/run.mjs  # dist/; the session and screenshots, on the real wasm
cd website   && node serve.mjs                       # http://localhost:8080, live from the sources
```

- Test against the real thing where it matters. The site's tests load the
  real `relic.wasm` and the captures in `advisor/js/test/images`. A new screen
  or a misread goes in as a capture there; `node advisor/js/test/make-glyphs.mjs`
  rebuilds the digit examples from them.
- Node 16: the wasm needs `--experimental-wasm-reftypes`. `website/test/run.mjs`
  restarts itself with it.
- No GitHub Actions. `node website/publish.mjs` builds, runs both test
  suites and copies `website/dist` to `docs/`, which Pages serves from `main`
  (Settings → Pages → Deploy from a branch: main, /docs). `docs/` is generated:
  never edit it, publish again; it goes live when committed and pushed.
- Reload the extension in `chrome://extensions` after rebuilding it.

## Git

- Do not commit or push unless asked.
- Move files with `git mv` so their history follows them.
