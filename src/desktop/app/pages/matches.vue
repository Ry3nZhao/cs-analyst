<template>
  <div class="matches-page">
    <!-- 概览 -->
    <div class="stats-row">
      <StatTile
        v-for="s in stats"
        :key="s.label"
        :label="s.label"
        :value="s.value"
        :tone="s.tone"
        compact
      />
    </div>

    <!-- 工具栏 -->
    <div class="toolbar">
      <select v-model="filterPlatform" class="select">
        <option value="">全部来源</option>
        <option value="valve">Valve</option>
        <option value="renown">Renown</option>
        <option value="5eplay">5EPlay</option>
        <option value="imported">导入</option>
      </select>
      <select v-model="filterStatus" class="select">
        <option value="">全部状态</option>
        <option value="queued">排队中</option>
        <option value="downloading">下载中</option>
        <option value="completed">已完成</option>
        <option value="failed">失败</option>
        <option value="skipped">跳过</option>
        <option value="expired">过期</option>
      </select>
      <input v-model="search" class="input search-input" placeholder="搜索地图、文件名、来源、Match ID…" />

      <!-- 列显示 -->
      <div class="col-picker-wrap">
        <button class="btn" :class="{ 'is-open': colPickerOpen }" type="button" @click="colPickerOpen = !colPickerOpen">
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
          <button class="btn btn-sm col-picker-reset" type="button" @click="resetColumns">恢复默认</button>
        </div>
      </div>

      <!-- 右侧操作 -->
      <div class="toolbar-right">
        <button
          class="btn queue-toggle"
          :class="{ active: !status.queuePaused }"
          :disabled="Boolean(status.backgroundTask)"
          @click="runToggleAutoUpdate"
        >
          <span class="queue-dot" />
          {{ status.queuePaused ? '自动更新：关' : '自动更新：开' }}
        </button>
        <button class="btn" :disabled="Boolean(status.backgroundTask)" @click="runDiscover">
          立即发现
        </button>
        <button class="btn" :disabled="Boolean(status.backgroundTask)" @click="runForceDownload">
          立即下载
        </button>
        <button class="btn" :disabled="Boolean(status.backgroundTask)" @click="runRefreshMetadata">
          更新元数据
        </button>
        <button class="btn-primary" @click="runImport">
          导入对局
        </button>
      </div>
    </div>

    <!-- 表格 -->
    <div class="table-wrap match-table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th class="fav-col" title="收藏的对局不会被自动清理"></th>
            <th
              v-for="col in visibleColumns"
              :key="col.key"
              :class="col.className"
            >
              {{ col.label }}
            </th>
            <th class="actions"></th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="job in visibleJobs"
            :key="job.id"
            class="clickable"
            :class="{ selected: drawerOpen && selected?.id === job.id }"
            @click="openDetail(job)"
          >
            <td class="fav-col" @click.stop>
              <button
                class="fav-btn"
                :class="{ on: isFavorite(job.id) }"
                :title="isFavorite(job.id) ? '取消收藏' : '收藏（不会被自动清理）'"
                :aria-label="isFavorite(job.id) ? '取消收藏' : '收藏'"
                @click="toggleFavorite(job.id)"
              >{{ isFavorite(job.id) ? '★' : '☆' }}</button>
            </td>
            <td
              v-for="col in visibleColumns"
              :key="col.key"
              :class="cellClass(col.key)"
            >
              <template v-if="col.key === 'source'">
                <span class="tag" :title="job.source">{{ job.source }}</span>
              </template>
              <template v-else-if="col.key === 'progress'">
                <div v-if="job.status === 'downloading'" class="prog-cell">
                  <div class="progress">
                    <div class="progress-fill" :style="{ width: `${Math.round((job.progress || 0) * 100)}%` }" />
                  </div>
                  <span class="prog-pct num">{{ Math.round((job.progress || 0) * 100) }}%</span>
                </div>
                <span v-else class="dim">—</span>
              </template>
              <template v-else-if="col.key === 'analysis'">
                <StatusChip
                  v-if="job.analysisStatus"
                  :tone="analysisTone(job.analysisStatus)"
                  :pulse="job.analysisStatus === 'running'"
                  :label="analysisLabel(job.analysisStatus)"
                  :title="job.analysisError ?? (job.analysisMs != null ? `耗时 ${(job.analysisMs / 1000).toFixed(1)} 秒` : '')"
                />
                <span v-else class="dim">—</span>
              </template>
              <template v-else-if="col.key === 'status'">
                <StatusChip
                  :tone="statusTone(job.status)"
                  :pulse="job.status === 'downloading'"
                  :label="statusLabel(job.status)"
                />
              </template>
              <template v-else>
                {{ formatCell(col.key, job) }}
              </template>
            </td>
            <td class="actions" @click.stop>
              <button
                v-if="['failed','expired'].includes(job.status)"
                class="btn btn-sm"
                @click="retry(job.id)"
              >重试</button>
              <button
                v-if="['queued','downloading'].includes(job.status)"
                class="btn-danger btn-sm"
                @click="cancel(job.id)"
              >取消</button>
              <button
                v-if="job.status === 'completed' && job.filePath && !['pending','running'].includes(job.analysisStatus ?? '')"
                class="btn btn-sm"
                :title="job.analysisStatus === 'failed' ? (job.analysisError ?? '') : ''"
                @click="reanalyze(job.id)"
              >{{ job.analysisStatus ? '重新解析' : '解析' }}</button>
            </td>
          </tr>
          <tr v-if="visibleJobs.length === 0">
            <td :colspan="visibleColumns.length + 2" class="empty">暂无对局记录</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 详情抽屉 -->
    <USlideover v-model:open="drawerOpen" side="right">
      <template #content="{ close }">
        <div class="drawer-body">
          <div class="drawer-header">
            <span class="drawer-title">对局详情</span>
            <button class="btn-ghost btn-sm btn-icon" aria-label="关闭" @click="close">✕</button>
          </div>
          <template v-if="selected">
            <div class="drawer-status">
              <StatusChip :tone="statusTone(selected.status)" :label="statusLabel(selected.status)" />
              <StatusChip
                v-if="selected.analysisStatus"
                :tone="analysisTone(selected.analysisStatus)"
                :label="analysisLabel(selected.analysisStatus)"
              />
            </div>
            <dl class="kv-grid drawer-kv">
              <template v-for="[k, v] in detailRows" :key="k">
                <dt>{{ k }}</dt>
                <dd :class="{ mono: k === 'Game Info' }">{{ v }}</dd>
              </template>
            </dl>
            <button
              v-if="['failed','expired'].includes(selected.status)"
              class="btn-primary retry-full-btn"
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
import { listenAnalysisUpdated, listenBackgroundTask, listenImportCompleted, listenJobProgress, listenJobsUpdated, listenMetadataRefreshCompleted, tauriInvoke } from '~/composables/useTauri';
import { formatMatchCell, useMatchColumns, type MatchColumnKey } from '~/composables/useMatchColumns';

