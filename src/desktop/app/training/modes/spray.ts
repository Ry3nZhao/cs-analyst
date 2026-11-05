/**
 * 压枪练习：准星对准目标，按住左键连发，把准星沿弹道反方向往回拉，让子弹留在第一发的位置。
 *
 * 弹道来自 spray-patterns.json（W1-D 从样例 demo 统计的 recoil = −2 × Δpunch，按弹序取中位数）。
 * 误差算法和 W1-D（docs/aim.md）一致，以第一发为原点，屏幕方向 v 向上、h 向右：
 *   实际弹道 = 视角 + recoil
 *   aim  = Δ视角（玩家把准星拉到了哪）
 *   err  = recoil + aim（err_v > 0 表示压少了）
 *   control = 在 |recoil| ≥ 1° 的弹序上 clamp(1 − |err| / |recoil|, 0, 1) 的平均 × 100
 *   mean_err = 第 2–n 发 |err| 的平均；break_shot = 第一个 |err| > 2° 的弹序
 * 画面上的视角抖动按 view_recoil_tracking 0.45 模拟（相机上抬 recoil × 0.225），只影响画面不影响弹道。
 */
import patterns from '../spray-patterns.json';
import { mean, median, round, screenPoint, type Camera, type ModeContext, type ModeResult, type Palette, type TrainingModeImpl } from '../core';
import { CachedText, drawBackdrop, drawCrosshair, drawHud } from '../draw';
import type { TrainingTrial } from '@cs-analyst/shared-types';

export interface SprayPattern {
  label: string;
  cycleMs: number;
  magazine: number;
  sprays: number;
  samples: number[];
  shots: [number, number][];
}

export const SPRAY_PATTERNS = (patterns as unknown as { weapons: Record<string, SprayPattern> }).weapons;
export const SPRAY_SOURCE = patterns as unknown as { source: string; generatedAt: string; matches: number; minSamples: number };
export type SprayWeapon = keyof typeof SPRAY_PATTERNS & string;

export interface SprayOptions {
  weapon: string;
  sprays: number;
}

export interface SprayStats {
  meanErr: number | null;
  control: number | null;
  breakShot: number | null;
}

const MIN_SHOTS = 4;
const BREAK_DEG = 2;
const VIEW_TRACKING = 0.225;
const REVIEW_MS = 1600;
const TARGET_R = 0.35;

/** 一次连发的汇总（和 crates/aim/src/spray.rs 一致） */
export function sprayStats(errV: ArrayLike<number>, errH: ArrayLike<number>, recV: ArrayLike<number>, recH: ArrayLike<number>, n: number): SprayStats {
  let errSum = 0;
  let ctlSum = 0;
  let ctlN = 0;
  let breakShot: number | null = null;
  for (let i = 0; i < n; i++) {
    const e = Math.hypot(errV[i]!, errH[i]!);
    const r = Math.hypot(recV[i]!, recH[i]!);
    if (i > 0) errSum += e;
    if (r >= 1) {
      ctlSum += Math.min(1, Math.max(0, 1 - e / r));
      ctlN++;
    }
    if (breakShot == null && e > BREAK_DEG) breakShot = i + 1;
  }
  return {
    meanErr: n > 1 ? errSum / (n - 1) : null,
    control: ctlN ? (ctlSum / ctlN) * 100 : null,
    breakShot,
  };
}

type Phase = 'ready' | 'firing' | 'review';

export class SprayMode implements TrainingModeImpl {
  readonly usesView = true;
  done = false;
  private cam!: Camera;
  private pal!: Palette;
  private pat: SprayPattern;
  private len: number;
  private phase: Phase = 'ready';
  private t0 = 0;
  private nextShot = 0;
  private n = 0;
  private yaw0 = 0;
  private pitch0 = 0;
  private reviewUntil = 0;
  private reviewMsg = '';
  private renderK = 0;
  // 当前连发（预分配）
  private bY: Float64Array;
  private bP: Float64Array;
  private eV: Float64Array;
  private eH: Float64Array;
  private rV: Float64Array;
  private rH: Float64Array;
  // 累计
  private sprays: SprayStats[] = [];
  private shotErrSum: Float64Array;
  private shotErrN: Float64Array;
  private bulletsV: number[] = [];
  private bulletsH: number[] = [];
  private trials: TrainingTrial[] = [];
  private pt = screenPoint();
  private hudL = new CachedText();
  private hudC = new CachedText();

