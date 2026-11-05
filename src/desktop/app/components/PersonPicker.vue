<template>
  <div class="person-picker">
    <input
      v-model="query"
      class="input"
      :class="{ 'input-sm': small }"
      :placeholder="placeholder"
      @focus="open = true"
      @keydown.esc="open = false"
    />
    <div v-if="open && (results.length || query.trim())" class="picker-pop" @mousedown.prevent>
      <button
        v-for="p in results"
        :key="p.id"
        type="button"
        class="picker-item"
        @click="pick(p)"
      >
        <span class="strong truncate">{{ p.displayName }}</span>
        <span class="muted mono sid">{{ p.steamids[0] ?? `#${p.id}` }}</span>
        <span class="muted num cnt">{{ p.matchCount }} 场</span>
      </button>
      <div v-if="!results.length" class="picker-empty">{{ loading ? '搜索中…' : '没有匹配的选手' }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { PersonSummaryDto } from '@cs-analyst/shared-types';
import { searchPersons } from '~/composables/usePlayers';

const props = withDefaults(defineProps<{ exclude?: number[]; placeholder?: string; small?: boolean }>(), {
  exclude: () => [],
  placeholder: '搜索名字、曾用名或 SteamID',
  small: false,
});
const emit = defineEmits<{ pick: [p: PersonSummaryDto] }>();

const query = ref('');
const open = ref(false);
const loading = ref(false);
const raw = ref<PersonSummaryDto[]>([]);
const results = computed(() => raw.value.filter(p => !props.exclude.includes(p.id)).slice(0, 12));

let seq = 0;
let timer: ReturnType<typeof setTimeout> | undefined;
watch(query, () => {
  clearTimeout(timer);
  timer = setTimeout(async () => {
    const my = ++seq;
    loading.value = true;
    try {
      const r = await searchPersons(query.value, { limit: 20 });
      if (my === seq) raw.value = r;
    } finally {
      if (my === seq) loading.value = false;
    }
  }, 150);
}, { immediate: true });

function pick(p: PersonSummaryDto) {
  emit('pick', p);
  open.value = false;
  query.value = '';
}

function onDocClick(e: MouseEvent) {
  if (!(e.target as HTMLElement).closest('.person-picker')) open.value = false;
}
onMounted(() => document.addEventListener('mousedown', onDocClick));
onUnmounted(() => document.removeEventListener('mousedown', onDocClick));
</script>

<style scoped>
.person-picker { position: relative; }
.person-picker .input { width: 100%; }
.picker-pop {
  position: absolute;
  z-index: 50;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  max-height: 300px;
  overflow-y: auto;
  padding: 4px;
  background: var(--surface-raised);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-pop);
}
.picker-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  height: 30px;
  padding: 0 var(--space-2);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text);
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
}
.picker-item:hover { background: var(--surface-hover); }
.sid, .cnt { font-size: var(--text-xs); }
.picker-empty { padding: var(--space-2); font-size: var(--text-xs); color: var(--text-3); }
</style>
