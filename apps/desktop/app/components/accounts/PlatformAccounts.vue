<template>
  <div class="platform-panel">
    <!-- Header -->
    <div class="section-header">
      <span class="section-title">{{ platformName }} 账号</span>
      <div class="header-actions">
        <label class="toggle-wrap">
          <span class="toggle-label">平台启用</span>
          <button
            class="toggle-btn"
            :class="{ on: platformEnabled }"
            @click="setPlatformEnabled(!platformEnabled)"
          >
            <span class="toggle-knob" />
          </button>
        </label>
        <button class="add-btn" :disabled="props.users.length === 0" @click="showAdd = true">
          + 添加账号
        </button>
      </div>
    </div>

    <p v-if="props.users.length === 0" class="warn-msg">请先在「用户」标签创建用户</p>

    <div v-else class="card-table">
      <table class="inner-table">
        <thead>
          <tr>
            <th>显示名</th>
            <th>External ID</th>
            <th>所属用户</th>
            <th class="col-status">启用</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="acc in accounts" :key="acc.id" class="inner-row">
            <td class="fw">{{ acc.displayName }}</td>
            <td class="mono">{{ acc.externalId }}</td>
            <td>
              <select
                class="user-sel"
                :value="acc.userId"
                @change="(e) => reassign(acc.id, (e.target as HTMLSelectElement).value)"
              >
                <option v-for="u in props.users" :key="u.id" :value="u.id">{{ u.displayName }}</option>
              </select>
            </td>
            <td class="col-status">
              <button
                class="toggle-btn sm"
                :class="{ on: acc.status === 'active' }"
                @click="toggleStatus(acc.id, acc.status !== 'active')"
              >
                <span class="toggle-knob" />
              </button>
            </td>
          </tr>
          <tr v-if="accounts.length === 0">
            <td colspan="4" class="empty-row">暂无账号</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Add modal -->
    <UModal v-model:open="showAdd" title="添加账号">
      <template #body>
        <div class="modal-fields">
          <div class="field">
            <label class="field-label">所属用户</label>
            <select v-model="form.userId" class="field-select">
              <option value="">请选择用户</option>
              <option v-for="u in props.users" :key="u.id" :value="u.id">{{ u.displayName }}</option>
            </select>
          </div>
          <div class="field">
            <label class="field-label">{{ externalPlaceholder }}</label>
            <input v-model="form.externalId" class="field-input" :placeholder="externalPlaceholder" />
          </div>
          <div class="field">
            <label class="field-label">显示名</label>
            <input v-model="form.displayName" class="field-input" placeholder="可选，默认用 ID" />
          </div>
          <template v-if="props.platform === 'valve'">
            <div class="field">
              <label class="field-label">Valve 频道</label>
              <select v-model="form.valveChannel" class="field-select">
                <option value="authcode">Auth Code</option>
                <option value="local">Local Login</option>
              </select>
            </div>
            <template v-if="form.valveChannel === 'authcode'">
              <div class="field">
                <label class="field-label">Auth Code</label>
                <input v-model="form.authCode" class="field-input" placeholder="XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" />
              </div>
              <div class="field">
                <label class="field-label">Oldest Share Code</label>
                <input v-model="form.oldestShareCode" class="field-input" placeholder="CSGO-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" />
              </div>
            </template>
          </template>
        </div>
      </template>
      <template #footer>
        <div class="modal-footer">
          <button class="modal-cancel" @click="showAdd = false">取消</button>
          <button class="modal-save" @click="addAccount">添加</button>
        </div>
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
const form = reactive({ userId: '', externalId: '', displayName: '', valveChannel: 'authcode', authCode: '', oldestShareCode: '' });

const platformName = computed(() => ({ valve: 'Valve', renown: 'Renown', '5eplay': '5EPlay' }[props.platform] ?? props.platform));
const externalPlaceholder = computed(() => props.platform === '5eplay' ? '5E UUID' : 'Steam ID 64');

