<template>
  <div class="flex min-h-screen bg-[#1a1d24] text-gray-100">
    <aside class="w-56 border-r border-[#2e3340] bg-[#242830] p-4 flex flex-col gap-2">
      <div class="text-lg font-semibold mb-4 text-blue-400">CS Demo Analyst</div>
      <UButton
        v-for="item in nav"
        :key="item.to"
        :to="item.to"
        variant="ghost"
        color="neutral"
        class="justify-start"
        :class="route.path === item.to ? 'bg-[#2e3340]' : ''"
      >
        {{ item.label }}
      </UButton>
      <div class="mt-auto text-xs text-gray-500 pt-4 border-t border-[#2e3340]">
        <div>Sidecar: {{ status.sidecarRunning ? '●' : '○' }}</div>
        <div>Orchestrator: {{ status.orchestratorRunning ? '●' : '○' }}</div>
        <div class="truncate mt-1" :title="status.dataRoot">{{ status.dataRoot }}</div>
      </div>
    </aside>
    <div class="flex-1 flex flex-col min-w-0">
      <header class="h-14 border-b border-[#2e3340] flex items-center justify-between px-6 bg-[#242830]">
        <h1 class="font-medium">{{ title }}</h1>
        <div class="flex gap-2">
          <UButton size="sm" variant="soft" @click="discoverNow">立即发现</UButton>
          <UButton size="sm" variant="soft" color="warning" @click="toggleQueue">
            {{ status.queuePaused ? '恢复队列' : '暂停队列' }}
          </UButton>
        </div>
      </header>
      <main class="flex-1 p-6 overflow-auto">
        <slot />
      </main>
    </div>
  </div>
</template>

<script setup lang="ts">
const route = useRoute();
const { status, refreshStatus, discoverNow, toggleQueue } = useAppStatus();

const titles: Record<string, string> = {
  '/downloads': '下载统筹',
  '/accounts': '账号管理',
  '/settings': '设置',
};

const title = computed(() => titles[route.path] ?? 'CS Demo Analyst');

const nav = [
  { to: '/downloads', label: '▶ 下载' },
  { to: '/accounts', label: '■ 账号' },
  { to: '/settings', label: '⚙ 设置' },
];

onMounted(() => {
  refreshStatus();
  const unlisten = listenJobsUpdated(() => refreshStatus());
  onUnmounted(() => unlisten());
});
</script>
