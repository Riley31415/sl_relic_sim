// The advisor's JavaScript library (../advisor/js), copied into chrome/ - an
// unpacked extension loads nothing from outside its own folder.  The copies
// are made, not kept: edit them in advisor/js.
import { copyFileSync, existsSync, readdirSync } from 'fs';
import { fileURLToPath } from 'url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

/** Copy every advisor module into chrome/lib/, and relic.wasm (once built) into chrome/. */
export function sync() {
  const from = here('../advisor/js/');
  for (const name of readdirSync(from).filter((f) => f.endsWith('.js'))) copyFileSync(from + name, here(`./chrome/lib/${name}`));
  if (existsSync(from + 'relic.wasm')) copyFileSync(from + 'relic.wasm', here('./chrome/relic.wasm'));
}
