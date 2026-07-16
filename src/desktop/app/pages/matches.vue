<template>
  <div class="matches-page">
    <!-- Stat cards -->
    <div class="stats-row">
      <div v-for="s in stats" :key="s.label" class="stat-card">
        <div class="stat-num" :style="{ color: s.color }">{{ s.value }}</div>
        <div class="stat-label">{{ s.label }}</div>
      </div>
    </div>

    <!-- Toolbar -->
    <div class="toolbar">
      <select v-model="filterPlatform" class="sel sel-sm">
        <option value="">全部来源</option>
        <option value="valve">Valve</option>
        <option value="renown">Renown</option>
        <option value="5eplay">5EPlay</option>
        <option value="imported">导入</option>
      </select>
      <select v-model="filterStatus" class="sel sel-sm">
        <option value="">全部状态</option>
        <option value="queued">排队中</option>
        <option value="downloading">下载中</option>
        <option value="completed">已完成</option>
        <option value="failed">失败</option>
        <option value="skipped">跳过</option>
        <option value="expired">过期</option>
      </select>
      <input v-model="search" class="search-input" placeholder="搜索地图、文件名、来源、Match ID…" />

      <!-- Column picker -->
      <div class="col-picker-wrap">
        <button class="col-picker-btn" type="button" @click="colPickerOpen = !colPickerOpen">
          列显示
        </button>
        <div v-if="colPickerOpen" class="col-picker-panel">
          <label v-for="col in MATCH_COLUMNS" :key="col.key" class="col-picker-item">
            <input
              type="checkbox"
              :checked="visibleKeys.includes(col.key)"
              @change="toggleColumn(col.key)"
            />
            {{ col.label }}
          </label>
          <button class="col-picker-reset" type="button" @click="resetColumns">恢复默认</button>
        </div>
      </div>

      <!-- Right-side action buttons -->
      <div class="toolbar-right">
        <button
          class="queue-toggle"
          :class="{ active: !status.queuePaused }"
          :disabled="Boolean(status.backgroundTask)"
          @click="runToggleAutoUpdate"
        >
          <span class="queue-dot" />
          {{ status.queuePaused ? '自动更新：关' : '自动更新：开' }}
        </button>
        <button class="act-btn act-discover" :disabled="Boolean(status.backgroundTask)" @click="runDiscover">
          立即发现
        </button>
        <button class="act-btn act-download" :disabled="Boolean(status.backgroundTask)" @click="runForceDownload">
          立即下载
        </button>
        <button class="act-btn act-import" @click="runImport">
          导入对局
        </button>
        <button class="act-btn act-metadata" :disabled="Boolean(status.backgroundTask)" @click="runRefreshMetadata">
          更新元数据
        </button>
      </div>
    </div>

    <!-- Table -->
    <div class="table-wrap">
      <table class="match-table">
        <thead>
          <tr>
            <th
              v-for="col in visibleColumns"
              :key="col.key"
              :class="col.className"
            >
              {{ col.label }}
            </th>
            <th class="col-actions"></th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="job in visibleJobs"
            :key="job.id"
            class="match-row"
            @click="openDetail(job)"
          >
            <td
              v-for="col in visibleColumns"
              :key="col.key"
              :class="cellClass(col.key)"
            >
              <template v-if="col.key === 'source'">
                <span class="source-chip" :class="`platform-${job.platform}`">{{ job.source }}</span>
              </template>
              <template v-else-if="col.key === 'progress'">
                <template v-if="job.status === 'downloading'">
                  <div class="prog-bar">
                    <div class="prog-fill" :style="{ width: `${Math.round((job.progress || 0) * 100)}%` }" />
                  </div>
                  <span class="prog-pct">{{ Math.round((job.progress || 0) * 100) }}%</span>
                </template>
                <span v-else class="dash">—</span>
              </template>
              <template v-else-if="col.key === 'status'">
                <span class="status-chip" :style="statusStyle(job.status)">
                  <span class="status-dot2" :style="{ background: statusColor(job.status) }" />
                  {{ statusLabel(job.status) }}
                </span>
              </template>
              <template v-else>
                {{ formatCell(col.key, job) }}
              </template>
            </td>
            <td class="col-actions" @click.stop>
              <button
                v-if="['failed','expired'].includes(job.status)"
                class="row-btn row-retry"
                @click="retry(job.id)"
              >重试</button>
              <button
                v-if="['queued','downloading'].includes(job.status)"
                class="row-btn row-cancel"
                @click="cancel(job.id)"
              >取消</button>
            </td>
          </tr>
          <tr v-if="visibleJobs.length === 0">
            <td :colspan="visibleColumns.length + 1" class="empty-row">暂无对局记录</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Detail drawer -->
    <USlideover v-model:open="drawerOpen" side="right">
      <template #content="{ close }">
        <div class="drawer-body">
          <div class="drawer-header">
            <span class="drawer-title">对局详情</span>
            <button class="drawer-close" @click="close">✕</button>
          </div>
          <template v-if="selected">
            <div class="detail-grid">
              <template v-for="[k, v] in detailRows" :key="k">
                <div class="detail-key">{{ k }}</div>
                <div class="detail-val">{{ v }}</div>
              </template>
            </div>
            <button
              v-if="['failed','expired'].includes(selected.status)"
              class="retry-full-btn"
              @click="retry(selected.id); drawerOpen = false"
            >立即重试</button>
          </template>
        </div>
      </template>
    </USlideover>
  </div>
