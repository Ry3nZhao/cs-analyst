<template>
  <div class="space-y-4">
    <div class="flex flex-wrap gap-2">
      <USelect v-model="filterUser" :items="userOptions" placeholder="用户" class="w-40" />
      <USelect v-model="filterPlatform" :items="platformOptions" placeholder="平台" class="w-32" />
      <USelect v-model="filterStatus" :items="statusOptions" placeholder="状态" class="w-32" />
      <UInput v-model="search" placeholder="搜索..." class="w-48" />
    </div>

    <div class="grid grid-cols-2 md:grid-cols-6 gap-3">
      <UCard v-for="card in statCards" :key="card.label" class="bg-[#242830] border-[#2e3340]">
        <div class="text-sm text-gray-400">{{ card.label }}</div>
        <div class="text-2xl font-semibold">{{ card.count }}</div>
      </UCard>
    </div>

    <UCard class="bg-[#242830] border-[#2e3340]">
      <UTable :data="filteredJobs" :columns="columns">
        <template #status-cell="{ row }">
          <UBadge :color="statusColor(row.original.status)" variant="subtle">{{ row.original.status }}</UBadge>
        </template>
        <template #progress-cell="{ row }">
          <span v-if="row.original.status === 'downloading'">{{ Math.round(row.original.progress * 100) }}%</span>
          <span v-else>—</span>
        </template>
        <template #actions-cell="{ row }">
          <div class="flex gap-1">
            <UButton
              v-if="['failed', 'expired'].includes(row.original.status)"
              size="xs"
              @click="retry(row.original.jobId)"
            >重试</UButton>
            <UButton
              v-if="['queued', 'downloading'].includes(row.original.status)"
              size="xs"
              color="warning"
              @click="cancel(row.original.jobId)"
            >取消</UButton>
            <UButton size="xs" variant="ghost" @click="openDetail(row.original)">详情</UButton>
          </div>
        </template>
      </UTable>
    </UCard>

    <USlideover v-model:open="drawerOpen" title="任务详情">
      <div v-if="selected" class="space-y-3 text-sm">
        <p><b>match_id:</b> {{ selected.matchId }}</p>
        <p><b>share_code:</b> {{ selected.shareCode ?? '—' }}</p>
        <p><b>demo_url:</b> <span class="break-all">{{ selected.demoUrl ?? '—' }}</span></p>
        <p><b>错误:</b> {{ selected.lastError ?? '—' }}</p>
        <p><b>attempts:</b> {{ selected.attempts }}</p>
        <UButton
          v-if="['failed', 'expired'].includes(selected.status)"
          @click="retry(selected.jobId)"
        >立即重试</UButton>
      </div>
    </USlideover>
  </div>
</template>

<script setup lang="ts">
import type { DownloadJobDto } from '@cs-demo-analyst/shared-types';
import { listenJobProgress, listenJobsUpdated, tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: 'default' });

const jobs = ref<DownloadJobDto[]>([]);
const filterUser = ref('all');
const filterPlatform = ref('all');
const filterStatus = ref('all');
const search = ref('');
const drawerOpen = ref(false);
const selected = ref<DownloadJobDto | null>(null);

const userOptions = computed(() => [
  { label: '全部用户', value: 'all' },
  ...[...new Set(jobs.value.map((j) => j.userId))].map((id) => ({
    label: jobs.value.find((j) => j.userId === id)?.userName ?? id,
    value: id,
  })),
]);

const platformOptions = [
  { label: '全部', value: 'all' },
  { label: 'Valve', value: 'valve' },
  { label: 'Renown', value: 'renown' },
  { label: '5EPlay', value: '5eplay' },
];

const statusOptions = [
  { label: '全部', value: 'all' },
  { label: '下载中', value: 'downloading' },
  { label: '队列', value: 'queued' },
  { label: '完成', value: 'completed' },
  { label: '失败', value: 'failed' },
  { label: '跳过', value: 'skipped' },
];

const columns = [
  { accessorKey: 'userName', header: '用户' },
  { accessorKey: 'platform', header: '平台' },
  { accessorKey: 'accountName', header: '子账号' },
  { accessorKey: 'mapName', header: '地图' },
  { accessorKey: 'playedAt', header: '时间' },
  { accessorKey: 'valveChannel', header: '通道' },
  { id: 'progress', header: '进度' },
  { id: 'status', header: '状态' },
  { id: 'actions', header: '操作' },
];

const filteredJobs = computed(() =>
  jobs.value.filter((j) => {
    if (filterUser.value !== 'all' && j.userId !== filterUser.value) return false;
    if (filterPlatform.value !== 'all' && j.platform !== filterPlatform.value) return false;
    if (filterStatus.value !== 'all' && j.status !== filterStatus.value) return false;
    if (search.value) {
      const q = search.value.toLowerCase();
      return (
        j.matchId.toLowerCase().includes(q) ||
        j.accountName.toLowerCase().includes(q) ||
        (j.mapName ?? '').toLowerCase().includes(q)
      );
    }
    return true;
  }),
);

const statCards = computed(() => {
  const counts: Record<string, number> = {};
  for (const j of jobs.value) counts[j.status] = (counts[j.status] ?? 0) + 1;
  return [
    { label: '发现', count: counts.discovered ?? 0 },
    { label: '队列', count: counts.queued ?? 0 },
    { label: '下载中', count: counts.downloading ?? 0 },
    { label: '完成', count: counts.completed ?? 0 },
    { label: '失败', count: counts.failed ?? 0 },
    { label: '跳过', count: counts.skipped ?? 0 },
  ];
});

function statusColor(status: string) {
  const map: Record<string, string> = {
    discovered: 'neutral',
    queued: 'warning',
    downloading: 'info',
    completed: 'success',
    failed: 'error',
    expired: 'error',
    skipped: 'warning',
  };
  return map[status] ?? 'neutral';
}

async function loadJobs() {
  jobs.value = await tauriInvoke<DownloadJobDto[]>('list_download_jobs');
}

async function retry(jobId: string) {
  await tauriInvoke('retry_download_job', { jobId });
  drawerOpen.value = false;
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
    const job = jobs.value.find((j) => j.jobId === jobId);
    if (job) job.progress = progress;
  });
  onUnmounted(() => {
    u1();
    u2();
  });
});
</script>
