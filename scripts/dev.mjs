#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const node = process.execPath;
const pnpm = path.join(path.dirname(node), process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm');

function run(cmd, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: 'inherit', shell: process.platform === 'win32' });
    child.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`))));
  });
}

async function main() {
  await run(pnpm, ['--filter', '@cs-demo-analyst/shared-types', 'build'], root);
  await run(pnpm, ['--filter', '@cs-demo-analyst/sidecar', 'build'], root);

  const tauriDir = path.join(root, 'crates', 'tauri-app');
  const sidecar = spawn('cargo', ['run'], { cwd: tauriDir, stdio: 'inherit', shell: true });
  const desktop = spawn(pnpm, ['--filter', '@cs-demo-analyst/desktop', 'dev'], {
    cwd: root,
    stdio: 'inherit',
    shell: true,
  });

  const cleanup = () => {
    sidecar.kill();
    desktop.kill();
    process.exit(0);
  };
  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