definePageMeta({ layout: 'default' });

const { status, toggleQueue, discoverNow } = useAppStatus();
const toast = useToast();
const { visibleKeys, visibleColumns, toggleColumn, resetColumns, MATCH_COLUMNS } = useMatchColumns();
const { isFavorite, toggleFavorite } = useMatchStorage();

const jobs = ref<MatchDto[]>([]);
const filterPlatform = ref('');
const filterStatus = ref('');
const search = ref('');
const drawerOpen = ref(false);
const selected = ref<MatchDto | null>(null);
const colPickerOpen = ref(false);

const stats = computed(() => {
  const c = (s: string) => jobs.value.filter(j => j.status === s).length;
  const failed = c('failed') + c('expired');
  // 只有需要关注的数字才带语义色块；数字本身保持白色
  return [
    { label: '已完成', value: c('completed'), tone: 'neutral' },
    { label: '排队中', value: c('queued'), tone: 'neutral' },
    { label: '下载中', value: c('downloading'), tone: c('downloading') > 0 ? 'info' : 'neutral' },
    { label: '失败', value: failed, tone: failed > 0 ? 'danger' : 'neutral' },
    { label: '已解析', value: jobs.value.filter(j => j.analysisStatus === 'done').length, tone: 'success' },
  ] as const;
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
  const num = MATCH_COLUMNS.find(c => c.key === key)?.className === 'num' ? 'num ' : '';
  return num + ({
    fileName: 'truncate',
    mapName: 'strong',
    durationSeconds: 'muted',
    playedAt: 'muted nowrap',
    gameMode: 'muted nowrap',
    gameType: 'muted',
    matchId: 'mono muted',
    shareCode: 'mono muted nowrap',
    filePath: 'truncate muted',
    demoVersionGuid: 'mono muted',
    demGameInfo: 'truncate mono muted',
  }[key] ?? '');
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

function analysisLabel(s: string) {
  return { pending: '待解析', running: '解析中', done: '已解析', failed: '解析失败' }[s] ?? s;
}

type Tone = 'neutral' | 'success' | 'warning' | 'danger' | 'info';

function analysisTone(s: string): Tone {
  return ({ pending: 'neutral', running: 'info', done: 'success', failed: 'danger' } as Record<string, Tone>)[s] ?? 'neutral';
}

async function reanalyze(id: string) {
  try {
    await tauriInvoke<number>('reanalyze_matches', { ids: [id] });
    const job = jobs.value.find(j => j.id === id);
    if (job) job.analysisStatus = 'pending';
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '无法开始解析', description: message, color: 'error' });
  }
}

