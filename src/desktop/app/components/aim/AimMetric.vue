<template>
  <div class="aim-metric" :class="{ unavailable, compact }">
    <div class="m-head">
      <span class="m-label">{{ label }}</span>
      <MetricHelp v-if="metric" :metric="metric">
        <p v-if="note" class="m-note">{{ note }}</p>
      </MetricHelp>
      <AimHelp v-else-if="help" :title="label" :text="help" :align="helpAlign" />
    </div>
    <div class="m-value num">
      <template v-if="unavailable">
        <span class="m-na">不可用</span>
      </template>
      <template v-else>
        <span>{{ value ?? '—' }}</span>
        <span v-if="unit && value != null && value !== '—'" class="m-unit">{{ unit }}</span>
      </template>
    </div>
    <div v-if="unavailable" class="m-sub">{{ unavailableText }}</div>
    <div v-else-if="sub || $slots.sub" class="m-sub num"><slot name="sub">{{ sub }}</slot></div>
    <div v-if="!unavailable && $slots.default" class="m-chart"><slot /></div>
  </div>
</template>

<script setup lang="ts">
/**
 * 枪法指标卡：标签 + 「?」说明 + 数值 + 次要说明 + 可选图表（默认插槽）。
 * unavailable 时数值显示「不可用」而不是 0，图表不渲染。
 * 传 metric（utils/metricGlossary.ts 的 key）时用统一的 <MetricHelp> 说明，note 附在说明末尾；
 * 否则退回 help 文本。
 */
import { AIM_HELP } from '~/composables/useAim';

withDefaults(defineProps<{
  label: string;
  value?: string | number | null;
  unit?: string;
  sub?: string;
  help?: string;
  /** 指标释义 key，优先于 help */
  metric?: string;
  /** 附加在 MetricHelp 说明末尾的补充 */
  note?: string;
  helpAlign?: 'left' | 'right' | 'center';
  unavailable?: boolean;
  unavailableText?: string;
  compact?: boolean;
}>(), {
  value: null,
  helpAlign: 'left',
  unavailable: false,
  unavailableText: AIM_HELP.unavailable,
  compact: false,
});
</script>

<style scoped>
.aim-metric {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  padding: var(--space-3) var(--space-4);
  background: var(--surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
}
.m-head { display: flex; align-items: center; gap: 6px; font-size: var(--text-xs); color: var(--text-3); white-space: nowrap; }
.m-value { display: flex; align-items: baseline; gap: 2px; font-size: var(--text-2xl); font-weight: 600; line-height: 1.1; color: var(--text); }
.m-unit { font-size: var(--text-sm); font-weight: 500; color: var(--text-3); }
.m-na { font-size: var(--text-lg); font-weight: 500; color: var(--text-3); }
.m-sub { font-size: var(--text-xs); color: var(--text-3); line-height: 1.45; }
.unavailable .m-sub { color: var(--text-disabled); }
.m-note { margin: 0; white-space: pre-line; }
.m-chart { margin-top: var(--space-2); display: flex; flex-direction: column; gap: var(--space-2); }

.compact { padding: var(--space-2) var(--space-3); }
.compact .m-value { font-size: var(--text-xl); }
</style>
