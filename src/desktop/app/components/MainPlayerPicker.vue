<template>
  <div class="picker">
    <!-- 推测的候选人：只是建议，点「是我」才写入 -->
    <div v-if="!isSet && suggestion" class="suggest">
      <div class="suggest-info">
        <span class="suggest-label">看起来你是</span>
        <span class="suggest-name">{{ suggestion.displayName ?? suggestion.steamid }}</span>
        <span class="mono muted">{{ suggestion.steamid }}</span>
        <span class="tag">{{ mainPlayerSourceLabel(suggestion.source) }}</span>
        <span v-if="suggestion.matchCount" class="muted num">{{ suggestion.matchCount }} 场</span>
      </div>
      <button type="button" class="btn-primary btn-sm" :disabled="saving" @click="choose(suggestion.steamid)">是我</button>
    </div>

    <div class="search-row">
      <input
        v-model="query"
        class="input flex-1"
        placeholder="按名字或 SteamID 搜索已解析对局里的选手"
        @input="scheduleSearch"
      />
    </div>

    <div class="table-wrap list">
      <table class="data-table compact">
        <thead>
          <tr>
            <th>选手</th>
            <th>SteamID</th>
            <th class="num">场次</th>
            <th class="actions"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in candidates" :key="c.steamid" :class="{ selected: c.steamid === steamid }">
            <td class="strong truncate">
              {{ c.name }}
              <span v-if="c.followed" class="tag follow-tag">关注</span>
            </td>
            <td class="mono muted">{{ c.steamid }}</td>
            <td class="num">{{ c.matchCount }}</td>
            <td class="actions">
              <StatusChip v-if="c.steamid === steamid" tone="success" label="当前" />
              <button v-else type="button" class="btn btn-sm" :disabled="saving" @click="choose(c.steamid)">选择</button>
            </td>
          </tr>
          <tr v-if="candidates.length === 0">
            <td colspan="4" class="empty">
              {{ query ? '没有匹配的选手' : '还没有已解析的对局。导入或下载 demo 后，这里会列出出现过的选手。' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div class="manual">
      <div class="field flex-1">
        <label class="field-label">或者直接输入</label>
        <input
          v-model="manual"
          class="input mono"
          placeholder="SteamID64 / steamcommunity.com/profiles/… / [U:1:…]"
          @keyup.enter="chooseManual"
        />
      </div>
      <button type="button" class="btn" :disabled="saving || !manual.trim()" @click="chooseManual">设为主玩家</button>
    </div>
    <p v-if="error" class="form-error">{{ error }}</p>
  </div>
</template>

<script setup lang="ts">
import type { PlayerCandidateDto } from '@cs-analyst/shared-types';
import { listenAnalysisUpdated, tauriInvoke } from '~/composables/useTauri';
import { mainPlayerSourceLabel } from '~/composables/useMainPlayer';

const emit = defineEmits<{ selected: [steamid: string] }>();

const { steamid, isSet, suggestion, set } = useMainPlayer();
const query = ref('');
const manual = ref('');
const candidates = ref<PlayerCandidateDto[]>([]);
const saving = ref(false);
const error = ref('');

async function search() {
  try {
    candidates.value = await tauriInvoke<PlayerCandidateDto[]>('list_main_player_candidates', {
      query: query.value.trim() || null,
      limit: 30,
    });
  } catch {
    candidates.value = [];
  }
}

let timer: ReturnType<typeof setTimeout> | null = null;
function scheduleSearch() {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void search(), 200);
}

async function choose(id: string) {
  saving.value = true;
  error.value = '';
  try {
    await set(id);
    emit('selected', id);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
}

async function chooseManual() {
  if (!manual.value.trim()) return;
  await choose(manual.value.trim());
  if (!error.value) manual.value = '';
}

// 解析完新对局时刷新列表
const off = listenAnalysisUpdated((p) => {
  if (p.status === 'done') scheduleSearch();
});
onBeforeUnmount(off);
onMounted(search);
</script>

<style scoped>
.picker { display: flex; flex-direction: column; gap: var(--space-3); }
.flex-1 { flex: 1; min-width: 0; }

.suggest {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--accent-line);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}
.suggest-info { display: flex; align-items: baseline; flex-wrap: wrap; gap: var(--space-2); min-width: 0; }
.suggest-label { font-size: var(--text-xs); color: var(--text-3); }
.suggest-name { font-size: var(--text-md); font-weight: 600; color: var(--text); }

.search-row { display: flex; gap: var(--space-2); }
.list { max-height: 260px; overflow: auto; }
.follow-tag { margin-left: var(--space-1); }

.manual { display: flex; align-items: flex-end; gap: var(--space-2); }
</style>