function statusLabel(s: string) {
  return { queued: '排队中', downloading: '下载中', completed: '已完成', failed: '失败', skipped: '跳过', expired: '过期' }[s] ?? s;
}

function statusTone(s: string): Tone {
  return ({ queued: 'neutral', downloading: 'info', completed: 'success', failed: 'danger', expired: 'warning', skipped: 'neutral' } as Record<string, Tone>)[s] ?? 'neutral';
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
  let reloadTimer: ReturnType<typeof setTimeout> | null = null;
  const u6 = listenAnalysisUpdated(({ matchId, status: analysisStatus, error }) => {
    const job = jobs.value.find(j => j.id === matchId);
    if (job) {
      job.analysisStatus = analysisStatus;
      job.analysisError = error ?? null;
    }
    // 解析完成后回合数等字段变化，合并刷新
    if (analysisStatus === 'done' || analysisStatus === 'failed') {
      if (reloadTimer) clearTimeout(reloadTimer);
      reloadTimer = setTimeout(loadJobs, 800);
    }
  });
  onUnmounted(() => { u1(); u2(); u3(); u4(); u5(); u6(); if (reloadTimer) clearTimeout(reloadTimer); });
});
</script>

<style scoped>
/* 收藏（W3-D） */
.fav-col { width: 28px; padding-right: 0 !important; }
.fav-btn { width: 22px; height: 22px; border: 0; background: none; cursor: pointer; color: var(--text-disabled); font-size: var(--text-md); line-height: 1; border-radius: var(--radius-sm); }
.fav-btn:hover { color: var(--text-2); background: var(--surface-hover); }
.fav-btn.on { color: var(--text); }
.matches-page { display: flex; flex-direction: column; gap: var(--space-4); height: 100%; }

.stats-row { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: var(--space-3); }

/* 工具栏 */
.toolbar { display: flex; flex-wrap: wrap; gap: var(--space-2); align-items: center; }
.toolbar-right { margin-left: auto; display: flex; gap: var(--space-2); align-items: center; }
.select { min-width: 100px; }
.search-input { flex: 1; min-width: 160px; max-width: 260px; }

.col-picker-wrap { position: relative; }
.btn.is-open { border-color: var(--border-strong); color: var(--text); }
.col-picker-panel {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 20;
  min-width: 180px;
  max-height: 340px;
  overflow-y: auto;
  padding: var(--space-1);
  background: var(--surface-raised);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
}
.col-picker-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 28px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  color: var(--text-2);
  cursor: pointer;
}
.col-picker-item:hover { background: var(--surface-hover); color: var(--text); }
.col-picker-item input { accent-color: var(--gray-0); }
.col-picker-reset { width: 100%; margin-top: var(--space-1); }

.queue-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--text-disabled); flex-shrink: 0; }
.queue-toggle.active { color: var(--text); }
.queue-toggle.active .queue-dot { background: var(--success); }

/* 表格 */
.match-table-wrap { flex: 1; }
.col-prog { width: 120px; }
.prog-cell { display: flex; align-items: center; gap: var(--space-2); }
.prog-cell .progress { flex: 1; }
.prog-pct { width: 32px; text-align: right; font-size: var(--text-xs); color: var(--text-3); }

/* 抽屉 */
.drawer-body {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4) var(--space-5);
  background: var(--surface);
  box-sizing: border-box;
  overflow-y: auto;
}
.drawer-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
}
.drawer-title { font-size: var(--text-lg); font-weight: 600; color: var(--text); }
.drawer-status { display: flex; gap: var(--space-2); }
.drawer-kv { margin: 0; }
.retry-full-btn { width: 100%; height: 34px; margin-top: auto; flex-shrink: 0; }
</style>
