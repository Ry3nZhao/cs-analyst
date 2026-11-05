/**
 * 训练引擎：Canvas 2D + requestAnimationFrame。
 *
 * - 输入：Pointer Lock 下用 movementX / movementY（优先 `unadjustedMovement`，即不经过系统鼠标加速的原始位移）。
 *   拿不到锁时退回普通模式（光标位移），并在结果里记下 pointerLock = false。
 * - 时间：`now` = performance.now() − 累计暂停时长；事件的 timeStamp 换算到同一条时间轴。
 * - 每帧只调用 mode.update / mode.draw，不分配对象。
 */
import { Camera, degPerCount, readPalette, type ModeResult, type Palette, type TrainingModeImpl } from './core';
import { CachedText } from './draw';

export type EngineState = 'ready' | 'running' | 'paused' | 'done';

export interface EngineFinish {
  result: ModeResult;
  durationMs: number;
  pointerLock: boolean;
  hfovDeg: number;
  avgFps: number | null;
}

export interface EngineOptions {
  sensitivity: number;
  /** 模式需要转动视角时才会请求锁定 */
  wantPointerLock: boolean;
  onState?: (state: EngineState, message?: string) => void;
  onDone?: (finish: EngineFinish) => void;
}

/** 本地存储里的调试开关：设为 1 时不请求 Pointer Lock（自动化测试用，避免锁住真实鼠标） */
export const DISABLE_LOCK_KEY = 'training.disablePointerLock';

export class TrainingEngine {
  state: EngineState = 'ready';
  readonly camera = new Camera();
  private palette!: Palette;
  private g: CanvasRenderingContext2D;
  private dpr = 1;
  private raf = 0;
  private last = 0;
  private pausedTotal = 0;
  private pauseStart = 0;
  private startedAt = 0;
  private locked = false;
  private everLocked = false;
  private lastClientX = NaN;
  private lastClientY = NaN;
  private ro: ResizeObserver | null = null;
  private mounted = false;
  // 帧率
  private fpsFrames = 0;
  private fpsT0 = 0;
  private fps = 0;
  private fpsSum = 0;
  private fpsSamples = 0;
  private fpsText = new CachedText();

  constructor(
    private canvas: HTMLCanvasElement,
    readonly mode: TrainingModeImpl,
    private opts: EngineOptions,
  ) {
    const g = canvas.getContext('2d', { alpha: false, desynchronized: true });
    if (!g) throw new Error('Canvas 2D 不可用');
    this.g = g;
  }

  get dpc() {
    return degPerCount(this.opts.sensitivity);
  }

  private now(t = performance.now()) {
    return t - this.pausedTotal;
  }

  mount() {
    this.palette = readPalette(this.canvas);
    // 先按画布尺寸设好视口并 init 模式，之后的 resize 才能安全地重绘
    const rect = this.canvas.getBoundingClientRect();
    this.camera.setViewport(rect.width, rect.height);
    this.mode.init({ camera: this.camera, palette: this.palette, dpc: this.dpc }, this.now());
    this.mounted = true;
    this.resize();
    this.ro = new ResizeObserver(() => this.resize());
    this.ro.observe(this.canvas);
    this.canvas.addEventListener('mousedown', this.onMouseDown);
    this.canvas.addEventListener('contextmenu', this.prevent);
    window.addEventListener('mouseup', this.onMouseUp);
    window.addEventListener('mousemove', this.onMouseMove);
    window.addEventListener('keydown', this.onKeyDown, true);
    window.addEventListener('keyup', this.onKeyUp, true);
    document.addEventListener('pointerlockchange', this.onLockChange);
    (window as unknown as { __training?: unknown }).__training = this;
  }

