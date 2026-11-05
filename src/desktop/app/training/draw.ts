/** 公共绘制：世界点阵、准星、目标、命中特效、HUD 文本。都不在每帧分配对象。 */
import type { Camera, Palette, ScreenPoint } from './core';
import { screenPoint } from './core';

const tmp: ScreenPoint = screenPoint();

/** 世界里固定的点阵（每 10° 一个点），转动视角时提供参照。 */
export function drawBackdrop(g: CanvasRenderingContext2D, cam: Camera, pal: Palette) {
  g.fillStyle = pal.bg;
  g.fillRect(0, 0, cam.width, cam.height);
  const step = 10;
  const halfH = cam.hfov / 2 + step;
  const halfV = 45;
  const y0 = Math.floor((cam.yaw - halfH) / step) * step;
  const p0 = Math.max(-80, Math.floor((cam.pitch - halfV) / step) * step);
  const p1 = Math.min(80, cam.pitch + halfV);
  g.fillStyle = pal.borderStrong;
  for (let p = p0; p <= p1; p += step) {
    for (let y = y0; y <= cam.yaw + halfH; y += step) {
      if (!cam.project(y, p, tmp)) continue;
      const s = p === 0 ? 2.5 : 1.5;
      g.fillRect(tmp.x - s / 2, tmp.y - s / 2, s, s);
    }
  }
  // 地平线
  g.strokeStyle = pal.border;
  g.lineWidth = 1;
  g.beginPath();
  let first = true;
  for (let y = y0; y <= cam.yaw + halfH; y += 2) {
    if (!cam.project(y, 0, tmp)) continue;
    if (first) g.moveTo(tmp.x, tmp.y);
    else g.lineTo(tmp.x, tmp.y);
    first = false;
  }
  g.stroke();
}

/** 简单的十字准星：中间留空，带一个中心点 */
export function drawCrosshair(g: CanvasRenderingContext2D, cam: Camera, pal: Palette) {
  const cx = Math.round(cam.width / 2) + 0.5;
  const cy = Math.round(cam.height / 2) + 0.5;
  const gap = 4, len = 7;
  g.strokeStyle = pal.text;
  g.lineWidth = 1.5;
  g.beginPath();
  g.moveTo(cx - gap - len, cy); g.lineTo(cx - gap, cy);
  g.moveTo(cx + gap, cy); g.lineTo(cx + gap + len, cy);
  g.moveTo(cx, cy - gap - len); g.lineTo(cx, cy - gap);
  g.moveTo(cx, cy + gap); g.lineTo(cx, cy + gap + len);
  g.stroke();
  g.fillStyle = pal.text;
  g.fillRect(cx - 1, cy - 1, 2, 2);
}

/** 画一个球形目标（世界方向 + 角半径）。返回是否在画面内。 */
export function drawTarget(
  g: CanvasRenderingContext2D,
  cam: Camera,
  pal: Palette,
  yaw: number,
  pitch: number,
  radiusDeg: number,
  highlight = false,
): boolean {
  if (!cam.project(yaw, pitch, tmp)) return false;
  const r = Math.max(2, radiusDeg * tmp.pxPerDeg);
  g.fillStyle = pal.text;
  g.beginPath();
  g.arc(tmp.x, tmp.y, r, 0, Math.PI * 2);
  g.fill();
  if (highlight) {
    g.strokeStyle = pal.accent;
    g.lineWidth = 2;
    g.beginPath();
    g.arc(tmp.x, tmp.y, r + 4, 0, Math.PI * 2);
    g.stroke();
  }
  return tmp.x > -r && tmp.x < cam.width + r && tmp.y > -r && tmp.y < cam.height + r;
}

/** 目标在画面外时，在边缘画一个指向它的小三角 */
export function drawOffscreenHint(g: CanvasRenderingContext2D, cam: Camera, pal: Palette, yaw: number, pitch: number) {
  let dx = yaw - cam.yaw;
  dx = ((dx + 540) % 360) - 180;
  const dy = pitch - cam.pitch;
  const a = Math.atan2(-dy, dx);
  const rx = cam.width / 2 - 28, ry = cam.height / 2 - 28;
  const k = Math.min(Math.abs(rx / Math.cos(a) || Infinity), Math.abs(ry / Math.sin(a) || Infinity));
  const x = cam.width / 2 + Math.cos(a) * k;
  const y = cam.height / 2 + Math.sin(a) * k;
  g.save();
  g.translate(x, y);
  g.rotate(a);
  g.fillStyle = pal.text3;
  g.beginPath();
  g.moveTo(10, 0); g.lineTo(-6, -7); g.lineTo(-6, 7);
  g.closePath();
  g.fill();
  g.restore();
}

/** 世界坐标里的短暂特效（命中圈 / 未命中点），固定容量的环形池 */
export class EffectPool {
  private yaw: Float64Array;
  private pitch: Float64Array;
  private t: Float64Array;
  private kind: Uint8Array;
  private head = 0;
  constructor(private cap = 32, private lifeMs = 260) {
    this.yaw = new Float64Array(cap);
    this.pitch = new Float64Array(cap);
    this.t = new Float64Array(cap).fill(-1e9);
    this.kind = new Uint8Array(cap);
  }
  /** kind: 0 命中，1 未命中 */
  add(yaw: number, pitch: number, now: number, kind: 0 | 1) {
    const i = this.head;
    this.yaw[i] = yaw; this.pitch[i] = pitch; this.t[i] = now; this.kind[i] = kind;
    this.head = (i + 1) % this.cap;
  }
  clear() {
    this.t.fill(-1e9);
  }
  draw(g: CanvasRenderingContext2D, cam: Camera, pal: Palette, now: number) {
    for (let i = 0; i < this.cap; i++) {
      const age = now - this.t[i]!;
      if (age < 0 || age > this.lifeMs) continue;
      if (!cam.project(this.yaw[i]!, this.pitch[i]!, tmp)) continue;
      const k = age / this.lifeMs;
      g.globalAlpha = 1 - k;
      if (this.kind[i] === 0) {
        g.strokeStyle = pal.success;
        g.lineWidth = 2;
        g.beginPath();
        g.arc(tmp.x, tmp.y, 6 + 18 * k, 0, Math.PI * 2);
        g.stroke();
      } else {
        g.fillStyle = pal.danger;
        g.fillRect(tmp.x - 2, tmp.y - 2, 4, 4);
      }
    }
    g.globalAlpha = 1;
  }
}

/** 只在数值变化时重建字符串的 HUD 文本缓存 */
export class CachedText {
  private key = NaN;
  private text = '';
  get(key: number, make: (k: number) => string) {
    if (key !== this.key) {
      this.key = key;
      this.text = make(key);
    }
    return this.text;
  }
}

/** 顶部居中的 HUD：左中右三段，模式自己提供文本 */
export function drawHud(g: CanvasRenderingContext2D, cam: Camera, pal: Palette, left: string, center: string, right: string) {
  g.font = pal.fontHud;
  g.textBaseline = 'top';
  g.fillStyle = pal.text2;
  g.textAlign = 'left';
  g.fillText(left, 20, 18);
  g.textAlign = 'center';
  g.fillStyle = pal.text;
  g.fillText(center, cam.width / 2, 18);
  g.textAlign = 'right';
  g.fillStyle = pal.text2;
  g.fillText(right, cam.width - 20, 18);
  g.textAlign = 'left';
}
