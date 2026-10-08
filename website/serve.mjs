// node serve.mjs [port] - the site on http://localhost:8080, straight from
// its sources (reload to see an edit; rerun ../advisor/build.mjs after
// changing the solver)
import { createServer } from 'http';
import { readFile } from 'fs/promises';
import { extname } from 'path';

import { files } from './files.mjs';

const TYPES = {
  '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.png': 'image/png', '.svg': 'image/svg+xml',
};
const port = Number(process.argv[2]) || 8080;

createServer(async (req, res) => {
  const path = decodeURIComponent(new URL(req.url, 'http://x').pathname).replace(/^\/+/, '') || 'index.html';
  const source = files().get(path);
  if (!source) return res.writeHead(404).end();
  try {
    res.writeHead(200, { 'content-type': TYPES[extname(path)] || 'application/octet-stream', 'cache-control': 'no-store' }).end(await readFile(source));
  } catch {
    res.writeHead(404).end();
  }
}).listen(port, '127.0.0.1', () => console.log(`http://localhost:${port}/`));
