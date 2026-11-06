#!/usr/bin/env node
import path from 'node:path';
import { buildValidateDesktopDir, buildValidateDir, root } from '../lib/paths.mjs';
import { copyOutputDir, runPnpm, writeStamp } from '../lib/run.mjs';

// 前端运行时 import @cs-analyst/shared-types（入口在 dist/），单独跑这一步时也要先构建
runPnpm(['--filter', '@cs-analyst/shared-types', 'build'], { label: 'Validate: shared-types' });
runPnpm(['--filter', '@cs-analyst/desktop', 'generate'], { label: 'Validate: nuxt generate' });

const desktopDist = path.join(root, 'src', 'desktop', 'dist');
copyOutputDir(desktopDist, buildValidateDesktopDir);

writeStamp(path.join(buildValidateDir, 'frontend.json'), {
  job: 'frontend',
  status: 'ok',
  output: buildValidateDesktopDir,
});

console.log(`Validate frontend output: ${buildValidateDesktopDir}`);
