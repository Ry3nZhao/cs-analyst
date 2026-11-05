<template>
  <div ref="root" class="err-chart">
    <svg v-if="bullets.length > 1" :viewBox="`0 0 ${W} ${totalH}`" class="ec-svg" role="img" aria-label="第 N 发误差曲线" @mouseleave="emit('update:active', null)">
      <g v-for="(panel, pi) in panels" :key="panel.key" :transform="`translate(0, ${pi * (PH + GAP)})`">
        <!-- 超出阈值的区域 -->
        <rect :x="L" :y="0" :width="plotW" :height="y(panel, BREAK_DEG)" class="band" />
        <rect
          v-if="panel.signed"
          :x="L"
          :y="y(panel, -BREAK_DEG)"
          :width="plotW"
          :height="Math.max(0, PH - y(panel, -BREAK_DEG))"
          class="band"
        />
        <line :x1="L" :x2="W - R" :y1="y(panel, BREAK_DEG)" :y2="y(panel, BREAK_DEG)" class="thr" />
        <line v-if="panel.signed" :x1="L" :x2="W - R" :y1="y(panel, -BREAK_DEG)" :y2="y(panel, -BREAK_DEG)" class="thr" />
        <line :x1="L" :x2="W - R" :y1="y(panel, 0)" :y2="y(panel, 0)" class="zero" />

        <!-- y 轴刻度 -->
        <text v-if="roomy(panel)" :x="L - 6" :y="y(panel, BREAK_DEG) + 3" text-anchor="end" class="tick">{{ panel.signed ? '+' : '' }}2°</text>
        <text :x="L - 6" :y="y(panel, 0) + 3" text-anchor="end" class="tick">0</text>
        <text v-if="panel.signed && roomy(panel)" :x="L - 6" :y="y(panel, -BREAK_DEG) + 3" text-anchor="end" class="tick">−2°</text>
        <text v-if="!roomy(panel)" :x="L - 6" :y="y(panel, panel.max) + 9" text-anchor="end" class="tick">{{ panel.signed ? '+' : '' }}{{ panel.max.toFixed(0) }}°</text>

        <!-- 面板标题与方向标注 -->
        <text :x="L + 4" :y="11" class="ptitle">{{ panel.title }}</text>
        <template v-if="panel.signed">
          <text :x="W - R - 4" :y="11" text-anchor="end" class="dir">{{ panel.up }}</text>
          <text :x="W - R - 4" :y="PH - 4" text-anchor="end" class="dir">{{ panel.down }}</text>
        </template>

        <!-- 开始崩 -->
        <line v-if="breakShot" :x1="x(breakShot)" :x2="x(breakShot)" :y1="0" :y2="PH" class="brk" />
        <text v-if="breakShot && pi === 0" :x="x(breakShot) + 4" :y="24" class="brk-lbl">第 {{ breakShot }} 发开始崩</text>

        <!-- 第二条线：均值（总误差面板） -->
        <polyline v-if="panel.secondary" :points="line(panel, panel.secondary)" class="line2" />
        <polyline :points="line(panel, panel.values)" class="line" />
        <g v-for="(v, i) in panel.values" :key="i">
          <circle
            :cx="x(bullets[i]!.idx)"
            :cy="y(panel, v)"
            :r="active === bullets[i]!.idx ? 4.2 : 2.8"
            class="pt"
            :class="[toneOf(v), { faded: faded(i), active: active === bullets[i]!.idx }]"
          />
        </g>
      </g>

      <!-- x 轴 -->
      <g :transform="`translate(0, ${panels.length * (PH + GAP) - GAP + 14})`">
        <text v-for="t in xTicks" :key="t" :x="x(t)" y="0" text-anchor="middle" class="tick">{{ t }}</text>
      </g>

      <!-- 悬停热区（整列） -->
      <rect
        v-for="b in bullets"
        :key="`h${b.idx}`"
        :x="x(b.idx) - colW / 2"
        :y="0"
        :width="colW"
        :height="panels.length * (PH + GAP)"
        class="hot"
        @mouseenter="emit('update:active', b.idx)"
      >
        <title>{{ tip(b) }}</title>
      </rect>
    </svg>
    <div v-else class="ec-empty">没有足够的子弹可画</div>
  </div>
</template>

<script setup lang="ts">
/**
 * 第 N 发的误差曲线，三个面板共用 x 轴（弹序）：
 * - 总误差：|err| 中位数（实线）和均值（虚线）；单次连发时就是这一发的 |err|。
 * - 垂直：平均 errV，> 0 子弹偏高 = 压少了，< 0 = 压多了。
 * - 水平：平均 errH，> 0 偏右。
 * 超过 ±2° 的区域铺淡红底，点按偏差大小用 warning / danger；「开始崩」用一条红色竖线标出。
 */
import { useElementWidth } from '~/composables/useElementWidth';
import { BREAK_DEG, errTone, horizontalVerdict, verticalVerdict } from '~/composables/useAim';
import type { BulletLike } from '~/composables/useAim';

const props = withDefaults(defineProps<{
  bullets: BulletLike[];
  breakShot?: number | null;
  active?: number | null;
  minSamples?: number;
}>(), {
  breakShot: null,
  active: null,
  minSamples: 3,
});
const emit = defineEmits<{ 'update:active': [idx: number | null] }>();

