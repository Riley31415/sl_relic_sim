# How to use?

Have claude or codex help you set it up.

# Slayer Legend relic inheritance

Tools for Slayer Legend's relic inheritance (Memory Inheritance): an exact
solver for a single attempt, diamond-cost models for levelling the inheritor
and farming relics, a Chrome extension that plays attempts for you, and a
website that advises from screenshots while you play.

| Directory | What it is |
| --- | --- |
| [engine/](engine/) | The `relic` crate and CLI. Solves an attempt exactly (every branch, weighted by probability), scores simple rules against the optimum, and runs the Monte Carlo behind the cost tables. Results are in [STRATEGY.md](STRATEGY.md) and [COST.md](COST.md). |
| [advisor/](advisor/) | The library between the engine and the tools: the engine's solver and plan as WebAssembly, the advice built on them (which relic, which move, keep or replace, and why) and the screen reader, shared by the extension and the website; and `relic-advisor`, the same advice on the command line. See [advisor/README.md](advisor/README.md). |
| [extension/](extension/) | Relic Autoplayer, a Chrome extension that runs the advisor in the cloud-phone tab: reads the game from the tab and clicks through it. See [extension/README.md](extension/README.md). |
| [website/](website/) | Relic Advisor, a GitHub Pages site that runs the advisor with no clicking: add a screenshot, see the game redrawn with an arrow on the button to press, enter each result, and see why each move is best. See [website/README.md](website/README.md). |

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

2. Build the advisor (the solver as WebAssembly) and copy it, with its
   library, into `extension/chrome`, and optionally run the tests:

   ```
   cd extension
   node build.mjs
   node test/run.mjs                    # the run loop and the Smart Leveler
   node ../advisor/js/test/run.mjs      # the advisor: screen reader, rules, advice
   node ../advisor/js/test/browser.mjs  # solver and screen reader in headless Chrome
   ```

3. Open `chrome://extensions`, turn on **Developer mode**, click **Load unpacked**
   and choose the `extension/chrome` folder.

4. Open the game tab, go to Memory Inheritance, and click the extension's toolbar
   button to open the side panel. Press **Read screen** to check it reads the game
   correctly, then **Start**.

Rerun `node build.mjs` and click the reload icon on the extension's card after
changing the solver or anything in `advisor/js`. Modes, settings and when a run stops are covered in
[extension/README.md](extension/README.md).

## Disclaimer
Use at your own risk. I am not responsible if you get banned. 
Always monitor this tool, I am not responsible if it makes a bad decision (It shouldnt tho).
