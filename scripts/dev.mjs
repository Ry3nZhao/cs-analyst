#!/usr/bin/env node
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { msvcEnv } from './lib/msvc.mjs';
import { root } from './lib/paths.mjs';
import { resolveCargoCommand } from './lib/run.mjs';

const node = process.execPath;
const pnpm = path.join(path.dirname(node), process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm');
const tauriAppDir = path.join(root, 'crates', 'tauri-app');

function run(cmd, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: 'inherit', shell: process.platform === 'win32' });
    child.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`))));
  });
}

async function main() {
  await run(pnpm, ['--filter', '@cs-demo-analyst/shared-types', 'build'], root);
  await run(pnpm, ['--filter', '@cs-demo-analyst/sidecar', 'build'], root);

  const cargo = resolveCargoCommand();
  const tauri = spawn(cargo, ['tauri', 'dev'], {
    cwd: tauriAppDir,
    stdio: 'inherit',
    shell: process.platform === 'win32',
    env: { ...process.env, ...msvcEnv() },
  });

  const cleanup = () => {
    tauri.kill();
    process.exit(0);
  };
  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);

  await new Promise((resolve, reject) => {
    tauri.on('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`cargo tauri dev exited ${code}`)),
    );
  });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
