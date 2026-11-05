<template>
  <div class="trend">
    <svg v-if="points.length >= 1" :viewBox="`0 0 ${W} ${H}`" class="trend-svg" role="img" :aria-label="`${label}趋势`">
      <!-- 网格和刻度 -->
      <g v-for="t in ticks" :key="t.v">
        <line :x1="PL" :x2="W - PR" :y1="t.y" :y2="t.y" class="grid" />
        <text :x="PL - 6" :y="t.y" class="tick" text-anchor="end" dominant-baseline="middle">{{ t.label }}</text>
      </g>
      <!-- 最好成绩 -->
      <line v-if="best" :x1="PL" :x2="W - PR" :y1="best.y" :y2="best.y" class="best" />
      <polyline :points="line" class="line" />
      <circle
        v-for="(p, i) in points"
        :key="i"
        :cx="p.x"
        :cy="p.y"
        :r="i === points.length - 1 ? 4 : 2.5"
        :class="i === points.length - 1 ? 'dot last' : 'dot'"
      >
        <title>{{ p.title }}</title>
      </circle>
      <text :x="PL" :y="H - 4" class="tick">{{ firstDate }}</text>
      <text :x="W - PR" :y="H - 4" class="tick" text-anchor="end">{{ lastDate }}</text>
    </svg>
    <div v-else class="trend-empty">还没有记录</div>
    <div v-if="best" class="trend-legend">
      <span class="legend-best" />最好 <span class="num">{{ fmt(best.v) }}{{ unit }}</span>
      <span class="legend-sep">·</span>{{ points.length }} 次
    </div>
  </div>
</template>

<script setup lang="ts">
/** 训练成绩趋势：自己画的 SVG 折线，按时间顺序（等距排列每次训练）。 */
const props = defineProps<{
  values: { at: string; score: number | null }[];
  label: string;
  unit: string;
  higherIsBetter: boolean;
}>();

const W = 640, H = 180, PL = 44, PR = 12, PT = 12, PB = 22;

const valid = computed(() => props.values.filter((v): v is { at: string; score: number } => v.score != null));

const range = computed(() => {
  const vs = valid.value.map((v) => v.score);
  if (!vs.length) return { lo: 0, hi: 1 };
  let lo = Math.min(...vs), hi = Math.max(...vs);
  if (hi - lo < 1e-6) { lo -= 1; hi += 1; }
  const pad = (hi - lo) * 0.12;
  return { lo: lo - pad, hi: hi + pad };
});

function y(v: number) {
  const { lo, hi } = range.value;
  return PT + (1 - (v - lo) / (hi - lo)) * (H - PT - PB);
}

function fmt(v: number) {
  return Math.abs(v) >= 100 ? Math.round(v).toString() : v.toFixed(1);
}

const points = computed(() => {
  const n = valid.value.length;
  return valid.value.map((v, i) => ({
    x: n === 1 ? (PL + W - PR) / 2 : PL + (i / (n - 1)) * (W - PL - PR),
    y: y(v.score),
    title: `${new Date(v.at).toLocaleString()}  ${fmt(v.score)}${props.unit}`,
  }));
});

const line = computed(() => points.value.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' '));

const ticks = computed(() => {
  const { lo, hi } = range.value;
  return [0, 0.5, 1].map((k) => {
    const v = lo + (hi - lo) * k;
    return { v, y: y(v), label: fmt(v) };
  });
});

const best = computed(() => {
  const vs = valid.value.map((v) => v.score);
  if (!vs.length) return null;
  const v = props.higherIsBetter ? Math.max(...vs) : Math.min(...vs);
  return { v, y: y(v) };
});

const day = (s?: string) => (s ? new Date(s).toLocaleDateString() : '');
const firstDate = computed(() => day(valid.value[0]?.at));
const lastDate = computed(() => (valid.value.length > 1 ? day(valid.value[valid.value.length - 1]?.at) : ''));
</script>

<style scoped>
.trend-svg { width: 100%; height: auto; display: block; overflow: visible; }
.grid { stroke: var(--border-subtle); stroke-width: 1; vector-effect: non-scaling-stroke; }
.best { stroke: var(--text-3); stroke-width: 1; stroke-dasharray: 3 4; vector-effect: non-scaling-stroke; }
.line { fill: none; stroke: var(--text-2); stroke-width: 1.5; vector-effect: non-scaling-stroke; }
.dot { fill: var(--text-2); }
.dot.last { fill: var(--accent); }
.tick { fill: var(--text-3); font-size: 10px; font-variant-numeric: tabular-nums; }
.trend-empty { height: 120px; display: flex; align-items: center; justify-content: center; color: var(--text-disabled); font-size: var(--text-sm); }
.trend-legend { margin-top: var(--space-2); font-size: var(--text-xs); color: var(--text-3); display: flex; align-items: center; gap: 6px; }
.legend-best { display: inline-block; width: 14px; border-top: 1px dashed var(--text-3); }
.legend-sep { color: var(--text-disabled); }
</style>
