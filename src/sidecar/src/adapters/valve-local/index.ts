import type { EventEmitter } from '../../lib/emit-progress.js';
import { downloadValveDemo } from '../valve-authcode/download.js';

/** Valve local discovery requires an active Steam client session (CSDM boiler). */
export async function discoverValveLocal(_steamId: string): Promise<import('@cs-analyst/shared-types').DiscoveredMatchDto[]> {
  throw new Error(
    'Valve local discovery requires Steam login integration; use Auth Code channel or add share codes manually.',
  );
}

export async function downloadValveLocal(
  demoUrl: string,
  outputPath: string,
  options?: { jobId?: string; emit?: EventEmitter },
): Promise<void> {
  await downloadValveDemo({
    outputPath,
    demoUrl,
    jobId: options?.jobId,
    emit: options?.emit,
  });
}
