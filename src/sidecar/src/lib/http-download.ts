import fs from 'node:fs';
import fsp from 'node:fs/promises';
import stream from 'node:stream';
import util from 'node:util';
import axios from 'axios';
import bz2 from 'unbzip2-stream';
import type { DownloadProgressEvent } from '@cs-analyst/shared-types';
import { emitDownloadProgress, type EventEmitter } from './emit-progress.js';

const pipeline = util.promisify(stream.pipeline);

export interface HttpDownloadParams {
  demoUrl: string;
  outputPath: string;
  jobId?: string;
  emit?: EventEmitter;
  headers?: Record<string, string>;
}

function mapDownloadProgress(
  loaded: number,
  total: number | undefined,
  phase: DownloadProgressEvent['phase'],
): number {
  if (phase === 'extracting') return 0.9;
  if (phase === 'done') return 1;
  if (!total || total <= 0) return 0.5;
  return 0.1 + (loaded / total) * 0.75;
}

export async function httpDownloadDemo(params: HttpDownloadParams): Promise<void> {
  const { demoUrl, outputPath, jobId, emit, headers } = params;
  if (!demoUrl) throw new Error('No demo URL available');

  if (demoUrl.endsWith('.bz2')) {
    await downloadBz2(demoUrl, outputPath, jobId, emit, headers);
    return;
  }

  const resp = await axios.get(demoUrl, {
    responseType: 'arraybuffer',
    timeout: 120000,
    headers,
    onDownloadProgress: (event) => {
      if (!jobId) return;
      const total = event.total ?? undefined;
      const progress = mapDownloadProgress(event.loaded, total, 'downloading');
      emitDownloadProgress(emit, {
        jobId,
        progress,
        bytesDownloaded: event.loaded,
        totalBytes: total,
        phase: 'downloading',
      });
    },
  });
  await fsp.writeFile(outputPath, Buffer.from(resp.data));
  if (jobId) {
    emitDownloadProgress(emit, { jobId, progress: 1, phase: 'done' });
  }
}

async function downloadBz2(
  url: string,
  outputPath: string,
  jobId: string | undefined,
  emit: EventEmitter | undefined,
  headers?: Record<string, string>,
): Promise<void> {
  const tempPath = `${outputPath}.tmp`;
  try {
    const resp = await axios.get<stream.Duplex>(url, {
      responseType: 'stream',
      timeout: 120000,
      headers,
      onDownloadProgress: (event) => {
        if (!jobId) return;
        const total = event.total ?? undefined;
        const progress = mapDownloadProgress(event.loaded, total, 'downloading');
        emitDownloadProgress(emit, {
          jobId,
          progress,
          bytesDownloaded: event.loaded,
          totalBytes: total,
          phase: 'downloading',
        });
      },
    });
    if (jobId) {
      emitDownloadProgress(emit, { jobId, progress: 0.88, phase: 'extracting' });
    }
    await pipeline(resp.data, bz2(), fs.createWriteStream(tempPath, { flags: 'w' }));
    await fsp.rename(tempPath, outputPath);
    if (jobId) {
      emitDownloadProgress(emit, { jobId, progress: 1, phase: 'done' });
    }
  } catch (error) {
    await fsp.rm(tempPath, { force: true }).catch(() => undefined);
    throw error;
  }
}
