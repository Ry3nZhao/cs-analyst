// 比赛详情页（W2-A）用到的格式化和「我」的判定。
import type { BuyType, MatchDetailDto, PersonSummaryDto, PlayerMatchStatsDto, Side, SteamId, TeamId } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

/** 还没有「主玩家」设置时的兜底 steamid（用户本人）。 */
const FALLBACK_ME: SteamId = '76561198000000042';

const mySteamIds = ref<Set<SteamId>>(new Set([FALLBACK_ME]));
let meLoaded = false;

/**
 * 「我」= 「我关注的人」里的第一个（W1-C）。第三波做主玩家设置后改这里即可。
 * 查询失败或没有关注的人时用兜底 steamid。
 */
export function useMySteamIds() {
  if (!meLoaded) {
    meLoaded = true;
    tauriInvoke<PersonSummaryDto[]>('search_persons', { followedOnly: true, limit: 1 })
      .then((list) => {
        const ids = list[0]?.steamids ?? [];
        if (ids.length > 0) mySteamIds.value = new Set([...ids, FALLBACK_ME]);
      })
      .catch(() => { /* 保留兜底 */ });
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
