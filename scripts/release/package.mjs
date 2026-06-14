#!/usr/bin/env node
import path from 'node:path';
import fs from 'node:fs';
import { buildReleaseDir, ensureDir, root } from '../lib/paths.mjs';
import { runPnpm, writeStamp } from '../lib/run.mjs';

ensureDir(buildReleaseDir);

runPnpm(['generate-icons'], { label: 'Release: generate icons' });
runPnpm(['build'], { label: 'Release: build frontend and sidecar sources' });
runPnpm(['build:sidecar'], { label: 'Release: bundle sidecar resources' });
runPnpm(['sync-version'], { label: 'Release: sync version' });
runPnpm(['--filter', '@cs-demo-analyst/tauri-app', 'tauri', 'build'], { label: 'Release: tauri build' });

writeStamp(path.join(buildReleaseDir, 'package-step.json'), {
  job: 'package',
  status: 'ok',
  bundleRoot: path.join(root, 'crates', 'tauri-app', 'src-tauri', 'target', 'release', 'bundle'),
});

console.log(`Release package complete. Collect installers into ${buildReleaseDir}`);
