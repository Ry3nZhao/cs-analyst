#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import zlib from 'node:zlib';
import fs from 'node:fs';
import path from 'node:path';
import { root } from '../lib/paths.mjs';

const iconsDir = path.join(root, 'crates', 'tauri-app', 'src-tauri', 'icons');
const b64Path = path.join(iconsDir, 'icon.png.b64');
const pngPath = path.join(iconsDir, 'icon.png');
const tauriApp = path.join(root, 'crates', 'tauri-app');

const REQUIRED_BUNDLE_ICONS = [
  '32x32.png',
  '128x128.png',
  '128x128@2x.png',
  'icon.icns',
  'icon.ico',
];

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function hasPngSignature(buffer) {
  return buffer.length >= PNG_SIGNATURE.length && buffer.subarray(0, PNG_SIGNATURE.length).equals(PNG_SIGNATURE);
}

function createDefaultSourcePng() {
  const crcTable = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = (c & 1) ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1);
    }
    crcTable[n] = c >>> 0;
  }

  function crc32(buf, init = 0xffffffff) {
    let crc = init;
    for (let i = 0; i < buf.length; i++) {
      crc = crcTable[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
    }
    return (crc ^ 0xffffffff) >>> 0;
  }

  function chunk(type, data) {
    const typeBuf = Buffer.from(type, 'ascii');
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const crcBuf = Buffer.alloc(4);
    crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])));
    return Buffer.concat([len, typeBuf, data, crcBuf]);
  }

  const width = 32;
  const height = 32;
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const raw = Buffer.alloc(height * (1 + width * 4));
  for (let y = 0; y < height; y++) {
    const row = y * (1 + width * 4);
    raw[row] = 0;
    for (let x = 0; x < width; x++) {
      const i = row + 1 + x * 4;
      raw[i] = 45;
      raw[i + 1] = 125;
      raw[i + 2] = 210;
      raw[i + 3] = 255;
    }
  }

  const idat = zlib.deflateSync(raw, { level: 9 });
  return Buffer.concat([
    PNG_SIGNATURE,
    chunk('IHDR', ihdr),
    chunk('IDAT', idat),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function readSourcePng() {
  if (fs.existsSync(pngPath)) {
    const existing = fs.readFileSync(pngPath);
    if (hasPngSignature(existing)) {
      return existing;
    }
    console.warn('icons/icon.png has invalid PNG signature, regenerating from source');
  }

  if (fs.existsSync(b64Path)) {
    const b64 = fs.readFileSync(b64Path, 'utf8').replace(/\s+/g, '');
    if (b64.length > 0) {
      const decoded = Buffer.from(b64, 'base64');
      if (hasPngSignature(decoded)) {
        return decoded;
      }
      console.warn('icons/icon.png.b64 does not decode to a valid PNG, using built-in default');
    }
  } else {
    console.warn('Missing icons/icon.png.b64, using built-in default source PNG');
  }

  return createDefaultSourcePng();
}

function bundleIconsReady() {
  return REQUIRED_BUNDLE_ICONS.every((name) => fs.existsSync(path.join(iconsDir, name)));
}

const sourcePng = readSourcePng();
fs.writeFileSync(pngPath, sourcePng);

if (bundleIconsReady()) {
  console.log('Bundle icons already present, skipping tauri icon generation');
  console.log('Icons ready in', iconsDir);
  process.exit(0);
}

const result = spawnSync('pnpm', ['exec', 'tauri', 'icon', pngPath, '-o', 'src-tauri/icons'], {
  cwd: tauriApp,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

if (result.status !== 0) {
  const details = [];
  if (result.error) {
    details.push(result.error.message);
  }
  if (result.signal) {
    details.push(`signal ${result.signal}`);
  }
  console.error('tauri icon generation failed', details.length ? `(${details.join(', ')})` : '');
  process.exit(result.status ?? 1);
}

for (const name of REQUIRED_BUNDLE_ICONS) {
  const filePath = path.join(iconsDir, name);
  if (!fs.existsSync(filePath)) {
    console.error(`tauri icon finished but missing required bundle icon: ${name}`);
    process.exit(1);
  }
}

console.log('Icons ready in', iconsDir);
