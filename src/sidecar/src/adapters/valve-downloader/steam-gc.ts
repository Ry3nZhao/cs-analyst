/**
 * Adapted from cs-demo-downloader steam-gc.ts (MIT)
 * https://github.com/claabs/cs-demo-downloader
 */
import GlobalOffensive, { type GcMatch } from 'globaloffensive';
import PQueue from 'p-queue';
import { decodeMatchShareCode } from 'csgo-sharecode';
import promiseTimeout from 'p-timeout';
import { emitResolveBatch, emitResolveProgress, type EventEmitter } from '../../lib/emit-progress.js';
import { loginSteamClient } from './steam.js';
import type { BotLogin, ResolveItem, ResolveValveAuthcodeResult, ResolvedValveMatch } from './types.js';

type MatchRespFn = (match: GcMatch) => void;

function demoUrlFromMatch(match: GcMatch): string | undefined {
  const direct = (match as { demo_url?: string }).demo_url;
  if (direct) return direct;
  const rounds = match.roundstatsall;
  if (!rounds?.length) return undefined;
  const last = rounds.at(-1) as { map?: string } | undefined;
  return last?.map;
}

function mapNameFromMatch(match: GcMatch): string | undefined {
  const rounds = match.roundstatsall;
  if (!rounds?.length) return undefined;
  const first = rounds[0] as { map?: string } | undefined;
  return first?.map;
}

function playedAtFromMatch(match: GcMatch): string | undefined {
  const matchTime = match.matchtime;
  if (typeof matchTime !== 'number' || matchTime <= 0) return undefined;
  return new Date(matchTime * 1000).toISOString();
}

export async function resolveValveAuthcodeUrls(
  bot: BotLogin,
  items: ResolveItem[],
  emit?: EventEmitter,
): Promise<ResolveValveAuthcodeResult> {
  const uniqueByShare = new Map<string, ResolveItem>();
  for (const item of items) {
    if (!uniqueByShare.has(item.shareCode)) {
      uniqueByShare.set(item.shareCode, item);
    }
  }
  const workItems = [...uniqueByShare.values()];
  const total = workItems.length;

  if (total === 0) {
    return { resolved: [] };
  }

  emitResolveBatch(emit, { total, completed: 0, phase: 'logging_in' });

  const { client: steamUser, refreshToken } = await loginSteamClient(bot);

  try {
    // globaloffensive 只在收到 appLaunched 事件时才发起 GC 握手，所以实例必须在
    // gamesPlayed() 之前创建；否则事件早已错过，GC 会话永远建立不起来，
    // 之后每个 requestGame 都石沉大海直到超时（表现为「能发现对局但拿不到 demo URL」）。
    const csgo = new GlobalOffensive(steamUser);
    const pendingMatchResponses = new Map<string, MatchRespFn>();
    csgo.on('matchList', (matches) => {
      matches.forEach((match) => {
        const cb = pendingMatchResponses.get(match.matchid);
        if (cb) {
          pendingMatchResponses.delete(match.matchid);
          cb(match);
        }
      });
    });

    // 等 GC 真正握手完成，而不是只等游戏启动
    const waitForGC = promiseTimeout(
      new Promise<void>((resolve) => {
        if (csgo.haveGCSession) {
          resolve();
          return;
        }
        csgo.once('connectedToGC', () => resolve());
      }),
      { milliseconds: 60000, message: '等待 Steam 游戏协调器 (GC) 连接超时' },
    );
    steamUser.gamesPlayed(730, true);
    await waitForGC;

    emitResolveBatch(emit, { total, completed: 0, phase: 'resolving' });

    const requestGameQueue = new PQueue({ concurrency: 1 });
    const resolved: ResolvedValveMatch[] = [];
    let completed = 0;

    await Promise.all(
      workItems.map((item) =>
        requestGameQueue.add(async () => {
          emitResolveProgress(emit, {
            jobId: item.jobId,
            matchId: item.matchId,
            shareCode: item.shareCode,
            phase: 'started',
          });

            try {
            const { matchId } = decodeMatchShareCode(item.shareCode);
            const [match] = await Promise.all([
              promiseTimeout(
                new Promise<GcMatch>((resolve) => {
                  pendingMatchResponses.set(matchId.toString(), resolve);
                }),
                {
                  milliseconds: 30000,
                  message: `Error fetching match data for match ${item.shareCode}`,
                },
              ),
              csgo.requestGame(item.shareCode),
            ]);

            const demoUrl = demoUrlFromMatch(match);
            const playedAt = playedAtFromMatch(match);
            const mapName = mapNameFromMatch(match);

            if (!demoUrl) {
              const error = 'URL_EXPIRED';
              resolved.push({ ...item, error });
              emitResolveProgress(emit, {
                jobId: item.jobId,
                matchId: item.matchId,
                shareCode: item.shareCode,
                phase: 'failed',
                error,
              });
            } else {
              resolved.push({ ...item, demoUrl, playedAt, mapName });
              emitResolveProgress(emit, {
                jobId: item.jobId,
                matchId: item.matchId,
                shareCode: item.shareCode,
                phase: 'resolved',
                demoUrl,
                playedAt,
                mapName,
              });
            }
          } catch (err) {
            const error = err instanceof Error ? err.message : String(err);
            resolved.push({ ...item, error });
            emitResolveProgress(emit, {
              jobId: item.jobId,
              matchId: item.matchId,
              shareCode: item.shareCode,
              phase: 'failed',
              error,
            });
          } finally {
            completed += 1;
            emitResolveBatch(emit, { total, completed, phase: 'resolving' });
          }
        }),
      ),
    );

    const waitForQuit = promiseTimeout(
      new Promise<void>((resolve) => {
        steamUser.once('appQuit', (id: number) => {
          if (id === 730) resolve();
        });
      }),
      { milliseconds: 30000, message: 'Timed out waiting for game to quit' },
    );
    steamUser.gamesPlayed([], true);
    await waitForQuit;

    emitResolveBatch(emit, { total, completed, phase: 'done' });
    return { resolved, refreshToken };
  } finally {
    steamUser.logOff();
  }
}
