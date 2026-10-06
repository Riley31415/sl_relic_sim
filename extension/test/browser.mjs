// node test/browser.mjs [path-to-chrome] - test/browser.html in headless Chrome
import { createServer } from 'http';
import { readFile } from 'fs/promises';
import { execFile } from 'child_process';
import { fileURLToPath } from 'url';
import { extname, join, normalize } from 'path';

const root = fileURLToPath(new URL('../', import.meta.url));
const chrome = process.argv[2] || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.png': 'image/png' };

const server = createServer(async (req, res) => {
  const path = normalize(join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname)));
  if (!path.startsWith(normalize(root))) return res.writeHead(403).end();
  try {
    const body = await readFile(path);
    res.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream' }).end(body);
  } catch {
    res.writeHead(404).end();
  }
});

// headless Chrome can dump the page before the solving finishes (the page
// still says "running"): that is the harness, not the code - try again
const TRIES = 6;

server.listen(0, '127.0.0.1', () => {
  const url = `http://127.0.0.1:${server.address().port}/test/browser.html`;
  const attempt = (n) => execFile(chrome, ['--headless=new', '--disable-gpu', '--virtual-time-budget=60000', '--dump-dom', url], (err, stdout) => {
    const line = /RESULT (.*)<\/pre>/.exec(stdout);
    if (!err && !line && /running<\/pre>/.test(stdout) && n < TRIES) return attempt(n + 1);
    server.close();
    if (err || !line) {
      console.error(err || stdout);
      process.exitCode = 1;
      return;
    }
    const result = JSON.parse(line[1].replace(/&quot;/g, '"').replace(/&amp;/g, '&'));
    console.log(JSON.stringify(result, null, 2));
    const advice = result.advice || {};
    const ok = !result.error && result.kind === 'board'
      && JSON.stringify(result.state) === JSON.stringify({ gf: 2, gs: 2, df: 1, ds: 0, ms: 8, sp: 9 })
      && advice.step === 'quest' && advice.relic === 11 && advice.goal === 19
      && JSON.stringify(advice.objective) === JSON.stringify({ kind: 'target', glory: 8, despair: 2 })
      && JSON.stringify(result.keeps) === JSON.stringify([true, true, false])
      && JSON.stringify(result.pity) === JSON.stringify([9000, 180])
      && ['glory', 'despair', 'train'].includes(result.adviceMove)
      && result.firstAdvice?.step === 'quest' && result.firstAdvice.objective?.kind === 'target'
      && result.settled?.odds === 0 && result.settled.move === 'train' && result.settled.ampMove === 'train' && result.open === 'train';
    console.log(ok ? 'browser smoke test passed' : 'browser smoke test FAILED');
    if (!ok) process.exitCode = 1;
  });
  attempt(1);
});
