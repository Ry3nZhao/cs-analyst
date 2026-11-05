<template>
  <div class="spray-chart">
    <div class="sc-toolbar">
      <div class="seg" role="tablist">
        <button
          type="button"
          class="seg-btn"
          :class="{ on: view === 'comp' }"
          role="tab"
          :aria-selected="view === 'comp'"
          @click="view = 'comp'"
        >
          补偿轨迹
        </button>
        <button
          type="button"
          class="seg-btn"
          :class="{ on: view === 'impact' }"
          role="tab"
          :aria-selected="view === 'impact'"
          @click="view = 'impact'"
        >
          落点
        </button>
      </div>
      <AimHelp :title="view === 'comp' ? '补偿轨迹' : '落点'" align="right">
        <template v-if="view === 'comp'">
          <p>空心圈是理想补偿：把后坐力（−2 × Δpunch）镜像过来，就是准星应该拉到的位置。</p>
          <p>实心点是你实际拉准星的轨迹（−Δ视角），以第一发为原点，逐发编号。</p>
          <p>两者之差就是这一发没压住的量：偏上 = 压少了，偏下 = 压多了。</p>
        </template>
        <template v-else>
          <p>实心点是每一发子弹实际落在第一发的哪里（= 后坐力 + 你的补偿），原点是第一发。</p>
          <p>浅灰点是完全不压时子弹会去的位置。虚线圆半径 2°，超出即视为「崩了」。</p>
        </template>
      </AimHelp>
    </div>

    <svg
      v-if="pts.length > 1"
      :viewBox="`0 0 ${W} ${H}`"
      class="sc-svg"
      role="img"
      :aria-label="view === 'comp' ? '压枪补偿轨迹' : '子弹落点'"
      @mouseleave="setActive(null)"
    >
      <!-- 网格：每 grid° 一条 -->
      <g class="grid">
        <line v-for="gx in gridX" :key="`gx${gx}`" :x1="sx(gx)" :x2="sx(gx)" :y1="0" :y2="H" />
        <line v-for="gy in gridY" :key="`gy${gy}`" :x1="0" :x2="W" :y1="sy(gy)" :y2="sy(gy)" />
      </g>
      <line :x1="sx(0)" :x2="sx(0)" :y1="0" :y2="H" class="axis" />
      <line :x1="0" :x2="W" :y1="sy(0)" :y2="sy(0)" class="axis" />

      <template v-if="view === 'comp'">
        <!-- 理想补偿 -->
        <polyline :points="poly(pts.map((p) => p.ideal))" class="ideal-line" />
        <!-- 偏差连线 -->
        <line
          v-for="p in pts"
          v-show="p.tone !== 'ok'"
          :key="`d${p.idx}`"
          :x1="sx(p.ideal.x)"
          :y1="sy(p.ideal.y)"
          :x2="sx(p.actual.x)"
          :y2="sy(p.actual.y)"
          class="dev"
          :class="`t-${p.tone}`"
        />
        <circle v-for="p in pts" :key="`i${p.idx}`" :cx="sx(p.ideal.x)" :cy="sy(p.ideal.y)" r="4.5" class="ideal-pt" />
        <polyline :points="poly(pts.map((p) => p.actual))" class="actual-line" />
      </template>
      <template v-else>
        <circle :cx="sx(0)" :cy="sy(0)" :r="BREAK_DEG * scale" class="tol" />
        <circle v-for="p in pts" :key="`r${p.idx}`" :cx="sx(p.recoil.x)" :cy="sy(p.recoil.y)" r="2.5" class="recoil-pt" />
      </template>

      <!-- 实际点（补偿轨迹）/ 落点 -->
      <g
        v-for="p in pts"
        :key="`a${p.idx}`"
        class="pt"
        :class="[`t-${p.tone}`, { active: active === p.idx, brk: p.idx === breakShot }]"
        @mouseenter="setActive(p.idx)"
      >
        <circle
          v-if="p.idx === breakShot"
          :cx="sx(cur(p).x)"
          :cy="sy(cur(p).y)"
          r="9"
          class="brk-ring"
        />
        <circle :cx="sx(cur(p).x)" :cy="sy(cur(p).y)" :r="active === p.idx ? 5.5 : 4" class="dot" :class="{ faded: p.faded }" />
        <text
          v-if="showLabel(p)"
          :x="sx(cur(p).x) + labelDx(p)"
          :y="sy(cur(p).y) + 3.5"
          :text-anchor="labelDx(p) > 0 ? 'start' : 'end'"
          class="lbl"
        >
          {{ p.idx }}
        </text>
        <title>{{ tip(p) }}</title>
      </g>

      <text :x="W - 6" :y="14" text-anchor="end" class="scale-lbl">网格 {{ grid }}°</text>
    </svg>
    <div v-else class="sc-empty">没有足够的子弹可画</div>

    <div class="sc-legend">
      <template v-if="view === 'comp'">
        <span class="lg"><i class="lg-ideal" />理想补偿</span>
        <span class="lg"><i class="lg-actual" />实际补偿</span>
      </template>
      <template v-else>
        <span class="lg"><i class="lg-recoil" />不压时</span>
        <span class="lg"><i class="lg-actual" />实际落点</span>
      </template>
      <span class="lg"><i class="lg-dot t-warn" />偏差 1–2°</span>
      <span class="lg"><i class="lg-dot t-bad" />偏差 &gt; 2°</span>
      <span v-if="breakShot" class="lg"><i class="lg-ring" />第 {{ breakShot }} 发开始崩</span>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 压枪弹道图（单次连发或按武器的平均弹道都用它）。
 * - 补偿轨迹：理想补偿（−recoil）为空心圈，玩家实际补偿（aim）为实心点，逐发编号。
 * - 落点：子弹相对第一发的落点（err），叠在「不压时」的后坐力图案上。
 * 坐标是屏幕方向（v > 0 向上，h > 0 向右），x / y 等比例。
 * 偏差 1–2° 用 warning、> 2° 用 danger，其他保持灰白。
 */
