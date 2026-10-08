// node build.mjs - the advisor's build (the solver to WebAssembly), then its
// library and relic.wasm copied into chrome/
import { execFileSync } from 'child_process';
import { statSync } from 'fs';
import { fileURLToPath } from 'url';

import { sync } from './sync.mjs';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

execFileSync(process.execPath, [here('../advisor/build.mjs')], { stdio: 'inherit' });
sync();
console.log(`chrome/relic.wasm: ${statSync(here('./chrome/relic.wasm')).size} bytes, the advisor's library in chrome/lib/`);