  constructor(private opts: SprayOptions) {
    const pat = SPRAY_PATTERNS[opts.weapon];
    if (!pat) throw new Error(`没有 ${opts.weapon} 的弹道数据`);
    this.pat = pat;
    this.len = Math.min(pat.shots.length, pat.magazine);
    this.bY = new Float64Array(this.len);
    this.bP = new Float64Array(this.len);
    this.eV = new Float64Array(this.len);
    this.eH = new Float64Array(this.len);
    this.rV = new Float64Array(this.len);
    this.rH = new Float64Array(this.len);
    for (let i = 0; i < this.len; i++) {
      this.rV[i] = pat.shots[i]![0];
      this.rH[i] = pat.shots[i]![1];
    }
    this.shotErrSum = new Float64Array(this.len);
    this.shotErrN = new Float64Array(this.len);
  }

  init(ctx: ModeContext, now: number) {
    this.cam = ctx.camera;
    this.pal = ctx.palette;
    this.done = false;
    this.sprays = [];
    this.trials = [];
    this.bulletsV = [];
    this.bulletsH = [];
    this.shotErrSum.fill(0);
    this.shotErrN.fill(0);
    this.ready(now);
  }

  private ready(_now: number) {
    this.phase = 'ready';
    this.n = 0;
    this.cam.lookAt(0, 0);
  }

  private fire(t: number) {
    const i = this.n;
    if (i === 0) {
      this.yaw0 = this.cam.yaw;
      this.pitch0 = this.cam.pitch;
    }
    const aimH = this.cam.yaw - this.yaw0;
    const aimV = this.cam.pitch - this.pitch0;
    this.eV[i] = this.rV[i]! + aimV;
    this.eH[i] = this.rH[i]! + aimH;
    this.bY[i] = this.cam.yaw + this.rH[i]!;
    this.bP[i] = this.cam.pitch + this.rV[i]!;
    this.n++;
    this.renderK = 1;
    this.nextShot = t + this.pat.cycleMs;
    if (this.n >= this.len) this.endSpray(t);
  }

  private pump(now: number) {
    while (this.phase === 'firing' && this.nextShot <= now) this.fire(this.nextShot);
  }

  private endSpray(now: number) {
    if (this.phase !== 'firing') return;
    this.phase = 'review';
    this.reviewUntil = now + REVIEW_MS;
    const n = this.n;
    if (n < MIN_SHOTS) {
      this.reviewMsg = `连发太短（至少 ${MIN_SHOTS} 发），这次不计`;
      return;
    }
    const s = sprayStats(this.eV, this.eH, this.rV, this.rH, n);
    const idx = this.sprays.length;
    this.sprays.push(s);
    for (let i = 0; i < n; i++) {
      const e = Math.hypot(this.eV[i]!, this.eH[i]!);
      this.shotErrSum[i]! += e;
      this.shotErrN[i]! += 1;
      this.bulletsV.push(round(this.eV[i]!, 3)!);
      this.bulletsH.push(round(this.eH[i]!, 3)!);
      this.trials.push({
        value: round(e, 3),
        ok: e <= BREAK_DEG,
        data: { spray: idx, shot: i + 1, errV: round(this.eV[i]!, 3), errH: round(this.eH[i]!, 3), recoilV: this.rV[i], recoilH: this.rH[i] },
      });
    }
    const ctl = s.control != null ? `控枪 ${Math.round(s.control)}%` : '控枪 —';
    const err = s.meanErr != null ? `平均误差 ${s.meanErr.toFixed(2)}°` : '';
    const brk = s.breakShot != null ? `第 ${s.breakShot} 发开始偏出 2°` : '全程在 2° 以内';
    this.reviewMsg = `${ctl} · ${err} · ${brk}`;
    if (this.sprays.length >= this.opts.sprays) this.reviewUntil = now + 900;
  }

  update(now: number, dt: number) {
    this.pump(now);
    if (this.phase === 'review' && now >= this.reviewUntil) {
      if (this.sprays.length >= this.opts.sprays) this.done = true;
      else this.ready(now);
    }
    // 视角抖动：开枪时跟随当前 recoil，停火后回落
    if (this.phase !== 'firing') this.renderK *= Math.exp(-dt / 90);
    const last = Math.max(0, this.n - 1);
    this.cam.renderPitch = this.n ? this.rV[last]! * VIEW_TRACKING * this.renderK : 0;
    this.cam.renderYaw = this.n ? this.rH[last]! * VIEW_TRACKING * this.renderK : 0;
  }

  onMove(now: number) {
    this.pump(now);
  }

