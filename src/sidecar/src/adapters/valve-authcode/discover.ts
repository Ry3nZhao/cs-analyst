import axios from 'axios';
import PQueue from 'p-queue';
import { decodeMatchShareCode } from 'csgo-sharecode';
import type { DiscoveredMatchDto, ValveChannel } from '@cs-analyst/shared-types';

interface MatchHistoryResponse {
  result?: { nextcode?: string };
}

const SHARE_CODE_PATTERN =
  /^CSGO(-[ABCDEFGHJKLMNOPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789]{5}){5}$/;

/**
 * Steam 对这个接口只返回状态码、响应体是空的，原始的 axios 报错
 * （"Request failed with status code 412"）对用户毫无意义，这里翻译成可操作的提示。
 */
function describeSteamError(
  status: number | undefined,
  steamId: string,
  knownCode: string,
  body?: unknown,
): string {
  // 被 Steam 边缘限流时同样返回 403，但响应体是 HTML 拦截页而不是 API 的空 JSON
  if (typeof body === 'string' && body.toLowerCase().includes('<html')) {
    return `Steam 暂时拒绝了来自本机 IP 的请求（HTTP ${status}，触发了频率限制）。请等待一段时间后再试，不要连续重试。`;
  }
  switch (status) {
    case 403:
      return `Steam 拒绝了账号 ${steamId} 的身份验证码（HTTP 403）：Auth Code 与该 Steam ID 不匹配，或已被重新生成而失效。请在 Steam 帮助页面重新获取该账号的 Auth Code。`;
    case 412:
      return `Steam 不认识起始 Share Code（HTTP 412）：${knownCode} 不属于账号 ${steamId} 的对局记录。请确认这个 Share Code 来自该账号本人打过的官方匹配对局（在 CS2 内「观看 → 你的对局」复制），而不是别的账号或别人分享的对局。`;
    case 429:
      return 'Steam API 请求过于频繁（HTTP 429），已被临时限流，请稍后再试。';
    case 500:
    case 502:
    case 503:
      return `Steam 服务暂时不可用（HTTP ${status}），请稍后重试。`;
    default:
      return `Steam API 调用失败${status ? `（HTTP ${status}）` : ''}。`;
  }
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
  if (!SHARE_CODE_PATTERN.test(oldestShareCode.trim())) {
    throw new Error(
      `起始 Share Code 格式不正确：${oldestShareCode}。正确格式形如 CSGO-xxxxx-xxxxx-xxxxx-xxxxx-xxxxx。`,
    );
  }

  const queue = new PQueue({ concurrency: 1, interval: 1500, intervalCap: 1 });
  const codes: string[] = [];
  let lastCode = oldestShareCode;
  const seen = new Set<string>([oldestShareCode]);

  for (let page = 0; page < 100; page += 1) {
    let resp;
    try {
      resp = await queue.add(() =>
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
    } catch (error) {
      const status = axios.isAxiosError(error) ? error.response?.status : undefined;
      const body = axios.isAxiosError(error) ? error.response?.data : undefined;
      // 第一次调用就失败 = 配置有问题，必须让用户看到原因；
      // 翻页途中失败则保留已经拿到的结果，避免整批丢失
      if (page === 0) throw new Error(describeSteamError(status, steamId, lastCode, body));
      break;
    }
    if (!resp) break;
    const next = resp.data?.result?.nextcode;
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
