// 枪法与压枪（W2-D）前端工具：命令封装、格式化、武器名、客户端聚合。
// 数据口径见 docs/aim.md；类型见 @cs-analyst/shared-types 的 aim.ts。
import type {
  AimFilters,
  AimStats,
  BulletAgg,
  PersonSummaryDto,
  Spray,
  SprayShot,
  SpraysResult,
  WeaponPattern,
} from '@cs-analyst/shared-types';
import { tauriInvoke } from './useTauri';

/** 和 shared-types 的 SPRAY_WEAPONS 一致（shared-types 只做类型导入，没有运行时产物）。 */
export const SPRAY_WEAPONS = ['ak47', 'm4a1', 'm4a1_silencer', 'galilar', 'famas', 'mp9', 'mac10'] as const;

/** 没有「我关注的人」时的兜底 steamid。 */
export const FALLBACK_STEAMID = '76561198000000042';

/** 「开始崩」的阈值（°），和后端 `break_shot` 一致。 */
export const BREAK_DEG = 2;

export function getAimStats(steamid: string, filters?: AimFilters | null) {
  return tauriInvoke<AimStats>('get_aim_stats', { steamid, filters: filters ?? null });
}

export function getSprays(steamid: string, opts: { matchId?: string | null; weapon?: string | null } = {}) {
  return tauriInvoke<SpraysResult>('get_sprays', {
    steamid,
    matchId: opts.matchId ?? null,
    weapon: opts.weapon ?? null,
  });
}

export function listFollowedPersons() {
  return tauriInvoke<PersonSummaryDto[]>('search_persons', { followedOnly: true, limit: 50 });
}

const WEAPON_NAMES: Record<string, string> = {
  ak47: 'AK-47',
  m4a1: 'M4A4',
  m4a1_silencer: 'M4A1-S',
  galilar: 'Galil AR',
  famas: 'FAMAS',
  mp9: 'MP9',
  mac10: 'MAC-10',
  awp: 'AWP',
  ssg08: 'SSG 08',
  deagle: 'Desert Eagle',
  usp_silencer: 'USP-S',
  glock: 'Glock-18',
  hkp2000: 'P2000',
  p250: 'P250',
  tec9: 'Tec-9',
  fiveseven: 'Five-SeveN',
  cz75a: 'CZ75-Auto',
  elite: 'Dual Berettas',
  revolver: 'R8',
  aug: 'AUG',
  sg556: 'SG 553',
  mp7: 'MP7',
  mp5sd: 'MP5-SD',
  ump45: 'UMP-45',
  p90: 'P90',
  bizon: 'PP-Bizon',
  nova: 'Nova',
  xm1014: 'XM1014',
  mag7: 'MAG-7',
  sawedoff: 'Sawed-Off',
  m249: 'M249',
  negev: 'Negev',
  scar20: 'SCAR-20',
  g3sg1: 'G3SG1',
};

export function weaponName(w: string | null | undefined): string {
  if (!w) return '全部武器';
  return WEAPON_NAMES[w] ?? w;
}

export function isSprayWeapon(w: string | null | undefined): boolean {
  return !!w && (SPRAY_WEAPONS as readonly string[]).includes(w);
}

export function fmtPct(v: number | null | undefined, digits = 0): string {
  return v == null ? '—' : `${(v * 100).toFixed(digits)}`;
}

export function fmtNum(v: number | null | undefined, digits = 0): string {
  return v == null || Number.isNaN(v) ? '—' : v.toFixed(digits);
}

export function fmtDeg(v: number | null | undefined, digits = 1): string {
  return v == null ? '—' : `${v.toFixed(digits)}°`;
}

export function fmtDate(iso: string | null | undefined): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** 对局在列表里的短标签：日期 · 地图，没有日期时用 matchId。 */
export function matchLabel(m: { matchId: string; playedAt?: string | null; mapName?: string | null }): string {
  const map = (m.mapName ?? '').replace(/^de_/, '');
  const date = fmtDate(m.playedAt);
  return [date || m.matchId, map].filter(Boolean).join(' · ');
}

/** 「第 N 发」统一用的形状：BulletAgg 和 SprayShot 都满足。 */
export type BulletLike = Pick<BulletAgg, 'idx' | 'recoilV' | 'recoilH' | 'aimV' | 'aimH' | 'errV' | 'errH'> & {
  n?: number;
  err?: number;
  errMedian?: number;
};

function median(xs: number[]): number {
  if (!xs.length) return 0;
  const s = [...xs].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m]! : (s[m - 1]! + s[m]!) / 2;
}

/**
 * 客户端版的 `aim::aggregate`：用于按时间范围筛选时（`get_sprays` 没有时间筛选）。
 * 逐弹序求 recoil / aim / err 的均值、|err| 均值和中位数；「开始崩」= 第一个 |err| 中位数 > 2° 且 ≥ 3 次连发打到的弹序。
 */
