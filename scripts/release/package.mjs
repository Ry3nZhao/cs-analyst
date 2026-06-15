#!/usr/bin/env node
import path from 'node:path';
import fs from 'node:fs';
import { buildReleaseDir, cargoReleaseBundleDir, ensureDir, root } from '../lib/paths.mjs';
import { runPnpm, writeStamp } from '../lib/run.mjs';

function platformStamp(base) {
  const platform = process.env.RELEASE_PLATFORM;
  return platform ? `${base}-${platform}.json` : `${base}.json`;
}

ensureDir(buildReleaseDir);

runPnpm(['generate-icons'], { label: 'Release: generate icons' });
runPnpm(['build'], { label: 'Release: build frontend and sidecar sources' });
runPnpm(['build:sidecar'], { label: 'Release: bundle sidecar resources' });
runPnpm(['sync-version'], { label: 'Release: sync version' });
runPnpm(['--filter', '@cs-demo-analyst/tauri-app', 'tauri', 'build'], { label: 'Release: tauri build' });

writeStamp(path.join(buildReleaseDir, platformStamp('package-step')), {
  job: 'package',
  status: 'ok',
  bundleRoot: cargoReleaseBundleDir(),
});

console.log(`Release package complete. Collect installers into ${buildReleaseDir}`);
