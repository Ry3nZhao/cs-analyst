<template>
  <div class="spray-plot">
    <svg :viewBox="`${-S} ${-S} ${2 * S} ${2 * S}`" class="plot" role="img" aria-label="弹着点">
      <!-- 2° 容差圈与十字 -->
      <circle :r="2 * k" class="ring" />
      <line :x1="-S" :x2="S" y1="0" y2="0" class="axis" />
      <line x1="0" y1="-S" x2="0" :y2="S" class="axis" />
      <!-- 不压枪时的弹道 -->
      <polyline :points="recoilLine" class="recoil" />
      <circle v-for="(p, i) in recoilPts" :key="`r${i}`" :cx="p[0]" :cy="p[1]" r="2" class="recoil-dot" />
      <!-- 每一发的落点（相对第一发） -->
      <circle v-for="(p, i) in bullets" :key="`b${i}`" :cx="p[0]" :cy="p[1]" r="2.6" :class="p[2] ? 'bullet bad' : 'bullet'" />
    </svg>
    <div class="legend">
      <span><i class="lg-recoil" />不压枪时的弹道</span>
      <span><i class="lg-bullet" />你的落点（相对第一发）</span>
      <span><i class="lg-ring" />2° 容差</span>
    </div>
  </div>
</template>

<script setup lang="ts">
/** 压枪结果：所有连发的落点（err）叠在一起，和原始弹道对比。单位为度，向上为正。 */
const props = defineProps<{
  bulletsV: number[];
  bulletsH: number[];
  recoil: [number, number][];
}>();

const S = 100;
const span = computed(() => {
  let m = 3;
  for (const [v, h] of props.recoil) m = Math.max(m, Math.abs(v), Math.abs(h));
  for (let i = 0; i < props.bulletsV.length; i++) m = Math.max(m, Math.abs(props.bulletsV[i]!), Math.abs(props.bulletsH[i]!));
  return Math.min(m * 1.1, 20);
});
const k = computed(() => S / span.value);
const clamp = (x: number) => Math.max(-S, Math.min(S, x));

const recoilPts = computed(() => props.recoil.map(([v, h]) => [clamp(h * k.value), clamp(-v * k.value)] as const));
const recoilLine = computed(() => recoilPts.value.map((p) => `${p[0].toFixed(1)},${p[1].toFixed(1)}`).join(' '));
const bullets = computed(() =>
  props.bulletsV.map((v, i) => {
    const h = props.bulletsH[i]!;
    return [clamp(h * k.value), clamp(-v * k.value), Math.hypot(v, h) > 2] as const;
  }),
);
</script>

<style scoped>
.spray-plot { display: flex; flex-direction: column; align-items: center; gap: var(--space-2); }
.plot { width: 100%; max-width: 320px; height: auto; background: var(--bg-sunken); border-radius: var(--radius-md); }
.ring { fill: none; stroke: var(--border-strong); stroke-dasharray: 3 3; }
.axis { stroke: var(--border-subtle); }
.recoil { fill: none; stroke: var(--text-3); stroke-width: 1; stroke-dasharray: 4 3; }
.recoil-dot { fill: var(--text-3); }
.bullet { fill: var(--text); opacity: 0.8; }
.bullet.bad { fill: var(--danger); }
.legend { display: flex; gap: var(--space-4); font-size: var(--text-xs); color: var(--text-3); flex-wrap: wrap; justify-content: center; }
.legend i { display: inline-block; width: 10px; height: 10px; margin-right: 4px; vertical-align: -1px; border-radius: 50%; }
.lg-recoil { background: var(--text-3); }
.lg-bullet { background: var(--text); }
.lg-ring { border: 1px dashed var(--border-strong); }
</style>
