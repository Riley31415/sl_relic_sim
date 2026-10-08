// node test/run.mjs - every *.test.mjs here, against dist/ (node build.mjs first)
import { spawnSync } from 'child_process';
import { existsSync, readdirSync } from 'fs';
import { fileURLToPath } from 'url';
import { run } from '../../advisor/js/test/harness.mjs';

if (!existsSync(fileURLToPath(new URL('../dist/relic.wasm', import.meta.url)))) {
  console.error('No dist/ - run node build.mjs first.');
  process.exit(1);
}
// the solver's WebAssembly uses reference types, behind a flag before Node 18
if (Number(process.versions.node.split('.')[0]) < 18 && !process.execArgv.includes('--experimental-wasm-reftypes')) {
  const child = spawnSync(process.execPath, ['--experimental-wasm-reftypes', ...process.argv.slice(1)], { stdio: 'inherit' });
  process.exit(child.status ?? 1);
}
const here = new URL('./', import.meta.url);
for (const file of readdirSync(here).filter((f) => f.endsWith('.test.mjs')).sort()) {
  await import(new URL(file, here));
}
await run();
