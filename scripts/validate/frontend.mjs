#!/usr/bin/env node
import path from 'node:path';
import { buildValidateDesktopDir, buildValidateDir, root } from '../lib/paths.mjs';
import { copyOutputDir, runPnpm, writeStamp } from '../lib/run.mjs';

runPnpm(['--filter', '@cs-demo-analyst/desktop', 'generate'], { label: 'Validate: nuxt generate' });

const desktopDist = path.join(root, 'apps', 'desktop', 'dist');
copyOutputDir(desktopDist, buildValidateDesktopDir);

writeStamp(path.join(buildValidateDir, 'frontend.json'), {
  job: 'frontend',
  status: 'ok',
  output: buildValidateDesktopDir,
});

console.log(`Validate frontend output: ${buildValidateDesktopDir}`);
