<template>
  <div class="stat-tile" :class="[`tone-${tone}`, { compact }]">
    <div class="stat-label">
      <span v-if="tone !== 'neutral'" class="stat-mark" />
      {{ label }}
      <MetricHelp v-if="metric" :metric="metric" />
    </div>
    <div class="stat-value num">
      <slot>{{ value }}</slot>
      <span v-if="unit" class="stat-unit">{{ unit }}</span>
    </div>
    <div v-if="hint || delta != null || $slots.hint" class="stat-hint">
      <span v-if="delta != null" class="stat-delta num" :class="deltaClass">
        {{ delta > 0 ? '+' : '' }}{{ delta }}
      </span>
      <slot name="hint">{{ hint }}</slot>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 数据卡：一个指标一张卡。
 * - 数字本身始终是一级文字色，颜色只出现在标签前的小色块上，用来表明语义。
 * - delta 的好坏由 higherIsBetter 决定：变好用 success，变差用 danger。
 * - 传 metric 时标签旁显示 <MetricHelp>。
 */
const props = withDefaults(defineProps<{
  label: string;
  value?: string | number | null;
  unit?: string;
  hint?: string;
  tone?: 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';
  delta?: number | null;
  higherIsBetter?: boolean;
  compact?: boolean;
  /** utils/metricGlossary.ts 里的指标 key：标签旁显示「?」说明 */
  metric?: string;
}>(), {
  value: '—',
  tone: 'neutral',
  delta: null,
  higherIsBetter: true,
});

const deltaClass = computed(() => {
  if (props.delta == null || props.delta === 0) return '';
  return (props.delta > 0) === props.higherIsBetter ? 'good' : 'bad';
});
</script>

<style scoped>
.stat-tile {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  padding: var(--space-3) var(--space-4);
  background: var(--surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
}
.stat-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-xs);
  color: var(--text-3);
  white-space: nowrap;
}
.stat-mark { width: 6px; height: 6px; border-radius: 2px; flex-shrink: 0; background: var(--tone); }
.stat-value {
  font-size: var(--text-2xl);
  font-weight: 600;
  line-height: 1.1;
  color: var(--text);
  letter-spacing: -0.01em;
}
.stat-unit { margin-left: 2px; font-size: var(--text-sm); font-weight: 500; color: var(--text-3); }
.stat-hint { display: flex; gap: 6px; font-size: var(--text-xs); color: var(--text-3); }
.stat-delta { color: var(--text-2); }
.stat-delta.good { color: var(--value-good); }
.stat-delta.bad { color: var(--value-bad); }

.compact { padding: var(--space-2) var(--space-3); }
.compact .stat-value { font-size: var(--text-xl); }

.tone-accent  { --tone: var(--accent); }
.tone-success { --tone: var(--success); }
.tone-warning { --tone: var(--warning); }
.tone-danger  { --tone: var(--danger); }
.tone-info    { --tone: var(--info); }
</style>
