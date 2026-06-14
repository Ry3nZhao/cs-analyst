#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { buildReleaseDir, cargoReleaseBundleDir, ensureDir, root } from '../lib/paths.mjs';
import { writeStamp } from '../lib/run.mjs';

function detectPlatform(arg) {
  if (arg) return arg;
  if (process.platform === 'win32') return 'windows';
  if (process.platform === 'darwin') return 'macos';
  return 'linux';
}

const arg = process.argv[2];
const platform = ['windows', 'macos', 'linux'].includes(arg) ? arg : detectPlatform();

const bundleRoot = cargoReleaseBundleDir();
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const artifactsDir = process.env.BUILD_RELEASE_DIR
  ? path.resolve(process.env.BUILD_RELEASE_DIR)
  : buildReleaseDir;

ensureDir(artifactsDir);

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

function archLabel(targetPlatform) {
  if (targetPlatform === 'macos') {
    return process.arch === 'arm64' ? 'aarch64' : 'x64';
  }
  return 'x64';
}

const collected = [];

function collectOne(source, targetPlatform, suffix = '') {
  const ext = path.extname(source);
  const arch = archLabel(targetPlatform);
  const targetName = `CS-Demo-Analyst_${version}_${arch}${suffix}${ext}`;
  const targetPath = path.join(artifactsDir, targetName);
  fs.copyFileSync(source, targetPath);
  collected.push(targetPath);
  console.log(`Collected ${source} -> ${targetPath}`);
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

writeStamp(path.join(artifactsDir, 'collect.json'), {
  job: 'collect',
  status: 'ok',
  platform,
  artifacts: collected,
});

console.log(`Release collect output: ${artifactsDir}`);
