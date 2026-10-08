# Relic Advisor (website)

A page for GitHub Pages that gives the extension's advice without touching
the game. Add a screenshot and it shows the game's screen redrawn, with an
arrow on the button to press. You press it in the game and tell the page how
it went with the Success / Fail buttons beside that button (or tap the button
itself, for any other step), and it moves on to the next step. Beside the screen it shows why: every move the board allows, the
chance it succeeds, what each way it can go leads to, and every result the
attempt can end on.

It runs the advisor's library ([advisor/js](../advisor/js), the same one the
extension runs): the screen reader (`vision.js`, `digits.js`,
`mainpage.js`), the rules (`logic.js`), the advice (`advice.js`) and the
exact solver and Look Ahead plan compiled to WebAssembly
(`relic.wasm`). It covers each option on a board (every move and
Abandon Inheritance) and where it leads, which one is advised, the keep
rule and its reasons. Nothing is
copied into this folder. `files.mjs` lists what the site publishes and where each
file comes from. Screenshots are read in the browser and never uploaded.

## Build, run, test

```
cd website
node build.mjs          # the advisor's build (cargo -> relic.wasm), then the site in dist/
node serve.mjs          # http://localhost:8080, straight from the sources
node test/run.mjs       # session and screenshot tests, against dist/
```

You need the same tools as the extension: [Rust](https://rustup.rs) with
`rustup target add wasm32-unknown-unknown`, and Node.js. The tests need
`node build.mjs` first. On Node 16 they restart themselves with
`--experimental-wasm-reftypes`, which the WebAssembly needs there.

## Publishing

No GitHub Actions and no separate branch: the site is built and tested on
your machine, and published from `docs/` at the top of the repo, on `main`.

```
node publish.mjs          # build, run the tests, and copy dist/ to ../docs
git add docs && git commit -m "Publish the website" && git push
```

Turn it on once under the repository's **Settings -> Pages -> Build and
deployment: Deploy from a branch**, `main`, `/docs`. `docs/` is generated:
edit `website/src` and `advisor/js`, then publish again.

## Use

1. **Settings**: *Single Relic* (the default) plays the relic that's open for
   the strategy you set, the same three strategies as the extension. *Smart
   Leveler* follows the Look Ahead plan (COST.md) across all twelve relics
   toward the Final Goal. Adding a screenshot of the main page's counts view
   switches to it.
2. **Add screenshots.** Paste one with Ctrl+V, drop it on the page, or choose
   files in the box under Settings. The page reads:
   - the main page's **levels view**: inheritor level, pity bar, Level Up lit or not
   - its **counts view**: every relic's glory and despair, checked against
     the Total line, and each stock badge
   - a relic's **Hero's Legacy**: its memory and stock (`127/10`)
   - an **attempt in progress**: every slot, the leaves, spirit power and
     the rate on the Attempt buttons
   - **Complete Inheritance**, and the **results** screen's two cards
3. **Follow the arrow.** On a move, tap **Success** or **Fail** beside the
   Attempt button you pressed. On a wide screen they sit past the game's
   right edge; on a narrow one, under the button. Hovering one shows the
   rate after it and the next move, so you can check the game agrees. For
   anything else (Inheritance, a relic tile, Complete Inheritance, Keep /
   Replace, Abandon, back, Level Up), tap that button on the drawn screen.
   There is no advice text: the arrow is the advice, and the line under the
   upload box only says what the screen can't (a number to check, a
   question, a stop such as Summon). **Undo**, **Redo** and **Clear data**
   are at the foot of Settings; hover the screenshot's name to see what was
   read on it.
4. **Out of step?** Add a screenshot of whatever the game shows. The page
   picks up from that screen: a board mid-attempt, the results, Hero's
   Legacy after a Keep, and so on.
5. **Minimum Useful Amplification, left blank**, is 0. Like any mark at or
   under the memory in place, it is raised to beat that memory, since only a
   better result is kept. With no memory (or one at +0%) it plays for the
   most amplification, expected. The extension does the same.
6. **Every number on the drawn screen is a box** you can edit: the level,
   pity, each relic's glory, despair and stock, a memory, the stock on hand,
   spirit power, mental strength, each bar's successes and fails, the rate,
   and both results cards. They start at 0 (level 1) until something is read
   or typed. A number a screenshot didn't read keeps its old value and turns
   orange until it's checked. An attempt screenshot that doesn't fit the
   level set is read again as soon as the level box is fixed.

The page keeps the board between visits (in this browser's storage) and
updates it as you go: a kept result becomes the relic's memory, an attempt
spends 10 of the relic and adds one attempt's pity, and Level Up empties the
pity bar. The advice itself is worked out from that board each time, as in
the extension. A new screenshot of the main page always resets the board to
what the game shows.

## How it decides

Every decision is the extension's:

- **which relic, and how to play it** (Smart Leveler): `Game::advise`, the
  plan's next step from the board alone. That covers level up, summon,
  convert, done, or a roll with the objective and the bar it's rolled toward.
- **each move**: the exact solver's best move for that objective, played
  against the memory in place.
- **keep or replace**: the plan's keep rule (`keeps_roll`), or in Single Relic
  mode the strategy's (`chooseMemory`).
- **abandon**: a wiped attempt, or one where no result still reachable would
  be kept.

The "why" numbers come from three solver exports added for the site:

- `move_value`: what each move is worth: the objective's score and the
  tie-breaks the solver ranks moves by, in its order.
- `chance`: each move's success chance, the level's bonuses included.
- `outcomes`: every result the attempt can end on if you press a given move
  now and play on as advised.

A move's score is the chance-weighted mix of where its success and its
failure lead, and the page shows that sum.

## Screenshots at phone size

The reader was tuned on cloud-phone captures at about 1x, and a phone's own
screenshot is 2-3x that. Anything found above 1.4x is read again shrunk to
about 1x, and a number the two reads disagree on is left blank for you to
fill in, never guessed. This is tested from 1x to 3.3x on every screen in
`extension/test/images`, including a stock badge that misreads alone at 2.5x.

## Files

| File | What it is |
| --- | --- |
| `src/session.js` | The state machine: every input makes a new session, and `advise` works out the next press and its reasons (with the shared `advice.js`). No DOM, so it's tested in Node. |
| `src/screenshot.js` | A screenshot in, a reading out (the screen and what's on it), with the second read at ~1x. |
| `src/view.js` | The game's screens, drawn: main page, Hero's Legacy, the attempt, the results. |
| `src/why.js` | The reasons panel: the move table, each move's two branches and results, the keep rule. |
| `src/app.js` | The page: inputs, screenshots, corrections, settings, undo and storage. |
