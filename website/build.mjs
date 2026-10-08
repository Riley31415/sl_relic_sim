// node build.mjs - compile the solver to WebAssembly and put the site together in dist/
import { execFileSync } from 'child_process';
import { copyFileSync, mkdirSync, rmSync, writeFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

import { files } from './files.mjs';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

// the advisor's build: cargo, then advisor/js/relic.wasm
execFileSync(process.execPath, [here('../advisor/build.mjs')], { stdio: 'inherit' });

const dist = here('./dist/');
rmSync(dist, { recursive: true, force: true });
for (const [published, source] of files()) {
  const to = join(dist, published);
  mkdirSync(dirname(to), { recursive: true });
  copyFileSync(source, to);
}
// GitHub Pages: serve the files as they are (no Jekyll)
writeFileSync(join(dist, '.nojekyll'), '');
console.log(`dist/: ${files().size} files`);
