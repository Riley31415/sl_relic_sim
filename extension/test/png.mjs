// Just enough PNG for the fixtures: 8-bit RGB or RGBA, not interlaced.
import { readFileSync } from 'fs';
import { inflateSync } from 'zlib';

export function readPng(path) {
  const buf = readFileSync(path);
  let pos = 8, width = 0, height = 0, channels = 0;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString('ascii', pos + 4, pos + 8);
    const body = buf.subarray(pos + 8, pos + 8 + len);
    if (type === 'IHDR') {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      const [depth, colour, , , interlace] = body.subarray(8);
      channels = { 2: 3, 6: 4 }[colour];
      if (depth !== 8 || !channels || interlace) throw new Error(`${path}: unsupported PNG`);
    } else if (type === 'IDAT') idat.push(body);
    else if (type === 'IEND') break;
    pos += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const px = Buffer.alloc(height * stride);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? px[y * stride + x - channels] : 0;
      const b = y > 0 ? px[(y - 1) * stride + x] : 0;
      const c = x >= channels && y > 0 ? px[(y - 1) * stride + x - channels] : 0;
      let v = line[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      px[y * stride + x] = v & 255;
    }
  }
  const data = new Uint8ClampedArray(width * height * 4);
  for (let i = 0; i < width * height; i++) {
    for (let k = 0; k < 3; k++) data[i * 4 + k] = px[i * channels + k];
    data[i * 4 + 3] = 255;
  }
  return { width, height, data };
}

/**
 * `img` pasted into a `width` x `height` frame of a dark page colour at
 * (`left`, `top`), resized by `scale` (bilinear, like a browser would).
 */
export function compose(img, { width, height, left, top, scale = 1 }) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let i = 0; i < width * height; i++) data.set([24, 24, 27, 255], i * 4);
  const w = Math.round(img.width * scale), h = Math.round(img.height * scale);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const sx = Math.min(img.width - 1.001, Math.max(0, (x + 0.5) / scale - 0.5));
      const sy = Math.min(img.height - 1.001, Math.max(0, (y + 0.5) / scale - 0.5));
      const x0 = Math.floor(sx), y0 = Math.floor(sy), fx = sx - x0, fy = sy - y0;
      const o = ((top + y) * width + left + x) * 4;
      for (let k = 0; k < 3; k++) {
        const p = (xx, yy) => img.data[(yy * img.width + xx) * 4 + k];
        data[o + k] = (p(x0, y0) * (1 - fx) + p(x0 + 1, y0) * fx) * (1 - fy) + (p(x0, y0 + 1) * (1 - fx) + p(x0 + 1, y0 + 1) * fx) * fy;
      }
    }
  }
  return { width, height, data };
}
