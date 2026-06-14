#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { root } from '../lib/paths.mjs';

const iconsDir = path.join(root, 'crates', 'tauri-app', 'src-tauri', 'icons');
const b64Path = path.join(iconsDir, 'icon.png.b64');
const pngPath = path.join(iconsDir, 'icon.png');

if (!fs.existsSync(b64Path)) {
  console.error('Missing icons/icon.png.b64');
  process.exit(1);
}

const b64 = fs.readFileSync(b64Path, 'utf8').trim();
fs.writeFileSync(pngPath, Buffer.from(b64, 'base64'));

const tauriApp = path.join(root, 'crates', 'tauri-app');
const result = spawnSync('pnpm', ['exec', 'tauri', 'icon', pngPath, '-o', 'src-tauri/icons'], {
  cwd: tauriApp,
  stdio: 'inherit',
  shell: true,
});

if (result.status !== 0) {
  fs.copyFileSync(pngPath, path.join(iconsDir, 'icon.ico'));
  console.warn('tauri icon generation failed, using png copy as icon.ico fallback');
}

console.log('Icons ready in', iconsDir);
