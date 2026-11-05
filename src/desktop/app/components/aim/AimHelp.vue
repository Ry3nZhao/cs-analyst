<template>
  <span class="aim-help" tabindex="0" :aria-label="`${title ?? ''}说明`">
    <span class="aim-help-mark">?</span>
    <span class="aim-help-pop" :class="`align-${align}`" role="tooltip">
      <strong v-if="title" class="aim-help-title">{{ title }}</strong>
      <slot>{{ text }}</slot>
    </span>
  </span>
</template>

<script setup lang="ts">
/**
 * 指标旁的「?」：悬停或键盘聚焦时显示定义。
 * 文字放默认插槽或 text；align 控制弹层相对「?」的对齐方向，靠右的卡片用 right。
 */
withDefaults(defineProps<{
  text?: string;
  title?: string;
  align?: 'left' | 'right' | 'center';
}>(), {
  align: 'left',
});
</script>

<style scoped>
.aim-help {
  position: relative;
  display: inline-flex;
  align-items: center;
  vertical-align: middle;
  outline: none;
}
.aim-help-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-full);
  font-size: 9px;
  font-weight: 600;
  line-height: 1;
  color: var(--text-3);
  cursor: help;
}
.aim-help:hover .aim-help-mark,
.aim-help:focus-visible .aim-help-mark { color: var(--text); border-color: var(--text-3); }
.aim-help:focus-visible .aim-help-mark { box-shadow: var(--focus-ring); }

.aim-help-pop {
  position: absolute;
  top: calc(100% + 6px);
  z-index: 50;
  width: 280px;
  padding: var(--space-2) var(--space-3);
  background: var(--surface-raised);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: 0 8px 24px rgb(0 0 0 / 0.45);
  font-size: var(--text-xs);
  font-weight: 400;
  line-height: 1.55;
  color: var(--text-2);
  white-space: pre-line;
  text-align: left;
  letter-spacing: 0;
  visibility: hidden;
  opacity: 0;
  transition: opacity 0.12s;
  pointer-events: none;
}
.align-left { left: -6px; }
.align-right { right: -6px; }
.align-center { left: 50%; transform: translateX(-50%); }
.aim-help:hover .aim-help-pop,
.aim-help:focus-visible .aim-help-pop { visibility: visible; opacity: 1; }
.aim-help-title { display: block; margin-bottom: 2px; color: var(--text); font-weight: 600; }
.aim-help-pop :deep(p) { margin: 0 0 4px; }
.aim-help-pop :deep(p:last-child) { margin-bottom: 0; }
</style>