</template>

<script setup lang="ts">
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { MatchDto } from '@cs-analyst/shared-types';
import { listenBackgroundTask, listenImportCompleted, listenJobProgress, listenJobsUpdated, listenMetadataRefreshCompleted, tauriInvoke } from '~/composables/useTauri';
import { formatMatchCell, useMatchColumns, type MatchColumnKey } from '~/composables/useMatchColumns';

definePageMeta({ layout: 'default' });

const { status, toggleQueue, discoverNow } = useAppStatus();
const toast = useToast();
const { visibleKeys, visibleColumns, toggleColumn, resetColumns, MATCH_COLUMNS } = useMatchColumns();

const jobs = ref<MatchDto[]>([]);
const filterPlatform = ref('');
const filterStatus = ref('');
const search = ref('');
const drawerOpen = ref(false);
const selected = ref<MatchDto | null>(null);
const colPickerOpen = ref(false);

const stats = computed(() => {
  const c = (s: string) => jobs.value.filter(j => j.status === s).length;
  return [
    { label: '已完成', value: c('completed'),   color: 'var(--c-success)' },
    { label: '排队中', value: c('queued'),      color: '#818cf8' },
    { label: '下载中', value: c('downloading'), color: 'var(--c-accent-lt)' },
    { label: '失败',   value: c('failed') + c('expired'), color: 'var(--c-error)' },
  ];
});

const visibleJobs = computed(() =>
  jobs.value.filter(j => {
    if (filterPlatform.value && j.platform !== filterPlatform.value) return false;
    if (filterStatus.value && j.status !== filterStatus.value) return false;
    if (search.value) {
      const q = search.value.toLowerCase();
      return j.matchId.toLowerCase().includes(q)
        || j.source.toLowerCase().includes(q)
        || (j.mapName ?? '').toLowerCase().includes(q)
        || (j.fileName ?? '').toLowerCase().includes(q);
    }
    return true;
  }),
);

const detailRows = computed(() => {
  if (!selected.value) return [];
  const s = selected.value;
  return ([
    ['来源', s.source],
    ['文件名', s.fileName],
    ['Match ID', s.matchId],
    ['地图', s.mapName],
    ['模式', gameModeLabel(s.gameMode)],
    ['类型', gameTypeLabel(s.gameType)],
    ['持续时间', fmtDuration(s.durationSeconds)],
    ['时间', fmtTime(s.playedAt)],
    ['Share Code', s.shareCode],
    ['Valve 频道', s.valveChannel],
    ['校验和', s.checksum],
    ['路径', s.filePath],
    ['Demo Stamp', s.demoFileStamp],
    ['网络协议', s.networkProtocol != null ? String(s.networkProtocol) : null],
    ['服务器', s.serverName],
    ['客户端', s.clientName],
    ['游戏目录', s.gameDirectory],
    ['Fullpackets', s.fullpacketsVersion != null ? String(s.fullpacketsVersion) : null],
    ['客户端实体', s.allowClientsideEntities == null ? null : (s.allowClientsideEntities ? '是' : '否')],
    ['Header #9', s.demoHeaderField9 != null ? String(s.demoHeaderField9) : null],
    ['Header #10', s.demoHeaderField10],
    ['Demo 版本', s.demoVersionName],
    ['Demo GUID', s.demoVersionGuid],
    ['播放时长(s)', s.playbackTime != null ? s.playbackTime.toFixed(1) : null],
    ['Ticks', s.playbackTicks != null ? String(s.playbackTicks) : null],
    ['Frames', s.playbackFrames != null ? String(s.playbackFrames) : null],
    ['Game Info', s.demGameInfo ? JSON.stringify(s.demGameInfo, null, 2) : null],
    ['尝试次数', String(s.attempts)],
    ['错误信息', s.lastError],
  ] as [string, string | null | undefined][]).filter(([, v]) => v != null && v !== '' && v !== '—');
});