async function load() {
  accounts.value = await tauriInvoke<SubAccountDto[]>('list_sub_accounts', { platform: props.platform });
  const settings = await tauriInvoke<Array<{ platform: string; enabled: boolean }>>('list_platform_settings');
  platformEnabled.value = settings.find(s => s.platform === props.platform)?.enabled ?? false;
}

async function setPlatformEnabled(enabled: boolean) {
  platformEnabled.value = enabled;
  const all = await tauriInvoke<Array<Record<string, unknown>>>('list_platform_settings');
  const cur = all.find(s => s.platform === props.platform);
  if (cur) await tauriInvoke('update_platform_settings', { platform: { ...cur, enabled } });
}

async function addAccount() {
  const metadata: Record<string, unknown> = {};
  if (props.platform === 'valve') {
    metadata.source = form.valveChannel === 'authcode' ? 'auth_code' : 'steam_login';
    if (form.valveChannel === 'authcode') {
      metadata.auth_code = form.authCode;
      metadata.oldest_share_code = form.oldestShareCode;
    }
  }
  await tauriInvoke('add_sub_account', {
    userId: form.userId,
    platform: props.platform,
    externalId: form.externalId,
    displayName: form.displayName || form.externalId,
    metadata,
    valveChannel: props.platform === 'valve' ? form.valveChannel : null,
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

<style scoped>
.platform-panel { display: flex; flex-direction: column; gap: 14px; }

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.section-title { font-size: 13px; font-weight: 600; color: #fff; }
.header-actions { display: flex; align-items: center; gap: 12px; }

/* Toggle switch */
.toggle-wrap { display: flex; align-items: center; gap: 7px; cursor: pointer; }
.toggle-label { font-size: 12px; color: var(--c-text-2); }
.toggle-btn {
  position: relative;
  width: 32px;
  height: 18px;
  border-radius: 9px;
  background: var(--c-border);
  border: none;
  cursor: pointer;
  transition: background 0.2s;
  padding: 0;
  flex-shrink: 0;
}
.toggle-btn.on { background: var(--c-accent); }
.toggle-btn.sm { width: 28px; height: 16px; border-radius: 8px; }
.toggle-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #fff;
  transition: transform 0.2s;
  display: block;
}
.toggle-btn.sm .toggle-knob { width: 12px; height: 12px; }
.toggle-btn.on .toggle-knob { transform: translateX(14px); }
.toggle-btn.sm.on .toggle-knob { transform: translateX(12px); }

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
.add-btn:disabled { opacity: 0.35; cursor: default; }

.warn-msg {
  padding: 12px 14px;
  background: rgba(245,158,11,0.08);
  border: 1px solid rgba(245,158,11,0.2);
  border-radius: 8px;
  color: #f59e0b;
  font-size: 12px;
}

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
.inner-row:hover { background: var(--c-card-hover); }
.inner-row td {
  padding: 9px 14px;
  font-size: 12px;
  border-bottom: 1px solid var(--c-border-sub);
  vertical-align: middle;
}
.fw   { color: var(--c-text); font-weight: 500; }
.mono { color: var(--c-text-2); font-family: monospace; font-size: 11px; }
.col-status { width: 60px; }

.user-sel {
  background: var(--c-card);
  border: 1px solid var(--c-border);
  border-radius: 5px;
  color: var(--c-text-2);
  font-size: 12px;
  padding: 3px 6px;
  cursor: pointer;
  outline: none;
  max-width: 140px;
}
.user-sel:focus { border-color: var(--c-accent); }

.empty-row { text-align: center; padding: 32px; color: var(--c-text-faint); font-size: 13px; }

/* Modal */
.modal-fields { display: flex; flex-direction: column; gap: 12px; }
.field { display: flex; flex-direction: column; gap: 5px; }
.field-label { font-size: 11px; color: var(--c-text-3); }
.field-input, .field-select {
  padding: 8px 10px;
  border-radius: 7px;
  border: 1px solid var(--c-border);
  background: var(--c-card);
  color: var(--c-text);
  font-size: 13px;
  outline: none;
  width: 100%;
  box-sizing: border-box;
}
.field-input:focus, .field-select:focus { border-color: var(--c-accent); }
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
