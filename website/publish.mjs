// node publish.mjs - build the site, test it, and put it in docs/ at the top of the repo, which GitHub
// Pages serves from main (Settings -> Pages -> Deploy from a branch: main, /docs).  Commit docs/ and push
// to put it live; nothing is committed here.
import { execFileSync } from 'child_process';
import { cpSync, rmSync } from 'fs';
import { fileURLToPath } from 'url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const node = (script) => execFileSync(process.execPath, [here(script)], { stdio: 'inherit' });

// the site as it would be published, and the tests it has to pass first
node('./build.mjs');
node('../advisor/js/test/run.mjs');
node('./test/run.mjs');

// dist/ as docs/, exactly: a file the site no longer has goes too
const docs = here('../docs');
rmSync(docs, { recursive: true, force: true });
cpSync(here('./dist'), docs, { recursive: true });
console.log('docs/: the site, ready to commit (git add docs) and push.');
