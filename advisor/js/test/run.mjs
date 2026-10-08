// node test/run.mjs - every *.test.mjs here: the screen reader, the rules and the advice
// (the solver itself, as WebAssembly in Chrome: node test/browser.mjs)
import { readdirSync } from 'fs';
import { run } from './harness.mjs';

const here = new URL('./', import.meta.url);
for (const file of readdirSync(here).filter((f) => f.endsWith('.test.mjs')).sort()) {
  await import(new URL(file, here));
}
await run();
