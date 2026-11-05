/**
 * 反应测试：灰屏随机等待 1.2–3.5 秒后变成强调色，尽快点击。
 * 刺激时间取变色那一帧的 rAF 时间戳，反应时间 = 点击事件时间戳 − 刺激时间。
 * 变色前点击判为犯规，本次重来（不计成绩）。
 */
import { mean, median, rand, round, stdev, type ModeContext, type ModeResult, type Palette, type TrainingModeImpl } from '../core';
import { CachedText, drawHud } from '../draw';
import type { TrainingTrial } from '@cs-analyst/shared-types';

export interface ReactionOptions {
  count: number;
}

type Phase = 'wait' | 'go' | 'shown' | 'foul';

export class ReactionMode implements TrainingModeImpl {
  readonly usesView = false;
  done = false;
  private pal!: Palette;
  private w = 0;
  private h = 0;
  private phase: Phase = 'wait';
  private goAt = 0;
  private stimulusAt = 0;
  private phaseUntil = 0;
  private times: number[] = [];
  private trials: TrainingTrial[] = [];
  private fouls = 0;
  private lastRt = 0;
  private hudLeft = new CachedText();
  private hudRight = new CachedText();
  private big = new CachedText();
  private ctxRef!: ModeContext;

  constructor(private opts: ReactionOptions) {}

  init(ctx: ModeContext, now: number) {
    this.ctxRef = ctx;
    this.pal = ctx.palette;
    this.done = false;
    this.times = [];
    this.trials = [];
    this.fouls = 0;
    this.arm(now);
  }

  private arm(now: number) {
    this.phase = 'wait';
    this.goAt = now + rand(1200, 3500);
  }

  update(now: number) {
    if (this.phase === 'wait' && now >= this.goAt) {
      this.phase = 'go';
      // 这一帧就会画成强调色，以本帧时间作为刺激时间
      this.stimulusAt = now;
    } else if ((this.phase === 'shown' || this.phase === 'foul') && now >= this.phaseUntil) {
      if (this.times.length >= this.opts.count) this.done = true;
      else this.arm(now);
    }
  }

  mouseDown(button: number, now: number) {
    if (button !== 0) return;
    if (this.phase === 'wait') {
      this.fouls++;
      this.trials.push({ value: null, ok: false, data: { foul: true } });
      this.phase = 'foul';
      this.phaseUntil = now + 1100;
    } else if (this.phase === 'go') {
      const rt = Math.max(0, now - this.stimulusAt);
      this.lastRt = rt;
      this.times.push(rt);
      this.trials.push({ value: round(rt, 1), ok: true });
      this.phase = 'shown';
      this.phaseUntil = now + 900;
    }
  }

  draw(g: CanvasRenderingContext2D) {
    const cam = this.ctxRef.camera;
    this.w = cam.width;
    this.h = cam.height;
    const p = this.pal;
    g.fillStyle = this.phase === 'go' ? p.accent : this.phase === 'foul' ? p.surface : p.bg;
    g.fillRect(0, 0, this.w, this.h);
    if (this.phase === 'foul') {
      g.fillStyle = p.danger;
      g.fillRect(0, 0, this.w, 4);
    }
    const n = this.times.length;
    drawHud(
      g,
      cam,
      p,
      this.hudLeft.get(n, (k) => `第 ${Math.min(k + 1, this.opts.count)} / ${this.opts.count} 次`),
      '',
      this.hudRight.get(this.fouls, (k) => `犯规 ${k}`),
    );
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    if (this.phase === 'wait') {
      g.font = p.fontBig;
      g.fillStyle = p.text3;
      g.fillText('等待变色…', this.w / 2, this.h / 2);
    } else if (this.phase === 'go') {
      g.font = p.fontBig;
      g.fillStyle = p.bg;
      g.fillText('点击！', this.w / 2, this.h / 2);
    } else if (this.phase === 'shown') {
      g.font = p.fontHuge;
      g.fillStyle = p.text;
      g.fillText(this.big.get(Math.round(this.lastRt), (k) => `${k} ms`), this.w / 2, this.h / 2);
    } else {
      g.font = p.fontBig;
      g.fillStyle = p.danger;
      g.fillText('太早了，犯规', this.w / 2, this.h / 2);
    }
    g.textAlign = 'left';
  }

  result(): ModeResult {
    const t = this.times;
    return {
      score: round(median(t), 1),
      metrics: {
        median: round(median(t), 1),
        mean: round(mean(t), 1),
        best: t.length ? round(Math.min(...t), 1) : null,
        worst: t.length ? round(Math.max(...t), 1) : null,
        sd: round(stdev(t), 1),
        count: t.length,
        fouls: this.fouls,
      },
      trials: this.trials,
      detail: { times: t.slice() },
    };
  }

  debug() {
    return { phase: this.phase, count: this.times.length, fouls: this.fouls, goAt: this.goAt };
  }
}
