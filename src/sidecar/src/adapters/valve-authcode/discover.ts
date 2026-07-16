import axios from 'axios';
import PQueue from 'p-queue';
import { decodeMatchShareCode } from 'csgo-sharecode';
import type { DiscoveredMatchDto, ValveChannel } from '@cs-analyst/shared-types';

interface MatchHistoryResponse {
  result: { nextcode: string };
}

export async function fetchShareCodes(
  steamId: string,
  authCode: string,
  oldestShareCode: string,
  steamApiKey: string,
): Promise<string[]> {
  if (!steamId.trim()) throw new Error('Steam ID is required');
  if (!authCode.trim()) throw new Error('Auth Code is required');
  if (!oldestShareCode.trim()) throw new Error('Oldest Share Code is required');
  if (!steamApiKey.trim()) throw new Error('Steam Web API Key is required');

  const queue = new PQueue({ concurrency: 1, interval: 1500, intervalCap: 1 });
  const codes: string[] = [];
  let lastCode = oldestShareCode;
  const seen = new Set<string>([oldestShareCode]);

  for (let page = 0; page < 100; page += 1) {
    const resp = await queue.add(() =>
      axios.get<MatchHistoryResponse>(
        'https://api.steampowered.com/ICSGOPlayers_730/GetNextMatchSharingCode/v1',
        {
          timeout: 30000,
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
    if (seen.has(next)) break;
    seen.add(next);
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
  const hasCursor = Boolean(params.lastShareCode?.trim());
  const startCode = params.lastShareCode ?? params.oldestShareCode;
  const shareCodes = await fetchShareCodes(
    params.steamId,
    params.authCode,
    startCode,
    params.steamApiKey,
  );

  if (!hasCursor && startCode.trim()) {
    shareCodes.unshift(startCode);
  }

  const seen = new Set<string>();
  const uniqueCodes = shareCodes.filter((code) => {
    if (seen.has(code)) return false;
    seen.add(code);
    return true;
  });

  return uniqueCodes.map((shareCode) => {
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
