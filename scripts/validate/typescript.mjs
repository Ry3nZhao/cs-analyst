#!/usr/bin/env node
import path from 'node:path';
import { buildValidateDir } from '../lib/paths.mjs';
import { runPnpm, writeStamp } from '../lib/run.mjs';

runPnpm(['--filter', '@cs-demo-analyst/shared-types', 'build'], { label: 'Validate: shared-types' });
runPnpm(['--filter', '@cs-demo-analyst/sidecar', 'build'], { label: 'Validate: sidecar' });

writeStamp(path.join(buildValidateDir, 'typescript.json'), {
  job: 'typescript',
  status: 'ok',
});

console.log(`Validate typescript output: ${path.join(buildValidateDir, 'typescript.json')}`);
