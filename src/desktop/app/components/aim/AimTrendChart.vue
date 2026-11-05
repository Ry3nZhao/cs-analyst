<template>
  <div ref="root" class="trend">
    <svg v-if="valid.length" :viewBox="`0 0 ${W} ${H}`" class="tr-svg" role="img" :aria-label="`${label}趋势`">
      <line v-for="t in yTicks" :key="t" :x1="L" :x2="W - R" :y1="y(t)" :y2="y(t)" class="grid" />
      <text v-for="t in yTicks" :key="`l${t}`" :x="L - 6" :y="y(t) + 3" text-anchor="end" class="tick">{{ fmt(t) }}</text>
      <polyline v-for="(seg, i) in segments" :key="`s${i}`" :points="seg" class="line" />
      <g v-for="(p, i) in points" :key="p.matchId">
        <circle v-if="vals[i] != null" :cx="x(i)" :cy="y(vals[i]!)" r="3.2" class="pt" />
        <circle v-else :cx="x(i)" :cy="H - B + 1" r="2" class="pt-na" />
        <rect :x="x(i) - colW / 2" :y="0" :width="colW" :height="H - B" class="hot">
          <title>{{ tipFor(p, i) }}</title>
        </rect>
      </g>
      <text :x="L" :y="H - 3" class="tick">较早</text>
      <text :x="W - R" :y="H - 3" text-anchor="end" class="tick">最近</text>
    </svg>
    <div v-else class="tr-empty">{{ points.length ? '这项指标在所选对局里都不可用' : '没有对局' }}</div>
  </div>
</template>

<script setup lang="ts">
/**
 * 按对局的趋势折线（旧 → 新）。缺值的对局不连线，在 x 轴上画一个灰点，悬停说明原因。
 */
import type { AimTrendPoint } from '@cs-analyst/shared-types';
import { matchLabel } from '~/composables/useAim';
import { useElementWidth } from '~/composables/useElementWidth';

const props = withDefaults(defineProps<{
  points: AimTrendPoint[];
  metric: 'sprayControl' | 'accuracy' | 'counterStrafeRate' | 'ttdMedianMs' | 'reactionMedianMs' | 'crosshairMedianDeg';
  label: string;
  unit?: string;
  /** 比率类 0–1 → 显示成百分数 */
  percent?: boolean;
}>(), {
  unit: '',
  percent: false,
});

const root = ref<HTMLElement | null>(null);
const width = useElementWidth(root);
const W = computed(() => width.value);
const H = 140;
const L = 36;
const R = 10;
const B = 16;

const vals = computed(() =>
  props.points.map((p) => {
    const v = p[props.metric] as number | null;
    return v == null ? null : props.percent ? v * 100 : v;
  }),
);
const valid = computed(() => vals.value.filter((v): v is number => v != null));

const range = computed(() => {
  if (props.percent || props.metric === 'sprayControl') return [0, 100] as const;
  const hi = Math.max(...valid.value, 1);
  return [0, Math.ceil(hi * 1.15)] as const;
});

const colW = computed(() => (W.value - L - R) / Math.max(1, props.points.length));
function x(i: number) {
  const n = props.points.length;
  return n <= 1 ? (L + W.value - R) / 2 : L + colW.value / 2 + i * colW.value;
}
function y(v: number) {
  const [lo, hi] = range.value;
  return 6 + (1 - (v - lo) / (hi - lo)) * (H - B - 12);
}
const yTicks = computed(() => {
  const [lo, hi] = range.value;
  return [lo, (lo + hi) / 2, hi];
});
const segments = computed(() => {
  const out: string[] = [];
  let cur: string[] = [];
  vals.value.forEach((v, i) => {
    if (v == null) {
      if (cur.length > 1) out.push(cur.join(' '));
      cur = [];
    } else {
      cur.push(`${x(i).toFixed(1)},${y(v).toFixed(1)}`);
    }
  });
  if (cur.length > 1) out.push(cur.join(' '));
  return out;
});

function fmt(v: number) {
  return Math.abs(v) >= 10 || Number.isInteger(v) ? v.toFixed(0) : v.toFixed(1);
}
function tipFor(p: AimTrendPoint, i: number) {
  const v = vals.value[i];
  return `${matchLabel(p)}
${props.label}：${v == null ? naText(p) : `${fmt(v)}${props.unit}`}`;
}
function naText(p: AimTrendPoint) {
  const vis = ['ttdMedianMs', 'reactionMedianMs', 'crosshairMedianDeg'].includes(props.metric);
  if (vis && !p.visibilityReliable) return '不可用（可见性数据不可靠）';
  return '没有样本';
}
</script>

<style scoped>
.tr-svg { width: 100%; height: auto; display: block; }
.tr-empty { padding: var(--space-5) 0; text-align: center; font-size: var(--text-sm); color: var(--text-3); }
.grid { stroke: var(--border-subtle); stroke-width: 1; }
.tick { font-size: 10px; fill: var(--text-3); font-variant-numeric: tabular-nums; }
.line { fill: none; stroke: var(--text-2); stroke-width: 1.5; stroke-linejoin: round; }
.pt { fill: var(--text); stroke: var(--surface); stroke-width: 1; }
.pt-na { fill: var(--text-disabled); }
.hot { fill: transparent; }
.hot:hover { fill: var(--surface-hover); }
</style>
