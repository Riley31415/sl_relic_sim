// node build.mjs - compile the engine's solver and plan to WebAssembly (wasm/)
// and put it beside the JavaScript library that runs it: js/relic.wasm
import { execFileSync } from 'child_process';
import { copyFileSync, statSync } from 'fs';
import { fileURLToPath } from 'url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

execFileSync('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown'], {
  cwd: here('./wasm/'),
  stdio: 'inherit',
});
copyFileSync(here('./wasm/target/wasm32-unknown-unknown/release/relic_wasm.wasm'), here('./js/relic.wasm'));
console.log(`js/relic.wasm: ${statSync(here('./js/relic.wasm')).size} bytes`);
