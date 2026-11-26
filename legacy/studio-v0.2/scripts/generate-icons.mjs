#!/usr/bin/env node
// Sinh bộ icon chuẩn RGBA (32x32.png, 128x128.png, 128x128@2x.png, icon.ico cho Windows, icon.icns cho macOS Universal)
// bằng Node.js thuần (không phụ thuộc công cụ ngoài).
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ICONS_DIR = path.join(ROOT, "src-tauri", "icons");
fs.mkdirSync(ICONS_DIR, { recursive: true });

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return (c ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const t = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([t, data])), 0);
  return Buffer.concat([len, t, data, crc]);
}

function renderLogoPng(size) {
  const raw = Buffer.alloc(size * (1 + size * 4));
  const rCorner = size * 0.22;

  for (let y = 0; y < size; y++) {
    const rowOff = y * (1 + size * 4);
    raw[rowOff] = 0; // filter None
    for (let x = 0; x < size; x++) {
      const pxOff = rowOff + 1 + x * 4;
      // Bo góc rounded-rect
      const dx = Math.max(rCorner - x, 0, x - (size - 1 - rCorner));
      const dy = Math.max(rCorner - y, 0, y - (size - 1 - rCorner));
      const dist = Math.hypot(dx, dy);
      if (dist > rCorner + 0.6) {
        raw[pxOff] = 0;
        raw[pxOff + 1] = 0;
        raw[pxOff + 2] = 0;
        raw[pxOff + 3] = 0;
        continue;
      }

      // Nền chuyển sắc Cam -> Đỏ đậm + biểu tượng nút Play tam giác trắng ở giữa
      const t = (x + y) / (2 * size);
      let r = Math.round(249 * (1 - t) + 220 * t);
      let g = Math.round(115 * (1 - t) + 38 * t);
      let b = Math.round(22 * (1 - t) + 38 * t);

      const nx = (x / size) - 0.5;
      const ny = (y / size) - 0.5;
      if (nx >= -0.14 && nx <= 0.22 && Math.abs(ny) <= (0.22 - nx) * 0.62) {
        r = 255;
        g = 255;
        b = 255;
      }

      raw[pxOff] = r;
      raw[pxOff + 1] = g;
      raw[pxOff + 2] = b;
      raw[pxOff + 3] = 255;
    }
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // 8-bit
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", zlib.deflateSync(raw)),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

function makeIco(pngBuf, size = 256) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2); // ICO
  header.writeUInt16LE(1, 4); // 1 image

  const entry = Buffer.alloc(16);
  entry[0] = size >= 256 ? 0 : size;
  entry[1] = size >= 256 ? 0 : size;
  entry[2] = 0;
  entry[3] = 0;
  entry.writeUInt16LE(1, 4); // color planes
  entry.writeUInt16LE(32, 6); // 32 bpp
  entry.writeUInt32LE(pngBuf.length, 8);
  entry.writeUInt32LE(22, 12); // offset = 6 + 16

  return Buffer.concat([header, entry, pngBuf]);
}

function makeIcns(png128, png256) {
  const block = (ostype, data) => {
    const h = Buffer.alloc(8);
    h.write(ostype, 0, 4, "ascii");
    h.writeUInt32BE(8 + data.length, 4);
    return Buffer.concat([h, data]);
  };
  const b1 = block("ic07", png128);
  const b2 = block("ic08", png256);
  const total = 8 + b1.length + b2.length;
  const head = Buffer.alloc(8);
  head.write("icns", 0, 4, "ascii");
  head.writeUInt32BE(total, 4);
  return Buffer.concat([head, b1, b2]);
}

const p32 = renderLogoPng(32);
const p128 = renderLogoPng(128);
const p256 = renderLogoPng(256);

fs.writeFileSync(path.join(ICONS_DIR, "32x32.png"), p32);
fs.writeFileSync(path.join(ICONS_DIR, "128x128.png"), p128);
fs.writeFileSync(path.join(ICONS_DIR, "128x128@2x.png"), p256);
fs.writeFileSync(path.join(ICONS_DIR, "icon.ico"), makeIco(p256, 256));
fs.writeFileSync(path.join(ICONS_DIR, "icon.icns"), makeIcns(p128, p256));
console.log("✓ Đã sinh bộ icon Windows (.ico) + macOS (.icns) + PNG tại src-tauri/icons/");
