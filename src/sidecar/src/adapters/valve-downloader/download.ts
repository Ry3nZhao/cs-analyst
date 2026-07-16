import { httpDownloadDemo } from '../../lib/http-download.js';
import type { EventEmitter } from '../../lib/emit-progress.js';

export { gcpdUrlToFilename } from '../valve-authcode/discover.js';

export async function downloadValveDemoFile(params: {
  outputPath: string;
  demoUrl: string;
  jobId?: string;
  emit?: EventEmitter;
}): Promise<{ checksum?: string }> {
  const fs = await import('node:fs/promises');
  await httpDownloadDemo({
    demoUrl: params.demoUrl,
    outputPath: params.outputPath,
    jobId: params.jobId,
    emit: params.emit,
  });
  const stat = await fs.stat(params.outputPath);
  return { checksum: `${stat.size}` };
}
