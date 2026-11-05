<template>
  <div class="card card-pad downloader-card">
    <div class="head">
      <div>
        <div class="card-title">下载模块</div>
        <div class="card-desc">各平台 demo 的发现与下载由独立模块完成，主程序负责心跳检查和崩溃后自动重启。</div>
      </div>
      <StatusChip
        :tone="stateTone"
        :label="stateLabel"
        :pulse="status?.state === 'starting' || status?.state === 'restarting'"
        data-testid="downloader-state"
      />
    </div>

    <dl class="facts">
      <div><dt>版本</dt><dd class="mono" data-testid="downloader-version">{{ status?.moduleVersion ?? '—' }}</dd></div>
      <div>
        <dt>协议</dt>
        <dd class="mono" :class="{ bad: protocolMismatch }">
          {{ status?.protocolVersion != null ? `v${status.protocolVersion}` : '—' }}
          <span class="muted">/ 需要 v{{ status?.hostProtocolVersion ?? '—' }}</span>
        </dd>
      </div>
      <div><dt>进程</dt><dd class="mono" data-testid="downloader-pid">{{ status?.pid ?? '—' }}</dd></div>
      <div><dt>自动重启</dt><dd class="mono" data-testid="downloader-restarts">{{ status?.restartCount ?? 0 }} 次</dd></div>
      <div>
        <dt>心跳</dt>
        <dd class="mono">{{ status?.lastHeartbeatMs != null ? `${status.lastHeartbeatMs} ms · ${timeOf(status.lastHeartbeatAt)}` : '—' }}</dd>
      </div>
      <div v-if="status?.lastExit"><dt>上次退出</dt><dd class="mono">{{ status.lastExit }}</dd></div>
    </dl>

    <div v-if="status?.lastError" class="error-line">{{ status.lastError }}</div>

    <div class="adapters">
      <div v-for="a in adapterRows" :key="a.platform" class="adapter-row">
        <div class="setting-info">
          <span class="setting-name">{{ a.label }}</span>
          <span class="setting-desc">
            <template v-if="!a.available">加载失败：{{ a.error || '未知原因' }}</template>
            <template v-else-if="a.enabled">已启用</template>
            <template v-else>已停用，发现和下载都会跳过</template>
          </span>
        </div>
        <button
          class="toggle"
          :class="{ on: a.enabled }"
          :title="a.enabled ? '停用该平台' : '启用该平台'"
          :data-testid="`adapter-toggle-${a.platform}`"
          @click="toggleAdapter(a.platform)"
        >
          <span class="toggle-knob" />
        </button>
      </div>
    </div>

    <div class="actions">
      <button class="btn btn-sm" :disabled="restarting" @click="restart">{{ restarting ? '重启中…' : '重启模块' }}</button>
      <button class="btn btn-sm" @click="toggleLogs">{{ logsOpen ? '收起日志' : '模块日志' }}</button>
    </div>
    <pre v-if="logsOpen" class="logs mono">{{ logs.join('\n') || '暂无日志' }}</pre>
  </div>
</template>

