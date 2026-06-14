#!/usr/bin/env node
import path from 'node:path';
import {
  buildValidateDir,
  root,
  sidecarResourceIndex,
  tauriDir,
} from '../lib/paths.mjs';
import { assertFile, runCargo, runPnpm, writeStamp } from '../lib/run.mjs';

runPnpm(['--filter', '@cs-demo-analyst/shared-types', 'build'], { label: 'Validate: shared-types' });
runPnpm(['--filter', '@cs-demo-analyst/sidecar', 'build'], { label: 'Validate: sidecar' });
runPnpm(['build:sidecar'], { label: 'Validate: bundle sidecar' });

assertFile(sidecarResourceIndex);

runPnpm(['--filter', '@cs-demo-analyst/desktop', 'generate'], { label: 'Validate: nuxt generate' });
runPnpm(['generate-icons'], { label: 'Validate: generate icons' });

runCargo(['check', '--timings'], {
  cwd: tauriDir,
  label: 'Validate: cargo check',
  env: {
    CARGO_TERM_PROGRESS_WHEN: 'always',
    CARGO_TERM_PROGRESS_WIDTH: '80',
    CARGO_TERM_COLOR: 'always',
  },
});

writeStamp(path.join(buildValidateDir, 'rust.json'), {
  job: 'rust',
  status: 'ok',
});

console.log(`Validate rust output: ${path.join(buildValidateDir, 'rust.json')}`);
