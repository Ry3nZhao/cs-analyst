<template>
  <div class="dl-page">
    <!-- Stat cards -->
    <div class="stats-row">
      <div v-for="s in stats" :key="s.label" class="stat-card">
        <div class="stat-num" :style="{ color: s.color }">{{ s.value }}</div>
        <div class="stat-label">{{ s.label }}</div>
      </div>
    </div>

    <!-- Filters -->
    <div class="filters">
      <select v-model="filterUser" class="sel">
        <option value="">全部用户</option>
        <option v-for="u in uniqueUsers" :key="u.id" :value="u.id">{{ u.name }}</option>
      </select>
      <select v-model="filterPlatform" class="sel sel-sm">
        <option value="">全部平台</option>
        <option value="valve">Valve</option>
        <option value="renown">Renown</option>
        <option value="5eplay">5EPlay</option>
      </select>
      <select v-model="filterStatus" class="sel sel-sm">
        <option value="">全部状态</option>
        <option value="discovered">已发现</option>
        <option value="queued">排队中</option>
        <option value="downloading">下载中</option>
        <option value="completed">已完成</option>
        <option value="failed">失败</option>
        <option value="skipped">跳过</option>
        <option value="expired">过期</option>
      </select>
      <input v-model="search" class="search-input" placeholder="搜索地图、账号…" />
    </div>

    <!-- Table -->
    <div class="table-wrap">
      <table class="dl-table">
        <thead>
          <tr>
            <th>平台</th>
            <th>用户 · 账号</th>
            <th>地图</th>
            <th>时间</th>
            <th class="col-prog">进度</th>
            <th>状态</th>
            <th class="col-actions"></th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="job in visibleJobs"
            :key="job.jobId"
            class="dl-row"
            @click="openDetail(job)"
          >
            <td>
              <span class="platform-chip">{{ platformLabel(job.platform) }}</span>
            </td>
            <td>
              <div class="user-name">{{ job.userName }}</div>
              <div class="account-name">{{ job.accountName }}</div>
            </td>
            <td class="map-name">{{ job.mapName ?? '—' }}</td>
            <td class="time-col">{{ fmtTime(job.playedAt) }}</td>
            <td class="col-prog">
              <template v-if="job.status === 'downloading'">
                <div class="prog-bar">
                  <div class="prog-fill" :style="{ width: `${Math.round((job.progress || 0) * 100)}%` }" />
                </div>
                <span class="prog-pct">{{ Math.round((job.progress || 0) * 100) }}%</span>
              </template>
              <span v-else class="dash">—</span>
            </td>
            <td>
              <span class="status-chip" :style="statusStyle(job.status)">
                <span class="status-dot2" :style="{ background: statusColor(job.status) }" />
                {{ statusLabel(job.status) }}
              </span>
            </td>
            <td class="col-actions" @click.stop>
              <button
                v-if="['failed','expired'].includes(job.status)"
                class="act-btn act-retry"
                @click="retry(job.jobId)"
              >重试</button>
              <button
                v-if="['queued','downloading'].includes(job.status)"
                class="act-btn act-cancel"
                @click="cancel(job.jobId)"
              >取消</button>
            </td>
          </tr>
          <tr v-if="visibleJobs.length === 0">
            <td colspan="7" class="empty-row">暂无任务</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Detail drawer -->
    <USlideover v-model:open="drawerOpen" side="right">
      <div class="drawer-body">
        <div class="drawer-header">
          <span class="drawer-title">任务详情</span>
          <button class="drawer-close" @click="drawerOpen = false">✕</button>
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
            @click="retry(selected.jobId); drawerOpen = false"
          >立即重试</button>
        </template>
      </div>
    </USlideover>
  </div>
</template>

<script setup lang="ts">
import type { DownloadJobDto } from '@cs-demo-analyst/shared-types';
import { listenJobProgress, listenJobsUpdated, tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: 'default' });

const jobs = ref<DownloadJobDto[]>([]);
const filterUser = ref('');
const filterPlatform = ref('');
const filterStatus = ref('');
const search = ref('');
const drawerOpen = ref(false);
const selected = ref<DownloadJobDto | null>(null);

const uniqueUsers = computed(() => {
  const seen = new Map<string, string>();
  for (const j of jobs.value) seen.set(j.userId, j.userName);
  return [...seen.entries()].map(([id, name]) => ({ id, name }));
});

const stats = computed(() => {
  const c = (s: string) => jobs.value.filter(j => j.status === s).length;
  return [
    { label: '排队中', value: c('queued'),      color: '#818cf8' },
    { label: '下载中', value: c('downloading'), color: 'var(--c-accent-lt)' },
    { label: '已完成', value: c('completed'),   color: 'var(--c-success)' },
    { label: '失败',   value: c('failed') + c('expired'), color: 'var(--c-error)' },
  ];
});

