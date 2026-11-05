<template>
  <div class="hist">
    <svg :viewBox="`0 0 ${W} ${H}`" preserveAspectRatio="none" class="hist-svg" role="img" :aria-label="ariaLabel">
      <g v-for="(c, i) in hist.counts" :key="i">
        <rect
          :x="i * bw + 0.5"
          :y="H - barH(c)"
          :width="Math.max(0.5, bw - 1)"
          :height="barH(c)"
          class="bar"
          :class="{ 'is-median': i === medianBin, 'is-last': i === hist.counts.length - 1 }"
        >
          <title>{{ binLabel(i) }}：{{ c }} 次</title>
        </rect>
      </g>
      <line :x1="0" :x2="W" :y1="H - 0.5" :y2="H - 0.5" class="base" />
    </svg>
    <div class="hist-axis num">
      <span>0</span>
      <span>{{ fmt(hist.binWidth * Math.floor(hist.counts.length / 2)) }}</span>
      <span>{{ fmt(hist.binWidth * (hist.counts.length - 1)) }}+</span>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 直方图：第 i 个柱覆盖 [i × binWidth, (i + 1) × binWidth)，最后一柱包含溢出。
 * 中位数所在的柱用一级文字色，其他柱用灰色。
 */
import type { Histogram } from '@cs-analyst/shared-types';

const props = withDefaults(defineProps<{
  hist: Histogram;
  /** 中位数（用来高亮所在的柱） */
  median?: number | null;
  unit?: string;
  digits?: number;
}>(), {
  median: null,
  unit: '',
  digits: 0,
});

const W = 240;
const H = 48;
const bw = computed(() => W / Math.max(1, props.hist.counts.length));
const maxCount = computed(() => Math.max(1, ...props.hist.counts));

function barH(c: number) {
  return c === 0 ? 0 : Math.max(1.5, (c / maxCount.value) * (H - 2));
}

const medianBin = computed(() => {
  if (props.median == null || props.hist.binWidth <= 0) return -1;
  return Math.min(props.hist.counts.length - 1, Math.floor(props.median / props.hist.binWidth));
});

function fmt(v: number) {
  return `${v.toFixed(props.digits)}${props.unit}`;
}

function binLabel(i: number) {
  const n = props.hist.counts.length;
  const a = props.hist.binWidth * i;
  return i === n - 1 ? `≥ ${fmt(a)}` : `${fmt(a)} – ${fmt(a + props.hist.binWidth)}`;
}

const ariaLabel = computed(() => `分布直方图，共 ${props.hist.counts.reduce((a, b) => a + b, 0)} 个样本`);
</script>

<style scoped>
.hist { display: flex; flex-direction: column; gap: 2px; }
.hist-svg { width: 100%; height: 48px; display: block; }
.bar { fill: var(--border-strong); }
.bar:hover { fill: var(--text-3); }
.bar.is-median { fill: var(--text-2); }
.base { stroke: var(--border); stroke-width: 1; vector-effect: non-scaling-stroke; }
.hist-axis { display: flex; justify-content: space-between; font-size: 10px; color: var(--text-disabled); }
</style>
