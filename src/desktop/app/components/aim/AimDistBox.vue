<template>
  <div class="dist-box">
    <svg :viewBox="`0 0 ${W} ${H}`" preserveAspectRatio="none" class="dist-svg" aria-hidden="true">
      <line :x1="0" :x2="W" :y1="H / 2" :y2="H / 2" class="track" />
      <line v-if="signed" :x1="x(0)" :x2="x(0)" :y1="2" :y2="H - 2" class="zero" />
      <template v-if="ok">
        <rect
          :x="x(dist.p25!)"
          :y="5"
          :width="Math.max(1, x(dist.p75!) - x(dist.p25!))"
          :height="H - 10"
          class="box"
        />
        <line :x1="x(dist.median!)" :x2="x(dist.median!)" :y1="3" :y2="H - 3" class="median" />
        <circle v-if="dist.mean != null" :cx="x(dist.mean)" :cy="H / 2" r="2.6" class="mean" />
      </template>
    </svg>
    <div class="dist-axis num">
      <span>{{ fmt(lo) }}</span>
      <span>{{ fmt(hi) }}{{ overflow ? '+' : '' }}</span>
    </div>
    <div v-if="ok" class="dist-mid num">
      P25 {{ fmt(dist.p25) }} · 中位 <b>{{ fmt(dist.median) }}</b> · P75 {{ fmt(dist.p75) }}
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 箱线图（单行）：P25–P75 方框 + 中位数竖线 + 均值空心点。
 * 后端的 Dist 没有最小 / 最大值，所以不画须线；轴的范围用 max 指定，缺省取 P75 × 1.8。
 * signed：轴关于 0 对称，并画出 0 刻度（用于有正负的偏移）。
 */
import type { Dist } from '@cs-analyst/shared-types';

const props = withDefaults(defineProps<{
  dist: Dist;
  max?: number | null;
  signed?: boolean;
  unit?: string;
  digits?: number;
}>(), {
  max: null,
  signed: false,
  unit: '',
  digits: 0,
});

const W = 240;
const H = 22;

const ok = computed(() => props.dist.n > 0 && props.dist.median != null && props.dist.p25 != null && props.dist.p75 != null);

const hi = computed(() => {
  if (props.max != null) return props.max;
  const d = props.dist;
  const m = Math.max(Math.abs(d.p75 ?? 0), Math.abs(d.p25 ?? 0), Math.abs(d.mean ?? 0)) * 1.8;
  return m > 0 ? niceCeil(m) : 1;
});
const lo = computed(() => (props.signed ? -hi.value : 0));
const overflow = computed(() => ok.value && (props.dist.p75! > hi.value || (props.dist.mean ?? 0) > hi.value));

function niceCeil(v: number) {
  const p = 10 ** Math.floor(Math.log10(v));
  for (const s of [1, 2, 2.5, 5, 10]) if (s * p >= v) return s * p;
  return 10 * p;
}

function x(v: number) {
  const c = Math.min(hi.value, Math.max(lo.value, v));
  return ((c - lo.value) / (hi.value - lo.value)) * W;
}

function fmt(v: number | null | undefined) {
  if (v == null) return '—';
  return `${v.toFixed(props.digits)}${props.unit}`;
}
</script>

<style scoped>
.dist-box { display: flex; flex-direction: column; gap: 2px; }
.dist-svg { width: 100%; height: 22px; display: block; }
.track { stroke: var(--border); stroke-width: 1; vector-effect: non-scaling-stroke; }
.zero { stroke: var(--text-3); stroke-width: 1; stroke-dasharray: 2 2; vector-effect: non-scaling-stroke; }
.box { fill: var(--surface-active); stroke: var(--text-3); stroke-width: 1; vector-effect: non-scaling-stroke; }
.median { stroke: var(--text); stroke-width: 2; vector-effect: non-scaling-stroke; }
.mean { fill: var(--surface); stroke: var(--text-2); stroke-width: 1.2; vector-effect: non-scaling-stroke; }
.dist-axis { display: flex; justify-content: space-between; gap: var(--space-2); font-size: 10px; color: var(--text-disabled); }
.dist-mid { font-size: 10px; color: var(--text-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.dist-mid b { color: var(--text-2); font-weight: 600; }
</style>
