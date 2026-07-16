import { createWriteStream } from 'node:fs';
import { pipeline } from 'node:stream/promises';
import { createGunzip } from 'node:zlib';
import bz2 from 'unbzip2-stream';
import type { DiscoveredMatchDto } from '@cs-analyst/shared-types';
import { emitDownloadProgress, type EventEmitter } from '../../lib/emit-progress.js';
import { httpDownloadDemo } from '../../lib/http-download.js';
import { resolve5EPlayPlayerId } from './resolve-player-id.js';

type MatchListPayload =
  | { code: 0; data: { match_id: string }[] }
  | { code: 500; data: null };

type MatchPayload =
  | { code: 500; data: null }
  | {
      code: 0;
      data: {
        main: {
          cs_type: 0 | 1;
          start_time: number;
          end_time: number;
          demo_url: string;
          map: string;
        };
      };
    };

function downloadHeaders(demoUrl: string): Record<string, string> {
  try {
    const host = new URL(demoUrl).hostname;
    if (host.endsWith('5eplaycdn.com')) {
      return { Referer: 'https://arena.5eplay.com/' };
    }
  } catch {
    // ignore
  }
  return {};
}

async function fetchMatchIds(uuid: string, csType: number, limit: number): Promise<string[]> {
  const response = await fetch(
    `https://gate.5eplay.com/crane/http/api/data/match/list?uuid=${uuid}&limit=${limit}&cs_type=${csType}`,
  );
  const payload = (await response.json()) as MatchListPayload;
  if (payload.data === null) return [];
  return payload.data.map((m) => m.match_id);
}

async function fetchMatchDetail(matchId: string): Promise<DiscoveredMatchDto | null> {
  const response = await fetch(`https://gate.5eplay.com/crane/http/api/data/match/${matchId}`);
  const payload = (await response.json()) as MatchPayload;
  if (payload.data === null) return null;
  const { main } = payload.data;
  return {
    matchId,
    demoUrl: main.demo_url,
    mapName: main.map,
    playedAt: new Date(main.start_time * 1000).toISOString(),
    rawPayload: { cs_type: main.cs_type, end_time: main.end_time },
  };
}

export async function discover5EPlay(externalId: string): Promise<DiscoveredMatchDto[]> {
  const uuid = await resolve5EPlayPlayerId(externalId);
  const maxCount = 20;
  const matchIds = await fetchMatchIds(uuid, 0, maxCount);
  if (matchIds.length < maxCount) {
    const csgoIds = await fetchMatchIds(uuid, 1, maxCount - matchIds.length);
    matchIds.push(...csgoIds);
  }

  const matches: DiscoveredMatchDto[] = [];
  for (const matchId of matchIds) {
    const detail = await fetchMatchDetail(matchId);
    if (detail) matches.push(detail);
  }
  return matches;
}

export { resolve5EPlayPlayerId } from './resolve-player-id.js';

export async function download5EPlay(
  demoUrl: string,
  outputPath: string,
  options?: { jobId?: string; emit?: EventEmitter },
): Promise<void> {
  const { jobId, emit } = options ?? {};
  const headers = downloadHeaders(demoUrl);

  if (!demoUrl.endsWith('.zip') && !demoUrl.includes('.zip')) {
    const contentTypeHint = demoUrl.endsWith('.gz') || demoUrl.endsWith('.bz2') ? demoUrl : '';
    if (!contentTypeHint || demoUrl.endsWith('.bz2')) {
      await httpDownloadDemo({ demoUrl, outputPath, jobId, emit, headers });
      return;
    }
  }

  const response = await fetch(demoUrl, { headers });
  if (!response.ok) {
    if (response.status === 404) throw new Error('URL_EXPIRED');
    throw new Error(`HTTP ${response.status}`);
  }
  if (!response.body) throw new Error('Empty response body');

  if (jobId) {
    emitDownloadProgress(emit, { jobId, progress: 0.2, phase: 'downloading' });
  }

  const contentType = response.headers.get('content-type') ?? '';
  const stream = response.body as unknown as NodeJS.ReadableStream;

  if (contentType.includes('zip') || demoUrl.endsWith('.zip')) {
    if (jobId) {
      emitDownloadProgress(emit, { jobId, progress: 0.88, phase: 'extracting' });
    }
    const { extract5EPlayDemoFromZipStream } = await import('./extract-demo.js');
    await extract5EPlayDemoFromZipStream(stream, outputPath);
    if (jobId) {
      emitDownloadProgress(emit, { jobId, progress: 1, phase: 'done' });
    }
    return;
  }

  if (demoUrl.endsWith('.gz')) {
    const out = createWriteStream(outputPath);
    await pipeline(stream, createGunzip(), out);
    if (jobId) {
      emitDownloadProgress(emit, { jobId, progress: 1, phase: 'done' });
    }
    return;
  }

  const buffer = Buffer.from(await response.arrayBuffer());
  const fs = await import('node:fs/promises');
  await fs.writeFile(outputPath, buffer);
  if (jobId) {
    emitDownloadProgress(emit, { jobId, progress: 1, phase: 'done' });
  }
}
