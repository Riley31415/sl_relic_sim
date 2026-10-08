// node test/run.mjs - every *.test.mjs in this folder: the run loop and the
// Smart Leveler, on the advisor's library as the extension has it (copied in
// first); the library's own tests are in ../advisor/js/test
import { readdirSync } from 'fs';
import { sync } from '../sync.mjs';
import { run } from '../../advisor/js/test/harness.mjs';

sync();
const here = new URL('./', import.meta.url);
for (const file of readdirSync(here).filter((f) => f.endsWith('.test.mjs')).sort()) {
  await import(new URL(file, here));
}
await run();
