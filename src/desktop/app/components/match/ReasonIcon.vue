<template>
  <svg class="reason-icon" :width="size" :height="size" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" role="img" :aria-label="reasonLabel(reason)">
    <title>{{ reasonLabel(reason) }}</title>
    <!-- 全灭：骷髅 -->
    <template v-if="kind === 'elim'">
      <path d="M3.5 7.5a4.5 4.5 0 1 1 9 0c0 1.4-.6 2.3-1.5 2.9V12.5H5v-2.1c-.9-.6-1.5-1.5-1.5-2.9Z" />
      <circle cx="6.2" cy="7.4" r="1" fill="currentColor" stroke="none" />
      <circle cx="9.8" cy="7.4" r="1" fill="currentColor" stroke="none" />
      <path d="M7 12.5v1.5M9 12.5v1.5" />
    </template>
    <!-- 爆炸 -->
    <template v-else-if="kind === 'bomb'">
      <path d="M8 1.5 9.3 5l3.4-1.6L11.2 7l3.3 1.3-3.3 1.4 1.4 3.5-3.3-1.5L8 14.5 6.7 11.7l-3.4 1.5 1.5-3.5L1.5 8.3 4.8 7 3.3 3.4 6.7 5Z" />
    </template>
    <!-- 拆除：剪线钳 -->
    <template v-else-if="kind === 'defuse'">
      <circle cx="4.5" cy="11.5" r="2" />
      <circle cx="11.5" cy="11.5" r="2" />
      <path d="M6 10 12 2M10 10 4 2" />
    </template>
    <!-- 时间耗尽 -->
    <template v-else-if="kind === 'time'">
      <circle cx="8" cy="8.5" r="5.5" />
      <path d="M8 5.5v3l2 1.5M6.5 1.5h3" />
    </template>
    <template v-else>
      <circle cx="8" cy="8" r="1.5" fill="currentColor" stroke="none" />
    </template>
  </svg>
</template>

<script setup lang="ts">
import { reasonKind, reasonLabel } from '~/composables/useMatchDetail';

const props = withDefaults(defineProps<{ reason: string | null | undefined; size?: number }>(), { size: 14 });
const kind = computed(() => reasonKind(props.reason));
</script>

<style scoped>
.reason-icon { display: inline-block; flex-shrink: 0; vertical-align: middle; }
</style>
