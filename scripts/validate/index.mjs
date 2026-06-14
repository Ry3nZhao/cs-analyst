#!/usr/bin/env node
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildValidateDir, ensureDir } from '../lib/paths.mjs';
import { runPnpm, writeStamp } from '../lib/run.mjs';

const args = process.argv.slice(2);
const skipInstall = args.includes('--skip-install');

async function main() {
  ensureDir(buildValidateDir);

  if (!skipInstall) {
    runPnpm(['install', '--frozen-lockfile'], { label: 'Validate: install dependencies' });
  }

  const dir = path.dirname(fileURLToPath(import.meta.url));
  const steps = ['typescript.mjs', 'frontend.mjs', 'rust.mjs'];

  for (const step of steps) {
    await import(path.join(dir, step));
  }

  writeStamp(path.join(buildValidateDir, 'summary.json'), {
    workflow: 'validate',
    status: 'ok',
    jobs: ['typescript', 'frontend', 'rust'],
  });

  console.log(`\nValidate complete. Outputs under ${buildValidateDir}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