import { BREAK_DEG, errTone, horizontalVerdict, verticalVerdict } from '~/composables/useAim';
import type { BulletLike } from '~/composables/useAim';

const props = withDefaults(defineProps<{
  bullets: BulletLike[];
  breakShot?: number | null;
  /** 高亮的弹序（和误差曲线联动） */
  active?: number | null;
  /** 平均弹道时：打到这一发的连发少于该数的点画淡 */
  minSamples?: number;
  initialView?: 'comp' | 'impact';
  height?: number;
}>(), {
  breakShot: null,
  active: null,
  minSamples: 3,
  initialView: 'comp',
  height: 380,
});

const emit = defineEmits<{ 'update:active': [idx: number | null] }>();

const view = ref<'comp' | 'impact'>(props.initialView);
const W = 320;
const H = computed(() => props.height);

interface P { x: number; y: number }
interface Pt {
  idx: number;
  ideal: P;
  actual: P;
  impact: P;
  recoil: P;
  err: number;
  errV: number;
  errH: number;
  n: number | undefined;
  tone: 'ok' | 'warn' | 'bad';
  faded: boolean;
}

const pts = computed<Pt[]>(() =>
  props.bullets.map((b) => {
    const err = b.errMedian ?? Math.hypot(b.errV, b.errH);
    return {
      idx: b.idx,
      ideal: { x: -b.recoilH, y: -b.recoilV },
      actual: { x: b.aimH, y: b.aimV },
      impact: { x: b.errH, y: b.errV },
      recoil: { x: b.recoilH, y: b.recoilV },
      err,
      errV: b.errV,
      errH: b.errH,
      n: b.n,
      tone: errTone(err),
      faded: b.n != null && b.n < props.minSamples,
    };
  }),
);

function cur(p: Pt): P {
  return view.value === 'comp' ? p.actual : p.impact;
}

// 等比例的视口：所有可见点 + 原点，四周留白
const bounds = computed(() => {
  const xs: number[] = [0];
  const ys: number[] = [0];
  for (const p of pts.value) {
    const list = view.value === 'comp' ? [p.ideal, p.actual] : [p.impact, p.recoil];
    for (const q of list) {
      xs.push(q.x);
      ys.push(q.y);
    }
  }
  let x0 = Math.min(...xs);
  let x1 = Math.max(...xs);
  let y0 = Math.min(...ys);
  let y1 = Math.max(...ys);
  const minSpan = 4;
  if (x1 - x0 < minSpan) { const c = (x0 + x1) / 2; x0 = c - minSpan / 2; x1 = c + minSpan / 2; }
  if (y1 - y0 < minSpan) { const c = (y0 + y1) / 2; y0 = c - minSpan / 2; y1 = c + minSpan / 2; }
  return { x0, x1, y0, y1 };
});

const PAD = 24;
const scale = computed(() => {
  const b = bounds.value;
  return Math.min((W - 2 * PAD) / (b.x1 - b.x0), (H.value - 2 * PAD) / (b.y1 - b.y0));
});
const cx = computed(() => (bounds.value.x0 + bounds.value.x1) / 2);
const cy = computed(() => (bounds.value.y0 + bounds.value.y1) / 2);

function sx(x: number) {
  return W / 2 + (x - cx.value) * scale.value;
}
function sy(y: number) {
  // 屏幕向上为正
  return H.value / 2 - (y - cy.value) * scale.value;
}
function poly(list: P[]) {
  return list.map((p) => `${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`).join(' ');
}

