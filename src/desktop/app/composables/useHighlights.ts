// W3-A 集锦：命令封装、细节文字、复制到剪贴板。类型见 @cs-analyst/shared-types 的 highlights.ts。
import { listen } from '@tauri-apps/api/event';
import type {
  HighlightFilters,
  HighlightItem,
  HighlightLaunchResult,
  HighlightList,
  HighlightNetconEvent,
  HighlightPlayback,
  HighlightSettings,
} from '@cs-analyst/shared-types';
import { tauriInvoke } from './useTauri';

export function listHighlights(filters: HighlightFilters) {
  return tauriInvoke<HighlightList>('list_highlights', { filters });
}

export function getHighlightSettings() {
  return tauriInvoke<HighlightSettings>('get_highlight_settings');
}

export function updateHighlightSettings(settings: HighlightSettings) {
  return tauriInvoke<HighlightSettings>('update_highlight_settings', { settings });
}

export function recomputeHighlights(matchIds?: string[] | null) {
  return tauriInvoke<number>('recompute_highlights', { matchIds: matchIds ?? null });
}

export function prepareHighlightPlayback(id: number) {
  return tauriInvoke<HighlightPlayback>('prepare_highlight_playback', { id });
}

export function launchHighlightInCs2(id: number) {
  return tauriInvoke<HighlightLaunchResult>('launch_highlight_in_cs2', { id });
}

export function listenHighlightNetcon(cb: (e: HighlightNetconEvent) => void) {
  let unlisten: (() => void) | null = null;
  listen<HighlightNetconEvent>('highlight_netcon', (e) => cb(e.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

/** 写剪贴板；WebView 不给 navigator.clipboard 时退回 execCommand。 */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand('copy');
    } catch {
      ok = false;
    }
    ta.remove();
    return ok;
  }
}

const WEAPON_NAMES: Record<string, string> = {
  ak47: 'AK-47', m4a1: 'M4A4', m4a1_silencer: 'M4A1-S', galilar: 'Galil', famas: 'FAMAS', aug: 'AUG', sg556: 'SG 553',
  awp: 'AWP', ssg08: 'SSG 08', scar20: 'SCAR-20', g3sg1: 'G3SG1', mp9: 'MP9', mac10: 'MAC-10', mp7: 'MP7', mp5sd: 'MP5-SD',
  ump45: 'UMP-45', p90: 'P90', bizon: 'PP-Bizon', negev: 'Negev', m249: 'M249', nova: 'Nova', xm1014: 'XM1014',
  mag7: 'MAG-7', sawedoff: 'Sawed-Off', glock: 'Glock-18', hkp2000: 'P2000', usp_silencer: 'USP-S', p250: 'P250',
  elite: 'Dual Berettas', fiveseven: 'Five-SeveN', tec9: 'Tec-9', cz75a: 'CZ75', deagle: 'Desert Eagle', revolver: 'R8',
  hegrenade: 'HE', inferno: '燃烧', molotov: '燃烧瓶', incgrenade: '燃烧弹', knife: '刀', taser: '电击枪', world: '环境',
};

function hlWeapon(w: string | null | undefined): string {
  if (!w) return '—';
  if (WEAPON_NAMES[w]) return WEAPON_NAMES[w]!;
  if (w.startsWith('knife') || w.includes('bayonet')) return '刀';
  return w;
}

/** 一条集锦的一句话描述（不含类型名）。 */
export function describeHighlight(h: HighlightItem, names: Record<string, string>): string {
  const d = h.detail ?? {};
  const who = (s: string | null | undefined) => (s ? names[s] ?? s.slice(-6) : '?');
  const pp = (v: number | undefined) => (v == null ? '' : `${v > 0 ? '+' : ''}${v.toFixed(1)}`);
  switch (h.kind) {
    case 'multi_kill': {
      const n = d.count ?? 0;
      const label = n >= 5 ? 'ACE' : `${n}K`;
      const kills = Array.isArray(d.kills) ? d.kills : [];
      const hs = kills.filter((k) => k.headshot).length;
      return `${label}，用时 ${((d.spanMs ?? 0) / 1000).toFixed(1)} 秒${hs ? `，${hs} 个爆头` : ''}`;
    }
    case 'quick_kills':
      return `${d.count} 杀只用了 ${((d.spanMs ?? 0) / 1000).toFixed(2)} 秒`;
    case 'clutch_win':
      return `1v${d.vs} 残局获胜，残局中击杀 ${typeof d.kills === 'number' ? d.kills : 0} 人`;
    case 'flick_headshot':
      return `${d.angle}° 甩枪，${d.ttdMs} ms 爆头 ${who(d.victim)}（${hlWeapon(d.weapon)}）`;
    case 'wallbang':
    case 'smoke_kill':
    case 'blind_kill':
    case 'airborne_kill':
    case 'noscope_kill':
      return `${hlWeapon(d.weapon)} 击杀 ${who(d.victim)}${d.headshot ? '（爆头）' : ''}${d.distance != null ? `，${d.distance} m` : ''}`;
    case 'swing_kill':
      return `击杀 ${who(d.victim)}，本方胜率 ${d.before}% → ${d.after}%（${pp(d.deltaPp)}）`;
    case 'key_defuse':
    case 'ninja_defuse':
      return `T 还剩 ${d.tAlive} 人时拆包，C4 剩 ${d.remainingSecs} 秒${d.killsAfterPlant ? `，下包后击杀 ${d.killsAfterPlant} 人` : ''}`;
    case 'untraded_opening_death':
      return `首个阵亡，被 ${who(d.killer)}（${hlWeapon(d.weapon)}）击杀，没有被补${d.roundLost ? '，本回合输了' : ''}`;
    case 'unused_utility_death':
      return `死亡时身上约 $${d.value} 的道具没用`;
    case 'team_flash': {
      const vs = (d.victims ?? []).map((v) => `${who(v.steamid)} ${v.duration}s`).join('、');
      return `闪到队友 ${vs}${d.enemySecs ? `（敌人合计 ${d.enemySecs}s）` : ''}`;
    }
    case 'team_damage': {
      const vs = (d.victims ?? []).map((v) => `${who(v.steamid)} ${v.damage}`).join('、');
      const tk = (d.teamKills ?? []).length;
      return `${tk ? `误杀 ${tk} 名队友；` : ''}对队友造成 ${d.damage} 伤害${vs ? `（${vs}）` : ''}`;
    }
    case 'lost_hp_duel':
      return `${d.hp} 血对 ${d.enemyHp} 血，被 ${who(d.killer)}（${hlWeapon(d.weapon)}）击杀，自己打出 ${d.dealt ?? 0} 伤害`;
    case 'whiffed_spray':
      return `${hlWeapon(d.weapon)} 连发 ${d.shots} 发全空，${d.distance} m 被 ${who(d.killer)} 击杀`;
    case 'swing_loss': {
      const what = d.event === 'team_kill' ? '误杀队友' : d.event === 'suicide' ? '自杀' : `被 ${who(d.killer)} 击杀`;
      return `${what}，本方胜率 ${d.before}% → ${d.after}%（${pp(d.deltaPp)}）`;
    }
    default:
      return '';
  }
}
