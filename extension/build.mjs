// node build.mjs - compile the solver to WebAssembly and drop it into chrome/
import { execFileSync } from 'child_process';
import { copyFileSync, statSync } from 'fs';
import { fileURLToPath } from 'url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

execFileSync('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown'], {
  cwd: here('./solver/'),
  stdio: 'inherit',
});
const out = here('./solver/target/wasm32-unknown-unknown/release/relic_wasm.wasm');
copyFileSync(out, here('./chrome/relic.wasm'));
console.log(`chrome/relic.wasm: ${statSync(here('./chrome/relic.wasm')).size} bytes`);
