/**
 * 急停节奏：按住 A / D 横移，松开或按反方向键急停，在速度降到精准阈值以内时点左键开枪。
 *
 * 速度模型（前端模拟，64 tick 固定步长，参数取 CS2 默认值）：
 *   摩擦：speed > 0 时 drop = max(speed, sv_stopspeed) × sv_friction × dt
 *   加速：wishspeed = 武器最大速度；add = wishspeed − v·wishdir；
 *         accelspeed = min(sv_accelerate × wishspeed × dt, add)
 *   sv_friction = 5.2，sv_accelerate = 5.5，sv_stopspeed = 80。
 * AK-47（215 u/s）全速时反向急停约 5 tick（≈ 78 ms）进入阈值，只松键约 13 tick（≈ 200 ms）。
 *
 * 判定：开枪时 |v| ≤ 最大速度 × 34%（和 W1-D 急停率的阈值一致）为成功；开枪前 0.7 秒内必须
 * 跑到过最大速度的 80%，否则这一枪不计（提示先移动）。时机 = 进入阈值到开枪的时间。
 */
import { mean, median, round, type Camera, type ModeContext, type ModeResult, type Palette, type TrainingModeImpl } from '../core';
import { CachedText, drawHud } from '../draw';
import type { TrainingTrial } from '@cs-analyst/shared-types';

export const SV_FRICTION = 5.2;
export const SV_ACCELERATE = 5.5;
export const SV_STOPSPEED = 80;
export const TICK_MS = 1000 / 64;
export const ACCURATE_FRACTION = 0.34;

export const STRAFE_WEAPONS = {
  ak47: { label: 'AK-47', maxSpeed: 215 },
  m4a1: { label: 'M4A4 / M4A1-S', maxSpeed: 225 },
  famas: { label: 'FAMAS', maxSpeed: 220 },
  mp9: { label: 'MP9 / MAC-10', maxSpeed: 240 },
} as const;
export type StrafeWeapon = keyof typeof STRAFE_WEAPONS;

export interface StrafeOptions {
  count: number;
  weapon: StrafeWeapon;
}

/** 一个 tick 的速度更新（一维，向右为正）。导出给结果页 / 测试复用。 */
export function stepVelocity(v: number, wish: number, maxSpeed: number, dt = TICK_MS / 1000): number {
  const speed = Math.abs(v);
  if (speed > 0.1) {
    const control = Math.max(speed, SV_STOPSPEED);
    const drop = control * SV_FRICTION * dt;
    const ns = Math.max(0, speed - drop);
    v *= ns / speed;
  } else {
    v = 0;
  }
  if (wish !== 0) {
    const add = maxSpeed - v * wish;
    if (add > 0) v += Math.min(SV_ACCELERATE * maxSpeed * dt, add) * wish;
  }
  return v;
}

const TRACE_TICKS = 64 * 3;
const COOLDOWN_MS = 250;
const MOVE_LOOKBACK_TICKS = 45; // 0.7 秒
const MAX_MARKS = 16;

type Verdict = 'none' | 'ok' | 'moving' | 'idle';

export class StrafeMode implements TrainingModeImpl {
  readonly usesView = false;
  done = false;
  private cam!: Camera;
  private pal!: Palette;
  private maxSpeed = 215;
  private thr = 73;
  private v = 0;
  private simT = 0;
  private tick = 0;
  private keyA = false;
  private keyD = false;
  private windowAt = NaN;
  private lastShotAt = -1e9;
  private trace = new Float32Array(TRACE_TICKS);
  /** 开枪标记：tick 序号 + 是否成功 */
  private markTick = new Float64Array(MAX_MARKS).fill(-1e9);
  private markOk = new Uint8Array(MAX_MARKS);
  private markHead = 0;
  private verdict: Verdict = 'none';
  private verdictSpeed = 0;
  private verdictTiming = 0;
  private counted = 0;
  private success = 0;
  private timings: number[] = [];
  private speeds: number[] = [];
  private trials: TrainingTrial[] = [];
  private hudL = new CachedText();
  private hudC = new CachedText();
  private speedText = new CachedText();
  private verdictText = new CachedText();

  constructor(private opts: StrafeOptions) {}

  init(ctx: ModeContext, now: number) {
    this.cam = ctx.camera;
    this.pal = ctx.palette;
    this.maxSpeed = STRAFE_WEAPONS[this.opts.weapon].maxSpeed;
    this.thr = this.maxSpeed * ACCURATE_FRACTION;
    this.done = false;
    this.v = 0;
    this.simT = now;
    this.tick = 0;
    this.keyA = this.keyD = false;
    this.windowAt = NaN;
    this.trace.fill(0);
    this.markTick.fill(-1e9);
    this.verdict = 'none';
    this.counted = this.success = 0;
    this.timings = [];
    this.speeds = [];
    this.trials = [];
  }

