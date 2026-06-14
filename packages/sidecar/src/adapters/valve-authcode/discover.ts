import axios from 'axios';
import PQueue from 'p-queue';
import { decodeMatchShareCode } from 'csgo-sharecode';
import type { DiscoveredMatchDto, ValveChannel } from '@cs-demo-analyst/shared-types';

interface MatchHistoryResponse {
  result: { nextcode: string };
}

export async function fetchShareCodes(
  steamId: string,
  authCode: string,
  oldestShareCode: string,
  steamApiKey: string,
): Promise<string[]> {
  const queue = new PQueue({ concurrency: 1, interval: 1500, intervalCap: 1 });
  const codes: string[] = [];
  let lastCode = oldestShareCode;

  while (true) {
    const resp = await queue.add(() =>
      axios.get<MatchHistoryResponse>(
        'https://api.steampowered.com/ICSGOPlayers_730/GetNextMatchSharingCode/v1',
        {
          params: {
            key: steamApiKey,
            steamid: steamId,
            steamidkey: authCode,
            knowncode: lastCode,
          },
        },
      ),
    );
    if (!resp) break;
    const next = resp.data.result.nextcode;
    if (!next || next === 'n/a') break;
    codes.push(next);
    lastCode = next;
  }

  return codes;
}

export async function discoverValveAuthCode(params: {
  steamId: string;
  authCode: string;
  oldestShareCode: string;
  steamApiKey: string;
  lastShareCode?: string | null;
}): Promise<DiscoveredMatchDto[]> {
  const startCode = params.lastShareCode ?? params.oldestShareCode;
  const shareCodes = await fetchShareCodes(
    params.steamId,
    params.authCode,
    startCode,
    params.steamApiKey,
  );

  return shareCodes.map((shareCode) => {
    const decoded = decodeMatchShareCode(shareCode);
    return {
      matchId: decoded.matchId.toString(),
      shareCode,
      demoUrl: null,
      mapName: null,
      playedAt: null,
      valveChannel: 'authcode' as ValveChannel,
      rawPayload: { shareCode, matchId: decoded.matchId.toString() },
    };
  });
}

export function gcpdUrlToFilename(url: string): string {
  const matchGroups = url.match(
    /^https?:\/\/replay(\d+)\.(?:valve\.net|csgo\.com\.cn)\/(\d+)\/(\d+_\d+)\.dem\.bz2$/,
  );
  if (!matchGroups) throw new Error(`Invalid GCPD URL: ${url}`);
  const [, regionId, gameId, matchId] = matchGroups;
  return `match${gameId}_${matchId}_${regionId}.dem`;
}
