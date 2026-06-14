#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const platform = process.argv[2];
if (!['windows', 'macos', 'linux'].includes(platform)) {
  console.error('Usage: collect-bundle.mjs <windows|macos|linux>');
  process.exit(1);
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bundleRoot = path.join(root, 'crates', 'tauri-app', 'src-tauri', 'target', 'release', 'bundle');
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const artifactsDir = path.join(root, 'artifacts');

function walk(dir) {
  if (!fs.existsSync(dir)) return [];
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

function archLabel(platform) {
  if (platform === 'macos') {
    return process.arch === 'arm64' ? 'aarch64' : 'x64';
  }
  return 'x64';
}

function collectOne(source, platform, suffix = '') {
  const ext = path.extname(source);
  const arch = archLabel(platform);
  const targetName = `CS-Demo-Analyst_${version}_${arch}${suffix}${ext}`;
  fs.mkdirSync(artifactsDir, { recursive: true });
  const targetPath = path.join(artifactsDir, targetName);
  fs.copyFileSync(source, targetPath);
  console.log(`Collected ${source} -> ${targetPath}`);
  return targetPath;
}

const files = walk(bundleRoot);
const lower = (p) => p.toLowerCase();

if (platform === 'windows') {
  const source =
    files.find((f) => lower(f).includes('nsis') && lower(f).endsWith('.exe')) ??
    files.find((f) => /setup\.exe$/i.test(f)) ??
    files.find((f) => lower(f).endsWith('.exe'));
  if (!source) {
    console.error(`No bundle artifact found for ${platform} under ${bundleRoot}`);
    console.error('Files seen:', files);
    process.exit(1);
  }
  collectOne(source, platform, '-setup');
} else if (platform === 'macos') {
  const source = files.find((f) => lower(f).endsWith('.dmg'));
  if (!source) {
    console.error(`No bundle artifact found for ${platform} under ${bundleRoot}`);
    console.error('Files seen:', files);
    process.exit(1);
  }
  collectOne(source, platform);
} else {
  const linuxFormats = [
    { ext: '.appimage', suffix: '' },
    { ext: '.deb', suffix: '' },
    { ext: '.rpm', suffix: '' },
  ];
  let count = 0;
  for (const { ext, suffix } of linuxFormats) {
    const source = files.find((f) => lower(f).endsWith(ext));
    if (source) {
      collectOne(source, platform, suffix);
      count += 1;
    }
  }
  if (count === 0) {
    console.error(`No Linux bundle artifact found under ${bundleRoot}`);
    console.error('Files seen:', files);
    process.exit(1);
  }
}
