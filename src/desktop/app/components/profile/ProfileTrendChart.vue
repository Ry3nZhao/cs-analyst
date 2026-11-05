<template>
  <div ref="root" class="ptc">
    <svg v-if="hasData" :viewBox="`0 0 ${W} ${H}`" class="ptc-svg" role="img" :aria-label="`${label}趋势`">
      <line v-for="t in yTicks" :key="t" :x1="L" :x2="W - R" :y1="y(t)" :y2="y(t)" class="grid" />
      <text v-for="t in yTicks" :key="`l${t}`" :x="L - 6" :y="y(t) + 3" text-anchor="end" class="tick">{{ fmtTick(t) }}</text>
      <line v-if="median != null" :x1="L" :x2="W - R" :y1="y(median)" :y2="y(median)" class="median" />
      <text v-if="median != null" :x="W - R" :y="y(median) - 4" text-anchor="end" class="median-label">样本中位数</text>
      <polyline v-for="(seg, i) in segments" :key="`s${i}`" :points="seg" class="line" />
      <g v-for="(m, i) in matches" :key="m.matchId">
        <circle v-if="values[i] != null" :cx="x(i)" :cy="y(values[i]!)" r="2.6" class="pt" />
        <circle v-else :cx="x(i)" :cy="H - B + 1" r="2" class="pt-na" />
        <circle v-if="rolling[i] != null" :cx="x(i)" :cy="y(rolling[i]!)" r="3.4" class="pt-roll" />
        <rect :x="x(i) - colW / 2" :y="0" :width="colW" :height="H - B" class="hot" @click="emit('open', m.matchId)">
          <title>{{ tip(i) }}</title>
        </rect>
      </g>
      <text :x="L" :y="H - 3" class="tick">较早</text>
      <text :x="W - R" :y="H - 3" text-anchor="end" class="tick">最近</text>
    </svg>
    <div v-else class="ptc-empty">{{ matches.length ? `${label}在这些对局里都不可用` : '没有对局' }}</div>
    <div v-if="hasData" class="legend">
      <span><i class="lg-line" />最近 {{ window }} 场滑动平均</span>
      <span><i class="lg-dot" />单场</span>
      <span><i class="lg-na" />该场不可用</span>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 个人主页趋势：单场小点 + 滑动平均折线（旧 → 新）。缺值不连线、不当 0，在 x 轴上画灰点。
 * 点击某场的竖条发出 `open(matchId)`。
 */
import type { ProfileMetricKey, ProfileTrendMatchDto } from '@cs-analyst/shared-types';
import { useElementWidth } from '~/composables/useElementWidth';
import { fmtMetric, mapShort, resultLabel, shortDate } from '~/composables/useProfile';

const props = defineProps<{
  metric: ProfileMetricKey;
  label: string;
  unit: string;
  window: number;
  matches: ProfileTrendMatchDto[];
  values: (number | null)[];
  rolling: (number | null)[];
  median?: number | null;
}>();
const emit = defineEmits<{ open: [matchId: string] }>();

const root = ref<HTMLElement | null>(null);
const W = useElementWidth(root);
const H = 180;
const L = 40;
const R = 12;
const B = 16;

const all = computed(() =>
  [...props.values, ...props.rolling, ...(props.median != null ? [props.median] : [])].filter((v): v is number => v != null),
);
const hasData = computed(() => props.values.some((v) => v != null));
const range = computed<[number, number]>(() => {
  const vs = all.value;
  if (!vs.length) return [0, 1];
  let lo = Math.min(...vs);
  let hi = Math.max(...vs);
  if (['kast', 'openingDuel', 'clutch', 'counterStrafe'].includes(props.metric)) {
    lo = Math.max(0, Math.min(lo, 0));
    hi = Math.min(100, Math.max(hi, 100));
  } else {
    const pad = (hi - lo) * 0.15 || Math.abs(hi) * 0.2 || 1;
    lo = props.metric === 'swing' ? lo - pad : Math.max(0, lo - pad);
    hi += pad;
  }
  return [lo, hi];
});
const colW = computed(() => (W.value - L - R) / Math.max(1, props.matches.length));
function x(i: number) {
  return L + colW.value / 2 + i * colW.value;
}
function y(v: number) {
  const [lo, hi] = range.value;
  return 8 + (1 - (v - lo) / (hi - lo || 1)) * (H - B - 16);
}
const yTicks = computed(() => {
  const [lo, hi] = range.value;
  return [lo, (lo + hi) / 2, hi];
});
const segments = computed(() => {
  const out: string[] = [];
  let cur: string[] = [];
  props.rolling.forEach((v, i) => {
    if (v == null) {
      if (cur.length) out.push(cur.join(' '));
      cur = [];
    } else cur.push(`${x(i).toFixed(1)},${y(v).toFixed(1)}`);
  });
  if (cur.length) out.push(cur.join(' '));
  return out.filter((s) => s.includes(' '));
});

function fmtTick(v: number) {
  return Math.abs(v) >= 10 ? v.toFixed(0) : v.toFixed(1);
}
function tip(i: number) {
  const m = props.matches[i]!;
  const v = props.values[i];
  const r = props.rolling[i];
  return [
    `${shortDate(m.at)} · ${mapShort(m.mapName)} · ${resultLabel(m.result)}`,
    `单场：${v == null ? '不可用' : fmtMetric(props.metric, v) + props.unit}`,
    `滑动平均：${r == null ? '不可用' : fmtMetric(props.metric, r) + props.unit}`,
    '点击打开比赛详情',
  ].join('\n');
}
</script>

<style scoped>
.ptc-svg { width: 100%; height: auto; display: block; }
.ptc-empty { padding: var(--space-6) 0; text-align: center; font-size: var(--text-sm); color: var(--text-3); }
.grid { stroke: var(--border-subtle); stroke-width: 1; }
.tick { font-size: 10px; fill: var(--text-3); font-variant-numeric: tabular-nums; }
.median { stroke: var(--text-3); stroke-width: 1; stroke-dasharray: 3 3; }
.median-label { font-size: 10px; fill: var(--text-3); }
.line { fill: none; stroke: var(--accent); stroke-width: 1.8; stroke-linejoin: round; }
.pt { fill: var(--text-3); }
.pt-roll { fill: var(--surface); stroke: var(--accent); stroke-width: 1.5; }
.pt-na { fill: var(--text-disabled); }
.hot { fill: transparent; cursor: pointer; }
.hot:hover { fill: var(--surface-hover); }
.legend { display: flex; gap: var(--space-4); margin-top: var(--space-2); font-size: var(--text-xs); color: var(--text-3); }
.legend span { display: inline-flex; align-items: center; gap: 6px; }
.lg-line { width: 14px; height: 2px; background: var(--accent); display: inline-block; }
.lg-dot { width: 5px; height: 5px; border-radius: 50%; background: var(--text-3); display: inline-block; }
.lg-na { width: 4px; height: 4px; border-radius: 50%; background: var(--text-disabled); display: inline-block; }
</style>