const grid = computed(() => {
  const spanDeg = W / scale.value;
  return spanDeg > 30 ? 5 : spanDeg > 12 ? 2 : 1;
});
function gridLines(lo: number, hi: number) {
  const g = grid.value;
  const out: number[] = [];
  for (let v = Math.ceil(lo / g) * g; v <= hi; v += g) if (v !== 0) out.push(v);
  return out;
}
const gridX = computed(() => gridLines(cx.value - W / 2 / scale.value, cx.value + W / 2 / scale.value));
const gridY = computed(() => gridLines(cy.value - H.value / 2 / scale.value, cy.value + H.value / 2 / scale.value));

function showLabel(p: Pt) {
  if (pts.value.length <= 20) return true;
  return p.idx === 1 || p.idx % 5 === 0 || p.idx === props.breakShot || p.idx === active.value || p.idx === pts.value.length;
}
function labelDx(p: Pt) {
  // 编号放在远离理想点的一侧，减少和空心圈重叠
  const c = cur(p);
  const ref = view.value === 'comp' ? p.ideal : { x: 0, y: 0 };
  return c.x >= ref.x ? 8 : -8;
}

const active = computed(() => props.active);
function setActive(idx: number | null) {
  emit('update:active', idx);
}

function tip(p: Pt) {
  const parts = [`第 ${p.idx} 发`];
  if (p.n != null) parts.push(`${p.n} 次连发`);
  parts.push(`偏差 ${p.err.toFixed(2)}°${p.n != null ? '（中位数）' : ''}`);
  parts.push(`垂直 ${p.errV >= 0 ? '+' : ''}${p.errV.toFixed(2)}° ${verticalVerdict(p.errV)}`);
  parts.push(`水平 ${p.errH >= 0 ? '+' : ''}${p.errH.toFixed(2)}° ${horizontalVerdict(p.errH)}`);
  return parts.join('\n');
}
</script>

<style scoped>
.spray-chart { display: flex; flex-direction: column; gap: var(--space-2); }
.sc-toolbar { display: flex; align-items: center; justify-content: space-between; }
.seg { display: inline-flex; padding: 2px; background: var(--bg-sunken); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); }
.seg-btn {
  height: 22px;
  padding: 0 var(--space-3);
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-3);
  font-size: var(--text-xs);
  cursor: pointer;
}
.seg-btn:hover { color: var(--text); }
.seg-btn.on { background: var(--surface-active); color: var(--text); }

.sc-svg { width: 100%; height: auto; display: block; background: var(--bg-sunken); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); }
.sc-empty { padding: var(--space-8) 0; text-align: center; font-size: var(--text-sm); color: var(--text-3); }

.grid line { stroke: var(--border-subtle); stroke-width: 1; }
.axis { stroke: var(--border); stroke-width: 1; }
.ideal-line { fill: none; stroke: var(--text-3); stroke-width: 1; stroke-dasharray: 3 3; }
.ideal-pt { fill: var(--bg-sunken); stroke: var(--text-3); stroke-width: 1.2; }
.actual-line { fill: none; stroke: var(--text-2); stroke-width: 1.4; stroke-linejoin: round; }
.recoil-pt { fill: var(--border-strong); }
.tol { fill: none; stroke: var(--text-3); stroke-width: 1; stroke-dasharray: 3 3; }

.dev { stroke-width: 1.2; }
.dev.t-warn { stroke: var(--warning-line); }
.dev.t-bad { stroke: var(--danger-line); }

.pt { cursor: default; }
.dot { fill: var(--text); stroke: var(--bg-sunken); stroke-width: 1.2; transition: r 0.1s; }
.t-warn .dot { fill: var(--warning); }
.t-bad .dot { fill: var(--danger); }
.dot.faded { opacity: 0.35; }
.pt.active .dot { stroke: var(--accent); stroke-width: 2; }
.brk-ring { fill: none; stroke: var(--danger); stroke-width: 1.4; }
.lbl { font-size: 10px; fill: var(--text-3); font-variant-numeric: tabular-nums; pointer-events: none; }
.pt.active .lbl, .pt.brk .lbl { fill: var(--text); font-weight: 600; }
.scale-lbl { font-size: 9px; fill: var(--text-disabled); }

.sc-legend { display: flex; flex-wrap: wrap; gap: var(--space-1) var(--space-3); font-size: 10px; color: var(--text-3); }
.lg { display: inline-flex; align-items: center; gap: 4px; }
.lg i { display: inline-block; flex-shrink: 0; }
.lg-ideal { width: 9px; height: 9px; border: 1.2px solid var(--text-3); border-radius: 50%; }
.lg-actual { width: 8px; height: 8px; background: var(--text); border-radius: 50%; }
.lg-recoil { width: 6px; height: 6px; background: var(--border-strong); border-radius: 50%; }
.lg-dot { width: 8px; height: 8px; border-radius: 50%; }
.lg-dot.t-warn { background: var(--warning); }
.lg-dot.t-bad { background: var(--danger); }
.lg-ring { width: 10px; height: 10px; border: 1.4px solid var(--danger); border-radius: 50%; }
</style>
