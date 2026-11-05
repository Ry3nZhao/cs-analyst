<template>
  <div class="player-page">
    <NuxtLink to="/players" class="back muted">← 选手</NuxtLink>

    <p v-if="error" class="form-error">{{ error }}</p>
    <div v-else-if="notFound" class="callout callout-warning">本地库里没有 {{ param }} 对应的选手。</div>
    <div v-else-if="!detail" class="muted">加载中…</div>

    <template v-else>
      <!-- ── 头部 ── -->
      <section class="card card-pad head">
        <div class="head-top">
          <div class="head-name">
            <h1 class="name">{{ person.displayName }}</h1>
            <StatusChip v-if="isMe" tone="accent" label="我" />
            <span v-if="person.customName" class="muted small">（手动命名）</span>
          </div>
          <div class="head-actions">
            <label class="follow" :title="followHint">
              <button
                type="button"
                class="toggle"
                :class="{ on: person.followed }"
                role="switch"
                :aria-checked="person.followed"
                @click="toggleFollow"
              ><span class="toggle-knob" /></button>
              <span>关注</span>
            </label>
            <button type="button" class="btn btn-sm" @click="openEdit">编辑</button>
            <button type="button" class="btn btn-sm" @click="mergeOpen = true">合并到…</button>
            <button type="button" class="btn btn-sm" @click="manageOpen = true">身份与合并</button>
          </div>
        </div>

        <dl class="kv-grid head-kv">
          <dt>绑定身份</dt>
          <dd class="ids">
            <span v-for="i in detail.identities" :key="i.id" class="ident">
              <span class="tag">{{ platformLabel(i.platform) }}</span>
              <span class="mono">{{ i.externalId }}</span>
              <span v-if="i.platform !== 'steam' && i.steamid" class="dim mono">→ {{ i.steamid }}</span>
            </span>
          </dd>
          <dt>曾用名</dt>
          <dd>
            <span v-for="a in detail.aliases" :key="`${a.steamid}-${a.name}`" class="alias" :title="`${a.matches} 场，最近 ${fmtDate(a.lastSeenAt)}`">
              {{ a.name }}<span class="dim num"> ×{{ a.matches }}</span>
            </span>
            <span v-if="!detail.aliases.length" class="dim">—</span>
          </dd>
          <dt>标签</dt>
          <dd>
            <span v-for="t in person.tags" :key="t" class="tag tag-gap">{{ t }}</span>
            <span v-if="!person.tags.length" class="dim">—</span>
          </dd>
          <dt>备注</dt>
          <dd :class="{ dim: !person.note }">{{ person.note || '—' }}</dd>
          <dt>最近出现</dt>
          <dd>{{ fmtDate(person.lastSeenAt) }}<span class="dim">（{{ fmtAgo(person.lastSeenAt) }}）</span></dd>
        </dl>
      </section>

      <!-- ── 基础数据 ── -->
      <section>
        <SectionHeader
          title="基础数据"
          size="lg"
          :description="stats ? `本地库全部 ${stats.matches} 场、${stats.rounds} 回合${person.steamids.length > 1 ? `，合并 ${person.steamids.length} 个 SteamID` : ''}` : undefined"
        />
        <div v-if="statsError" class="form-error">{{ statsError }}</div>
        <div v-else-if="!stats" class="callout">还没有已解析的对局数据。</div>
        <div v-else class="stat-grid">
          <StatTile label="场次" :value="stats.matches" compact :hint="`${stats.wins} 胜 ${stats.losses} 负${stats.draws ? ` ${stats.draws} 平` : ''}`" />
          <StatTile label="胜率" :value="fmtPct(stats.winRate)" compact />
          <StatTile label="K/D" :value="fmtNum(stats.kd)" compact />
          <StatTile label="ADR" :value="fmtNum(stats.adr, 1)" compact />
          <StatTile label="KAST" :value="fmtNum(stats.kast, 1)" unit="%" compact />
          <StatTile label="HLTV 2.0" :value="fmtNum(stats.rating)" compact hint="近似值，仅供参考" />
          <StatTile label="爆头率" :value="fmtNum(stats.hsPct, 1)" unit="%" compact />
        </div>
      </section>

      <!-- ── 交手 ── -->
      <section>
        <SectionHeader title="交手" size="lg" :description="compareWith ? `${person.displayName} 视角，对比 ${compareWith.displayName}` : '选择一个人对比'">
          <button
            v-if="me && !isMe && compareWith?.id !== me.id"
            type="button"
            class="btn btn-sm"
            @click="compareWith = me"
          >对比「我」</button>
          <PersonPicker class="cmp-picker" small :exclude="[person.id]" placeholder="对比其他人…" @pick="p => (compareWith = p)" />
        </SectionHeader>
        <div v-if="meLoaded && !me" class="callout callout-info">
          还没有设置「我」：在自己的选手页打开「关注」，就会把你设为「我」，这里默认显示和你的交手。
        </div>
        <div v-else-if="isMe && !compareWith" class="callout">这是你自己的页面。在右上角选一个人，查看你和他的交手。</div>
        <HeadToHeadPanel v-if="compareWith" :a="person.id" :b="compareWith.id" :refresh-key="refreshKey" />
      </section>

      <!-- ── 组排 ── -->
      <section>
        <SectionHeader
          title="常组排队友"
          size="lg"
          :count="teammates?.teammates.length ?? null"
          :description="teammates ? `总战绩 ${fmtRecord(teammates.overall)}，胜率 ${fmtPct(teammates.overall.winRate)}` : undefined"
        >
          <select v-model.number="minTogether" class="select input-sm" aria-label="最少同队场次">
            <option :value="1">同队 ≥ 1 场</option>
            <option :value="2">同队 ≥ 2 场</option>
            <option :value="3">同队 ≥ 3 场</option>
            <option :value="5">同队 ≥ 5 场</option>
          </select>
        </SectionHeader>
        <div class="table-wrap">
          <table class="data-table compact">
            <thead>
              <tr>
                <th>队友</th>
                <th class="num">同队场次</th>
                <th class="num">战绩</th>
                <th class="num">组排胜率</th>
                <th class="num">相对总胜率</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="t in teammates?.teammates ?? []" :key="t.person.id" class="clickable" @click="navigateTo(playerPath(t.person))">
                <td class="strong">{{ t.person.displayName }}</td>
                <td class="num">{{ t.record.matches }}</td>
                <td class="num muted">{{ fmtRecord(t.record) }}</td>
                <td class="num">{{ fmtPct(t.record.winRate) }}</td>
                <td class="num" :class="deltaTone(t.record.winRate, t.record.matches)">{{ fmtDelta(t.record.winRate) }}</td>
              </tr>
              <tr v-if="!teammates?.teammates.length">
                <td colspan="5" class="empty">没有同队 ≥ {{ minTogether }} 场的队友</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <!-- ── 比赛 ── -->
      <section>
        <SectionHeader title="比赛" size="lg" :count="matchRows.length" description="这个人在本地库里的全部已解析对局，点击查看比赛详情" />
        <div class="table-wrap">
          <table class="data-table compact">
            <thead>
              <tr>
                <th>时间</th>
                <th>地图</th>
                <th>平台</th>
                <th>用名</th>
                <th>比分</th>
                <th>结果</th>
                <th class="num">K</th>
                <th class="num">D</th>
                <th class="num">A</th>
                <th class="num">ADR</th>
                <th class="num">Rating</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="m in shownMatches" :key="`${m.matchId}-${m.stats.steamid}`" class="clickable" @click="navigateTo(`/matches/${m.matchId}`)">
                <td class="muted">{{ fmtDate(m.playedAt) }}</td>
                <td class="strong">{{ mapLabel(m.mapName) }}</td>
                <td><span class="tag">{{ platformLabel(m.platform) }}</span></td>
                <td class="muted truncate">{{ m.stats.name }}</td>
                <td class="num">{{ m.scoreOwn }} : {{ m.scoreOpponent }}</td>
                <td><ResultMark :result="m.result" /></td>
                <td class="num">{{ m.stats.kills }}</td>
                <td class="num">{{ m.stats.deaths }}</td>
                <td class="num">{{ m.stats.assists }}</td>
                <td class="num">{{ fmtNum(m.stats.derived.adr, 1) }}</td>
                <td class="num">{{ fmtNum(m.stats.derived.rating) }}</td>
              </tr>
              <tr v-if="!matchRows.length">
                <td colspan="11" class="empty">没有已解析的对局</td>
              </tr>
            </tbody>
          </table>
        </div>
        <button v-if="matchRows.length > 20 && !showAll" type="button" class="btn btn-sm more" @click="showAll = true">
          显示全部 {{ matchRows.length }} 场
        </button>
      </section>
    </template>

    <!-- ── 编辑 ── -->
    <AppModal v-model:open="editOpen" title="编辑选手">
      <div class="fields">
        <p v-if="editError" class="form-error">{{ editError }}</p>
        <div class="field">
          <label class="field-label">显示名</label>
          <input v-model="editForm.displayName" class="input" :placeholder="detail?.aliases[0]?.name ?? '留空则用最近一场的游戏名'" />
          <span class="field-hint">留空则自动使用最近一场的游戏名</span>
        </div>
        <div class="field">
          <label class="field-label">标签</label>
          <input v-model="editForm.tags" class="input" placeholder="用逗号分隔，例如：队友, 狙击手" />
        </div>
        <div class="field">
          <label class="field-label">备注</label>
          <textarea v-model="editForm.note" class="input textarea" rows="3" />
        </div>
      </div>
      <template #footer="{ close }">
        <button type="button" class="btn" @click="close">取消</button>
        <button type="button" class="btn-primary" :disabled="busy" @click="saveEdit">保存</button>
      </template>
    </AppModal>

    <!-- ── 合并到… ── -->
    <AppModal v-model:open="mergeOpen" title="合并到另一个人">
      <div v-if="detail" class="fields">
        <p class="muted small">把「{{ person.displayName }}」并入另一个人：适用于小号、换号。之后可以在目标的「身份与合并」里撤销。</p>
        <PersonPicker :exclude="[person.id]" placeholder="搜索合并目标" @pick="p => (mergeTarget = p)" />
        <template v-if="mergeTarget">
          <div class="merge-pair">
            <div class="merge-side">
              <span class="field-label">被合并</span>
              <span class="strong">{{ person.displayName }}</span>
              <span class="muted mono small">{{ person.steamids.join(', ') || '无 SteamID' }}</span>
            </div>
            <span class="arrow muted">→</span>
            <div class="merge-side">
              <span class="field-label">目标</span>
              <span class="strong">{{ mergeTarget.displayName }}</span>
              <span class="muted mono small">{{ mergeTarget.steamids.join(', ') || '无 SteamID' }}</span>
            </div>
          </div>
          <div class="callout callout-warning">
            <ul class="consequences">
              <li>「{{ person.displayName }}」的 {{ detail.identities.length }} 个身份全部移到「{{ mergeTarget.displayName }}」名下，两人从此算作同一个人。</li>
              <li>基础数据、交手、组排都按合并后的人重新汇总；两人之间原有的同场对局会变成「自己和自己」，不再计入交手。</li>
              <li>标签取并集，备注拼接，显示名优先保留目标的；只要一方被关注，合并后就是关注状态。</li>
              <li>「{{ person.displayName }}」不再单独出现在搜索里。</li>
            </ul>
          </div>
        </template>
      </div>
      <template #footer="{ close }">
        <button type="button" class="btn" @click="close">取消</button>
        <button type="button" class="btn-primary" :disabled="!mergeTarget || busy" @click="doMerge">确认合并</button>
      </template>
    </AppModal>

    <!-- ── 身份与合并（拆分 / 撤销） ── -->
    <AppModal v-model:open="manageOpen" title="身份与合并">
      <div v-if="detail" class="fields manage">
        <SectionHeader title="绑定的身份" :count="detail.identities.length" description="拆分：把身份拿出来，单独成为一个新的人" />
        <div class="table-wrap">
          <table class="data-table compact">
            <thead>
              <tr><th>平台</th><th>ID</th><th class="num">场次</th><th class="actions"></th></tr>
            </thead>
            <tbody>
              <tr v-for="i in detail.identities" :key="i.id">
                <td><span class="tag">{{ platformLabel(i.platform) }}</span></td>
                <td>
                  <div class="mono small">{{ i.externalId }}</div>
                  <div v-if="i.lastName" class="muted small">{{ i.lastName }}<span v-if="i.manual" class="dim">（手动）</span></div>
                </td>
                <td class="num">{{ i.matchCount }}</td>
                <td class="actions">
                  <button
                    type="button"
                    class="btn btn-sm"
                    :disabled="detail.identities.length < 2 || busy"
                    :title="detail.identities.length < 2 ? '只有这一个身份，无需拆分' : '拆成一个新的人'"
                    @click="doSplit(i.id)"
                  >拆分</button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <SectionHeader title="合并记录" :count="detail.merges.length" description="并进这个人、还能撤销的合并（新的在前）" />
        <div class="table-wrap">
          <table class="data-table compact">
            <thead>
              <tr><th>时间</th><th>来源</th><th class="num">身份数</th><th class="actions"></th></tr>
            </thead>
            <tbody>
              <tr v-for="m in detail.merges" :key="m.id">
                <td class="muted">{{ fmtDate(m.createdAt) }}</td>
                <td>
                  <span class="small">{{ mergeSourceLabel(m.identityIds) }}</span>
                  <span v-if="m.auto" class="tag tag-gap">自动</span>
                </td>
                <td class="num">{{ m.identityIds.length }}</td>
                <td class="actions">
                  <button
                    type="button"
                    class="btn btn-sm"
                    :disabled="busy || mergeIsStale(m.identityIds)"
                    :title="mergeIsStale(m.identityIds) ? '这次合并带来的身份都已经拆走了，撤销只会恢复出一个空的人' : '把这次合并带来的身份还给原来的人'"
                    @click="doUndo(m.id)"
                  >撤销合并</button>
                </td>
              </tr>
              <tr v-if="!detail.merges.length">
                <td colspan="4" class="empty">没有可撤销的合并</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="muted small">拆分或撤销后，这些身份会标记为「手动」，自动关联不会再把它们并回来。</p>
      </div>
    </AppModal>
  </div>