  private advance(now: number) {
    while (this.simT + TICK_MS <= now) {
      this.simT += TICK_MS;
      const wish = (this.keyD ? 1 : 0) - (this.keyA ? 1 : 0);
      this.v = stepVelocity(this.v, wish, this.maxSpeed);
      const inWindow = Math.abs(this.v) <= this.thr;
      if (inWindow && Number.isNaN(this.windowAt)) this.windowAt = this.simT;
      else if (!inWindow) this.windowAt = NaN;
      this.trace[this.tick % TRACE_TICKS] = this.v;
      this.tick++;
    }
  }

  private recentPeak() {
    let peak = 0;
    const n = Math.min(this.tick, MOVE_LOOKBACK_TICKS);
    for (let i = 1; i <= n; i++) {
      const s = Math.abs(this.trace[(this.tick - i) % TRACE_TICKS]!);
      if (s > peak) peak = s;
    }
    return peak;
  }

  update(now: number) {
    this.advance(now);
  }

  key(code: string, down: boolean, now: number) {
    if (code !== 'KeyA' && code !== 'KeyD') return;
    this.advance(now);
    if (code === 'KeyA') this.keyA = down;
    else this.keyD = down;
  }

  mouseDown(button: number, now: number) {
    if (button !== 0 || now - this.lastShotAt < COOLDOWN_MS) return;
    this.advance(now);
    this.lastShotAt = now;
    const speed = Math.abs(this.v);
    if (this.recentPeak() < this.maxSpeed * 0.8) {
      this.verdict = 'idle';
      return;
    }
    const ok = speed <= this.thr;
    const timing = ok && !Number.isNaN(this.windowAt) ? now - this.windowAt : 0;
    this.verdict = ok ? 'ok' : 'moving';
    this.verdictSpeed = speed;
    this.verdictTiming = timing;
    this.counted++;
    if (ok) {
      this.success++;
      this.timings.push(timing);
    }
    this.speeds.push(speed);
    this.trials.push({ value: ok ? round(timing, 1) : null, ok, data: { speed: round(speed, 1) } });
    this.markTick[this.markHead] = this.tick;
    this.markOk[this.markHead] = ok ? 1 : 0;
    this.markHead = (this.markHead + 1) % MAX_MARKS;
    if (this.counted >= this.opts.count) this.done = true;
  }