<script setup lang="ts">
import { listen } from '@tauri-apps/api/event';
import type { DownloaderStatusDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

interface PlatformSetting {
  platform: string;
  enabled: boolean;
  [key: string]: unknown;
}

const status = ref<DownloaderStatusDto | null>(null);
const platforms = ref<PlatformSetting[]>([]);
const logs = ref<string[]>([]);
const logsOpen = ref(false);
const restarting = ref(false);
const toast = useToast();

const STATE: Record<string, { label: string; tone: 'success' | 'info' | 'warning' | 'danger' | 'neutral' }> = {
  running: { label: '运行中', tone: 'success' },
  starting: { label: '启动中', tone: 'info' },
  restarting: { label: '等待重启', tone: 'warning' },
  failed: { label: '已停止（连续崩溃）', tone: 'danger' },
  incompatible: { label: '版本不兼容', tone: 'danger' },
  stopped: { label: '未运行', tone: 'neutral' },
};
const stateLabel = computed(() => STATE[status.value?.state ?? 'stopped']?.label ?? status.value?.state ?? '—');
const stateTone = computed(() => STATE[status.value?.state ?? 'stopped']?.tone ?? 'neutral');
const protocolMismatch = computed(
  () => status.value?.protocolVersion != null && status.value.protocolVersion !== status.value.hostProtocolVersion,
);

const LABELS: Record<string, string> = { valve: 'Valve', renown: 'Renown', '5eplay': '5EPlay' };
/** 开关以数据库为准；可用性和加载错误来自模块。 */
const adapterRows = computed(() =>
  platforms.value.map((p) => {
    const a = status.value?.adapters.find((x) => x.platform === p.platform);
    return {
      platform: p.platform,
      label: a?.label ?? LABELS[p.platform] ?? p.platform,
      enabled: p.enabled,
      available: a ? a.available : true,
      error: a?.error ?? null,
    };
  }),
);

function timeOf(iso: string | null) {
  return iso ? new Date(iso).toLocaleTimeString() : '';
}

async function refresh() {
  status.value = await tauriInvoke<DownloaderStatusDto>('get_downloader_status');
  if (logsOpen.value) logs.value = await tauriInvoke<string[]>('get_downloader_logs', { limit: 120 });
}

async function loadPlatforms() {
  platforms.value = await tauriInvoke<PlatformSetting[]>('list_platform_settings');
}

async function toggleAdapter(platform: string) {
  const p = platforms.value.find((x) => x.platform === platform);
  if (!p) return;
  p.enabled = !p.enabled;
  try {
    await tauriInvoke('update_platform_settings', { platform: p });
    await refresh();
  } catch (error) {
    toast.add({ title: '保存失败', description: String(error), color: 'error' });
    await loadPlatforms();
  }
}

async function restart() {
  restarting.value = true;
  try {
    status.value = await tauriInvoke<DownloaderStatusDto>('restart_downloader');
  } catch (error) {
    toast.add({ title: '重启下载模块失败', description: String(error), color: 'error' });
    await refresh();
  } finally {
    restarting.value = false;
  }
}

async function toggleLogs() {
  logsOpen.value = !logsOpen.value;
  if (logsOpen.value) logs.value = await tauriInvoke<string[]>('get_downloader_logs', { limit: 120 });
}

let unlisten: (() => void) | null = null;
let timer: ReturnType<typeof setInterval> | null = null;
onMounted(async () => {
  await Promise.all([refresh(), loadPlatforms()]);
  unlisten = await listen<DownloaderStatusDto>('downloader_status', (e) => {
    status.value = e.payload;
  });
  // 事件之外再低频轮询一次，防止漏掉
  timer = setInterval(() => void refresh(), 5000);
});
onBeforeUnmount(() => {
  unlisten?.();
  if (timer) clearInterval(timer);
});
</script>

<style scoped>
.head { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--space-4); }
.card-title { font-size: var(--text-md); font-weight: 600; color: var(--text); margin-bottom: 2px; }
.card-desc { font-size: var(--text-xs); color: var(--text-3); margin-bottom: var(--space-3); line-height: 1.5; }

.facts {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2) var(--space-4);
  margin: 0 0 var(--space-3);
}
.facts dt { font-size: var(--text-xs); color: var(--text-3); }
.facts dd { margin: 0; color: var(--text); font-variant-numeric: tabular-nums; }
.facts .muted { color: var(--text-3); }
.facts .bad { color: var(--danger); }

.error-line {
  margin-bottom: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--danger-line);
  border-radius: var(--radius-sm);
  background: var(--danger-soft);
  color: var(--text);
  font-size: var(--text-xs);
}

.adapters { border-top: 1px solid var(--border-subtle); }
.adapter-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-height: 40px;
  border-bottom: 1px solid var(--border-subtle);
}
.setting-info { display: flex; flex-direction: column; gap: 2px; }
.setting-name { font-size: var(--text-sm); color: var(--text); }
.setting-desc { font-size: var(--text-xs); color: var(--text-3); }

.actions { display: flex; gap: var(--space-2); margin-top: var(--space-3); }
.logs {
  margin-top: var(--space-3);
  max-height: 260px;
  overflow: auto;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  color: var(--text-2);
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
