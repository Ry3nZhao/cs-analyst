<template>
  <div class="space-y-4">
    <UCard class="bg-[#242830] border-[#2e3340]">
      <div class="flex items-center justify-between mb-4">
        <h3 class="font-medium capitalize">{{ platform }} 子账号</h3>
        <div class="flex items-center gap-3">
          <label class="flex items-center gap-2 text-sm">
            <USwitch v-model="platformEnabled" @update:model-value="savePlatformEnabled" />
            平台启用
          </label>
          <UButton size="sm" :disabled="users.length === 0" @click="showAdd = true">添加子账号</UButton>
        </div>
      </div>
      <p v-if="users.length === 0" class="text-sm text-amber-400">请先创建用户</p>
      <UTable v-else :data="accounts" :columns="columns">
        <template #status-cell="{ row }">
          <USwitch
            :model-value="row.original.status === 'active'"
            @update:model-value="(v: boolean) => toggleStatus(row.original.id, v)"
          />
        </template>
        <template #user-cell="{ row }">
          <USelect
            :model-value="row.original.userId"
            :items="userItems"
            class="w-36"
            @update:model-value="(v: string) => reassign(row.original.id, v)"
          />
        </template>
      </UTable>
    </UCard>

    <UModal v-model:open="showAdd" title="添加子账号">
      <template #body>
        <div class="space-y-3">
          <USelect v-model="form.userId" :items="userItems" placeholder="所属用户" />
          <UInput v-model="form.externalId" :placeholder="externalPlaceholder" />
          <UInput v-model="form.displayName" placeholder="显示名" />
          <template v-if="platform === 'valve'">
            <USelect v-model="form.valveChannel" :items="channelItems" />
            <UInput v-if="form.valveChannel === 'authcode'" v-model="form.authCode" placeholder="Auth Code" />
            <UInput v-if="form.valveChannel === 'authcode'" v-model="form.oldestShareCode" placeholder="Oldest Share Code" />
          </template>
        </div>
      </template>
      <template #footer>
        <UButton @click="addAccount">保存</UButton>
      </template>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import type { Platform, SubAccountDto, UserDto } from '@cs-demo-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

const props = defineProps<{ platform: string; users: UserDto[] }>();
const emit = defineEmits<{ changed: [] }>();

const accounts = ref<SubAccountDto[]>([]);
const platformEnabled = ref(false);
const showAdd = ref(false);
const form = reactive({
  userId: '',
  externalId: '',
  displayName: '',
  valveChannel: 'authcode',
  authCode: '',
  oldestShareCode: '',
});

const platform = computed(() => props.platform as Platform);
const userItems = computed(() => props.users.map((u) => ({ label: u.displayName, value: u.id })));
const channelItems = [
  { label: 'Auth Code', value: 'authcode' },
  { label: 'Local Login', value: 'local' },
];

const externalPlaceholder = computed(() => {
  if (platform.value === '5eplay') return '5E UUID';
  return 'Steam ID 64';
});

const columns = [
  { accessorKey: 'displayName', header: '显示名' },
  { accessorKey: 'externalId', header: 'External ID' },
  { id: 'user', header: '所属用户' },
  { id: 'status', header: '启用' },
];

async function load() {
  accounts.value = await tauriInvoke<SubAccountDto[]>('list_sub_accounts', { platform: platform.value });
  const settings = await tauriInvoke<Array<{ platform: string; enabled: boolean }>>('list_platform_settings');
  platformEnabled.value = settings.find((s) => s.platform === platform.value)?.enabled ?? false;
}

async function savePlatformEnabled(enabled: boolean) {
  const settings = await tauriInvoke<Array<Record<string, unknown>>>('list_platform_settings');
  const current = settings.find((s) => s.platform === platform.value);
  if (current) {
    await tauriInvoke('update_platform_settings', {
      platform: { ...current, enabled },
    });
  }
}

async function addAccount() {
  const metadata: Record<string, unknown> = {};
  if (platform.value === 'valve') {
    metadata.source = form.valveChannel === 'authcode' ? 'auth_code' : 'steam_login';
    if (form.valveChannel === 'authcode') {
      metadata.auth_code = form.authCode;
      metadata.oldest_share_code = form.oldestShareCode;
    }
  }
  await tauriInvoke('add_sub_account', {
    userId: form.userId,
    platform: platform.value,
    externalId: form.externalId,
    displayName: form.displayName || form.externalId,
    metadata,
    valveChannel: platform.value === 'valve' ? form.valveChannel : null,
  });
  showAdd.value = false;
  await load();
  emit('changed');
}

async function toggleStatus(id: string, active: boolean) {
  await tauriInvoke('set_sub_account_status', { id, status: active ? 'active' : 'disabled' });
  await load();
}

async function reassign(id: string, userId: string) {
  await tauriInvoke('update_sub_account', { id, userId });
  await load();
}

watch(() => props.platform, load, { immediate: true });
</script>
