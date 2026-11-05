// 选手 / 对位页面共用的工具（W2-C）。数据来自 W1-C 的身份命令和 W1-B 的 list_player_stats。
import type {
  PersonDetailDto,
  PersonSummaryDto,
  PlayerMatchStatsDto,
  PlayerStatsListDto,
  RecordDto,
} from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

/** 选手页的路由参数：有 steam 身份用 steamid，否则用 `p<personId>` */
export function playerPath(p: Pick<PersonSummaryDto, 'id' | 'steamids'>): string {
  return `/players/${p.steamids[0] ?? `p${p.id}`}`;
}

export function searchPersons(query?: string, opts: { followedOnly?: boolean; limit?: number } = {}) {
  return tauriInvoke<PersonSummaryDto[]>('search_persons', {
    query: query?.trim() || null,
    followedOnly: opts.followedOnly ?? false,
    limit: opts.limit ?? 50,
  });
}

/** 路由参数 → 人。steamid 跟着身份走，所以合并 / 拆分后同一个 URL 仍然指向这个 steamid 当前所属的人。 */
export async function resolvePersonParam(param: string): Promise<PersonDetailDto | null> {
  const m = /^p(\d+)$/.exec(param);
  if (m) return tauriInvoke<PersonDetailDto>('get_person', { id: Number(m[1]) });
  if (!/^\d+$/.test(param)) return null;
  const hits = await searchPersons(param, { limit: 20 });
  const hit = hits.find(p => p.steamids.includes(param));
  return hit ? tauriInvoke<PersonDetailDto>('get_person', { id: hit.id }) : null;
}

/**
 * 「我」：主玩家（W3-C）所属的人。没设置主玩家，或主玩家的 steamid 不在本地库里时为 null。
 * 主玩家变化时自动重新查询；合并 / 拆分后调用 refreshMe() 重新按 steamid 解析。
 */
export function useMe() {
  const me = useState<PersonSummaryDto | null>('players-me', () => null);
  const loaded = useState('players-me-loaded', () => false);
  const main = useMainPlayer();
  let seq = 0;
  async function refreshMe() {
    const my = ++seq;
    await main.ready;
    const sid = main.steamid.value;
    let next: PersonSummaryDto | null = null;
    if (sid) {
      try {
        next = (await resolvePersonParam(sid))?.person ?? null;
      } catch {
        next = null;
      }
    }
    if (my !== seq) return;
    me.value = next;
    loaded.value = true;
  }
  main.onChange(() => void refreshMe());
  return { me, loaded, refreshMe, mainSteamid: main.steamid, isMainSet: main.isSet };
}

/**
 * 一个人可能挂多个 steamid：逐个取 list_player_stats，再按可加的计数合并。
 * HLTV 2.0 没法从计数精确复原，按回合数加权平均。
 */
export async function loadPlayerStats(steamids: string[]) {
  const lists = await Promise.all(
    steamids.map(s => tauriInvoke<PlayerStatsListDto>('list_player_stats', { steamid: s, filters: { limit: 200 } })),
  );
  const summaries = lists.map(l => l.summary).filter((s): s is PlayerMatchStatsDto => !!s);
  const matches = lists
    .flatMap(l => l.matches)
    .sort((a, b) => (b.playedAt ? parseTime(b.playedAt) : 0) - (a.playedAt ? parseTime(a.playedAt) : 0));
  return { summary: combineSummaries(summaries), matches, total: lists.reduce((n, l) => n + l.total, 0) };
}

export interface CombinedStats {
  matches: number;
  wins: number;
  losses: number;
  draws: number;
  rounds: number;
  kd: number | null;
  adr: number | null;
  kast: number | null;
  rating: number | null;
  hsPct: number | null;
  winRate: number | null;
}

