<template>
  <span class="status-chip" :class="[`tone-${tone}`, { pulse }]" :title="title || undefined">
    <span v-if="dot" class="chip-dot" />
    <slot>{{ label }}</slot>
  </span>
</template>

<script setup lang="ts">
/**
 * 状态标签：表达一个对象当前所处的状态（排队、进行中、成功、失败……）。
 * - neutral 用于「无需关注」的状态（排队、跳过），不上色。
 * - info 用于进行中，配合 pulse。
 * - 不要把它当分类标签（来源、地图）用，分类标签用灰色的 .tag。
 */
withDefaults(defineProps<{
  tone?: 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';
  label?: string;
  dot?: boolean;
  pulse?: boolean;
  title?: string;
}>(), {
  tone: 'neutral',
  dot: true,
  pulse: false,
});
</script>

<style scoped>
.status-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 20px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  border: 1px solid var(--chip-line);
  background: var(--chip-soft);
  color: var(--chip);
  font-size: var(--text-xs);
  font-weight: 500;
  line-height: 1;
  white-space: nowrap;
}
.chip-dot { width: 6px; height: 6px; border-radius: 50%; background: currentColor; flex-shrink: 0; }
.pulse .chip-dot { animation: chip-pulse 1.4s ease-in-out infinite; }
@keyframes chip-pulse { 50% { opacity: 0.3; } }

.tone-neutral { --chip: var(--text-3);  --chip-soft: transparent;         --chip-line: var(--border); }
.tone-accent  { --chip: var(--accent);  --chip-soft: var(--accent-soft);  --chip-line: var(--accent-line); }
.tone-success { --chip: var(--success); --chip-soft: var(--success-soft); --chip-line: var(--success-line); }
.tone-warning { --chip: var(--warning); --chip-soft: var(--warning-soft); --chip-line: var(--warning-line); }
.tone-danger  { --chip: var(--danger);  --chip-soft: var(--danger-soft);  --chip-line: var(--danger-line); }
.tone-info    { --chip: var(--info);    --chip-soft: var(--info-soft);    --chip-line: var(--info-line); }
</style>
