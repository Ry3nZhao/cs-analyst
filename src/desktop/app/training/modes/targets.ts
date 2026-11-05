/**
 * 甩枪和微调共用的「点目标」模式。一次只有一个目标。
 *
 * - 甩枪：目标出现在当前视线 10–38° 之外的随机方向，有限时；统计命中率（命中目标数 / 目标数）和单个目标用时。
 * - 微调：目标出现在准星附近 1.2–4.5° 内，目标很小；统计精度（命中 / 开枪次数）、用时和未命中时离目标中心的距离。
 *
 * 命中判定：开枪那一刻视线和目标中心的夹角 ≤ 目标角半径。
 */
import { angleBetween, mean, median, rand, round, type Camera, type ModeContext, type ModeResult, type Palette, type TrainingModeImpl } from '../core';
import { CachedText, EffectPool, drawBackdrop, drawCrosshair, drawHud, drawOffscreenHint, drawTarget } from '../draw';
import type { TrainingTrial } from '@cs-analyst/shared-types';

export interface TargetsOptions {
  kind: 'flick' | 'micro';
  count: number;
  /** 目标角半径（度） */
  radiusDeg: number;
  /** 单个目标的时限（毫秒） */
  timeLimitMs: number;
  /** 出现位置距准星的角距离范围（度） */
  minDist: number;
  maxDist: number;
}

export const FLICK_SIZES = { L: 1.6, M: 1.1, S: 0.7 } as const;
export const MICRO_SIZES = { L: 0.6, M: 0.42, S: 0.3 } as const;

const GAP_MS = 180;
const PITCH_LIMIT = 24;

export class TargetsMode implements TrainingModeImpl {
  readonly usesView = true;
  done = false;
  private cam!: Camera;
  private pal!: Palette;
  private fx = new EffectPool(24);
  private active = false;
  private ty = 0;
  private tp = 0;
  private spawnAt = 0;
  private nextAt = 0;
  private startDist = 0;
  private targetShots = 0;
  private spawned = 0;
  private hits = 0;
  private shots = 0;
  private missDist: number[] = [];
  private hitTimes: number[] = [];
  private trials: TrainingTrial[] = [];
  private hudL = new CachedText();
  private hudC = new CachedText();
  private hudR = new CachedText();
  private startAt = 0;

  constructor(private opts: TargetsOptions) {}

  init(ctx: ModeContext, now: number) {
    this.cam = ctx.camera;
    this.pal = ctx.palette;
    this.cam.lookAt(0, 0);
    this.done = false;
    this.active = false;
    this.spawned = this.hits = this.shots = 0;
    this.missDist = [];
    this.hitTimes = [];
    this.trials = [];
    this.fx.clear();
    this.startAt = now;
    this.nextAt = now + 400;
  }

  private spawn(now: number) {
    const a = rand(0, Math.PI * 2);
    const d = rand(this.opts.minDist, this.opts.maxDist);
    // 甩枪的竖直分量压扁一些，接近游戏里的横向为主
    const vk = this.opts.kind === 'flick' ? 0.55 : 1;
    let p = this.cam.pitch + Math.sin(a) * d * vk;
    if (p > PITCH_LIMIT || p < -PITCH_LIMIT) p = this.cam.pitch - Math.sin(a) * d * vk;
    this.ty = this.cam.yaw + Math.cos(a) * d;
    this.tp = Math.max(-PITCH_LIMIT, Math.min(PITCH_LIMIT, p));
    this.startDist = angleBetween(this.cam.yaw, this.cam.pitch, this.ty, this.tp);
    this.spawnAt = now;
    this.targetShots = 0;
    this.active = true;
    this.spawned++;
  }

  private close(now: number, hit: boolean) {
    const t = now - this.spawnAt;
    if (hit) this.hitTimes.push(t);
    this.trials.push({
      value: hit ? round(t, 1) : null,
      ok: hit,
      data: { dist: round(this.startDist, 2), shots: this.targetShots },
    });
    this.active = false;
    if (this.spawned >= this.opts.count) this.done = true;
    else this.nextAt = now + GAP_MS;
  }

  update(now: number) {
    if (this.active) {
      if (now - this.spawnAt >= this.opts.timeLimitMs) this.close(now, false);
    } else if (!this.done && now >= this.nextAt) {
      this.spawn(now);
    }
  }

  mouseDown(button: number, now: number) {
    if (button !== 0) return;
    this.shots++;
    if (!this.active) {
      this.fx.add(this.cam.yaw, this.cam.pitch, now, 1);
      return;
    }
    this.targetShots++;
    const d = angleBetween(this.cam.yaw, this.cam.pitch, this.ty, this.tp);
    if (d <= this.opts.radiusDeg) {
      this.hits++;
      this.fx.add(this.ty, this.tp, now, 0);
      this.close(now, true);
    } else {
      this.missDist.push(d - this.opts.radiusDeg);
      this.fx.add(this.cam.yaw, this.cam.pitch, now, 1);
    }
  }

  draw(g: CanvasRenderingContext2D, now: number) {
    const cam = this.cam, p = this.pal;
    drawBackdrop(g, cam, p);
    if (this.active) {
      const onScreen = drawTarget(g, cam, p, this.ty, this.tp, this.opts.radiusDeg);
      if (!onScreen) drawOffscreenHint(g, cam, p, this.ty, this.tp);
      // 剩余时间：目标下方的细进度条
      const left = 1 - (now - this.spawnAt) / this.opts.timeLimitMs;
      g.fillStyle = p.border;
      g.fillRect(cam.width / 2 - 60, cam.height - 40, 120, 3);
      g.fillStyle = p.text3;
      g.fillRect(cam.width / 2 - 60, cam.height - 40, 120 * Math.max(0, left), 3);
    }
    this.fx.draw(g, cam, p, now);
    drawCrosshair(g, cam, p);
    const n = Math.min(this.spawned, this.opts.count);
    drawHud(
      g,
      cam,
      p,
      this.hudL.get(n, (k) => `目标 ${k} / ${this.opts.count}`),
      this.hudC.get(this.hits, (k) => `命中 ${k}`),
      this.hudR.get(this.shots, (k) => `开枪 ${k}`),
    );
  }

  result(): ModeResult {
    const targets = this.trials.length;
    const hitRate = targets ? (this.hits / targets) * 100 : null;
    const accuracy = this.shots ? (this.hits / this.shots) * 100 : null;
    const metrics = {
      targets,
      hits: this.hits,
      shots: this.shots,
      hitRate: round(hitRate, 1),
      accuracy: round(accuracy, 1),
      avgTime: round(mean(this.hitTimes), 0),
      medianTime: round(median(this.hitTimes), 0),
      missDist: round(mean(this.missDist), 2),
    };
    return {
      score: this.opts.kind === 'flick' ? metrics.hitRate : metrics.accuracy,
      metrics,
      trials: this.trials,
      detail: { hitTimes: this.hitTimes.slice() },
    };
  }

  debug() {
    // 目标相对视线的角度差：dyaw > 0 在右边，dpitch > 0 在上面
    return {
      active: this.active,
      dyaw: this.active ? this.ty - this.cam.yaw : null,
      dpitch: this.active ? this.tp - this.cam.pitch : null,
      radius: this.opts.radiusDeg,
      spawned: this.spawned,
      hits: this.hits,
      shots: this.shots,
      startAt: this.startAt,
    };
  }
}
