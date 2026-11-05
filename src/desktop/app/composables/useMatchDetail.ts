// 比赛详情页（W2-A）用到的格式化和「我」的判定。
import type { BuyType, MatchDetailDto, MatchWinProbabilityDto, PersonSummaryDto, PlayerMatchStatsDto, Side, SteamId, TeamId } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

/**
 * 「我」的 steamid 集合：主玩家（W3-C）所属的人名下的全部 steamid（小号、换号合并后都算「我」）。
 * 没设置主玩家时为空集合，页面上不标「我」。主玩家变化时自动重新计算。
 */
const mySteamIds = ref<Set<SteamId>>(new Set());
let meWatching = false;
let currentMain: string | null = null;

async function resolveMySteamIds(steamid: string | null) {
  currentMain = steamid;
  if (!steamid) {
    mySteamIds.value = new Set();
    return;
  }
  // 先只放主玩家本身，再补上同一个人名下的其他 steamid
  mySteamIds.value = new Set([steamid]);
  try {
    const hits = await tauriInvoke<PersonSummaryDto[]>('search_persons', { query: steamid, followedOnly: false, limit: 20 });
    const hit = hits.find(p => p.steamids.includes(steamid));
    if (hit && currentMain === steamid) mySteamIds.value = new Set(hit.steamids);
  } catch { /* 保留主玩家本身 */ }
}

export function useMySteamIds() {
  if (!meWatching) {
    meWatching = true;
    // 全局状态：放在独立的 effect scope 里，不随调用它的组件卸载而停止
    effectScope(true).run(() => {
      const main = useMainPlayer();
      void main.ready.then(() => resolveMySteamIds(main.steamid.value));
      watch(main.steamid, (id) => void resolveMySteamIds(id));
    });
  }
  return mySteamIds;
}

const MAP_NAMES: Record<string, string> = {
  de_dust2: 'Dust II', de_mirage: 'Mirage', de_inferno: 'Inferno', de_nuke: 'Nuke',
  de_ancient: 'Ancient', de_anubis: 'Anubis', de_vertigo: 'Vertigo', de_overpass: 'Overpass',
  de_train: 'Train', de_office: 'Office', cs_italy: 'Italy', cs_office: 'Office',
};

export function mapLabel(map: string | null | undefined) {
  if (!map) return '未知地图';
  return MAP_NAMES[map] ?? map.replace(/^(de|cs|ar)_/, '').replace(/^\w/, c => c.toUpperCase());
}

export function platformLabel(p: string | null | undefined) {
  if (!p) return '—';
  return ({ valve: 'Valve 官匹', renown: 'Renown', '5eplay': '5EPlay', imported: '导入' } as Record<string, string>)[p] ?? p;
}

export function fmtDateTime(iso: string | null | undefined) {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
}

export function fmtClock(secs: number | null | undefined) {
  if (secs == null || !Number.isFinite(secs)) return '—';
  const s = Math.max(0, Math.round(secs));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

export function fmtMoney(v: number | null | undefined) {
  if (v == null) return '—';
  return `$${Math.round(v).toLocaleString('en-US')}`;
}

export function fmtMoneyK(v: number) {
  return v >= 1000 ? `$${(v / 1000).toFixed(v >= 10000 ? 0 : 1)}k` : `$${v}`;
}

export const BUY_LABEL: Record<BuyType, string> = { pistol: '手枪局', eco: 'ECO', force: '强起', full: '全起' };
export const BUY_SHORT: Record<BuyType, string> = { pistol: 'P', eco: 'E', force: 'F', full: 'B' };

export function reasonLabel(r: string | null | undefined) {
  if (!r) return '未知';
  return ({
    t_killed: 'T 全灭', ct_killed: 'CT 全灭', bomb_exploded: '炸弹爆炸', bomb_defused: '拆除炸弹',
    target_saved: '时间耗尽', t_surrender: 'T 投降', ct_surrender: 'CT 投降', draw: '平局',
  } as Record<string, string>)[r] ?? r;
}

/** 胜负原因 → 图标类别。 */
export function reasonKind(r: string | null | undefined): 'elim' | 'bomb' | 'defuse' | 'time' | 'other' {
  switch (r) {
    case 't_killed': case 'ct_killed': return 'elim';
    case 'bomb_exploded': return 'bomb';
    case 'bomb_defused': return 'defuse';
    case 'target_saved': return 'time';
    default: return 'other';
  }
}

const WEAPON_NAMES: Record<string, string> = {
  ak47: 'AK-47', m4a1: 'M4A4', m4a1_silencer: 'M4A1-S', awp: 'AWP', galilar: 'Galil AR', famas: 'FAMAS',
  aug: 'AUG', sg556: 'SG 553', ssg08: 'SSG 08', scar20: 'SCAR-20', g3sg1: 'G3SG1',
  mp9: 'MP9', mac10: 'MAC-10', mp7: 'MP7', mp5sd: 'MP5-SD', ump45: 'UMP-45', p90: 'P90', bizon: 'PP-Bizon',
  glock: 'Glock-18', usp_silencer: 'USP-S', hkp2000: 'P2000', p250: 'P250', deagle: 'Desert Eagle',
  fiveseven: 'Five-SeveN', tec9: 'Tec-9', cz75a: 'CZ75-Auto', elite: 'Dual Berettas', revolver: 'R8',
  nova: 'Nova', xm1014: 'XM1014', mag7: 'MAG-7', sawedoff: 'Sawed-Off', negev: 'Negev', m249: 'M249',
  hegrenade: 'HE', inferno: '燃烧', molotov: '燃烧瓶', incgrenade: '燃烧弹', flashbang: '闪光', smokegrenade: '烟雾',
  decoy: '诱饵', planted_c4: 'C4', taser: '电击枪', knife: '刀', world: '环境',
};

export function weaponLabel(w: string | null | undefined) {
  if (!w) return '—';
  // 5E 平台的皮肤武器会带前缀，例如 5e_2023pass1_ak47
  const base = w.replace(/^weapon_/, '').replace(/^5e_[a-z0-9]+_/, '');
  if (base.startsWith('knife') || base === 'bayonet') return '刀';
  return WEAPON_NAMES[base] ?? base;
}

export function sideOf(team: TeamId, sideA: Side): Side {
  return team === 'A' ? sideA : (5 - sideA) as Side;
}

export function teamLabel(team: TeamId, mine: boolean) {
  return mine ? '我方' : `${team} 队`;
}

/** steamid → 选手名、队伍。 */
export function usePlayerIndex(detail: Ref<MatchDetailDto | null>) {
  return computed(() => {
    const m = new Map<SteamId, PlayerMatchStatsDto>();
    for (const p of detail.value?.scoreboard ?? []) m.set(p.steamid, p);
    return m;
  });
}

/** 回合胜率曲线与每人每回合 Swing（W2-B）；对局未解析时返回 null。 */
export function fetchWinProbability(matchId: string) {
  return tauriInvoke<MatchWinProbabilityDto | null>('get_round_win_probability', { matchId });
}
