/**
 * 训练引擎的基础：视角换算、投影、统计工具和模式接口。
 *
 * 角度约定和 docs/aim.md 一致，全部换成屏幕方向：yaw 向右为正、pitch 向上为正，单位为度。
 *
 * 灵敏度换算（和 CS2 一致）：
 *   每个鼠标计数转过的角度 = sensitivity × m_yaw（m_yaw = m_pitch = 0.022）
 *   画面用 CS2 的投影：竖直视场固定 73.74°（4:3 下水平 90°），水平视场随宽高比变化（Hor+），
 *   焦距 f = (H / 2) / tan(vfov / 2) = H / 1.5（像素）。
 * 所以同样的鼠标位移在这里和游戏里转过的角度相同，和训练区域的像素大小无关。
 */
import type { TrainingTrial } from '@cs-analyst/shared-types';

export const M_YAW = 0.022;
/** CS2 默认 fov_cs_debug 90（4:3 水平），对应的竖直视场 */
export const VFOV_DEG = (2 * Math.atan(Math.tan((45 * Math.PI) / 180) * 0.75) * 180) / Math.PI;
const TAN_HALF_VFOV = Math.tan((VFOV_DEG / 2) * (Math.PI / 180));
const DEG = Math.PI / 180;

export function degPerCount(sensitivity: number) {
  return sensitivity * M_YAW;
}

/** 某个宽高比下的水平视场（度） */
export function hfovDeg(width: number, height: number) {
  return (2 * Math.atan(TAN_HALF_VFOV * (width / height)) * 180) / Math.PI;
}

/** 屏幕坐标输出（预分配，避免每帧新建对象） */
export interface ScreenPoint {
  x: number;
  y: number;
  /** 投影深度，> 0 表示在前方 */
  z: number;
  /** 1° 在该点大约对应多少像素 */
  pxPerDeg: number;
}

export function screenPoint(): ScreenPoint {
  return { x: 0, y: 0, z: 0, pxPerDeg: 0 };
}

export class Camera {
  yaw = 0;
  pitch = 0;
  /** 只影响画面、不影响弹道的附加偏移（压枪时的视角抖动） */
  renderYaw = 0;
  renderPitch = 0;
  width = 1;
  height = 1;
  f = 1;
  // 当前帧的基向量
  private fx = 0; private fy = 0; private fz = 1;
  private rx = 1; private rz = 0;
  private ux = 0; private uy = 1; private uz = 0;

  setViewport(width: number, height: number) {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.f = this.height / 2 / TAN_HALF_VFOV;
  }

  get hfov() {
    return hfovDeg(this.width, this.height);
  }

  /** 按鼠标计数旋转。鼠标向下（dy > 0）视角向下。 */
  rotate(dxCounts: number, dyCounts: number, dpc: number) {
    this.yaw += dxCounts * dpc;
    this.pitch -= dyCounts * dpc;
    if (this.pitch > 89) this.pitch = 89;
    else if (this.pitch < -89) this.pitch = -89;
  }

  lookAt(yaw: number, pitch: number) {
    this.yaw = yaw;
    this.pitch = pitch;
  }

  /** 每帧开始时调用一次，计算基向量 */
  prepare() {
    const y = (this.yaw + this.renderYaw) * DEG;
    const p = (this.pitch + this.renderPitch) * DEG;
    const cy = Math.cos(y), sy = Math.sin(y), cp = Math.cos(p), sp = Math.sin(p);
    this.fx = cp * sy; this.fy = sp; this.fz = cp * cy;
    this.rx = cy; this.rz = -sy;
    this.ux = -sp * sy; this.uy = cp; this.uz = -sp * cy;
  }

  /** 把世界里的方向 (yaw, pitch) 投影到屏幕。返回 false 表示在身后。 */
  project(yaw: number, pitch: number, out: ScreenPoint): boolean {
    const y = yaw * DEG, p = pitch * DEG;
    const cp = Math.cos(p);
    const dx = cp * Math.sin(y), dy = Math.sin(p), dz = cp * Math.cos(y);
    const z = dx * this.fx + dy * this.fy + dz * this.fz;
    if (z < 0.05) {
      out.z = z;
      return false;
    }
    const x = dx * this.rx + dz * this.rz;
    const u = dx * this.ux + dy * this.uy + dz * this.uz;
    out.x = this.width / 2 + (this.f * x) / z;
    out.y = this.height / 2 - (this.f * u) / z;
    out.z = z;
    out.pxPerDeg = (this.f * DEG) / (z * z);
    return true;
  }
}

