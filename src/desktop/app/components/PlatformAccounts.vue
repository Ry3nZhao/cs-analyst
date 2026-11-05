<template>
  <div class="platform-panel">
    <!-- Header -->
    <SectionHeader :title="`${platformName} 账号`" :count="accounts.length" size="lg">
      <div class="header-actions">
        <label class="toggle-wrap">
          <span class="toggle-label">平台启用</span>
          <button
            class="toggle"
            :class="{ on: platformEnabled }"
            @click="setPlatformEnabled(!platformEnabled)"
          >
            <span class="toggle-knob" />
          </button>
        </label>
        <button type="button" class="btn-primary" :disabled="props.users.length === 0" @click.stop="openAddModal">
          + 添加账号
        </button>
      </div>
    </SectionHeader>

    <p v-if="props.users.length === 0" class="callout callout-warning">请先在「用户」标签创建用户</p>

    <template v-else>
      <p v-if="props.platform === 'valve' && !botReady" class="callout callout-warning">
        Auth Code 下载需先在
        <NuxtLink to="/settings">系统设置</NuxtLink>
        中配置 Valve Bot 账号（用户名、密码、Steam Web API Key）。
      </p>

      <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>显示名</th>
            <th>External ID</th>
            <th>所属用户</th>
            <th class="col-status">启用</th>
            <th class="actions"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="acc in accounts" :key="acc.id">
            <td class="strong">{{ acc.displayName }}</td>
            <td class="mono">{{ acc.externalId }}</td>
            <td>
              <select
                class="select input-sm user-sel"
                :value="acc.userId"
                @change="(e) => reassign(acc.id, (e.target as HTMLSelectElement).value)"
              >
                <option v-for="u in props.users" :key="u.id" :value="u.id">{{ u.displayName }}</option>
              </select>
            </td>
            <td class="col-status">
              <button
                class="toggle sm"
                :class="{ on: acc.status === 'active' }"
                @click="toggleStatus(acc.id, acc.status !== 'active')"
              >
                <span class="toggle-knob" />
              </button>
            </td>
            <td class="actions">
              <button type="button" class="btn btn-sm" @click.stop="openEditModal(acc)">编辑</button>
              <button type="button" class="btn-danger btn-sm" @click.stop="removeAccount(acc.id)">删除</button>
            </td>
          </tr>
          <tr v-if="accounts.length === 0">
            <td colspan="5" class="empty">暂无账号</td>
          </tr>
        </tbody>
      </table>
      </div>
    </template>

    <!-- Edit modal -->
    <AppModal v-model:open="showEdit" title="编辑账号">
      <div class="modal-fields">
        <div class="field">
          <label class="field-label">显示名</label>
          <input v-model="editForm.displayName" class="input" placeholder="显示名" @keyup.enter="saveEdit" />
        </div>
        <div class="field">
          <label class="field-label">所属用户</label>
          <select v-model="editForm.userId" class="select">
            <option v-for="u in props.users" :key="u.id" :value="u.id">{{ u.displayName }}</option>
          </select>
        </div>
        <template v-if="props.platform === 'valve'">
          <div class="field">
            <label class="field-label">Auth Code</label>
            <input v-model="editForm.authCode" class="input" placeholder="XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" />
          </div>
          <div class="field">
            <label class="field-label">Oldest Share Code</label>
            <input v-model="editForm.oldestShareCode" class="input" placeholder="CSGO-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" />
          </div>
        </template>
      </div>
      <template #footer>
        <div class="modal-footer">
          <p v-if="editError" class="footer-error">{{ editError }}</p>
          <button class="btn" type="button" :disabled="editSaving" @click="showEdit = false">取消</button>
          <button class="btn-primary" type="button" :disabled="editSaving" @click="saveEdit">
            {{ editSaving ? '保存中...' : '保存' }}
          </button>
        </div>
      </template>
    </AppModal>

    <!-- Add modal -->
    <AppModal v-model:open="showAdd" title="添加账号">
      <div class="modal-fields">
        <div class="field">
          <label class="field-label">所属用户</label>
          <select v-model="form.userId" class="select">
            <option value="">请选择用户</option>
            <option v-for="u in props.users" :key="u.id" :value="u.id">{{ u.displayName }}</option>
          </select>
        </div>
        <div class="field">
          <label class="field-label">{{ externalPlaceholder }}</label>
          <input v-model="form.externalId" class="input" :placeholder="externalPlaceholder" />
        </div>
        <p v-if="props.platform === '5eplay'" class="field-hint hint-5e">
          与 arena 个人页 URL 一致，如 …/player/boketto 则填 boketto
        </p>
        <div class="field">
          <label class="field-label">显示名</label>
          <input v-model="form.displayName" class="input" placeholder="可选，默认用 ID" />
        </div>
        <template v-if="props.platform === 'valve'">
          <div class="field">
            <label class="field-label">Valve 频道</label>
            <select v-model="form.valveChannel" class="select">
              <option value="authcode">Auth Code</option>
            </select>
          </div>
          <template v-if="form.valveChannel === 'authcode'">
            <div class="field">
              <label class="field-label">Auth Code</label>
              <input v-model="form.authCode" class="input" placeholder="XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" />
            </div>
            <div class="field">
              <label class="field-label">Oldest Share Code</label>
              <input v-model="form.oldestShareCode" class="input" placeholder="CSGO-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" />
            </div>
          </template>
        </template>
      </div>
      <template #footer>
        <div class="modal-footer">
          <p v-if="formError" class="footer-error">{{ formError }}</p>
          <button class="btn" type="button" :disabled="formSaving" @click="showAdd = false">取消</button>
          <button class="btn-primary" type="button" :disabled="formSaving" @click="addAccount">
            {{ formSaving ? '添加中...' : '添加' }}
          </button>
        </div>
      </template>
    </AppModal>
  </div>
