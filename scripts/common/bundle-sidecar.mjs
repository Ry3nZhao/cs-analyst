#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { root } from '../lib/paths.mjs';
import { runPnpm } from '../lib/run.mjs';

const sidecarDist = path.join(root, 'src', 'sidecar', 'dist');
const protocolDist = path.join(root, 'src', 'sidecar', 'protocol', 'dist');
const targetDir = path.join(root, 'src', 'tauri', 'src-tauri', 'resources', 'sidecar');

runPnpm(['--filter', '@cs-analyst/sidecar', 'build']);

fs.mkdirSync(targetDir, { recursive: true });

// Copy all .js files and subdirectories from sidecar dist
fs.cpSync(sidecarDist, targetDir, {
  recursive: true,
  filter: (src) => {
    const stat = fs.statSync(src);
    return stat.isDirectory() || src.endsWith('.js');
  },
});

const bundledJs = fs.readdirSync(targetDir).filter((file) => file.endsWith('.js'));
if (bundledJs.length === 0) {
  throw new Error(`no sidecar .js files copied from ${sidecarDist}`);
}

// 下载模块只依赖协议包（src/sidecar/protocol），不依赖主仓库的其他代码
const protocolTarget = path.join(targetDir, 'node_modules', '@cs-analyst', 'downloader-protocol');
fs.mkdirSync(path.dirname(protocolTarget), { recursive: true });
fs.cpSync(protocolDist, path.join(protocolTarget, 'dist'), { recursive: true });
fs.writeFileSync(
  path.join(protocolTarget, 'package.json'),
  JSON.stringify(
    {
      name: '@cs-analyst/downloader-protocol',
      version: '1.0.0',
      type: 'module',
      exports: { '.': { import: './dist/index.js' } },
    },
    null,
    2,
  ),
);

const sidecarPkg = JSON.parse(
  fs.readFileSync(path.join(root, 'src', 'sidecar', 'package.json'), 'utf8'),
);
const bundlePkg = {
  name: 'cs-analyst-sidecar-bundle',
  version: sidecarPkg.version,
  type: 'module',
  main: 'index.js',
  dependencies: Object.fromEntries(
    Object.entries(sidecarPkg.dependencies).filter(([k]) => !k.startsWith('@cs-analyst/')),
  ),
};
fs.writeFileSync(path.join(targetDir, 'package.json'), JSON.stringify(bundlePkg, null, 2));

const npmCmd = process.platform === 'win32' ? 'npm.cmd' : 'npm';
const install = spawnSync(npmCmd, ['install', '--omit=dev'], { cwd: targetDir, stdio: 'inherit', shell: true });
if (install.status !== 0) {
  throw new Error('npm install failed in sidecar bundle directory');
}
console.log('Sidecar bundled to', targetDir);