const cellHelpers = {
  fmtTime,
  fmtDuration,
  gameModeLabel,
  gameTypeLabel,
  statusLabel,
};

function formatCell(key: MatchColumnKey, job: MatchDto) {
  return formatMatchCell(key, job, cellHelpers);
}

function cellClass(key: MatchColumnKey) {
  return {
    fileName: 'file-name-col',
    mapName: 'map-name',
    durationSeconds: 'duration-col',
    playedAt: 'time-col',
    gameMode: 'mode-col',
    gameType: 'type-col',
    filePath: 'path-col',
    demGameInfo: 'json-col',
  }[key] ?? '';
}

async function runToggleAutoUpdate() {
  try {
    await toggleQueue();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '操作失败', description: message, color: 'error' });
  }
}

async function runDiscover() {
  try {
    await discoverNow();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '无法启动发现任务', description: message, color: 'error' });
  }
}

async function runForceDownload() {
  try {
    await tauriInvoke('force_download_now');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '立即下载失败', description: message, color: 'error' });
  }
}

async function runRefreshMetadata() {
  try {
    await tauriInvoke('refresh_match_metadata');
    toast.add({ title: '正在更新元数据…', color: 'neutral' });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '无法启动元数据更新', description: message, color: 'error' });
  }
}

async function runImport() {
  const files = await openDialog({
    multiple: true,
    filters: [{ name: 'CS2 Demo', extensions: ['dem'] }],
  });
  if (!files || (Array.isArray(files) && files.length === 0)) return;
  const filePaths = Array.isArray(files) ? files : [files];
  try {
    await tauriInvoke('import_demos', { filePaths });
    toast.add({ title: '正在导入…', description: `处理 ${filePaths.length} 个文件`, color: 'neutral' });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '无法启动导入', description: message, color: 'error' });
  }
}

function statusLabel(s: string) {
  return { queued: '排队中', downloading: '下载中', completed: '已完成', failed: '失败', skipped: '跳过', expired: '过期' }[s] ?? s;
}

function statusColor(s: string) {
  return { downloading: '#a78bfa', completed: '#22d3a0', failed: '#f16060', expired: '#f97316', queued: '#818cf8', skipped: '#3d4460' }[s] ?? '#5c647a';
}

function statusStyle(s: string) {
  const c = statusColor(s);
  return { color: c, background: `${c}18`, borderColor: `${c}30` };
}

function gameModeLabel(mode?: string | null) {
  if (!mode) return '—';
  return { casual: '休闲', competitive: '竞技', wingman: '双人', armsrace: '军备竞赛', demolition: '拆弹', deathmatch: '死亡竞赛', premier: 'Premier', retakes: '夺回' }[mode] ?? mode;
}

function fmtDuration(secs?: number | null) {
  if (!secs) return '—';
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

function fmtTime(iso?: string | null) {
  if (!iso) return '—';
  return new Date(iso).toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}

function gameTypeLabel(type?: number | null) {
  if (type == null) return '—';
  return String(type);
}

async function loadJobs() {
  try {
    jobs.value = await tauriInvoke<MatchDto[]>('list_matches');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '加载对局失败', description: message, color: 'error' });
    jobs.value = [];
  }
}

async function retry(jobId: string) {
  await tauriInvoke('retry_download_job', { jobId });
  await loadJobs();
}

async function cancel(jobId: string) {
  await tauriInvoke('cancel_download_job', { jobId });
  await loadJobs();
}

function openDetail(job: MatchDto) {
  selected.value = job;
  drawerOpen.value = true;
}

