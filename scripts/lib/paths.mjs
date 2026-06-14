import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptsDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const root = path.resolve(scriptsDir, '..');

export const buildRoot = path.join(root, 'build');
export const buildValidateDir = path.join(buildRoot, 'validate');
export const buildReleaseDir = path.join(buildRoot, 'release');
export const buildValidateDesktopDir = path.join(buildValidateDir, 'desktop');

export const tauriDir = path.join(root, 'crates', 'tauri-app', 'src-tauri');
export const sidecarResourceIndex = path.join(
  root,
  'crates',
  'tauri-app',
  'src-tauri',
  'resources',
  'sidecar',
  'index.js',
);

/** Respects CARGO_TARGET_DIR when set (e.g. short path on Windows CI). */
export function cargoTargetDir() {
  if (process.env.CARGO_TARGET_DIR) {
    return path.resolve(process.env.CARGO_TARGET_DIR);
  }
  return path.join(tauriDir, 'target');
}

export function cargoReleaseBundleDir() {
  return path.join(cargoTargetDir(), 'release', 'bundle');
}

export function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}
