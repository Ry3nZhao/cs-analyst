<template>
  <span class="result" :class="cls">{{ text }}</span>
</template>

<script setup lang="ts">
/** 胜 / 负 / 平：接受 1 / 0 / -1 或 'win' / 'draw' / 'loss' */
const props = defineProps<{ result: number | string }>();
const cls = computed(() => {
  const r = props.result;
  if (r === 1 || r === 'win') return 'good';
  if (r === -1 || r === 'loss') return 'bad';
  return 'muted';
});
const text = computed(() => ({ good: '胜', bad: '负', muted: '平' })[cls.value]);
</script>

<style scoped>
.result { font-weight: 600; font-size: var(--text-sm); }
</style>