const root = ref<HTMLElement | null>(null);
const width = useElementWidth(root);
const W = computed(() => width.value);
const L = 34;
const R = 10;
const PH = 92;
const GAP = 10;
const plotW = computed(() => W.value - L - R);

interface Panel {
  key: string;
  title: string;
  signed: boolean;
  up?: string;
  down?: string;
  values: number[];
  secondary?: number[];
  max: number;
}

const isAgg = computed(() => props.bullets.some((b) => b.errMedian != null));

const panels = computed<Panel[]>(() => {
  const bs = props.bullets;
  const total = bs.map((b) => b.errMedian ?? Math.hypot(b.errV, b.errH));
  const mean = isAgg.value ? bs.map((b) => b.err ?? Math.hypot(b.errV, b.errH)) : undefined;
  const v = bs.map((b) => b.errV);
  const h = bs.map((b) => b.errH);
  const cap = (xs: number[]) => Math.max(BREAK_DEG * 1.75, ...xs.map(Math.abs)) * 1.1;
  return [
    { key: 'total', title: isAgg.value ? '总误差（中位数 / 均值）' : '总误差', signed: false, values: total, secondary: mean, max: cap([...total, ...(mean ?? [])]) },
    { key: 'v', title: '垂直', signed: true, up: '↑ 压少了', down: '↓ 压多了', values: v, max: cap(v) },
    { key: 'h', title: '水平', signed: true, up: '→ 偏右', down: '← 偏左', values: h, max: cap(h) },
  ];
});

const totalH = computed(() => panels.value.length * (PH + GAP) - GAP + 20);
const maxIdx = computed(() => Math.max(2, ...props.bullets.map((b) => b.idx)));
const colW = computed(() => plotW.value / Math.max(1, maxIdx.value - 1));

function x(idx: number) {
  return L + ((idx - 1) / (maxIdx.value - 1)) * plotW.value;
}
function y(p: Panel, v: number) {
  const lo = p.signed ? -p.max : 0;
  const c = Math.min(p.max, Math.max(lo, v));
  return PH - ((c - lo) / (p.max - lo)) * PH;
}
/** ±2° 和 0 的刻度之间是否放得下文字 */
function roomy(p: Panel) {
  return Math.abs(y(p, BREAK_DEG) - y(p, 0)) >= 12;
}
function line(p: Panel, vals: number[]) {
  return vals.map((v, i) => `${x(props.bullets[i]!.idx).toFixed(1)},${y(p, v).toFixed(1)}`).join(' ');
}
function toneOf(v: number) {
  return `t-${errTone(Math.abs(v))}`;
}
function faded(i: number) {
  const n = props.bullets[i]!.n;
  return n != null && n < props.minSamples;
}

const xTicks = computed(() => {
  const n = maxIdx.value;
  const step = n > 20 ? 5 : n > 10 ? 2 : 1;
  const out = [1];
  for (let i = step; i <= n; i += step) if (i !== 1) out.push(i);
  return out;
});

function tip(b: BulletLike) {
  const lines = [`第 ${b.idx} 发${b.n != null ? `（${b.n} 次连发）` : ''}`];
  if (b.errMedian != null) lines.push(`|误差| 中位数 ${b.errMedian.toFixed(2)}° · 均值 ${(b.err ?? 0).toFixed(2)}°`);
  else lines.push(`|误差| ${Math.hypot(b.errV, b.errH).toFixed(2)}°`);
  lines.push(`垂直 ${b.errV >= 0 ? '+' : ''}${b.errV.toFixed(2)}° ${verticalVerdict(b.errV)}`);
  lines.push(`水平 ${b.errH >= 0 ? '+' : ''}${b.errH.toFixed(2)}° ${horizontalVerdict(b.errH)}`);
  return lines.join('\n');
}
</script>

<style scoped>
.err-chart { width: 100%; }
.ec-svg { width: 100%; height: auto; display: block; }
.ec-empty { padding: var(--space-6) 0; text-align: center; font-size: var(--text-sm); color: var(--text-3); }
.band { fill: var(--danger-soft); opacity: 0.35; }
.thr { stroke: var(--danger-line); stroke-width: 1; stroke-dasharray: 4 3; }
.zero { stroke: var(--border-strong); stroke-width: 1; }
.tick { font-size: 10px; fill: var(--text-3); font-variant-numeric: tabular-nums; }
.tick.dim { fill: var(--text-disabled); }
.ptitle { font-size: 10.5px; fill: var(--text-2); font-weight: 600; }
.dir { font-size: 10px; fill: var(--text-3); }
.line { fill: none; stroke: var(--text-2); stroke-width: 1.5; stroke-linejoin: round; }
.line2 { fill: none; stroke: var(--text-3); stroke-width: 1; stroke-dasharray: 3 3; }
.pt { fill: var(--text); stroke: var(--bg); stroke-width: 1; }
.pt.t-warn { fill: var(--warning); }
.pt.t-bad { fill: var(--danger); }
.pt.faded { opacity: 0.35; }
.pt.active { stroke: var(--accent); stroke-width: 2; }
.brk { stroke: var(--danger); stroke-width: 1.2; }
.brk-lbl { font-size: 10.5px; fill: var(--danger); font-weight: 600; }
.hot { fill: transparent; }
.hot:hover { fill: var(--surface-hover); }
</style>
