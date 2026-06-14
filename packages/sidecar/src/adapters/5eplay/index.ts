import type { DiscoveredMatchDto } from '@cs-demo-analyst/shared-types';

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

export async function discover5EPlay(uuid: string): Promise<DiscoveredMatchDto[]> {
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

export async function download5EPlay(demoUrl: string, outputPath: string): Promise<void> {
  const response = await fetch(demoUrl, {
    headers: { Referer: 'https://arena.5eplay.com/' },
  });
  if (!response.ok) {
    if (response.status === 404) throw new Error('URL_EXPIRED');
    throw new Error(`HTTP ${response.status}`);
  }
  if (!response.body) throw new Error('Empty response body');

  const contentType = response.headers.get('content-type') ?? '';
  if (contentType.includes('zip') || demoUrl.endsWith('.zip')) {
    const { extract5EPlayDemoFromZipStream } = await import('./extract-demo.js');
    await extract5EPlayDemoFromZipStream(response.body as unknown as NodeJS.ReadableStream, outputPath);
    return;
  }

  const buffer = Buffer.from(await response.arrayBuffer());
  await fsWrite(outputPath, buffer);
}

async function fsWrite(path: string, data: Buffer) {
  const fs = await import('node:fs/promises');
  await fs.writeFile(path, data);
}
