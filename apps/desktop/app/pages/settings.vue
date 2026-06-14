<template>
  <div class="space-y-6 max-w-2xl">
    <UCard class="bg-[#242830] border-[#2e3340]">
      <h3 class="font-medium mb-4">数据目录</h3>
      <div class="flex gap-2">
        <UInput v-model="settings.dataRoot" class="flex-1" />
        <UButton @click="save">保存</UButton>
        <UButton variant="soft" @click="openDataRoot">打开目录</UButton>
      </div>
    </UCard>

    <UCard class="bg-[#242830] border-[#2e3340]">
      <h3 class="font-medium mb-4">调度</h3>
      <div class="space-y-3">
        <div class="flex items-center justify-between">
          <span>全局最大并发</span>
          <UInput v-model.number="settings.globalMaxConcurrent" type="number" class="w-24" />
        </div>
        <div class="flex items-center justify-between">
          <span>启动时自动发现</span>
          <USwitch v-model="settings.autoDiscoverOnStartup" />
        </div>
      </div>
      <UButton class="mt-4" @click="save">保存设置</UButton>
    </UCard>

    <UCard class="bg-[#242830] border-[#2e3340]">
      <h3 class="font-medium mb-4">平台重试</h3>
      <div v-for="p in platforms" :key="p.platform" class="border-b border-[#2e3340] py-3 last:border-0">
        <div class="font-medium capitalize mb-2">{{ p.platform }}</div>
        <div class="flex items-center gap-4 text-sm">
          <label class="flex items-center gap-2">
            <USwitch v-model="p.autoRetryFailed" @update:model-value="savePlatform(p)" />
            下周期自动重试
          </label>
          <UInput
            v-model.number="p.maxAutoAttempts"
            type="number"
            placeholder="不限"
            class="w-24"
            @blur="savePlatform(p)"
          />
        </div>
      </div>
    </UCard>

    <UCard class="bg-[#242830] border-[#2e3340]">
      <h3 class="font-medium mb-2">运行时</h3>
      <p class="text-sm text-gray-400">版本 0.1.0</p>
      <div class="flex gap-2 mt-3">
        <UButton size="sm" variant="soft" @click="openDb">打开数据库目录</UButton>
        <UButton size="sm" variant="soft" @click="openLogs">打开日志目录</UButton>
      </div>
    </UCard>
  </div>
</template>

<script setup lang="ts">
import type { AppSettingsDto } from '@cs-demo-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: 'default' });

const settings = reactive<AppSettingsDto>({
  dataRoot: '',
  globalMaxConcurrent: 3,
  autoDiscoverOnStartup: true,
});

const platforms = ref<
  { platform: string; autoRetryFailed: boolean; maxAutoAttempts: number | null }[]
>([]);

async function load() {
  Object.assign(settings, await tauriInvoke<AppSettingsDto>('get_settings'));
  platforms.value = await tauriInvoke('list_platform_settings');
}

async function save() {
  await tauriInvoke('update_settings', { settings });
}

async function savePlatform(p: (typeof platforms.value)[0]) {
  await tauriInvoke('update_platform_settings', { platform: p });
}

async function openDataRoot() {
  await tauriInvoke('open_path', { path: settings.dataRoot });
}

async function openDb() {
  await tauriInvoke('open_path', { path: `${settings.dataRoot}/db` });
}

async function openLogs() {
  await tauriInvoke('open_path', { path: `${settings.dataRoot}/state` });
}

onMounted(load);
</script>
