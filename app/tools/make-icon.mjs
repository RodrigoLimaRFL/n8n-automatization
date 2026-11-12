// Gera app/assets/icon.ico (envelope com selo verde de confirmação) sem dependências.
// Uso: node app/tools/make-icon.mjs
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const SIZES = [16, 32];
const OUTLINE = [0x3a, 0x4a, 0x5c];
const PAPER = [0xfa, 0xfa, 0xf4];
const GREEN = [0x1f, 0x9d, 0x3d];
const WHITE = [0xff, 0xff, 0xff];

function segDist(px, py, ax, ay, bx, by) {
  const dx = bx - ax, dy = by - ay;
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)));
  return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
}

// Cor (ou null = transparente) no ponto (x, y) do quadrado unitário.
function sample(x, y) {
  // selo verde (canto inferior direito)
  const bd = Math.hypot(x - 0.7, y - 0.7);
  if (bd <= 0.3) {
    if (bd > 0.25) return WHITE;
    const c = Math.min(
      segDist(x, y, 0.57, 0.7, 0.67, 0.8),
      segDist(x, y, 0.67, 0.8, 0.85, 0.58));
    return c < 0.045 ? WHITE : GREEN;
  }
  // envelope
  const [x0, y0, x1, y1, t] = [0.04, 0.16, 0.96, 0.8, 0.07];
  if (x >= x0 && x <= x1 && y >= y0 && y <= y1) {
    if (x < x0 + t || x > x1 - t || y < y0 + t || y > y1 - t) return OUTLINE;
    const f = Math.min(
      segDist(x, y, x0, y0, 0.5, 0.52),
      segDist(x, y, x1, y0, 0.5, 0.52));
    return f < 0.035 ? OUTLINE : PAPER;
  }
  return null;
}

function render(size) {
  const SS = 4;
  const px = Buffer.alloc(size * size * 4); // BGRA, de cima para baixo
  for (let j = 0; j < size; j++) for (let i = 0; i < size; i++) {
    let r = 0, g = 0, b = 0, a = 0;
    for (let sj = 0; sj < SS; sj++) for (let si = 0; si < SS; si++) {
      const c = sample((i + (si + 0.5) / SS) / size, (j + (sj + 0.5) / SS) / size);
      if (c) { r += c[0]; g += c[1]; b += c[2]; a++; }
    }
    const o = (j * size + i) * 4;
    if (a) { px[o] = Math.round(b / a); px[o + 1] = Math.round(g / a); px[o + 2] = Math.round(r / a); }
    px[o + 3] = Math.round((a / (SS * SS)) * 255);
  }
  return px;
}

function bmpEntry(size) {
  const px = render(size);
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(size, 4);
  header.writeInt32LE(size * 2, 8); // altura dobrada (cor + máscara)
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);
  header.writeUInt32LE(size * size * 4, 20);
  const rows = [];
  for (let j = size - 1; j >= 0; j--) rows.push(px.subarray(j * size * 4, (j + 1) * size * 4));
  const maskRow = Math.ceil(size / 32) * 4;
  return Buffer.concat([header, ...rows, Buffer.alloc(maskRow * size)]);
}

const images = SIZES.map(bmpEntry);
const dir = Buffer.alloc(6 + 16 * SIZES.length);
dir.writeUInt16LE(1, 2); // tipo: ícone
dir.writeUInt16LE(SIZES.length, 4);
let offset = dir.length;
SIZES.forEach((s, k) => {
  const o = 6 + 16 * k;
  dir[o] = s; dir[o + 1] = s;
  dir.writeUInt16LE(1, o + 4);
  dir.writeUInt16LE(32, o + 6);
  dir.writeUInt32LE(images[k].length, o + 8);
  dir.writeUInt32LE(offset, o + 12);
  offset += images[k].length;
});

const out = join(dirname(fileURLToPath(import.meta.url)), '..', 'assets', 'icon.ico');
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, Buffer.concat([dir, ...images]));
console.log(`icon.ico: ${offset} bytes -> ${out}`);
