#!/usr/bin/env node
import path from 'node:path';
import {
  buildValidateDir,
  root,
  sidecarResourceIndex,
  tauriDir,
} from '../lib/paths.mjs';
import { assertFile, runCargo, runPnpm, writeStamp } from '../lib/run.mjs';

runPnpm(['--filter', '@cs-analyst/shared-types', 'build'], { label: 'Validate: shared-types' });
runPnpm(['--filter', '@cs-analyst/sidecar', 'build'], { label: 'Validate: sidecar' });
runPnpm(['build:sidecar'], { label: 'Validate: bundle sidecar' });

assertFile(sidecarResourceIndex);

runPnpm(['--filter', '@cs-analyst/desktop', 'generate'], { label: 'Validate: nuxt generate' });
runPnpm(['generate-icons'], { label: 'Validate: generate icons' });

runCargo(['check', '--workspace', '--timings'], {
  cwd: tauriDir,
  label: 'Validate: cargo check',
  env: {
    CARGO_TERM_PROGRESS_WHEN: 'always',
    CARGO_TERM_PROGRESS_WIDTH: '80',
    CARGO_TERM_COLOR: 'always',
  },
});

// 单元测试 + schema 测试；本机设置了 CS_ANALYST_SAMPLE_DEMOS / CS_ANALYST_SAMPLE_STEAMID（见 docs/development.md）时还会跑样例 demo 回归
runCargo(['test', '--workspace'], {
  cwd: tauriDir,
  label: 'Validate: cargo test',
  env: { CARGO_TERM_COLOR: 'always' },
});

writeStamp(path.join(buildValidateDir, 'rust.json'), {
  job: 'rust',
  status: 'ok',
});

console.log(`Validate rust output: ${path.join(buildValidateDir, 'rust.json')}`);
