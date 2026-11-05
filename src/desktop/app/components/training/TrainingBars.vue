<template>
  <svg :viewBox="`0 0 ${W} ${H}`" class="bars" role="img">
    <line :x1="0" :x2="W" :y1="H - PB" :y2="H - PB" class="axis" />
    <g v-for="(b, i) in bars" :key="i">
      <rect :x="b.x" :y="b.y" :width="bw" :height="b.h" :class="b.cls">
        <title>{{ b.title }}</title>
      </rect>
      <text v-if="showLabels" :x="b.x + bw / 2" :y="b.y - 4" class="lbl" text-anchor="middle">{{ b.label }}</text>
    </g>
    <text v-for="(t, i) in xTicks" :key="`x${i}`" :x="t.x" :y="H - 6" class="tick" text-anchor="middle">{{ t.label }}</text>
  </svg>
</template>

<script setup lang="ts">
/** 简单柱状图：每次尝试一根柱子；ok === false 的画成红色短柱（犯规 / 未命中）。 */
const props = withDefaults(defineProps<{
  values: { value: number | null; ok?: boolean | null }[];
  unit?: string;
  /** 固定纵轴上限（例如百分比用 100） */
  max?: number | null;
  digits?: number;
}>(), { unit: '', max: null, digits: 0 });

const W = 640, H = 150, PB = 20, PT = 16;

const vmax = computed(() => props.max ?? Math.max(1, ...props.values.map((v) => v.value ?? 0)) * 1.1);
const bw = computed(() => Math.max(3, Math.min(28, (W / Math.max(1, props.values.length)) * 0.7)));
const step = computed(() => W / Math.max(1, props.values.length));
const showLabels = computed(() => props.values.length <= 20);

const bars = computed(() =>
  props.values.map((v, i) => {
    const x = i * step.value + (step.value - bw.value) / 2;
    const val = v.value;
    const h = val == null ? 4 : Math.max(1, (val / vmax.value) * (H - PB - PT));
    return {
      x,
      y: H - PB - h,
      h,
      cls: val == null || v.ok === false ? 'bar bad' : 'bar',
      label: val == null ? '×' : val.toFixed(props.digits),
      title: val == null ? `#${i + 1} 无效` : `#${i + 1}  ${val.toFixed(props.digits)}${props.unit}`,
    };
  }),
);

const xTicks = computed(() => {
  const n = props.values.length;
  if (n <= 1) return [];
  const every = n <= 10 ? 1 : n <= 30 ? 5 : 10;
  const out: { x: number; label: string }[] = [];
  for (let i = 0; i < n; i += every) out.push({ x: i * step.value + step.value / 2, label: String(i + 1) });
  return out;
});
</script>

<style scoped>
.bars { width: 100%; height: auto; display: block; }
.axis { stroke: var(--border); }
.bar { fill: var(--text-3); }
.bar.bad { fill: var(--danger); }
.lbl { fill: var(--text-2); font-size: 10px; font-variant-numeric: tabular-nums; }
.tick { fill: var(--text-3); font-size: 10px; font-variant-numeric: tabular-nums; }
</style>