  destroy() {
    cancelAnimationFrame(this.raf);
    this.ro?.disconnect();
    this.canvas.removeEventListener('mousedown', this.onMouseDown);
    this.canvas.removeEventListener('contextmenu', this.prevent);
    window.removeEventListener('mouseup', this.onMouseUp);
    window.removeEventListener('mousemove', this.onMouseMove);
    window.removeEventListener('keydown', this.onKeyDown, true);
    window.removeEventListener('keyup', this.onKeyUp, true);
    document.removeEventListener('pointerlockchange', this.onLockChange);
    if (document.pointerLockElement === this.canvas) document.exitPointerLock();
    const w = window as unknown as { __training?: unknown };
    if (w.__training === this) delete w.__training;
  }

  /** 必须在用户点击的处理函数里调用（Pointer Lock 需要用户手势） */
  async start() {
    if (this.state !== 'ready') return;
    await this.lock();
    this.state = 'running';
    this.startedAt = this.now();
    // 模式在 mount 时已经 init 过（用于绘制第一帧），这里按真正的开始时间重新 init
    this.mode.init({ camera: this.camera, palette: this.palette, dpc: this.dpc }, this.startedAt);
    this.opts.onState?.('running');
    this.loop();
  }

  pause() {
    if (this.state !== 'running') return;
    this.state = 'paused';
    this.pauseStart = performance.now();
    cancelAnimationFrame(this.raf);
    if (document.pointerLockElement === this.canvas) document.exitPointerLock();
    this.opts.onState?.('paused');
  }

  async resume() {
    if (this.state !== 'paused') return;
    const ok = await this.lock();
    if (!ok && this.opts.wantPointerLock && this.everLocked) {
      // Chromium 在退出锁定后约 1 秒内拒绝再次锁定
      this.opts.onState?.('paused', '没能锁定鼠标，请稍等一秒再点击继续');
      return;
    }
    this.pausedTotal += performance.now() - this.pauseStart;
    this.state = 'running';
    this.opts.onState?.('running');
    this.loop();
  }

  /** 提前结束并按当前进度出结果 */
  finishNow() {
    if (this.state === 'done') return;
    if (this.state === 'paused') this.pausedTotal += performance.now() - this.pauseStart;
    this.finish();
  }

  debug() {
    return {
      state: this.state,
      locked: this.locked,
      yaw: this.camera.yaw,
      pitch: this.camera.pitch,
      dpc: this.dpc,
      fps: this.fps,
      width: this.camera.width,
      height: this.camera.height,
      hfov: this.camera.hfov,
      mode: this.mode.debug?.() ?? null,
    };
  }

  private async lock(): Promise<boolean> {
    if (!this.opts.wantPointerLock) return false;
    let disabled = false;
    try {
      disabled = localStorage.getItem(DISABLE_LOCK_KEY) === '1';
    } catch {
      /* ignore */
    }
    if (disabled) return false;
    const c = this.canvas as HTMLCanvasElement & {
      requestPointerLock(opts?: { unadjustedMovement?: boolean }): Promise<void> | void;
    };
    try {
      await c.requestPointerLock({ unadjustedMovement: true });
    } catch {
      try {
        await c.requestPointerLock();
      } catch {
        return false;
      }
    }
    // 某些实现不返回 Promise，等一帧再看结果
    if (document.pointerLockElement !== this.canvas) {
      await new Promise((r) => requestAnimationFrame(() => r(null)));
    }
    this.locked = document.pointerLockElement === this.canvas;
    if (this.locked) this.everLocked = true;
    return this.locked;
  }

  private resize() {
    const rect = this.canvas.getBoundingClientRect();
    this.dpr = window.devicePixelRatio || 1;
    this.canvas.width = Math.max(1, Math.round(rect.width * this.dpr));
    this.canvas.height = Math.max(1, Math.round(rect.height * this.dpr));
    this.camera.setViewport(rect.width, rect.height);
    if (this.mounted && this.state !== 'running') this.drawFrame(this.now());
  }