function combineSummaries(list: PlayerMatchStatsDto[]): CombinedStats | null {
  if (list.length === 0) return null;
  if (list.length === 1) {
    const s = list[0]!;
    return {
      matches: s.matches, wins: s.wins, losses: s.losses, draws: s.draws, rounds: s.roundsPlayed,
      kd: s.derived.kd, adr: s.derived.adr, kast: s.derived.kast, rating: s.derived.rating, hsPct: s.derived.hsPct,
      winRate: s.matches ? s.wins / s.matches : null,
    };
  }
  const sum = (f: (s: PlayerMatchStatsDto) => number) => list.reduce((n, s) => n + f(s), 0);
  const rounds = sum(s => s.roundsPlayed);
  const kills = sum(s => s.kills);
  const deaths = sum(s => s.deaths);
  const matches = sum(s => s.matches);
  const wins = sum(s => s.wins);
  return {
    matches, wins, losses: sum(s => s.losses), draws: sum(s => s.draws), rounds,
    kd: deaths ? kills / deaths : kills,
    adr: rounds ? sum(s => s.damage) / rounds : null,
    kast: rounds ? (sum(s => s.kastRounds) / rounds) * 100 : null,
    rating: rounds ? sum(s => s.derived.rating * s.roundsPlayed) / rounds : null,
    hsPct: kills ? (sum(s => s.headshotKills) / kills) * 100 : null,
    winRate: matches ? wins / matches : null,
  };
}

// ── 格式化 ──

export function fmtPct(rate: number | null | undefined, digits = 0): string {
  return rate == null ? '—' : `${(rate * 100).toFixed(digits)}%`;
}

export function fmtNum(v: number | null | undefined, digits = 2): string {
  return v == null || Number.isNaN(v) ? '—' : v.toFixed(digits);
}

export function fmtRecord(r: RecordDto): string {
  return r.ties ? `${r.wins}-${r.losses}-${r.ties}` : `${r.wins}-${r.losses}`;
}

/** SQLite 的 datetime('now') 是不带时区的 UTC（`2026-11-05 12:29:52`），按 UTC 解析 */
export function parseTime(s: string): number {
  const m = /^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?)$/.exec(s.trim());
  return Date.parse(m ? `${m[1]}T${m[2]}Z` : s);
}

/** 相对时间，例如「3 天前」 */
export function fmtAgo(iso: string | null | undefined): string {
  if (!iso) return '—';
  const t = parseTime(iso);
  if (Number.isNaN(t)) return iso;
  const d = (Date.now() - t) / 1000;
  if (d < 60) return '刚刚';
  if (d < 3600) return `${Math.floor(d / 60)} 分钟前`;
  if (d < 86400) return `${Math.floor(d / 3600)} 小时前`;
  if (d < 86400 * 30) return `${Math.floor(d / 86400)} 天前`;
  if (d < 86400 * 365) return `${Math.floor(d / 86400 / 30)} 个月前`;
  return `${Math.floor(d / 86400 / 365)} 年前`;
}

export function fmtDate(iso: string | null | undefined): string {
  if (!iso) return '—';
  const d = new Date(parseTime(iso));
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

const PLATFORM_LABELS: Record<string, string> = {
  steam: 'Steam',
  valve: '官匹',
  '5eplay': '5E',
  renown: 'Renown',
  perfect: '完美',
  manual: '本地导入',
  imported: '本地导入',
};

export function platformLabel(p: string | null | undefined): string {
  if (!p) return '—';
  return PLATFORM_LABELS[p] ?? p;
}

export function mapLabel(m: string | null | undefined): string {
  return m ? m.replace(/^de_/, '') : '—';
}

/** 数值好坏：rate 相对 0.5 的方向，用于胜率 / 对枪胜率 */
export function rateTone(rate: number | null | undefined, minSample = 0, sample = Infinity): 'good' | 'bad' | '' {
  if (rate == null || sample < minSample) return '';
  if (rate > 0.55) return 'good';
  if (rate < 0.45) return 'bad';
  return '';
}

export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