export function aggregateSprays(weapon: string, sprays: Spray[]): WeaponPattern {
  const byIdx = new Map<number, SprayShot[]>();
  let shots = 0;
  let hits = 0;
  const controls: number[] = [];
  const errs: number[] = [];
  for (const s of sprays) {
    shots += s.shots.length;
    hits += s.hits;
    if (s.control != null) controls.push(s.control);
    errs.push(s.meanErr);
    for (const b of s.shots) {
      const arr = byIdx.get(b.idx) ?? [];
      arr.push(b);
      byIdx.set(b.idx, arr);
    }
  }
  const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);
  const bullets: BulletAgg[] = [...byIdx.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([idx, bs]) => {
      const abs = bs.map((b) => Math.hypot(b.errV, b.errH));
      return {
        idx,
        n: bs.length,
        recoilV: mean(bs.map((b) => b.recoilV)),
        recoilH: mean(bs.map((b) => b.recoilH)),
        aimV: mean(bs.map((b) => b.aimV)),
        aimH: mean(bs.map((b) => b.aimH)),
        errV: mean(bs.map((b) => b.errV)),
        errH: mean(bs.map((b) => b.errH)),
        err: mean(abs),
        errMedian: median(abs),
      };
    });
  const brk = bullets.find((b) => b.n >= 3 && b.errMedian > BREAK_DEG);
  return {
    weapon,
    sprays: sprays.length,
    shots,
    hits,
    control: controls.length ? mean(controls) : null,
    meanErr: errs.length ? mean(errs) : null,
    breakShot: brk?.idx ?? null,
    bullets,
  };
}

/** 偏差等级：用于给逐发偏差上色。 */
export function errTone(err: number): 'ok' | 'warn' | 'bad' {
  if (err > BREAK_DEG) return 'bad';
  if (err > BREAK_DEG / 2) return 'warn';
  return 'ok';
}

/** 垂直方向的压枪判断：errV > 0 压少了（子弹偏高），< 0 压多了。 */
export function verticalVerdict(errV: number, tol = 0.5): string {
  if (errV > tol) return '压少了';
  if (errV < -tol) return '压多了';
  return '到位';
}

export function horizontalVerdict(errH: number, tol = 0.5): string {
  if (errH > tol) return '偏右';
  if (errH < -tol) return '偏左';
  return '到位';
}

/** 指标定义（「?」里的文字），口径同 docs/aim.md、docs/roadmap.md 2.3 / 2.4。 */
export const AIM_HELP = {
  ttd: '敌人进入视野（被游戏标记为「可见」）到你第一次对他造成伤害的时间，取中位数，越短越好。只统计可见性数据可靠的对局。',
  reaction: '敌人进入视野到你第一次开枪的时间，取中位数。和 TTD 的差就是「开枪到命中」花掉的时间。',
  crosshair: '敌人进入视野那一刻，准星和「眼睛 → 敌人头部」方向的夹角，越小说明预瞄越准。拆成垂直分量（正数 = 准星在头上方）和水平分量（取绝对值）。夹角大于 90°（敌人在屏幕外）的样本不计。',
  counterStrafe: '所有首发子弹中，开枪时水平速度 ≤ 武器最大速度 34% 的比例，也就是开枪前有没有急停到位。开镜的狙击枪、AUG、SG 553 按开镜后的最大速度算。',
  visibleAccuracy: '只统计开枪时至少能看到一个敌人的射击：命中次数 / 射击次数。去掉了穿烟、架点等盲射的影响。',
  firstShot: '能看到敌人时打出的首发子弹的命中率。首发 = 同一把枪在此前 1.5 个射击间隔内没开过枪。',
  headshot: '打中头部的次数 / 命中次数（按子弹算，不是按击杀算）。',
  accuracy: '全部射击的命中率：开枪的同一个 tick 上对敌人造成了伤害就算命中，霰弹枪一枪只算一次。',
  sprayControl: '每次连发中，在后坐力 ≥ 1° 的那几发上计算「抵消掉的后坐力比例」，再取平均 × 100。和连发长度、武器无关，100 = 完全压住。',
  sprayErr: '连发第 2 发起，每发子弹偏离第一发落点的角度（°）的平均值，越小越好。',
  breakShot: '第一个「误差中位数超过 2°、且至少有 3 次连发打到这一发」的弹序。2° 大约是 25 米外一个身体的宽度。',
  bias: 'spotted 标记比真实进入视野稍晚，所以 TTD 和准星偏移会比用地图几何做射线检测的工具显得更快、更准，适合做人和人、场和场之间的相对比较。',
  unavailable: '可见性数据不可靠，无法计算',
} as const;