const visibleJobs = computed(() =>
  jobs.value.filter(j => {
    if (filterUser.value && j.userId !== filterUser.value) return false;
    if (filterPlatform.value && j.platform !== filterPlatform.value) return false;
    if (filterStatus.value && j.status !== filterStatus.value) return false;
    if (search.value) {
      const q = search.value.toLowerCase();
      return j.matchId.toLowerCase().includes(q)
        || j.accountName.toLowerCase().includes(q)
        || (j.mapName ?? '').toLowerCase().includes(q);
    }
    return true;
  }),
);

const detailRows = computed(() => {
  if (!selected.value) return [];
  const s = selected.value;
  return ([
    ['平台', platformLabel(s.platform)],
    ['Match ID', s.matchId],
    ['Share Code', s.shareCode],
    ['Demo URL', s.demoUrl],
    ['Valve 频道', s.valveChannel],
    ['尝试次数', String(s.attempts)],
    ['错误信息', s.lastError],
  ] as [string, string | null | undefined][]).filter(([, v]) => v != null && v !== '');
});

function platformLabel(p: string) {
  return { valve: 'Valve', renown: 'Renown', '5eplay': '5EPlay' }[p] ?? p;
}

function statusLabel(s: string) {
  return { discovered: '已发现', queued: '排队中', downloading: '下载中', completed: '已完成', failed: '失败', skipped: '跳过', expired: '过期' }[s] ?? s;
}

function statusColor(s: string) {
  return { downloading: '#a78bfa', completed: '#22d3a0', failed: '#f16060', expired: '#f97316', queued: '#818cf8', discovered: '#5c647a', skipped: '#3d4460' }[s] ?? '#5c647a';
}

function statusStyle(s: string) {
  const c = statusColor(s);
  return { color: c, background: `${c}18`, borderColor: `${c}30` };
}

function fmtTime(iso?: string | null) {
  if (!iso) return '—';
  return new Date(iso).toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}

async function loadJobs() {
  jobs.value = await tauriInvoke<DownloadJobDto[]>('list_download_jobs');
}

async function retry(jobId: string) {
  await tauriInvoke('retry_download_job', { jobId });
  await loadJobs();
}

async function cancel(jobId: string) {
  await tauriInvoke('cancel_download_job', { jobId });
  await loadJobs();
}

function openDetail(job: DownloadJobDto) {
  selected.value = job;
  drawerOpen.value = true;
}

onMounted(() => {
  loadJobs();
  const u1 = listenJobsUpdated(loadJobs);
  const u2 = listenJobProgress(({ jobId, progress }) => {
    const job = jobs.value.find(j => j.jobId === jobId);
    if (job) job.progress = progress;
  });
  onUnmounted(() => { u1(); u2(); });
});
</script>

<style scoped>
.dl-page { display: flex; flex-direction: column; gap: 16px; height: 100%; }

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

/* Filters */
.filters { display: flex; gap: 8px; }
.sel {
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: 7px;
  color: var(--c-text-2);
  padding: 6px 10px;
  font-size: 12px;
  cursor: pointer;
  outline: none;
  min-width: 120px;
}
.sel:focus { border-color: var(--c-accent); }
.sel-sm { min-width: 90px; }
.search-input {
  flex: 1;
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

/* Table */
.table-wrap {
  flex: 1;
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: 10px;
  overflow: auto;
  min-height: 0;
}
.dl-table { width: 100%; border-collapse: collapse; }
.dl-table thead th {
  padding: 10px 14px;
  text-align: left;
  font-size: 11px;
  font-weight: 500;
  color: var(--c-text-3);
  border-bottom: 1px solid var(--c-border);
  white-space: nowrap;
}
.dl-row {
  cursor: pointer;
  transition: background 0.1s;
}
.dl-row:hover { background: var(--c-card-hover); }
.dl-row td {
  padding: 10px 14px;
  font-size: 12px;
  border-bottom: 1px solid var(--c-border-sub);
  vertical-align: middle;
}

.platform-chip {
  display: inline-block;
  padding: 2px 7px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
  background: var(--c-accent-bg);
  color: var(--c-accent-lt);
  border: 1px solid var(--c-accent-bdr);
}
.user-name   { color: var(--c-text); font-weight: 500; }
.account-name { color: var(--c-text-3); font-size: 11px; margin-top: 1px; }
.map-name    { color: var(--c-text-2); }
.time-col    { color: var(--c-text-3); white-space: nowrap; }
.col-prog    { width: 120px; }
.col-actions { width: 80px; }

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

.act-btn {
  padding: 3px 9px;
  border-radius: 5px;
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid;
  transition: opacity 0.1s;
}
.act-btn:hover { opacity: 0.75; }
.act-retry  { background: rgba(129,140,248,.12); color: #818cf8; border-color: rgba(129,140,248,.3); }
.act-cancel { background: rgba(241,96,96,.1);   color: #f16060; border-color: rgba(241,96,96,.25); }

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
