// node extension/test/run.mjs - every *.test.mjs in this folder
import { readdirSync } from 'fs';
import { run } from './harness.mjs';

const here = new URL('./', import.meta.url);
for (const file of readdirSync(here).filter((f) => f.endsWith('.test.mjs')).sort()) {
  await import(new URL(file, here));
}
await run();