  private loop = () => {
    this.last = performance.now();
    this.fpsT0 = this.last;
    this.fpsFrames = 0;
    const frame = (t: number) => {
      if (this.state !== 'running') return;
      const dt = Math.min(50, Math.max(0, t - this.last));
      this.last = t;
      const now = this.now(t);
      this.mode.update(now, dt);
      this.drawFrame(now);
      this.fpsFrames++;
      if (t - this.fpsT0 >= 500) {
        this.fps = (this.fpsFrames * 1000) / (t - this.fpsT0);
        this.fpsSum += this.fps;
        this.fpsSamples++;
        this.fpsFrames = 0;
        this.fpsT0 = t;
      }
      if (this.mode.done) {
        this.finish();
        return;
      }
      this.raf = requestAnimationFrame(frame);
    };
    this.raf = requestAnimationFrame(frame);
  };

  private drawFrame(now: number) {
    const g = this.g;
    g.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    this.camera.prepare();
    this.mode.draw(g, now);
    if (this.state === 'running' && this.fps > 0) {
      g.font = this.palette.fontSmall;
      g.textAlign = 'right';
      g.textBaseline = 'bottom';
      g.fillStyle = this.palette.disabled;
      g.fillText(this.fpsText.get(Math.round(this.fps), (k) => `${k} FPS`), this.camera.width - 12, this.camera.height - 10);
      g.textAlign = 'left';
    }
  }

  private finish() {
    cancelAnimationFrame(this.raf);
    this.state = 'done';
    if (document.pointerLockElement === this.canvas) document.exitPointerLock();
    const durationMs = Math.max(0, Math.round(this.now() - this.startedAt));
    this.opts.onState?.('done');
    this.opts.onDone?.({
      result: this.mode.result(),
      durationMs,
      pointerLock: this.everLocked,
      hfovDeg: this.camera.hfov,
      avgFps: this.fpsSamples ? this.fpsSum / this.fpsSamples : null,
    });
  }

  // ── 输入 ──

  private prevent = (e: Event) => e.preventDefault();

  private onMouseMove = (e: MouseEvent) => {
    let dx = e.movementX;
    let dy = e.movementY;
    if (!this.locked) {
      // 没有锁定时，movementX 在部分环境下为 0（例如合成事件），用光标位移兜底
      if (dx === 0 && dy === 0 && Number.isFinite(this.lastClientX)) {
        dx = e.clientX - this.lastClientX;
        dy = e.clientY - this.lastClientY;
      }
      this.lastClientX = e.clientX;
      this.lastClientY = e.clientY;
    }
    if (this.state !== 'running' || !this.mode.usesView) return;
    this.camera.rotate(dx, dy, this.dpc);
    this.mode.onMove?.(this.now(e.timeStamp));
  };

  private onMouseDown = (e: MouseEvent) => {
    e.preventDefault();
    if (this.state !== 'running') return;
    this.mode.mouseDown(e.button, this.now(e.timeStamp));
  };

  private onMouseUp = (e: MouseEvent) => {
    if (this.state !== 'running' && this.state !== 'paused') return;
    this.mode.mouseUp?.(e.button, this.now(e.timeStamp));
  };

  private onKeyDown = (e: KeyboardEvent) => {
    if (e.code === 'Escape') {
      if (this.state === 'running') {
        e.preventDefault();
        this.pause();
      }
      return;
    }
    if (this.state !== 'running') return;
    if (e.code === 'KeyA' || e.code === 'KeyD' || e.code === 'Space') e.preventDefault();
    if (!e.repeat) this.mode.key?.(e.code, true, this.now(e.timeStamp));
  };

  private onKeyUp = (e: KeyboardEvent) => {
    // 暂停中松开的按键也要转给模式，否则恢复后会当成一直按着
    if (this.state !== 'running' && this.state !== 'paused') return;
    this.mode.key?.(e.code, false, this.now(e.timeStamp));
  };

  private onLockChange = () => {
    const nowLocked = document.pointerLockElement === this.canvas;
    const lost = this.locked && !nowLocked;
    this.locked = nowLocked;
    if (lost && this.state === 'running') this.pause();
  };
}
