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
        <div class="section-header">
          <span class="section-title">用户管理</span>
          <button class="add-btn" @click="openUserForm()">+ 新建用户</button>
        </div>
        <div class="card-table">
          <table class="inner-table">
            <thead>
              <tr>
                <th>显示名</th>
                <th>备注</th>
                <th class="col-ops"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="u in users" :key="u.id" class="inner-row">
                <td class="fw">{{ u.displayName }}</td>
                <td class="muted">{{ u.note ?? '—' }}</td>
                <td class="col-ops">
                  <button class="op-btn" @click="openUserForm(u)">编辑</button>
                  <button class="op-btn op-del" @click="removeUser(u.id)">删除</button>
                </td>
              </tr>
              <tr v-if="users.length === 0">
                <td colspan="3" class="empty-row">暂无用户，请新建</td>
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

    <!-- User modal -->
    <UModal v-model:open="showUserModal" :title="editingUserId ? '编辑用户' : '新建用户'">
      <template #body>
        <div class="modal-fields">
          <div class="field">
            <label class="field-label">显示名</label>
            <input v-model="userForm.displayName" class="field-input" placeholder="玩家昵称" />
          </div>
          <div class="field">
            <label class="field-label">备注</label>
            <input v-model="userForm.note" class="field-input" placeholder="可选备注" />
          </div>
        </div>
      </template>
      <template #footer>
        <div class="modal-footer">
          <button class="modal-cancel" @click="showUserModal = false">取消</button>
          <button class="modal-save" @click="saveUser">保存</button>
        </div>
      </template>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import type { UserDto } from '@cs-demo-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

definePageMeta({ layout: 'default' });

const activeTab = ref('users');
const users = ref<UserDto[]>([]);
const showUserModal = ref(false);
const editingUserId = ref<string | null>(null);
const userForm = reactive({ displayName: '', note: '' });

const tabs = [
  { id: 'users',  label: '用户' },
  { id: 'valve',  label: 'Valve' },
  { id: 'renown', label: 'Renown' },
  { id: '5eplay', label: '5EPlay' },
];

async function loadUsers() {
  users.value = await tauriInvoke<UserDto[]>('list_users');
}

function openUserForm(user?: UserDto) {
  if (user) {
    editingUserId.value = user.id;
    userForm.displayName = user.displayName;
    userForm.note = user.note ?? '';
  } else {
    editingUserId.value = null;
    userForm.displayName = '';
    userForm.note = '';
  }
  showUserModal.value = true;
}

async function saveUser() {
  if (editingUserId.value) {
    await tauriInvoke('update_user', { id: editingUserId.value, displayName: userForm.displayName, note: userForm.note || null });
  } else {
    await tauriInvoke('create_user', { displayName: userForm.displayName, note: userForm.note || null });
  }
  showUserModal.value = false;
  await loadUsers();
}

async function removeUser(id: string) {
  await tauriInvoke('delete_user', { id });
  await loadUsers();
}

onMounted(loadUsers);
</script>

<style scoped>
.accounts-page {
  display: flex;
  gap: 0;
  height: 100%;
  margin: -20px;
}

.sub-nav {
  width: 130px;
  flex-shrink: 0;
  background: var(--c-sidebar);
  border-right: 1px solid var(--c-border);
  padding: 12px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sub-nav-item {
  display: block;
  width: 100%;
  padding: 8px 12px;
  text-align: left;
  font-size: 13px;
  border-radius: 7px;
  border: none;
  cursor: pointer;
  transition: background 0.1s, color 0.1s;
  color: var(--c-text-2);
  background: none;
}
.sub-nav-item:hover { background: rgba(255,255,255,0.04); color: var(--c-text); }
.sub-nav-item.active { background: var(--c-accent-bg); color: var(--c-accent-lt); font-weight: 500; }

.tab-content {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}
.section-title { font-size: 13px; font-weight: 600; color: #fff; }

.add-btn {
  padding: 6px 14px;
  border-radius: 7px;
  font-size: 12px;
  font-weight: 500;
  background: var(--c-accent-bg);
  color: var(--c-accent-lt);
  border: 1px solid var(--c-accent-bdr);
  cursor: pointer;
}
.add-btn:hover { opacity: 0.8; }
.add-btn:disabled { opacity: 0.4; cursor: default; }

.card-table {
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: 10px;
  overflow: hidden;
}
.inner-table { width: 100%; border-collapse: collapse; }
.inner-table thead th {
  padding: 9px 14px;
  text-align: left;
  font-size: 11px;
  font-weight: 500;
  color: var(--c-text-3);
  border-bottom: 1px solid var(--c-border);
}
.inner-row { transition: background 0.1s; }
.inner-row:hover { background: var(--c-card-hover); }
.inner-row td {
  padding: 10px 14px;
  font-size: 12px;
  border-bottom: 1px solid var(--c-border-sub);
}
.fw   { color: var(--c-text); font-weight: 500; }
.muted { color: var(--c-text-3); }
.col-ops { width: 120px; }

.op-btn {
  padding: 3px 9px;
  border-radius: 5px;
  font-size: 11px;
  font-weight: 500;
  border: 1px solid var(--c-border);
  background: var(--c-card);
  color: var(--c-text-2);
  cursor: pointer;
  margin-right: 4px;
}
.op-btn:hover { border-color: var(--c-accent-lt); color: var(--c-accent-lt); }
.op-del:hover { border-color: var(--c-error); color: var(--c-error); }

.empty-row { text-align: center; padding: 32px; color: var(--c-text-faint); font-size: 13px; }

/* Modal */
.modal-fields { display: flex; flex-direction: column; gap: 12px; padding: 4px 0; }
.field { display: flex; flex-direction: column; gap: 5px; }
.field-label { font-size: 11px; color: var(--c-text-3); }
.field-input {
  padding: 8px 10px;
  border-radius: 7px;
  border: 1px solid var(--c-border);
  background: var(--c-card);
  color: var(--c-text);
  font-size: 13px;
  outline: none;
}
.field-input:focus { border-color: var(--c-accent); }
.field-input::placeholder { color: var(--c-text-faint); }

.modal-footer { display: flex; justify-content: flex-end; gap: 8px; }
.modal-cancel {
  padding: 7px 16px;
  border-radius: 7px;
  border: 1px solid var(--c-border);
  background: none;
  color: var(--c-text-2);
  font-size: 13px;
  cursor: pointer;
}
.modal-cancel:hover { background: var(--c-card); }
.modal-save {
  padding: 7px 16px;
  border-radius: 7px;
  background: var(--c-accent);
  color: #fff;
  border: none;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
}
.modal-save:hover { opacity: 0.85; }
</style>
