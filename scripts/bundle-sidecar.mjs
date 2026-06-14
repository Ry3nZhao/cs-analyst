#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function runPnpm(args, cwd = root) {
  const result = spawnSync('pnpm', args, { cwd, stdio: 'inherit', shell: true });
  if (result.status !== 0) {
    throw new Error(`pnpm ${args.join(' ')} failed`);
  }
}

const sidecarDist = path.join(root, 'packages', 'sidecar', 'dist');
const sharedDist = path.join(root, 'packages', 'shared-types', 'dist');
const targetDir = path.join(root, 'crates', 'tauri-app', 'src-tauri', 'resources', 'sidecar');

runPnpm(['--filter', '@cs-demo-analyst/shared-types', 'build']);
runPnpm(['--filter', '@cs-demo-analyst/sidecar', 'build']);

fs.mkdirSync(targetDir, { recursive: true });

for (const file of fs.readdirSync(sidecarDist)) {
  if (file.endsWith('.js')) {
    fs.copyFileSync(path.join(sidecarDist, file), path.join(targetDir, file));
  }
}

const bundledJs = fs.readdirSync(targetDir).filter((file) => file.endsWith('.js'));
if (bundledJs.length === 0) {
  throw new Error(`no sidecar .js files copied from ${sidecarDist}`);
}

const sharedTarget = path.join(targetDir, 'node_modules', '@cs-demo-analyst', 'shared-types');
fs.mkdirSync(path.dirname(sharedTarget), { recursive: true });
fs.cpSync(sharedDist, path.join(sharedTarget, 'dist'), { recursive: true });
fs.writeFileSync(
  path.join(sharedTarget, 'package.json'),
  JSON.stringify(
    {
      name: '@cs-demo-analyst/shared-types',
      version: '0.1.0',
      type: 'module',
      exports: { '.': { import: './dist/index.js' } },
    },
    null,
    2,
  ),
);

const sidecarPkg = JSON.parse(
  fs.readFileSync(path.join(root, 'packages', 'sidecar', 'package.json'), 'utf8'),
);
const bundlePkg = {
  name: 'cs-demo-analyst-sidecar-bundle',
  version: sidecarPkg.version,
  type: 'module',
  main: 'index.js',
  dependencies: Object.fromEntries(
    Object.entries(sidecarPkg.dependencies).filter(([k]) => !k.startsWith('@cs-demo-analyst/')),
  ),
};
fs.writeFileSync(path.join(targetDir, 'package.json'), JSON.stringify(bundlePkg, null, 2));

const npmCmd = process.platform === 'win32' ? 'npm.cmd' : 'npm';
const install = spawnSync(npmCmd, ['install', '--omit=dev'], { cwd: targetDir, stdio: 'inherit', shell: true });
if (install.status !== 0) {
  throw new Error('npm install failed in sidecar bundle directory');
}
console.log('Sidecar bundled to', targetDir);