onMounted(() => {
  loadJobs();
  const u1 = listenJobsUpdated(loadJobs);
  const u2 = listenJobProgress(({ jobId, progress }) => {
    const job = jobs.value.find(j => j.id === jobId);
    if (job) job.progress = progress;
  });
  const u3 = listenBackgroundTask(loadJobs);
  const u4 = listenImportCompleted((result) => {
    loadJobs();
    const errCount = result.results.filter(r => r.action === 'error').length;
    if (errCount > 0) {
      toast.add({ title: '导入完成（有错误）', description: `已导入 ${result.imported} 个，跳过 ${result.skipped} 个，失败 ${errCount} 个`, color: 'warning' });
    } else {
      toast.add({ title: '导入完成', description: `已导入 ${result.imported} 个，跳过 ${result.skipped} 个`, color: 'success' });
    }
  });
  const u5 = listenMetadataRefreshCompleted((result) => {
    loadJobs();
    if (result.failed > 0) {
      toast.add({
        title: '元数据更新完成（有错误）',
        description: `已更新 ${result.updated} 个，合并 ${result.merged} 个，跳过 ${result.skipped} 个，失败 ${result.failed} 个`,
        color: 'warning',
      });
    } else {
      toast.add({
        title: '元数据更新完成',
        description: `已更新 ${result.updated} 个，合并 ${result.merged} 个，跳过 ${result.skipped} 个`,
        color: 'success',
      });
    }
  });
  onUnmounted(() => { u1(); u2(); u3(); u4(); u5(); });
});
</script>

<style scoped>
.matches-page { display: flex; flex-direction: column; gap: 16px; height: 100%; }

/* Stats */
.stats-row { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
.stat-card {
  background: var(--c-card);
  border: 1px solid var(--c-border);
  border-radius: 10px;
  padding: 14px 16px;
}
.stat-num  { font-size: 26px; font-weight: 700; line-height: 1; }
.stat-label { font-size: 11px; color: var(--c-text-3); margin-top: 5px; }

/* Toolbar */
.toolbar { display: flex; gap: 8px; align-items: center; }
.toolbar-right {
  margin-left: auto;
  display: flex;
  gap: 8px;
  align-items: center;
}

.sel {
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: 7px;
  color: var(--c-text-2);
  padding: 6px 10px;
  font-size: 12px;
  cursor: pointer;
  outline: none;
  min-width: 90px;
}
.sel:focus { border-color: var(--c-accent); }
.search-input {
  flex: 1;
  max-width: 240px;
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: 7px;
  color: var(--c-text);
  padding: 6px 12px;
  font-size: 12px;
  outline: none;
}
.search-input::placeholder { color: var(--c-text-3); }
.search-input:focus { border-color: var(--c-accent); }

.col-picker-wrap { position: relative; }
.col-picker-btn {
  padding: 6px 12px;
  border-radius: 7px;
  border: 1px solid var(--c-border);
  background: var(--c-surface);
  color: var(--c-text-2);
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}
.col-picker-btn:hover { border-color: var(--c-accent); color: var(--c-text); }
.col-picker-panel {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  z-index: 20;
  min-width: 180px;
  max-height: 320px;
  overflow-y: auto;
  background: var(--c-card);
  border: 1px solid var(--c-border);
  border-radius: 8px;
  padding: 8px;
  box-shadow: 0 8px 24px rgba(0,0,0,.35);
}
.col-picker-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 6px;
  font-size: 12px;
  color: var(--c-text-2);
  cursor: pointer;
  border-radius: 4px;
}
.col-picker-item:hover { background: var(--c-card-hover); }
.col-picker-reset {
  margin-top: 6px;
  width: 100%;
  padding: 5px;
  border: 1px solid var(--c-border);
  border-radius: 5px;
  background: transparent;
  color: var(--c-text-3);
  font-size: 11px;
  cursor: pointer;
}
.col-picker-reset:hover { color: var(--c-text); border-color: var(--c-accent); }

.queue-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 13px;
  border-radius: 7px;
  border: 1px solid rgba(255,255,255,0.08);
  background: rgba(255,255,255,0.05);
  color: var(--c-text-2);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
}
.queue-toggle:disabled { opacity: 0.45; cursor: default; }
.queue-toggle:not(:disabled):hover { background: rgba(255,255,255,0.09); color: var(--c-text); }
.queue-toggle.active {
  background: color-mix(in srgb, var(--c-accent) 14%, transparent);
  color: var(--c-accent-lt);
  border-color: var(--c-accent-bdr);
}
.queue-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--c-text-faint);
  flex-shrink: 0;
}
.queue-toggle.active .queue-dot {
  background: var(--c-accent-lt);
  box-shadow: 0 0 5px var(--c-accent-lt);
}

.act-btn {
  padding: 6px 12px;
  border-radius: 7px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid;
  transition: opacity 0.12s;
  white-space: nowrap;
}
.act-btn:disabled { opacity: 0.45; cursor: default; }
.act-btn:not(:disabled):hover { opacity: 0.8; }

