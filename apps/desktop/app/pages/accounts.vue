<template>
  <div class="flex gap-6">
    <nav class="w-40 space-y-1">
      <UButton
        v-for="tab in tabs"
        :key="tab.id"
        variant="ghost"
        class="w-full justify-start"
        :class="activeTab === tab.id ? 'bg-[#2e3340]' : ''"
        @click="activeTab = tab.id"
      >{{ tab.label }}</UButton>
    </nav>

    <div class="flex-1">
      <section v-if="activeTab === 'users'" class="space-y-4">
        <div class="flex justify-between items-center">
          <h2 class="text-lg font-medium">用户管理</h2>
          <UButton @click="showUserForm = true">+ 新建用户</UButton>
        </div>
        <UCard class="bg-[#242830] border-[#2e3340]">
          <UTable :data="users" :columns="userColumns">
            <template #actions-cell="{ row }">
              <UButton size="xs" variant="ghost" @click="editUser(row.original)">编辑</UButton>
              <UButton size="xs" color="error" variant="ghost" @click="removeUser(row.original.id)">删除</UButton>
            </template>
          </UTable>
        </UCard>
      </section>

      <section v-else class="space-y-4">
        <PlatformAccounts :platform="activeTab" :users="users" @changed="reload" />
      </section>
    </div>

    <UModal v-model:open="showUserForm" title="用户">
      <template #body>
        <div class="space-y-3">
          <UInput v-model="userForm.displayName" placeholder="显示名" />
          <UInput v-model="userForm.note" placeholder="备注" />
        </div>
      </template>
      <template #footer>
        <UButton @click="saveUser">保存</UButton>
      </template>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import type { UserDto } from '@cs-demo-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: 'default' });

const route = useRoute();
const activeTab = ref((route.query.tab as string) ?? 'users');
const users = ref<UserDto[]>([]);
const showUserForm = ref(false);
const editingUserId = ref<string | null>(null);
const userForm = reactive({ displayName: '', note: '' });

const tabs = [
  { id: 'users', label: '用户' },
  { id: 'valve', label: 'Valve' },
  { id: 'renown', label: 'Renown' },
  { id: '5eplay', label: '5EPlay' },
];

const userColumns = [
  { accessorKey: 'displayName', header: '显示名' },
  { accessorKey: 'note', header: '备注' },
  { id: 'actions', header: '操作' },
];

async function reload() {
  users.value = await tauriInvoke<UserDto[]>('list_users');
}

function editUser(user: UserDto) {
  editingUserId.value = user.id;
  userForm.displayName = user.displayName;
  userForm.note = user.note ?? '';
  showUserForm.value = true;
}

async function saveUser() {
  if (editingUserId.value) {
    await tauriInvoke('update_user', {
      id: editingUserId.value,
      displayName: userForm.displayName,
      note: userForm.note || null,
    });
  } else {
    await tauriInvoke('create_user', {
      displayName: userForm.displayName,
      note: userForm.note || null,
    });
  }
  showUserForm.value = false;
  editingUserId.value = null;
  userForm.displayName = '';
  userForm.note = '';
  await reload();
}

async function removeUser(id: string) {
  await tauriInvoke('delete_user', { id });
  await reload();
}

onMounted(reload);
</script>