  draw(g: CanvasRenderingContext2D) {
    const cam = this.cam, p = this.pal;
    const W = cam.width, H = cam.height;
    g.fillStyle = p.bg;
    g.fillRect(0, 0, W, H);
    drawHud(
      g,
      cam,
      p,
      this.hudL.get(this.counted, (k) => `第 ${Math.min(k + 1, this.opts.count)} / ${this.opts.count} 枪`),
      this.hudC.get(this.success, (k) => `成功 ${k}`),
      STRAFE_WEAPONS[this.opts.weapon].label,
    );

    // ── 速度表 ──
    const gw = Math.min(720, W * 0.7);
    const gx = (W - gw) / 2;
    const gy = H * 0.3;
    const sx = (v: number) => gx + ((v + this.maxSpeed) / (2 * this.maxSpeed)) * gw;
    g.fillStyle = p.surface;
    g.fillRect(gx, gy - 14, gw, 28);
    g.fillStyle = p.accentSoft;
    g.fillRect(sx(-this.thr), gy - 14, sx(this.thr) - sx(-this.thr), 28);
    g.strokeStyle = p.accent;
    g.lineWidth = 1;
    g.beginPath();
    g.moveTo(sx(-this.thr) + 0.5, gy - 14); g.lineTo(sx(-this.thr) + 0.5, gy + 14);
    g.moveTo(sx(this.thr) - 0.5, gy - 14); g.lineTo(sx(this.thr) - 0.5, gy + 14);
    g.stroke();
    const nx = sx(Math.max(-this.maxSpeed, Math.min(this.maxSpeed, this.v)));
    g.fillStyle = p.text;
    g.fillRect(nx - 1.5, gy - 22, 3, 44);
    g.font = p.fontSmall;
    g.textBaseline = 'top';
    g.fillStyle = p.text3;
    g.textAlign = 'left';
    g.fillText(`← A  ${this.maxSpeed}`, gx, gy + 22);
    g.textAlign = 'right';
    g.fillText(`${this.maxSpeed}  D →`, gx + gw, gy + 22);
    g.textAlign = 'center';
    g.fillText(`精准阈值 ±${Math.round(this.thr)} u/s`, W / 2, gy + 22);
    g.font = p.fontHuge;
    g.textBaseline = 'bottom';
    g.fillStyle = Math.abs(this.v) <= this.thr ? p.text : p.text2;
    g.fillText(this.speedText.get(Math.round(Math.abs(this.v)), (k) => `${k}`), W / 2, gy - 30);

    // ── 速度曲线（最近 3 秒） ──
    const ty = H * 0.3 + 70, th = Math.min(180, H * 0.28);
    const mid = ty + th / 2;
    const k = th / 2 / this.maxSpeed;
    g.fillStyle = p.surface;
    g.fillRect(gx, ty, gw, th);
    g.fillStyle = p.accentSoft;
    g.fillRect(gx, mid - this.thr * k, gw, 2 * this.thr * k);
    g.strokeStyle = p.text2;
    g.lineWidth = 1.5;
    g.beginPath();
    const n = Math.min(this.tick, TRACE_TICKS);
    for (let i = 0; i < n; i++) {
      const t = this.tick - n + i;
      const x = gx + gw - (n - i) * (gw / TRACE_TICKS);
      const y = mid - this.trace[t % TRACE_TICKS]! * k;
      if (i === 0) g.moveTo(x, y);
      else g.lineTo(x, y);
    }
    g.stroke();
    for (let i = 0; i < MAX_MARKS; i++) {
      const age = this.tick - this.markTick[i]!;
      if (age < 0 || age >= TRACE_TICKS) continue;
      const x = gx + gw - age * (gw / TRACE_TICKS);
      g.fillStyle = this.markOk[i] ? p.success : p.danger;
      g.fillRect(x - 1, ty, 2, th);
    }

    // ── 按键 ──
    const ky = ty + th + 28;
    this.drawKey(g, W / 2 - 46, ky, 'A', this.keyA);
    this.drawKey(g, W / 2 + 10, ky, 'D', this.keyD);

    // ── 上一枪 ──
    g.font = p.fontBig;
    g.textAlign = 'center';
    g.textBaseline = 'top';
    const vy = ky + 56;
    if (this.verdict === 'ok') {
      g.fillStyle = p.success;
      g.fillText(this.verdictText.get(1e6 + Math.round(this.verdictTiming), () => `急停成功 · 进入阈值后 ${Math.round(this.verdictTiming)} ms 开枪`), W / 2, vy);
    } else if (this.verdict === 'moving') {
      g.fillStyle = p.danger;
      g.fillText(this.verdictText.get(2e6 + Math.round(this.verdictSpeed), () => `移动中开枪 · ${Math.round(this.verdictSpeed)} u/s`), W / 2, vy);
    } else if (this.verdict === 'idle') {
      g.fillStyle = p.text3;
      g.fillText('先跑起来再急停（这一枪不计）', W / 2, vy);
    } else {
      g.fillStyle = p.text3;
      g.fillText('按住 A 或 D 跑起来，反向急停，进入阈值时点左键', W / 2, vy);
    }
    g.textAlign = 'left';
  }

  private drawKey(g: CanvasRenderingContext2D, x: number, y: number, label: string, down: boolean) {
    const p = this.pal;
    g.fillStyle = down ? p.text : p.surface;
    g.fillRect(x, y, 36, 36);
    g.strokeStyle = p.border;
    g.strokeRect(x + 0.5, y + 0.5, 35, 35);
    g.font = p.fontBig;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    g.fillStyle = down ? p.bg : p.text2;
    g.fillText(label, x + 18, y + 19);
  }

  result(): ModeResult {
    const rate = this.counted ? (this.success / this.counted) * 100 : null;
    return {
      variant: null,
      score: round(rate, 1),
      metrics: {
        successRate: round(rate, 1),
        shots: this.counted,
        success: this.success,
        avgTiming: round(mean(this.timings), 0),
        medianTiming: round(median(this.timings), 0),
        avgSpeed: round(mean(this.speeds), 0),
        threshold: Math.round(this.thr),
        maxSpeed: this.maxSpeed,
      },
      trials: this.trials,
      detail: { timings: this.timings.slice(), speeds: this.speeds.slice() },
    };
  }

  debug() {
    return { v: this.v, keyA: this.keyA, keyD: this.keyD, counted: this.counted, success: this.success, verdict: this.verdict };
  }
}
