<template>
  <AppModal :open="open" :title="title" @update:open="onClose">
    <div class="plan-modal">
      <div v-if="loading" class="muted">正在计算…</div>
      <div v-else-if="error" class="form-error">{{ error }}</div>
      <template v-else-if="plan">
        <div class="plan-summary">
          <StatTile label="将处理" :value="plan.eligibleCount" unit="场" compact />
          <StatTile :label="plan.action === 'compress' ? '当前占用' : '可释放'" :value="fmtBytes(plan.eligibleBytes)" compact />
          <StatTile label="收藏跳过" :value="plan.skippedFavorites" unit="场" compact />
        </div>

        <div v-if="plan.action !== 'compress' && plan.eligibleCount > 0" class="callout callout-danger">
          删除原始 demo 后，这些对局<strong>无法重新解析</strong>，也<strong>无法在 CS2 里播放集锦</strong>。
          已入库的数据（计分板、事件、2D 回放）会保留。
        </div>
        <div v-else-if="plan.action === 'compress' && plan.eligibleCount > 0" class="callout callout-info">
          压缩后可透明参与重新解析；在 CS2 里播放集锦前需要先解压。每个文件都会先完整校验再替换原文件。
        </div>

        <div class="table-wrap plan-table">
          <table class="data-table compact">
            <thead>
              <tr><th>对局</th><th>时间</th><th class="num">大小</th><th>结果</th></tr>
            </thead>
            <tbody>
              <tr v-for="it in plan.items" :key="it.id">
                <td class="strong truncate">
                  <span v-if="it.favorite" class="fav" title="已收藏">★</span>{{ it.mapName || it.id.slice(0, 8) }}
                </td>
                <td class="muted nowrap">{{ fmtDate(it.playedAt) }}</td>
                <td class="num">{{ fmtBytes(it.bytes) }}</td>
                <td>
                  <StatusChip v-if="it.eligible" :tone="plan.action === 'compress' ? 'neutral' : 'danger'" :label="actionLabel" />
                  <span v-else class="muted reason" :title="it.reason ?? ''">跳过 · {{ it.reason }}</span>
                </td>
              </tr>
              <tr v-if="plan.items.length === 0"><td colspan="4" class="empty">没有符合条件的对局</td></tr>
            </tbody>
          </table>
        </div>

        <label v-if="destructive && plan.eligibleCount > 0 && !confirmOnly" class="ack">
          <input v-model="ack" type="checkbox" />
          我已了解：删除不可恢复，这 {{ plan.eligibleCount }} 场对局将无法重新解析
        </label>

        <div v-if="running" class="run-progress">
          <div class="progress"><div class="progress-fill" :style="{ width: `${pct}%` }" /></div>
          <span class="num muted">{{ progress.done }} / {{ progress.total }}</span>
        </div>
        <div v-if="result" class="callout" :class="result.failed.length ? 'callout-warning' : 'callout-info'">
          已处理 {{ result.processed }} 场，释放 {{ fmtBytes(result.bytesFreed) }}。
          <template v-if="result.failed.length">{{ result.failed.length }} 场跳过：{{ result.failed[0]?.reason }}</template>
        </div>
      </template>
    </div>
    <template #footer>
      <button class="btn" :disabled="running" @click="onClose(false)">{{ result ? '关闭' : '取消' }}</button>
      <template v-if="plan && !result">
        <button
          v-if="confirmOnly"
          class="btn-primary"
          :disabled="running"
          @click="emit('confirmed', plan)"
        >{{ confirmLabel }}</button>
        <button
          v-else-if="plan.eligibleCount > 0"
          :class="destructive ? 'btn btn-danger danger-solid' : 'btn-primary'"
          :disabled="running || (destructive && !ack)"
          @click="apply"
        >{{ running ? '执行中…' : `${actionLabel} ${plan.eligibleCount} 个 demo` }}</button>
      </template>
    </template>
  </AppModal>
</template>

<script setup lang="ts">
import { listen } from '@tauri-apps/api/event';
import type { CleanupRulesDto, StoragePlanDto, StoragePlanKind, StoragePlanResultDto, StorageProgressDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';
import { fmtBytes } from '~/composables/useStorageFormat';

const props = defineProps<{
  open: boolean;
  kind: StoragePlanKind;
  rules?: CleanupRulesDto | null;
  /** 只预览并确认（例如开启自动清理），不立即执行 */
  confirmOnly?: boolean;
  confirmLabel?: string;
}>();
const emit = defineEmits<{
  'update:open': [boolean];
  done: [StoragePlanResultDto];
  confirmed: [StoragePlanDto];
}>();

const plan = ref<StoragePlanDto | null>(null);
const loading = ref(false);
const error = ref('');
const ack = ref(false);
const running = ref(false);
const result = ref<StoragePlanResultDto | null>(null);
const progress = reactive({ done: 0, total: 0 });

const destructive = computed(() => props.kind !== 'compress');
const actionLabel = computed(() => (props.kind === 'compress' ? '压缩' : '删除'));
const title = computed(() => ({ cleanup: '清理预览', compress: '压缩已有 demo', delete: '删除已有 demo' }[props.kind]));
const pct = computed(() => (progress.total ? Math.round((progress.done / progress.total) * 100) : 0));

async function load() {
  plan.value = null;
  result.value = null;
  error.value = '';
  ack.value = false;
  loading.value = true;
  try {
    plan.value = await tauriInvoke<StoragePlanDto>('preview_storage_plan', {
      kind: props.kind,
      rules: props.kind === 'cleanup' ? props.rules ?? null : null,
      ids: null,
    });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

async function apply() {
  if (!plan.value) return;
  // 二次确认：删除类操作必须勾选确认框（按钮在未勾选时禁用），后端还会校验预览令牌
  if (destructive.value && !ack.value) return;
  running.value = true;
  progress.done = 0;
  progress.total = plan.value.eligibleCount;
  let unlisten: (() => void) | null = null;
  unlisten = await listen<StorageProgressDto>('storage_progress', e => Object.assign(progress, e.payload));
  try {
    result.value = await tauriInvoke<StoragePlanResultDto>('apply_storage_plan', { token: plan.value.token });
    emit('done', result.value);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    plan.value = null;
  } finally {
    unlisten?.();
    running.value = false;
  }
}

function onClose(v: boolean) {
  if (running.value) return;
  emit('update:open', v);
}

function fmtDate(s: string | null) {
  if (!s) return '—';
  const d = new Date(s.includes('T') ? s : s.replace(' ', 'T'));
  return Number.isNaN(d.getTime()) ? s : d.toLocaleDateString('zh-CN');
}

watch(() => props.open, v => { if (v) load(); }, { immediate: true });
</script>

<style scoped>
.plan-modal { display: flex; flex-direction: column; gap: var(--space-3); }
.plan-summary { display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--space-2); }
.plan-table { max-height: 260px; overflow-y: auto; }
.plan-table td.truncate { max-width: 140px; }
.reason { font-size: var(--text-xs); display: inline-block; max-width: 170px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; vertical-align: middle; }
.fav { color: var(--text); margin-right: 4px; }
.ack { display: flex; gap: var(--space-2); align-items: flex-start; font-size: var(--text-sm); color: var(--text-2); }
.ack input { margin-top: 2px; }
.run-progress { display: flex; align-items: center; gap: var(--space-2); }
.run-progress .progress { flex: 1; }
.danger-solid:not(:disabled) { color: var(--danger); border-color: var(--danger-line); }
</style>
