import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { root } from './paths.mjs';

function commandHint(command, error) {
  if (error?.code !== 'ENOENT') {
    return null;
  }
  const base = path.basename(command).replace(/\.exe$/i, '');
  if (base === 'cargo') {
    return 'Install Rust from https://rustup.rs/ (curl --proto \'=https\' --tlsv1.2 -sSf https://sh.rustup.rs | sh), then restart the terminal or add ~/.cargo/bin to PATH. You can also set CARGO=/path/to/cargo.';
  }
  return null;
}

export function resolveCargoCommand() {
  if (process.env.CARGO) {
    return process.env.CARGO;
  }
  const cargoName = process.platform === 'win32' ? 'cargo.exe' : 'cargo';
  const homeCargo = path.join(os.homedir(), '.cargo', 'bin', cargoName);
  if (fs.existsSync(homeCargo)) {
    return homeCargo;
  }
  return 'cargo';
}

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
  if (result.error || result.status !== 0) {
    const hint = commandHint(command, result.error);
    if (hint) {
      throw new Error(hint);
    }
    const details = [];
    if (result.error) {
      details.push(result.error.message);
    }
    if (result.signal) {
      details.push(`signal ${result.signal}`);
    }
    const suffix = details.length ? ` (${details.join(', ')})` : '';
    throw new Error(
      `${command} ${args.join(' ')} failed with exit code ${result.status ?? 'unknown'}${suffix}`,
    );
  }
}

export function runPnpm(args, options = {}) {
  run('pnpm', args, options);
}

export function runCargo(args, options = {}) {
  run(resolveCargoCommand(), args, {
    ...options,
    cwd: options.cwd ?? path.join(root, 'crates', 'tauri-app', 'src-tauri'),
  });
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
