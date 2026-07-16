import type { DiscoveredMatchDto } from '@cs-analyst/shared-types';
import { httpDownloadDemo } from '../../lib/http-download.js';
import type { EventEmitter } from '../../lib/emit-progress.js';

type ListResponse = { data: { match_id: number }[] };
type MatchResponse = {
  match_id: number;
  started_at: string;
  finished_at: string;
  demo_url: string | null;
  map: { name: string } | null;
};

async function assertOk(response: Response) {
  if (response.status === 404) throw new Error('NOT_FOUND');
  if (response.status === 429) throw new Error('RATE_LIMITED');
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
}

export async function discoverRenown(steamId: string): Promise<DiscoveredMatchDto[]> {
  const listResp = await fetch(
    `https://api.renown.gg/v1/player/${steamId}/matches?page=1&limit=20&status=FINISHED`,
  );
  await assertOk(listResp);
  const list = (await listResp.json()) as ListResponse;
  const matches: DiscoveredMatchDto[] = [];

  for (const item of list.data) {
    const detailResp = await fetch(`https://api.renown.gg/v1/match/${item.match_id}`);
    try {
      await assertOk(detailResp);
    } catch {
      continue;
    }
    const match = (await detailResp.json()) as MatchResponse;
    matches.push({
      matchId: String(match.match_id),
      demoUrl: match.demo_url,
      mapName: match.map?.name ?? null,
      playedAt: new Date(match.finished_at).toISOString(),
      rawPayload: { started_at: match.started_at },
    });
  }
  return matches;
}

export async function downloadRenown(
  demoUrl: string,
  outputPath: string,
  options?: { jobId?: string; emit?: EventEmitter },
): Promise<void> {
  try {
    await httpDownloadDemo({
      demoUrl,
      outputPath,
      jobId: options?.jobId,
      emit: options?.emit,
    });
  } catch (error) {
    if (error instanceof Error && error.message.includes('404')) {
      throw new Error('URL_EXPIRED');
    }
    throw error;
  }
}
