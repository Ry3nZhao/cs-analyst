<template>
  <svg v-if="points.length > 1" :viewBox="`0 0 ${W} ${H}`" class="spark" role="img" :aria-label="label">
    <polyline :points="line" class="spark-line" />
    <circle :cx="xs[xs.length - 1]" :cy="ys[ys.length - 1]" r="2.5" class="spark-end" />
    <title>{{ title }}</title>
  </svg>
  <span v-else class="spark-one">仅 1 场有分数</span>
</template>

<script setup lang="ts">
/** 平台分数历史的迷你折线（旧 → 新）。 */
import type { ScorePointDto } from '@cs-analyst/shared-types';
import { shortDate } from '~/composables/useProfile';

const props = defineProps<{ points: ScorePointDto[]; label: string }>();
const W = 120;
const H = 32;
const xs = computed(() => props.points.map((_, i) => 3 + (i * (W - 6)) / Math.max(1, props.points.length - 1)));
const ys = computed(() => {
  const v = props.points.map((p) => p.value);
  const lo = Math.min(...v);
  const hi = Math.max(...v);
  return v.map((x) => (hi === lo ? H / 2 : 3 + (1 - (x - lo) / (hi - lo)) * (H - 6)));
});
const line = computed(() => xs.value.map((x, i) => `${x.toFixed(1)},${ys.value[i]!.toFixed(1)}`).join(' '));
const title = computed(() => props.points.map((p) => `${shortDate(p.at)}  ${p.value}`).join('\n'));
</script>

<style scoped>
.spark { width: 120px; height: 32px; display: block; }
.spark-line { fill: none; stroke: var(--text-2); stroke-width: 1.5; stroke-linejoin: round; }
.spark-end { fill: var(--accent); }
.spark-one { font-size: var(--text-xs); color: var(--text-3); }
</style>
