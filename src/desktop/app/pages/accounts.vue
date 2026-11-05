<template>
  <div class="accounts-page">
    <!-- Sub-nav -->
    <aside class="sub-nav">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="sub-nav-item"
        :class="{ active: activeTab === tab.id }"
        @click="activeTab = tab.id"
      >
        {{ tab.label }}
      </button>
    </aside>

    <!-- Content -->
    <div class="tab-content">
      <!-- Users -->
      <section v-if="activeTab === 'users'">
        <SectionHeader title="用户管理" description="一个用户可以绑定多个平台账号" :count="users.length" size="lg">
          <button type="button" class="btn-primary" @click.stop="openUserForm()">+ 新建用户</button>
        </SectionHeader>
        <div class="table-wrap">
          <table class="data-table">
            <thead>
              <tr>
                <th>显示名</th>
                <th>备注</th>
                <th class="actions"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="u in users" :key="u.id">
                <td class="strong">{{ u.displayName }}</td>
                <td class="muted">{{ u.note ?? '—' }}</td>
                <td class="actions">
                  <button type="button" class="btn btn-sm" @click.stop="openUserForm(u)">编辑</button>
                  <button class="btn-danger btn-sm" @click="removeUser(u.id)">删除</button>
                </td>
              </tr>
              <tr v-if="users.length === 0">
                <td colspan="3" class="empty">暂无用户，请新建</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <!-- Platform tabs -->
      <section v-else>
        <PlatformAccounts :platform="activeTab" :users="users" @changed="loadUsers" />
      </section>
    </div>

    <AppModal
      v-model:open="showUserModal"
      :title="editingUserId ? '编辑用户' : '新建用户'"
    >
      <div class="modal-fields">
        <p v-if="userFormError" class="form-error">{{ userFormError }}</p>
        <div class="field">
          <label class="field-label">显示名</label>
          <input
            v-model="userForm.displayName"
            class="input"
            placeholder="玩家昵称"
            @keyup.enter="saveUser"
          />
        </div>
        <div class="field">
          <label class="field-label">备注</label>
          <input
            v-model="userForm.note"
            class="input"
            placeholder="可选备注"
            @keyup.enter="saveUser"
          />
        </div>
      </div>
      <template #footer="{ close }">
        <div class="modal-footer">
          <button class="btn" type="button" @click="close">取消</button>
          <button class="btn-primary" type="button" :disabled="userFormSaving" @click="saveUser">
            {{ userFormSaving ? '保存中…' : '保存' }}
          </button>
        </div>
      </template>
    </AppModal>
  </div>
</template>

<script setup lang="ts">
import type { UserDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: 'default' });

const activeTab = ref('users');
const users = ref<UserDto[]>([]);
const showUserModal = ref(false);
const editingUserId = ref<string | null>(null);
const userFormSaving = ref(false);
const userFormError = ref('');
const userForm = reactive({ displayName: '', note: '' });
const toast = useToast();

const tabs = [
  { id: 'users',  label: '用户' },
  { id: 'valve',  label: 'Valve' },
  { id: 'renown', label: 'Renown' },
  { id: '5eplay', label: '5EPlay' },
];

async function loadUsers() {
  try {
    users.value = await tauriInvoke<UserDto[]>('list_users');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '加载用户失败', description: message, color: 'error' });
  }
}

function openUserForm(user?: UserDto) {
  userFormError.value = '';
  if (user) {
    editingUserId.value = user.id;
    userForm.displayName = user.displayName;
    userForm.note = user.note ?? '';
  } else {
    editingUserId.value = null;
    userForm.displayName = '';
    userForm.note = '';
  }
  void nextTick(() => {
    showUserModal.value = true;
  });
}

async function saveUser() {
  const displayName = userForm.displayName.trim();
  const note = userForm.note.trim();

  if (!displayName) {
    userFormError.value = '请填写显示名';
    return;
  }

  userFormError.value = '';
  userFormSaving.value = true;
  try {
    if (editingUserId.value) {
      await tauriInvoke('update_user', {
        id: editingUserId.value,
        displayName,
        note: note || null,
      });
      toast.add({ title: '用户已更新', color: 'success' });
    } else {
      await tauriInvoke('create_user', { displayName, note: note || null });
      toast.add({ title: '用户已创建', color: 'success' });
    }
    showUserModal.value = false;
    await loadUsers();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    userFormError.value = message;
    toast.add({ title: '保存用户失败', description: message, color: 'error' });
  } finally {
    userFormSaving.value = false;
  }
}

async function removeUser(id: string) {
  try {
    await tauriInvoke('delete_user', { id });
    toast.add({ title: '用户已删除', color: 'success' });
    await loadUsers();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '删除用户失败', description: message, color: 'error' });
  }
}

onMounted(loadUsers);
</script>

<style scoped>
.accounts-page {
  display: flex;
  height: calc(100% + 2 * var(--space-5));
  margin: calc(-1 * var(--space-5));
}

/* 二级导航 */
.sub-nav {
  width: 132px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-3) var(--space-2);
  background: var(--bg-sunken);
  border-right: 1px solid var(--border-subtle);
}
.sub-nav-item {
  position: relative;
  display: block;
  width: 100%;
  height: 30px;
  padding: 0 var(--space-3);
  border: none;
  border-radius: var(--radius-md);
  background: none;
  color: var(--text-3);
  font-size: var(--text-md);
  text-align: left;
  cursor: pointer;
  transition: background var(--duration), color var(--duration);
}
.sub-nav-item:hover { background: var(--surface-hover); color: var(--text); }
.sub-nav-item.active { background: var(--surface-active); color: var(--text); font-weight: 500; }
.sub-nav-item.active::before {
  content: '';
  position: absolute;
  left: -8px;
  top: 7px;
  bottom: 7px;
  width: 2px;
  border-radius: 0 2px 2px 0;
  background: var(--accent);
}

.tab-content { flex: 1; min-width: 0; overflow-y: auto; padding: var(--space-5); }

/* 弹窗 */
.modal-fields { display: flex; flex-direction: column; gap: var(--space-3); }
.modal-footer { display: flex; justify-content: flex-end; gap: var(--space-2); }
</style>
