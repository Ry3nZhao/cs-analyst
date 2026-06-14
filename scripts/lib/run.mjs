import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { root } from './paths.mjs';

export function run(command, args = [], options = {}) {
  const { cwd = root, env, label } = options;
  if (label) {
    console.log(`\n==> ${label}`);
  }
  const result = spawnSync(command, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with exit code ${result.status ?? 'unknown'}`);
  }
}

export function runPnpm(args, options = {}) {
  run('pnpm', args, options);
}

export function runCargo(args, options = {}) {
  run('cargo', args, { ...options, cwd: options.cwd ?? path.join(root, 'crates', 'tauri-app', 'src-tauri') });
}

export function writeStamp(stampPath, payload) {
  fs.mkdirSync(path.dirname(stampPath), { recursive: true });
  fs.writeFileSync(
    stampPath,
    `${JSON.stringify({ ...payload, finishedAt: new Date().toISOString() }, null, 2)}\n`,
    'utf8',
  );
}

export function copyOutputDir(sourceDir, targetDir) {
  if (!fs.existsSync(sourceDir)) {
    throw new Error(`Expected build output directory missing: ${sourceDir}`);
  }
  fs.rmSync(targetDir, { recursive: true, force: true });
  fs.cpSync(sourceDir, targetDir, { recursive: true });
}

export function assertFile(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Missing required file: ${filePath}`);
  }
}
