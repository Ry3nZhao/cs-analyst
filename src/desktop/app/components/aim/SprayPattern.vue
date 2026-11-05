<template>
  <div class="spray-pattern">
    <div class="sp-stats">
      <AimMetric label="连发次数" :value="pattern.sprays" :sub="`${pattern.shots} 发 · 命中 ${pattern.hits}`" compact />
      <AimMetric label="压枪控制" :value="fmtNum(pattern.control)" :help="AIM_HELP.sprayControl" compact />
      <AimMetric label="平均误差" :value="pattern.meanErr == null ? null : pattern.meanErr.toFixed(2)" unit="°" :help="AIM_HELP.sprayErr" compact />
      <AimMetric
        label="开始崩"
        :value="pattern.breakShot == null ? null : `第 ${pattern.breakShot} 发`"
        :sub="breakSub"
        :help="AIM_HELP.breakShot"
        help-align="right"
        compact
      />
    </div>
    <div v-if="pattern.sprays < 3" class="callout callout-info sp-note">
      只有 {{ pattern.sprays }} 次连发，平均弹道参考价值有限；打到某一发的连发少于 3 次时，该点画成半透明。
    </div>
    <div class="sp-body">
      <div class="sp-chart">
        <SectionHeader title="平均弹道" :description="`${weaponName(pattern.weapon)} · 第一发为原点`" />
        <AimSprayChart v-model:active="active" :bullets="pattern.bullets" :break-shot="pattern.breakShot" />
      </div>
      <div class="sp-err">
        <SectionHeader title="第 N 发的误差" description="超过 ±2° 的区域视为没压住" />
        <AimSprayErrorChart v-model:active="active" :bullets="pattern.bullets" :break-shot="pattern.breakShot" />
        <div class="sp-verdict">
          <div v-for="seg in verdicts" :key="seg.label" class="sv-row">
            <span class="sv-label">{{ seg.label }}</span>
            <span class="sv-text" :class="seg.tone">{{ seg.text }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 按武器的压枪汇总：四个数字 + 平均弹道 + 第 N 发误差曲线 + 分段结论（前段 / 中段 / 后段 压多了还是压少了）。
 * 比赛详情页可以复用：<AimSprayPattern :pattern="pattern" />（pattern 来自 get_sprays 的 patterns）。
 */
import type { WeaponPattern } from '@cs-analyst/shared-types';
import { AIM_HELP, fmtNum, weaponName } from '~/composables/useAim';

const props = defineProps<{ pattern: WeaponPattern }>();
const active = ref<number | null>(null);

const breakSub = computed(() => {
  if (props.pattern.breakShot != null) return '误差中位数首次超过 2°';
  const reached = props.pattern.bullets.filter((b) => b.n >= 3).length;
  return reached ? `前 ${reached} 发都在 2° 以内` : '样本不足';
});

// 把弹道分成三段，看每段垂直 / 水平的平均偏差
const verdicts = computed(() => {
  const bs = props.pattern.bullets.filter((b) => b.idx > 1 && b.n >= Math.min(3, props.pattern.sprays));
  if (!bs.length) return [];
  const segs: [string, number, number][] = [
    ['第 2–5 发', 2, 5],
    ['第 6–10 发', 6, 10],
    ['第 11 发以后', 11, 999],
  ];
  const out: { label: string; text: string; tone: string }[] = [];
  for (const [label, a, b] of segs) {
    const xs = bs.filter((x) => x.idx >= a && x.idx <= b);
    if (!xs.length) continue;
    const v = xs.reduce((s, x) => s + x.errV, 0) / xs.length;
    const h = xs.reduce((s, x) => s + x.errH, 0) / xs.length;
    const parts: string[] = [];
    let worst = 0;
    if (Math.abs(v) > 0.5) {
      parts.push(`${v > 0 ? '压少了' : '压多了'} ${Math.abs(v).toFixed(1)}°`);
      worst = Math.max(worst, Math.abs(v));
    }
    if (Math.abs(h) > 0.5) {
      parts.push(`${h > 0 ? '偏右' : '偏左'} ${Math.abs(h).toFixed(1)}°`);
      worst = Math.max(worst, Math.abs(h));
    }
    out.push({
      label,
      text: parts.length ? parts.join('，') : '基本到位',
      tone: worst > 2 ? 'bad' : worst > 1 ? 'warn' : '',
    });
  }
  return out;
});
</script>

<style scoped>
.spray-pattern { display: flex; flex-direction: column; gap: var(--space-3); }
.sp-stats { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: var(--space-3); }
.sp-note { font-size: var(--text-xs); }
.sp-body { display: grid; grid-template-columns: minmax(260px, 360px) minmax(0, 1fr); gap: var(--space-5); align-items: start; }
.sp-err { min-width: 0; }
.sp-verdict { display: flex; flex-direction: column; gap: 4px; margin-top: var(--space-3); padding-top: var(--space-3); border-top: 1px solid var(--border-subtle); }
.sv-row { display: flex; gap: var(--space-3); font-size: var(--text-xs); }
.sv-label { width: 80px; color: var(--text-3); }
.sv-text { color: var(--text-2); }
.sv-text.warn { color: var(--warning); }
.sv-text.bad { color: var(--danger); }
@media (max-width: 1000px) {
  .sp-body { grid-template-columns: 1fr; }
  .sp-stats { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
</style>
