# How to use?

Have claude or codex help you set it up.

# Slayer Legend relic inheritance

Tools for Slayer Legend's relic inheritance (Memory Inheritance): an exact
solver for a single attempt, diamond-cost models for levelling the inheritor
and farming relics, and a Chrome extension that plays attempts for you.

| Directory | What it is |
| --- | --- |
| [engine/](engine/) | The `relic` crate and CLI. Solves an attempt exactly (every branch, weighted by probability), scores simple rules against the optimum, and runs the Monte Carlo behind the cost tables. Results are in [STRATEGY.md](engine/STRATEGY.md), [LEVELS.md](engine/LEVELS.md) and [COST.md](engine/COST.md). |
| [advisor/](advisor/) | `relic-advisor`: reads a screenshot of the Hero's Legacy grid and says what the look-ahead plan does next. See [advisor/README.md](advisor/README.md). |
| [extension/](extension/) | Relic Autoplayer, a Chrome extension that runs the solver as WebAssembly, reads the game from the tab and clicks through it. See [extension/README.md](extension/README.md). |

## Engine

```
cd engine
cargo install --path .     # puts `relic` on your PATH
relic solve --level 16     # exact solve, distribution and policy for one level
relic --help
```

## Setting up the extension

In order to use the extension, you must have access to Slayer Legend through a cloud service in the browser. I tested using RedFinger.

You need [Rust](https://rustup.rs) with the WebAssembly target, [Node.js](https://nodejs.org)
and Chrome.

1. Add the WebAssembly target (once):

   ```
   rustup target add wasm32-unknown-unknown
   ```

2. Build the solver into `extension/chrome/relic.wasm`, and optionally run the tests:

   ```
   cd extension
   node build.mjs
   node test/run.mjs         # unit tests
   node test/browser.mjs     # solver and screen reader in headless Chrome
   ```

3. Open `chrome://extensions`, turn on **Developer mode**, click **Load unpacked**
   and choose the `extension/chrome` folder.

4. Open the game tab, go to Memory Inheritance, and click the extension's toolbar
   button to open the side panel. Press **Read screen** to check it reads the game
   correctly, then **Start**.

Rerun `node build.mjs` and click the reload icon on the extension's card after
changing the solver. Modes, settings and when a run stops are covered in
[extension/README.md](extension/README.md).

## Disclaimer
Use at your own risk. I am not responsible if you get banned. 
Always monitor this tool, I am not responsible if it makes a bad decision (It shouldnt tho).
