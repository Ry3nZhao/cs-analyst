import type { DiscoveredMatchDto } from '@cs-demo-analyst/shared-types';

/** Valve local discovery requires an active Steam client session (CSDM boiler). */
export async function discoverValveLocal(_steamId: string): Promise<DiscoveredMatchDto[]> {
  throw new Error(
    'Valve local discovery requires Steam login integration; use Auth Code channel or add share codes manually.',
  );
}

export async function downloadValveLocal(demoUrl: string, outputPath: string): Promise<void> {
  const { downloadValveDemo } = await import('../valve-authcode/download.js');
  await downloadValveDemo({ outputPath, demoUrl });
}
