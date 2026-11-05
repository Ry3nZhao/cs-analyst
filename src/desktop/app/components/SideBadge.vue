<template>
  <span class="side-badge" :class="[side, `v-${variant}`]" :title="tooltip">{{ text }}</span>
</template>

<script setup lang="ts">
/**
 * 阵营标记。side 接受 'T' / 'CT'，或 demo 里的队伍编号 2（T）/ 3（CT）。
 * variant:
 *   soft  默认，列表、计分板里用
 *   solid 强调，例如回合胜方
 *   bar   只显示一条竖色条，放在行首做分组
 */
const props = withDefaults(defineProps<{
  side: string | number | null | undefined;
  variant?: 'soft' | 'solid' | 'bar';
}>(), {
  variant: 'soft',
});

const side = computed(() => {
  const s = String(props.side ?? '').toLowerCase();
  if (s === 't' || s === '2') return 't';
  if (s === 'ct' || s === '3') return 'ct';
  return 'none';
});
const text = computed(() => {
  if (props.variant === 'bar') return '';
  return side.value === 'none' ? '—' : side.value.toUpperCase();
});
const tooltip = computed(() => ({ t: '恐怖分子 (T)', ct: '反恐精英 (CT)', none: undefined })[side.value]);
</script>

<style scoped>
.side-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 26px;
  height: 18px;
  padding: 0 5px;
  border-radius: var(--radius-sm);
  border: 1px solid transparent;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.04em;
  line-height: 1;
  vertical-align: middle;
}
.t    { --side: var(--t-side);  --side-soft: var(--t-side-soft);  --side-line: var(--t-side-line); }
.ct   { --side: var(--ct-side); --side-soft: var(--ct-side-soft); --side-line: var(--ct-side-line); }
.none { --side: var(--text-disabled); --side-soft: transparent;   --side-line: var(--border); }

.v-soft  { color: var(--side); background: var(--side-soft); border-color: var(--side-line); }
.v-solid { color: var(--text-inverse); background: var(--side); }
.v-bar   { min-width: 3px; width: 3px; height: 16px; padding: 0; border-radius: 2px; background: var(--side); }
</style>
