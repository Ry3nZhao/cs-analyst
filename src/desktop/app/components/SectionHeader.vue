<template>
  <div class="section-header" :class="`size-${size}`">
    <div class="section-text">
      <component :is="size === 'lg' ? 'h2' : 'h3'" class="section-title">
        {{ title }}
        <span v-if="count != null" class="section-count num">{{ count }}</span>
      </component>
      <p v-if="description" class="section-desc">{{ description }}</p>
    </div>
    <div v-if="$slots.default" class="section-actions">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 区块标题：标题 + 可选计数 + 可选说明 + 右侧操作区（默认插槽）。
 * size="lg" 用于页面内的一级区块，"md"（默认）用于卡片内部。
 */
withDefaults(defineProps<{
  title: string;
  description?: string;
  count?: number | null;
  size?: 'md' | 'lg';
}>(), {
  size: 'md',
  count: null,
});
</script>

<style scoped>
.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  margin-bottom: var(--space-3);
}
.section-text { min-width: 0; }
.section-title {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-md);
  font-weight: 600;
  color: var(--text);
}
.size-lg .section-title { font-size: var(--text-lg); }
.section-count { font-size: var(--text-xs); font-weight: 500; color: var(--text-3); }
.section-desc { margin: 2px 0 0; font-size: var(--text-xs); color: var(--text-3); }
.section-actions { display: flex; align-items: center; gap: var(--space-2); flex-shrink: 0; }
</style>