.act-discover {
  background: rgba(99,102,241,.12);
  color: #a5b4fc;
  border-color: rgba(99,102,241,.3);
}
.act-download {
  background: var(--c-accent-bg);
  color: var(--c-accent-lt);
  border-color: var(--c-accent-bdr);
}
.act-import {
  background: rgba(34,197,94,.1);
  color: #4ade80;
  border-color: rgba(34,197,94,.25);
}
.act-metadata {
  background: rgba(14,165,233,.1);
  color: #38bdf8;
  border-color: rgba(14,165,233,.25);
}

/* Table */
.table-wrap {
  flex: 1;
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: 10px;
  overflow: auto;
  min-height: 0;
}
.match-table { width: 100%; border-collapse: collapse; }
.match-table thead th {
  padding: 10px 12px;
  text-align: left;
  font-size: 11px;
  font-weight: 500;
  color: var(--c-text-3);
  border-bottom: 1px solid var(--c-border);
  white-space: nowrap;
}
.match-row { cursor: pointer; transition: background 0.1s; }
.match-row:hover { background: var(--c-card-hover); }
.match-row td {
  padding: 9px 12px;
  font-size: 12px;
  border-bottom: 1px solid var(--c-border-sub);
  vertical-align: middle;
}

.source-chip {
  display: inline-block;
  padding: 2px 7px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
  background: var(--c-accent-bg);
  color: var(--c-accent-lt);
  border: 1px solid var(--c-accent-bdr);
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.platform-imported {
  background: rgba(34,197,94,.1);
  color: #4ade80;
  border-color: rgba(34,197,94,.25);
}

.time-col     { color: var(--c-text-3); white-space: nowrap; }
.duration-col { color: var(--c-text-3); font-variant-numeric: tabular-nums; white-space: nowrap; }
.file-name-col { color: var(--c-text-2); max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.map-name     { color: var(--c-text-2); }
.mode-col     { color: var(--c-text-3); white-space: nowrap; }
.type-col     { color: var(--c-text-3); white-space: nowrap; font-variant-numeric: tabular-nums; }
.path-col     { max-width: 240px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--c-text-3); }
.json-col     { max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: monospace; font-size: 11px; color: var(--c-text-3); }
.col-prog     { width: 110px; }
.col-actions  { width: 70px; }

.prog-bar {
  height: 4px;
  border-radius: 2px;
  background: var(--c-border);
  overflow: hidden;
  margin-bottom: 3px;
}
.prog-fill {
  height: 100%;
  border-radius: 2px;
  background: var(--c-accent-lt);
  transition: width 0.3s;
}
.prog-pct { font-size: 10px; color: var(--c-text-3); }
.dash { color: var(--c-text-faint); }

.status-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 8px;
  border-radius: 20px;
  font-size: 11px;
  font-weight: 500;
  border: 1px solid;
  white-space: nowrap;
}
.status-dot2 {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  flex-shrink: 0;
}

.row-btn {
  padding: 3px 9px;
  border-radius: 5px;
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid;
  transition: opacity 0.1s;
}
.row-btn:hover { opacity: 0.75; }
.row-retry  { background: rgba(129,140,248,.12); color: #818cf8; border-color: rgba(129,140,248,.3); }
.row-cancel { background: rgba(241,96,96,.1);   color: #f16060; border-color: rgba(241,96,96,.25); }

.empty-row {
  text-align: center;
  padding: 48px;
  color: var(--c-text-faint);
  font-size: 13px;
}

/* Drawer */
.drawer-body {
  height: 100%;
  background: var(--c-surface);
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.drawer-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.drawer-title { font-size: 14px; font-weight: 600; color: #fff; }
.drawer-close {
  background: none;
  border: none;
  color: var(--c-text-3);
  font-size: 14px;
  cursor: pointer;
  padding: 2px 6px;
}
.drawer-close:hover { color: var(--c-text); }

.detail-grid {
  display: grid;
  grid-template-columns: 80px 1fr;
  gap: 10px 12px;
}
.detail-key { font-size: 11px; color: var(--c-text-3); padding-top: 1px; }
.detail-val { font-size: 12px; color: var(--c-text-2); word-break: break-all; }

.retry-full-btn {
  margin-top: auto;
  padding: 9px;
  border-radius: 8px;
  background: var(--c-accent-bg);
  color: var(--c-accent-lt);
  border: 1px solid var(--c-accent-bdr);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  width: 100%;
}
.retry-full-btn:hover { opacity: 0.8; }
</style>
