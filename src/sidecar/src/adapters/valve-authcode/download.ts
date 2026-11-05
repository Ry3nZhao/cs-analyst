import { downloadValveDemoFile } from '../valve-downloader/download.js';

export async function downloadValveDemo(params: {
  outputPath: string;
  demoUrl?: string | null;
  jobId?: string;
  emit?: (event: import('@cs-analyst/downloader-protocol').SidecarEvent) => void;
}): Promise<{ checksum?: string }> {
  if (!params.demoUrl) throw new Error('No demo URL available');
  return downloadValveDemoFile({
    outputPath: params.outputPath,
    demoUrl: params.demoUrl,
    jobId: params.jobId,
    emit: params.emit,
  });
}

export { gcpdUrlToFilename } from './discover.js';
