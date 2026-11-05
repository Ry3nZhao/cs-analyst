// 2D 回放 / 热力图的纯逻辑：解码 get_replay 的二进制、加载雷达底图、世界坐标 → 画布坐标、密度图。
import { invoke } from '@tauri-apps/api/core';
import type { RadarCalibration, RadarInfo, ReplayHeader } from '@cs-analyst/shared-types';
import { REPLAY_PACK_VERSION } from '@cs-analyst/shared-types';

export interface ReplayData {
  header: ReplayHeader;
  ticks: Int32Array;
  x: Float32Array;
  y: Float32Array;
  z: Float32Array;
  yaw: Float32Array;
  health: Uint8Array;
  armor: Uint8Array;
  flags: Uint8Array;
  spottedBy: Uint16Array;
  byteLength: number;
}

/** 解析 `get_replay` 返回的 ArrayBuffer（布局见 shared-types/replay.ts），各列零拷贝。 */
export function decodeReplay(buf: ArrayBuffer): ReplayData {
  const dv = new DataView(buf);
  const magic = String.fromCharCode(dv.getUint8(0), dv.getUint8(1), dv.getUint8(2), dv.getUint8(3));
  if (magic !== 'CSRP') throw new Error('回放数据格式不对');
  const version = dv.getUint32(4, true);
  if (version !== REPLAY_PACK_VERSION) throw new Error(`回放数据版本 ${version} 不支持`);
  const hl = dv.getUint32(8, true);
  const header = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 12, hl))) as ReplayHeader;
  const base = 12 + hl;
  const c = header.columns;
  const n = header.nFrames * header.nPlayers;
  return {
    header,
    ticks: new Int32Array(buf, base + c.ticks, header.nFrames),
    x: new Float32Array(buf, base + c.x, n),
    y: new Float32Array(buf, base + c.y, n),
    z: new Float32Array(buf, base + c.z, n),
    yaw: new Float32Array(buf, base + c.yaw, n),
    health: new Uint8Array(buf, base + c.health, n),
    armor: new Uint8Array(buf, base + c.armor, n),
    flags: new Uint8Array(buf, base + c.flags, n),
    spottedBy: new Uint16Array(buf, base + c.spottedBy, n),
    byteLength: buf.byteLength,
  };
}

export async function loadReplay(matchId: string): Promise<ReplayData> {
  const buf = await invoke<ArrayBuffer>('get_replay', { matchId });
  return decodeReplay(buf);
}