  mouseDown(button: number, now: number) {
    if (button !== 0) return;
    if (this.phase === 'ready') {
      this.phase = 'firing';
      this.n = 0;
      this.fire(now);
    } else if (this.phase === 'review' && now > this.reviewUntil - REVIEW_MS + 300 && this.sprays.length < this.opts.sprays) {
      this.ready(now);
    }
  }

  mouseUp(button: number, now: number) {
    if (button !== 0 || this.phase !== 'firing') return;
    this.pump(now);
    this.endSpray(now);
  }

  draw(g: CanvasRenderingContext2D) {
    const cam = this.cam, p = this.pal, pt = this.pt;
    drawBackdrop(g, cam, p);
    // 目标和 2° 容差圈
    if (cam.project(0, 0, pt)) {
      g.strokeStyle = p.border;
      g.lineWidth = 1;
      g.beginPath();
      g.arc(pt.x, pt.y, BREAK_DEG * pt.pxPerDeg, 0, Math.PI * 2);
      g.stroke();
      g.fillStyle = p.text2;
      g.beginPath();
      g.arc(pt.x, pt.y, Math.max(3, TARGET_R * pt.pxPerDeg), 0, Math.PI * 2);
      g.fill();
    }
    if (this.phase === 'review' && this.n >= 1) {
      // 不压枪时的弹道（灰色连线）
      g.strokeStyle = p.text3;
      g.lineWidth = 1;
      g.setLineDash(LINE_DASH);
      g.beginPath();
      for (let i = 0; i < this.n; i++) {
        if (!cam.project(this.bY[0]! + this.rH[i]!, this.bP[0]! + this.rV[i]!, pt)) continue;
        if (i === 0) g.moveTo(pt.x, pt.y);
        else g.lineTo(pt.x, pt.y);
      }
      g.stroke();
      g.setLineDash(NO_DASH);
    }
    // 弹孔
    for (let i = 0; i < this.n; i++) {
      if (!cam.project(this.bY[i]!, this.bP[i]!, pt)) continue;
      const bad = Math.hypot(this.eV[i]!, this.eH[i]!) > BREAK_DEG;
      g.fillStyle = this.phase === 'review' && bad ? p.danger : p.text;
      g.beginPath();
      g.arc(pt.x, pt.y, 3, 0, Math.PI * 2);
      g.fill();
    }
    drawCrosshair(g, cam, p);
    drawHud(
      g,
      cam,
      p,
      // 回看阶段这次连发已经计入 sprays，序号不再加 1
      this.hudL.get(this.sprays.length * 2 + (this.phase === 'review' ? 1 : 0), (k) =>
        `连发 ${Math.min((k >> 1) + (k & 1 ? 0 : 1), this.opts.sprays)} / ${this.opts.sprays}`),
      this.pat.label,
      this.hudC.get(this.len, (k) => `弹道 ${k} 发`),
    );
    g.font = p.fontBig;
    g.textAlign = 'center';
    g.textBaseline = 'bottom';
    if (this.phase === 'ready') {
      g.fillStyle = p.text3;
      g.fillText('准星对准目标，按住左键连发', cam.width / 2, cam.height - 48);
    } else if (this.phase === 'review') {
      g.fillStyle = p.text;
      g.fillText(this.reviewMsg, cam.width / 2, cam.height - 48);
    }
    g.textAlign = 'left';
  }

  result(): ModeResult {
    const controls = this.sprays.map((s) => s.control).filter((v): v is number => v != null);
    const errs = this.sprays.map((s) => s.meanErr).filter((v): v is number => v != null);
    const breaks = this.sprays.map((s) => s.breakShot).filter((v): v is number => v != null);
    const perShot = Array.from(this.shotErrSum, (s, i) => (this.shotErrN[i]! ? round(s / this.shotErrN[i]!, 3) : null));
    return {
      variant: this.opts.weapon,
      score: round(mean(controls), 1),
      metrics: {
        sprays: this.sprays.length,
        control: round(mean(controls), 1),
        meanErr: round(mean(errs), 2),
        breakShot: round(median(breaks), 0),
        unbroken: this.sprays.filter((s) => s.breakShot == null).length,
        patternShots: this.len,
      },
      trials: this.trials,
      detail: {
        weapon: this.opts.weapon,
        perShotErr: perShot,
        bulletsV: this.bulletsV,
        bulletsH: this.bulletsH,
        recoil: this.pat.shots.slice(0, this.len),
      },
    };
  }

  debug() {
    return { phase: this.phase, n: this.n, sprays: this.sprays.length, len: this.len };
  }
}

const LINE_DASH = [4, 4];
const NO_DASH: number[] = [];