</template>

<script setup lang="ts">
import type { SubAccountDto, UserDto, ValveBotConfigDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

const props = defineProps<{ platform: string; users: UserDto[] }>();
const emit = defineEmits<{ changed: [] }>();

const accounts = ref<SubAccountDto[]>([]);
const platformEnabled = ref(false);
const showAdd = ref(false);
const showEdit = ref(false);
const editingId = ref('');
const editForm = reactive({ displayName: '', userId: '', authCode: '', oldestShareCode: '' });
const editError = ref('');
const editSaving = ref(false);
const valveBot = ref<ValveBotConfigDto | null>(null);
const form = reactive({ userId: '', externalId: '', displayName: '', valveChannel: 'authcode', authCode: '', oldestShareCode: '' });
const formError = ref('');
const formSaving = ref(false);
const toast = useToast();

const platformName = computed(() => ({ valve: 'Valve', renown: 'Renown', '5eplay': '5EPlay' }[props.platform] ?? props.platform));
const externalPlaceholder = computed(() =>
  props.platform === '5eplay'
    ? '5E ID / Domain（如 boketto）'
    : 'Steam ID 64',
);
const botReady = computed(() => {
  if (props.platform !== 'valve') return true;
  const bot = valveBot.value;
  if (!bot) return false;
  return Boolean(bot.username?.trim()) && bot.passwordSet && Boolean(bot.steamApiKey?.trim());
});

function openAddModal() {
  formError.value = '';
  form.userId = form.userId || props.users[0]?.id || '';
  void nextTick(() => {
    showAdd.value = true;
  });
}

async function load() {
  accounts.value = await tauriInvoke<SubAccountDto[]>('list_sub_accounts', { platform: props.platform });
  const settings = await tauriInvoke<Array<{ platform: string; enabled: boolean }>>('list_platform_settings');
  platformEnabled.value = settings.find(s => s.platform === props.platform)?.enabled ?? false;
  if (props.platform === 'valve') {
    valveBot.value = await tauriInvoke<ValveBotConfigDto>('get_valve_bot_config');
  } else {
    valveBot.value = null;
  }
}

async function setPlatformEnabled(enabled: boolean) {
  platformEnabled.value = enabled;
  const all = await tauriInvoke<Array<Record<string, unknown>>>('list_platform_settings');
  const cur = all.find(s => s.platform === props.platform);
  if (cur) await tauriInvoke('update_platform_settings', { platform: { ...cur, enabled } });
}

async function addAccount() {
  formError.value = '';
  const userId = form.userId.trim();
  const externalId = form.externalId.trim();
  const displayName = form.displayName.trim() || externalId;
  const authCode = form.authCode.trim();
  const oldestShareCode = form.oldestShareCode.trim();
  if (!userId) {
    formError.value = '请选择所属用户';
    return;
  }
  if (!externalId) {
    formError.value = 'External ID 不能为空';
    return;
  }
  if (props.platform === 'valve') {
    if (!authCode) {
      formError.value = 'Auth Code 不能为空';
      return;
    }
    if (!oldestShareCode) {
      formError.value = 'Oldest Share Code 不能为空';
      return;
    }
  }
  const metadata: Record<string, unknown> = {};
  if (props.platform === 'valve') {
    metadata.source = form.valveChannel === 'authcode' ? 'auth_code' : 'steam_login';
    if (form.valveChannel === 'authcode') {
      metadata.auth_code = authCode;
      metadata.oldest_share_code = oldestShareCode;
    }
  }
  formSaving.value = true;
  try {
    await tauriInvoke('add_sub_account', {
      userId,
      platform: props.platform,
      externalId,
      displayName,
      metadata,
      valveChannel: props.platform === 'valve' ? form.valveChannel : null,
    });
    showAdd.value = false;
    await load();
    emit('changed');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    formError.value = message || '添加账号失败';
    toast.add({ title: '添加账号失败', description: formError.value, color: 'error' });
  } finally {
    formSaving.value = false;
  }
}

function openEditModal(acc: SubAccountDto) {
  editingId.value = acc.id;
  editForm.displayName = acc.displayName;
  editForm.userId = acc.userId;
  editForm.authCode = String(acc.metadata.auth_code ?? '');
  editForm.oldestShareCode = String(acc.metadata.oldest_share_code ?? '');
  editError.value = '';
  void nextTick(() => { showEdit.value = true; });
}

async function saveEdit() {
  const displayName = editForm.displayName.trim();
  if (!displayName) { editError.value = '显示名不能为空'; return; }
  editError.value = '';
  editSaving.value = true;
  try {
    // Update user assignment if changed
    const acc = accounts.value.find((a: SubAccountDto) => a.id === editingId.value);
    if (acc && acc.userId !== editForm.userId) {
      await tauriInvoke('update_sub_account', { id: editingId.value, userId: editForm.userId });
    }
    // Build updated metadata
    let metadata: Record<string, unknown> = { ...(acc?.metadata ?? {}) };
    if (props.platform === 'valve') {
      metadata = {
        ...metadata,
        auth_code: editForm.authCode.trim() || metadata.auth_code,
        oldest_share_code: editForm.oldestShareCode.trim() || metadata.oldest_share_code,
      };
    }
    await tauriInvoke('update_sub_account_meta', { id: editingId.value, displayName, metadata });
    showEdit.value = false;
    toast.add({ title: '账号已更新', color: 'success' });
    await load();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    editError.value = message;
    toast.add({ title: '更新账号失败', description: message, color: 'error' });
  } finally {
    editSaving.value = false;
  }
}

async function removeAccount(id: string) {
  if (!window.confirm('确定要删除该账号吗？')) return;
  try {
    await tauriInvoke('delete_sub_account', { id });
    toast.add({ title: '账号已删除', color: 'success' });
    await load();
    emit('changed');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    toast.add({ title: '删除账号失败', description: message, color: 'error' });
  }
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
.platform-panel { display: flex; flex-direction: column; gap: var(--space-3); }
.platform-panel :deep(.section-header) { margin-bottom: 0; }

.header-actions { display: flex; align-items: center; gap: var(--space-4); }
.toggle-wrap { display: flex; align-items: center; gap: var(--space-2); cursor: pointer; }
.toggle-label { font-size: var(--text-sm); color: var(--text-2); }

.col-status { width: 60px; }
.user-sel { max-width: 160px; }

/* 弹窗 */
.modal-fields { display: flex; flex-direction: column; gap: var(--space-3); }
.modal-footer { display: flex; justify-content: flex-end; align-items: center; gap: var(--space-2); width: 100%; }
.footer-error { margin: 0 auto 0 0; color: var(--danger); font-size: var(--text-sm); }
.hint-5e { margin: calc(-1 * var(--space-2)) 0 0; }
</style>
