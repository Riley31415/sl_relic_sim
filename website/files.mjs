// Every file the site publishes and where it comes from: the site's own
// (src/), and the advisor's library (../advisor/js: the solver as
// WebAssembly, the advice, the rules, the screen reader) - shared with the
// extension, so a fix there is a fix here.  build.mjs copies these into
// dist/; serve.mjs serves them straight from where they live.
import { readdirSync } from 'fs';
import { fileURLToPath } from 'url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

/** The advisor's modules the site runs, as lib/<name>. */
export const SHARED = ['advice.js', 'digits.js', 'glyphdata.js', 'logic.js', 'mainpage.js', 'solver.js', 'vision.js'];

/** The advisor's test captures the site shows as examples of each screen, as examples/<name>. */
export const EXAMPLES = ['main-counts.png', 'board-l19.png'];

/** Published path -> source file. */
export function files() {
  const out = new Map();
  for (const name of readdirSync(here('./src/'))) out.set(name, here(`./src/${name}`));
  for (const name of SHARED) out.set(`lib/${name}`, here(`../advisor/js/${name}`));
  for (const name of EXAMPLES) out.set(`examples/${name}`, here(`../advisor/js/test/images/${name}`));
  out.set('relic.wasm', here('../advisor/js/relic.wasm'));
  return out;
}
