/**
 * 跟枪：目标像对手左右横移一样变向移动（每 0.35–1.1 秒换一次速度，带加减速），准星要一直留在目标上。
 * 不需要开枪。统计：在目标上的时间占比、平均偏离角度；按秒记录每一秒的占比。
 * 开始前有 1 秒准备时间，目标静止且准星已对准，不计入统计。
 */
import { angleBetween, rand, round, type Camera, type ModeContext, type ModeResult, type Palette, type TrainingModeImpl } from '../core';
import { CachedText, drawBackdrop, drawCrosshair, drawHud, drawOffscreenHint, drawTarget } from '../draw';

export interface TrackingOptions {
  durationS: number;
  radiusDeg: number;
  /** 最大横移角速度（度 / 秒） */
  maxSpeed: number;
}

export const TRACKING_SIZES = { L: 1.8, M: 1.3, S: 0.9 } as const;
export const TRACKING_SPEEDS = { slow: 10, medium: 16, fast: 24 } as const;

const PREP_MS = 1000;
const YAW_RANGE = 30;
const PITCH_RANGE = 6;
const ACCEL = 9; // 速度向目标速度靠拢的速率（1/秒）

export class TrackingMode implements TrainingModeImpl {
  readonly usesView = true;
  done = false;
  private cam!: Camera;
  private pal!: Palette;
  private ty = 0;
  private tp = 0;
  private vx = 0;
  private vy = 0;
  private wantVx = 0;
  private wantVy = 0;
  private changeAt = 0;
  private startAt = 0;
  private measureAt = 0;
  private endAt = 0;
  private onTime = 0;
  private total = 0;
  private errInt = 0;
  private on = false;
  private perSecOn!: Float64Array;
  private perSecTotal!: Float64Array;
  private hudL = new CachedText();
  private hudC = new CachedText();

  constructor(private opts: TrackingOptions) {}

  init(ctx: ModeContext, now: number) {
    this.cam = ctx.camera;
    this.pal = ctx.palette;
    this.done = false;
    this.ty = 0;
    this.tp = 0;
    this.vx = this.vy = this.wantVx = this.wantVy = 0;
    this.cam.lookAt(0, 0);
    this.startAt = now;
    this.measureAt = now + PREP_MS;
    this.endAt = this.measureAt + this.opts.durationS * 1000;
    this.changeAt = this.measureAt;
    this.onTime = this.total = this.errInt = 0;
    this.perSecOn = new Float64Array(this.opts.durationS);
    this.perSecTotal = new Float64Array(this.opts.durationS);
  }

  private pickVelocity(now: number) {
    const s = this.opts.maxSpeed;
    let dir = Math.sign(this.wantVx) || (Math.random() < 0.5 ? -1 : 1);
    if (Math.random() < 0.7) dir = -dir;
    // 离中心太远时总是往回走
    if (this.ty > YAW_RANGE) dir = -1;
    else if (this.ty < -YAW_RANGE) dir = 1;
    this.wantVx = dir * rand(0.4, 1) * s;
    this.wantVy = (this.tp > PITCH_RANGE ? -1 : this.tp < -PITCH_RANGE ? 1 : Math.random() < 0.5 ? -1 : 1) * rand(0, 0.3) * s;
    this.changeAt = now + rand(350, 1100);
  }

  update(now: number, dt: number) {
    if (now < this.measureAt) return;
    if (now >= this.endAt) {
      this.done = true;
      return;
    }
    if (now >= this.changeAt) this.pickVelocity(now);
    const s = dt / 1000;
    const k = 1 - Math.exp(-ACCEL * s);
    this.vx += (this.wantVx - this.vx) * k;
    this.vy += (this.wantVy - this.vy) * k;
    this.ty += this.vx * s;
    this.tp += this.vy * s;
    const err = angleBetween(this.cam.yaw, this.cam.pitch, this.ty, this.tp);
    this.on = err <= this.opts.radiusDeg;
    this.total += dt;
    this.errInt += err * dt;
    const sec = Math.min(this.opts.durationS - 1, Math.floor((now - this.measureAt) / 1000));
    this.perSecTotal[sec]! += dt;
    if (this.on) {
      this.onTime += dt;
      this.perSecOn[sec]! += dt;
    }
  }

  mouseDown() {
    /* 跟枪不需要开枪 */
  }

  draw(g: CanvasRenderingContext2D, now: number) {
    const cam = this.cam, p = this.pal;
    drawBackdrop(g, cam, p);
    const visible = drawTarget(g, cam, p, this.ty, this.tp, this.opts.radiusDeg, this.on && now >= this.measureAt);
    if (!visible) drawOffscreenHint(g, cam, p, this.ty, this.tp);
    drawCrosshair(g, cam, p);
    const prep = now < this.measureAt;
    const left = prep ? this.opts.durationS : Math.max(0, Math.ceil((this.endAt - now) / 1000));
    const pct = this.total > 0 ? Math.round((this.onTime / this.total) * 100) : 0;
    drawHud(
      g,
      cam,
      p,
      this.hudL.get(left, (k) => (prep ? '准备…' : `剩余 ${k} 秒`)),
      this.hudC.get(pct, (k) => `在目标上 ${k}%`),
      '',
    );
  }

  result(): ModeResult {
    const pct = this.total > 0 ? (this.onTime / this.total) * 100 : null;
    const trials = Array.from(this.perSecTotal, (t, i) => ({
      value: t > 0 ? round((this.perSecOn[i]! / t) * 100, 1) : null,
      ok: null,
    }));
    return {
      score: round(pct, 1),
      metrics: {
        onTarget: round(pct, 1),
        avgErrDeg: this.total > 0 ? round(this.errInt / this.total, 2) : null,
        measuredMs: Math.round(this.total),
      },
      trials,
      detail: { perSecond: trials.map((t) => t.value) },
    };
  }

  debug() {
    return { dyaw: this.ty - this.cam.yaw, dpitch: this.tp - this.cam.pitch, on: this.on, onTime: this.onTime, total: this.total };
  }
}