/** 最后一个 tick ≤ t 的帧号（二分）。 */
export function frameAt(ticks: Int32Array, t: number): number {
  let lo = 0;
  let hi = ticks.length - 1;
  if (hi < 0) return 0;
  if (t <= ticks[0]!) return 0;
  if (t >= ticks[hi]!) return hi;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (ticks[mid]! <= t) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

// ---- 雷达底图

export interface Radar {
  info: RadarInfo | null;
  images: { default?: ImageBitmap; lower?: ImageBitmap };
}

export async function loadRadar(mapName: string | null | undefined): Promise<Radar> {
  if (!mapName) return { info: null, images: {} };
  let info: RadarInfo | null = null;
  try {
    info = await invoke<RadarInfo>('get_radar', { mapName });
  } catch {
    return { info: null, images: {} };
  }
  const images: Radar['images'] = {};
  for (const l of info.levels) {
    try {
      const buf = await invoke<ArrayBuffer>('get_radar_image', { mapName, level: l.level });
      images[l.level] = await createImageBitmap(new Blob([buf]));
    } catch {
      /* 没有图就不画 */
    }
  }
  return { info, images };
}

// ---- 坐标

/** 世界坐标 → 底图像素（0..1024）。没有校准参数时用数据边界拟合。 */
export interface Projection {
  calibration: RadarCalibration;
  /** 底图边长（像素）。 */
  size: number;
  fitted: boolean;
}

export const RADAR_SIZE = 1024;

export function projectionFromCalibration(c: RadarCalibration): Projection {
  return { calibration: c, size: RADAR_SIZE, fitted: false };
}

/** 用点集边界拟合一个正方形区域（留 6% 边距），换算成与官方相同形式的参数。 */
export function fitProjection(xs: ArrayLike<number>, ys: ArrayLike<number>, mask?: (i: number) => boolean): Projection {
  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;
  for (let i = 0; i < xs.length; i++) {
    if (mask && !mask(i)) continue;
    const x = xs[i]!;
    const y = ys[i]!;
    if (!Number.isFinite(x) || !Number.isFinite(y) || (x === 0 && y === 0)) continue;
    if (x < minX) minX = x;
    if (x > maxX) maxX = x;
    if (y < minY) minY = y;
    if (y > maxY) maxY = y;
  }
  if (!Number.isFinite(minX)) {
    minX = -2048;
    maxX = 2048;
    minY = -2048;
    maxY = 2048;
  }
  const span = Math.max(maxX - minX, maxY - minY, 256) * 1.12;
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  return {
    calibration: { posX: cx - span / 2, posY: cy + span / 2, scale: span / RADAR_SIZE, lowerMaxZ: null },
    size: RADAR_SIZE,
    fitted: true,
  };
}

export function worldToRadar(p: Projection, x: number, y: number): [number, number] {
  const c = p.calibration;
  return [(x - c.posX) / c.scale, (c.posY - y) / c.scale];
}

export function isLower(p: Projection, z: number): boolean {
  const m = p.calibration.lowerMaxZ;
  return m != null && z < m;
}

// ---- 颜色

export function cssVar(name: string, fallback = '#888'): string {
  if (typeof document === 'undefined') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

export function withAlpha(color: string, a: number): string {
  return `color-mix(in srgb, ${color} ${Math.round(a * 100)}%, transparent)`;
}

// ---- 文案

export const WEAPON_LABEL: Record<string, string> = {
  ak47: 'AK-47', m4a1: 'M4A4', m4a1_silencer: 'M4A1-S', awp: 'AWP', deagle: 'Deagle', galilar: 'Galil', famas: 'FAMAS',
  ssg08: 'SSG 08', usp_silencer: 'USP-S', glock: 'Glock', p250: 'P250', tec9: 'Tec-9', fiveseven: 'Five-SeveN', cz75a: 'CZ75',
  hkp2000: 'P2000', elite: 'Dual Berettas', revolver: 'R8', mac10: 'MAC-10', mp9: 'MP9', mp7: 'MP7', mp5sd: 'MP5-SD',
  ump45: 'UMP-45', p90: 'P90', bizon: 'PP-Bizon', nova: 'Nova', xm1014: 'XM1014', mag7: 'MAG-7', sawedoff: 'Sawed-Off',
  m249: 'M249', negev: 'Negev', aug: 'AUG', sg556: 'SG 553', scar20: 'SCAR-20', g3sg1: 'G3SG1',
  hegrenade: 'HE', inferno: '燃烧', molotov: '燃烧', incgrenade: '燃烧', knife: '刀', knife_t: '刀', taser: '电击枪',
  world: '环境', planted_c4: 'C4',
};

export function weaponLabel(w: string): string {
  // 5E 平台的皮肤武器带前缀，例如 5e_2023pass1_ak47
  const k = w.replace(/^weapon_/, '').replace(/^5e_[a-z0-9]+_/, '');
  if (k.startsWith('knife') || k.startsWith('bayonet')) return '刀';
  return WEAPON_LABEL[k] ?? k;
}

export const GRENADE_LABEL: Record<string, string> = {
  flashbang: '闪光', hegrenade: '手雷', smokegrenade: '烟雾', inferno: '燃烧', decoy: '诱饵',
};

export const ROUND_REASON: Record<string, string> = {
  t_killed: '全歼 T', ct_killed: '全歼 CT', bomb_exploded: '炸弹爆炸', bomb_defused: '拆除炸弹', target_saved: '时间耗尽',
  t_surrender: 'T 投降', ct_surrender: 'CT 投降', hostages_rescued: '人质获救', hostages_not_rescued: '人质未救',
  draw: '平局',
};

export function fmtClock(sec: number): string {
  const s = Math.max(0, Math.floor(sec));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

// ---- 热力密度

/**
 * 把点（底图像素坐标）累加到 grid×grid 的网格并做可分离高斯模糊，返回归一化到 0..1 的密度
 * （按 99 分位截断，避免一个热点把其余压成全黑）。
 */
export function densityGrid(px: ArrayLike<number>, py: ArrayLike<number>, grid: number, size: number, sigmaCells: number): Float32Array {
  const g = new Float32Array(grid * grid);
  const k = grid / size;
  for (let i = 0; i < px.length; i++) {
    const gx = Math.floor(px[i]! * k);
    const gy = Math.floor(py[i]! * k);
    if (gx < 0 || gy < 0 || gx >= grid || gy >= grid) continue;
    g[gy * grid + gx]! += 1;
  }
  const r = Math.max(1, Math.ceil(sigmaCells * 3));
  const kernel = new Float32Array(2 * r + 1);
  for (let i = -r; i <= r; i++) kernel[i + r] = Math.exp(-(i * i) / (2 * sigmaCells * sigmaCells));
  const tmp = new Float32Array(grid * grid);
  for (let y = 0; y < grid; y++) {
    for (let x = 0; x < grid; x++) {
      let s = 0;
      for (let i = -r; i <= r; i++) {
        const xx = x + i;
        if (xx >= 0 && xx < grid) s += g[y * grid + xx]! * kernel[i + r]!;
      }
      tmp[y * grid + x] = s;
    }
  }
  for (let y = 0; y < grid; y++) {
    for (let x = 0; x < grid; x++) {
      let s = 0;
      for (let i = -r; i <= r; i++) {
        const yy = y + i;
        if (yy >= 0 && yy < grid) s += tmp[yy * grid + x]! * kernel[i + r]!;
      }
      g[y * grid + x] = s;
    }
  }
  const nz = Array.from(g).filter((v) => v > 1e-6).sort((a, b) => a - b);
  // 单个点的峰值约为 1；至少约 3 个点叠在一起才到顶色，避免点少时每个孤点都被染成强调色。
  const p99 = nz.length ? nz[Math.min(nz.length - 1, Math.floor(nz.length * 0.99))]! : 1;
  const top = Math.max(p99, 2.5);
  for (let i = 0; i < g.length; i++) g[i] = Math.min(1, g[i]! / (top || 1));
  return g;
}

function hexToRgb(c: string): [number, number, number] {
  const m = c.trim().match(/^#?([0-9a-f]{6})$/i);
  if (!m) return [60, 207, 220];
  const n = parseInt(m[1]!, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** 密度 → RGBA：低密度是半透明灰白，越密越亮，顶端过渡到强调色。 */
export function densityImage(g: Float32Array, grid: number, accent: string): ImageData {
  const img = new ImageData(grid, grid);
  const [ar, ag, ab] = hexToRgb(accent);
  for (let i = 0; i < g.length; i++) {
    const v = g[i]!;
    if (v < 0.02) continue;
    const t = Math.max(0, (v - 0.55) / 0.45); // 0..1 in the hot range
    const grey = 120 + 135 * Math.min(1, v / 0.55);
    img.data[i * 4] = grey * (1 - t) + ar * t;
    img.data[i * 4 + 1] = grey * (1 - t) + ag * t;
    img.data[i * 4 + 2] = grey * (1 - t) + ab * t;
    img.data[i * 4 + 3] = Math.round(255 * Math.min(0.9, 0.12 + 0.8 * Math.pow(v, 0.7)));
  }
  return img;
}