/** 两个方向之间的夹角（度），用于命中判定 */
export function angleBetween(yaw1: number, pitch1: number, yaw2: number, pitch2: number) {
  const p1 = pitch1 * DEG, p2 = pitch2 * DEG, dy = (yaw2 - yaw1) * DEG;
  const c = Math.sin(p1) * Math.sin(p2) + Math.cos(p1) * Math.cos(p2) * Math.cos(dy);
  return Math.acos(Math.min(1, Math.max(-1, c))) / DEG;
}

// ── 统计 ──

export function mean(xs: ArrayLike<number>): number | null {
  if (xs.length === 0) return null;
  let s = 0;
  for (let i = 0; i < xs.length; i++) s += xs[i]!;
  return s / xs.length;
}

export function median(xs: ArrayLike<number>): number | null {
  if (xs.length === 0) return null;
  const a = Array.from(xs).sort((p, q) => p - q);
  const m = a.length >> 1;
  return a.length % 2 ? a[m]! : (a[m - 1]! + a[m]!) / 2;
}

export function stdev(xs: ArrayLike<number>): number | null {
  const m = mean(xs);
  if (m == null || xs.length < 2) return null;
  let s = 0;
  for (let i = 0; i < xs.length; i++) s += (xs[i]! - m) ** 2;
  return Math.sqrt(s / (xs.length - 1));
}

export function round(v: number | null | undefined, digits = 1): number | null {
  if (v == null || !Number.isFinite(v)) return null;
  const k = 10 ** digits;
  return Math.round(v * k) / k;
}

export function rand(min: number, max: number) {
  return min + Math.random() * (max - min);
}

// ── 颜色：从 CSS token 读取，保持和界面一致 ──

export interface Palette {
  bg: string;
  surface: string;
  border: string;
  borderStrong: string;
  text: string;
  text2: string;
  text3: string;
  disabled: string;
  accent: string;
  accentSoft: string;
  success: string;
  danger: string;
  warning: string;
  fontSans: string;
  fontMono: string;
  /** 预先拼好的 canvas 字体，避免每帧拼字符串 */
  fontHud: string;
  fontSmall: string;
  fontBig: string;
  fontHuge: string;
}

export function readPalette(el: Element): Palette {
  const cs = getComputedStyle(el);
  const v = (name: string, fallback: string) => cs.getPropertyValue(name).trim() || fallback;
  const sans = v('--font-sans', 'Inter, sans-serif');
  const mono = v('--font-mono', 'monospace');
  return {
    bg: v('--bg', '#0a0a0b'),
    surface: v('--surface', '#18181b'),
    border: v('--border', '#2a2a2f'),
    borderStrong: v('--border-strong', '#3a3a40'),
    text: v('--text', '#f4f4f5'),
    text2: v('--text-2', '#a9a9b1'),
    text3: v('--text-3', '#7c7c86'),
    disabled: v('--text-disabled', '#56565e'),
    accent: v('--accent', '#3ccfdc'),
    accentSoft: v('--accent-soft', 'rgba(60,207,220,0.14)'),
    success: v('--success', '#4cc38a'),
    danger: v('--danger', '#f0686a'),
    warning: v('--warning', '#f0c75e'),
    fontSans: sans,
    fontMono: mono,
    fontHud: `500 13px ${sans}`,
    fontSmall: `500 12px ${sans}`,
    fontBig: `600 20px ${sans}`,
    fontHuge: `600 56px ${mono}`,
  };
}

// ── 模式接口 ──

export interface ModeContext {
  camera: Camera;
  palette: Palette;
  /** 每个鼠标计数转多少度 */
  dpc: number;
}

export interface ModeResult {
  variant?: string | null;
  score: number | null;
  metrics: Record<string, number | string | null>;
  trials: TrainingTrial[];
  /** 结果页的模式专属数据（不入库） */
  detail?: unknown;
}

/**
 * 一个训练模式。引擎负责 rAF 循环、输入、暂停和计时；`now` 是去掉暂停时间后的毫秒时间轴，
 * 鼠标和键盘事件的时间戳也换算到同一条时间轴上。
 *
 * 实现要求：`update` / `draw` 每帧调用，里面不要新建对象和数组（避免 GC 抖动）。
 */
export interface TrainingModeImpl {
  /** 是否使用视角（鼠标转动视角 + 准星）；反应测试和急停节奏不用 */
  readonly usesView: boolean;
  init(ctx: ModeContext, now: number): void;
  update(now: number, dt: number): void;
  draw(g: CanvasRenderingContext2D, now: number): void;
  /** 鼠标移动后（视角已转动）调用，供需要精确时机的模式使用 */
  onMove?(now: number): void;
  mouseDown(button: number, now: number): void;
  mouseUp?(button: number, now: number): void;
  key?(code: string, down: boolean, now: number): void;
  /** 训练是否已结束 */
  readonly done: boolean;
  result(): ModeResult;
  /** 调试 / 自动化测试用的状态快照（非每帧调用） */
  debug?(): Record<string, unknown>;
}
