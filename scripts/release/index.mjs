#!/usr/bin/env node
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { buildReleaseDir, ensureDir } from '../lib/paths.mjs';
import { runPnpm, writeStamp } from '../lib/run.mjs';

const args = process.argv.slice(2);
const skipInstall = args.includes('--skip-install');
const skipPackage = args.includes('--skip-package');

async function main() {
  ensureDir(buildReleaseDir);

  if (!skipInstall) {
    runPnpm(['install', '--frozen-lockfile'], { label: 'Release: install dependencies' });
  }

  const dir = path.dirname(fileURLToPath(import.meta.url));

  if (!skipPackage) {
    await import(pathToFileURL(path.join(dir, 'package.mjs')).href);
  }

  await import(pathToFileURL(path.join(dir, 'collect.mjs')).href);

  writeStamp(path.join(buildReleaseDir, 'summary.json'), {
    workflow: 'release',
    status: 'ok',
  });

  console.log(`\nRelease complete. Outputs under ${buildReleaseDir}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