</template>

<script setup lang="ts">
import type { PersonDetailDto, PersonSummaryDto, PlayerMatchRowDto, TeammatesDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';
import {
  type CombinedStats,
  errorMessage,
  fmtAgo,
  fmtDate,
  fmtNum,
  fmtPct,
  fmtRecord,
  loadPlayerStats,
  mapLabel,
  platformLabel,
  playerPath,
  resolvePersonParam,
  useMe,
} from '~/composables/usePlayers';

definePageMeta({ layout: 'default' });

const minTogether = ref(2);
const route = useRoute();
const router = useRouter();
const toast = useToast();
const { me, loaded: meLoaded, refreshMe } = useMe();

const param = computed(() => String(route.params.steamid ?? ''));
const detail = ref<PersonDetailDto | null>(null);
const notFound = ref(false);
const error = ref('');
const person = computed(() => detail.value!.person);
const isMe = computed(() => !!me.value && !!detail.value && me.value.id === detail.value.person.id);
const followHint = computed(() => (me.value ? '关注的人排在选手列表最上面' : '关注后，第一个关注的人会被当作「我」'));

const stats = ref<CombinedStats | null>(null);
const statsError = ref('');
const matchRows = ref<PlayerMatchRowDto[]>([]);
const showAll = ref(false);
const shownMatches = computed(() => (showAll.value ? matchRows.value : matchRows.value.slice(0, 20)));
const teammates = ref<TeammatesDto | null>(null);
const compareWith = ref<PersonSummaryDto | null>(null);
const refreshKey = ref(0);
const busy = ref(false);

async function loadAll(d: PersonDetailDto) {
  detail.value = d;
  refreshKey.value++;
  const id = d.person.id;
  statsError.value = '';
  const [s, t] = await Promise.allSettled([
    loadPlayerStats(d.person.steamids),
    tauriInvoke<TeammatesDto>('list_teammates', { person: id, minMatches: minTogether.value, limit: 30 }),
  ]);
  if (detail.value?.person.id !== id) return;
  if (s.status === 'fulfilled') {
    stats.value = s.value.summary;
    matchRows.value = s.value.matches;
  } else {
    stats.value = null;
    matchRows.value = [];
    statsError.value = errorMessage(s.reason);
  }
  teammates.value = t.status === 'fulfilled' ? t.value : null;
}

async function loadTeammates() {
  const id = detail.value?.person.id;
  if (id == null) return;
  try {
    teammates.value = await tauriInvoke<TeammatesDto>('list_teammates', { person: id, minMatches: minTogether.value, limit: 30 });
  } catch (e) {
    fail('加载队友失败', e);
  }
}
watch(minTogether, loadTeammates);

async function loadFromRoute() {
  error.value = '';
  notFound.value = false;
  try {
    const d = await resolvePersonParam(param.value);
    if (!d) {
      notFound.value = true;
      detail.value = null;
      return;
    }
    showAll.value = false;
    await loadAll(d);
  } catch (e) {
    error.value = errorMessage(e);
  }
}

/** 合并 / 拆分 / 撤销后：按 person id 重新取，URL 不再属于这个人时换成它现在的地址 */
async function reloadPerson(id: number) {
  const d = await tauriInvoke<PersonDetailDto>('get_person', { id });
  const path = playerPath(d.person);
  if (!d.person.steamids.includes(param.value) && route.path !== path) {
    await router.replace(path);
    return; // 路由变化会触发 loadFromRoute
  }
  await loadAll(d);
}

// 默认对比「我」；看的是自己时等用户选人
watch([() => me.value?.id, () => detail.value?.person.id], ([mid, pid], [, prevPid]) => {
  if (pid == null) return;
  const meOk = mid != null && mid !== pid;
  if (pid !== prevPid) compareWith.value = meOk ? me.value : null;
  else if (compareWith.value?.id === pid) compareWith.value = null;
  else if (!compareWith.value && meOk) compareWith.value = me.value;
});

watch(param, loadFromRoute);
onMounted(() => {
  loadFromRoute();
  refreshMe();
});

function fail(title: string, e: unknown) {
  toast.add({ title, description: errorMessage(e), color: 'error' });
}

// ── 关注 / 编辑 ──
async function toggleFollow() {
  if (!detail.value) return;
  try {
    const d = await tauriInvoke<PersonDetailDto>('update_person', { id: person.value.id, patch: { followed: !person.value.followed } });
    detail.value = d;
    await refreshMe();
  } catch (e) {
    fail('操作失败', e);
  }
}

const editOpen = ref(false);
const editError = ref('');
const editForm = reactive({ displayName: '', tags: '', note: '' });
function openEdit() {
  editForm.displayName = person.value.customName ?? '';
  editForm.tags = person.value.tags.join(', ');
  editForm.note = person.value.note ?? '';
  editError.value = '';
  editOpen.value = true;
}
async function saveEdit() {
  busy.value = true;
  try {
    const tags = editForm.tags.split(/[,，]/).map(t => t.trim()).filter(Boolean);
    detail.value = await tauriInvoke<PersonDetailDto>('update_person', {
      id: person.value.id,
      patch: { displayName: editForm.displayName.trim(), note: editForm.note.trim(), tags },
    });
    editOpen.value = false;
    await refreshMe();
  } catch (e) {
    editError.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}

// ── 合并 / 拆分 / 撤销 ──
const mergeOpen = ref(false);
const mergeTarget = ref<PersonSummaryDto | null>(null);
watch(mergeOpen, (o) => { if (!o) mergeTarget.value = null; });

async function doMerge() {
  if (!mergeTarget.value || !detail.value) return;
  busy.value = true;
  const sourceName = person.value.displayName;
  try {
    const d = await tauriInvoke<PersonDetailDto>('merge_persons', { target: mergeTarget.value.id, source: person.value.id });
    mergeOpen.value = false;
    toast.add({ title: '已合并', description: `「${sourceName}」已并入「${d.person.displayName}」，可在「身份与合并」里撤销`, color: 'success' });
    await refreshMe();
    await reloadPerson(d.person.id);
  } catch (e) {
    fail('合并失败', e);
  } finally {
    busy.value = false;
  }
}

const manageOpen = ref(false);

async function doSplit(identityId: number) {
  if (!detail.value) return;
  busy.value = true;
  const id = person.value.id;
  try {
    const d = await tauriInvoke<PersonDetailDto>('split_identity', { identityId });
    toast.add({ title: '已拆分', description: `身份已拆成新的人「${d.person.displayName}」`, color: 'success' });
    await refreshMe();
    await reloadPerson(id);
  } catch (e) {
    fail('拆分失败', e);
  } finally {
    busy.value = false;
  }
}

async function doUndo(mergeId: number) {
  if (!detail.value) return;
  busy.value = true;
  const id = person.value.id;
  try {
    const d = await tauriInvoke<PersonDetailDto>('undo_merge', { mergeId });
    toast.add({ title: '已撤销合并', description: `已恢复「${d.person.displayName}」`, color: 'success' });
    await refreshMe();
    await reloadPerson(id);
  } catch (e) {
    fail('撤销失败', e);
  } finally {
    busy.value = false;
  }
}

function mergeIsStale(ids: number[]) {
  return !detail.value?.identities.some(i => ids.includes(i.id));
}

/** 合并记录只有身份 id：用当前身份表还原出来源的名字 / ID */
function mergeSourceLabel(ids: number[]) {
  const list = detail.value?.identities.filter(i => ids.includes(i.id)) ?? [];
  if (!list.length) return '（身份已被拆走）';
  return list.map(i => i.lastName ?? i.externalId).join('、');
}

// ── 组排胜率相对总胜率 ──
function delta(rate: number | null) {
  const o = teammates.value?.overall.winRate;
  return rate == null || o == null ? null : rate - o;
}
function fmtDelta(rate: number | null) {
  const d = delta(rate);
  if (d == null) return '—';
  const p = Math.round(d * 100);
  return `${p > 0 ? '+' : ''}${p}%`;
}
/** 样本太少（< 3 场）不上色，避免把偶然当成规律 */
function deltaTone(rate: number | null, matches: number) {
  const d = delta(rate);
  if (d == null || matches < 3 || Math.abs(d) < 0.05) return '';
  return d > 0 ? 'good' : 'bad';
}
</script>

<style scoped>
.player-page { display: flex; flex-direction: column; gap: var(--space-6); max-width: 1100px; }
.back { font-size: var(--text-sm); text-decoration: none; margin-bottom: calc(-1 * var(--space-3)); }
.back:hover { color: var(--text); }
.small { font-size: var(--text-xs); }

.head { display: flex; flex-direction: column; gap: var(--space-4); }
.head-top { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
.head-name { display: flex; align-items: center; gap: var(--space-3); min-width: 0; }
.name { margin: 0; font-size: var(--text-2xl); font-weight: 600; color: var(--text); }
.head-actions { display: flex; align-items: center; gap: var(--space-2); flex-shrink: 0; }
.follow { display: flex; align-items: center; gap: var(--space-2); margin-right: var(--space-2); font-size: var(--text-sm); color: var(--text-2); cursor: pointer; }
.head-kv { grid-template-columns: 80px 1fr; }
.ids { display: flex; flex-wrap: wrap; gap: var(--space-2) var(--space-4); white-space: normal; }
.ident { display: inline-flex; align-items: center; gap: 6px; }
.alias { display: inline-block; margin-right: var(--space-3); color: var(--text-2); }
.tag-gap { margin-right: 4px; }

.stat-grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: var(--space-3); }
.cmp-picker { width: 240px; }
.more { margin-top: var(--space-2); }

.fields { display: flex; flex-direction: column; gap: var(--space-3); }
.textarea { height: auto; padding-top: 6px; padding-bottom: 6px; resize: vertical; }
.merge-pair { display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; gap: var(--space-3); }
.merge-side { display: flex; flex-direction: column; gap: 2px; min-width: 0; padding: var(--space-2) var(--space-3); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); }
.arrow { font-size: var(--text-lg); }
.consequences { margin: 0; padding-left: 16px; display: flex; flex-direction: column; gap: 4px; font-size: var(--text-xs); color: var(--text-2); }
.data-table td.good { color: var(--value-good); }
.data-table td.bad { color: var(--value-bad); }
.manage :deep(.section-header) { margin-bottom: 0; margin-top: var(--space-2); }
</style>
