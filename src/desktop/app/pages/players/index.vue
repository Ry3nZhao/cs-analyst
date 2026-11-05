<template>
  <div class="players-page">
    <div class="toolbar">
      <input
        v-model="query"
        class="input search"
        placeholder="搜索名字、曾用名、SteamID、备注或标签"
        autofocus
      />
      <span class="muted hint">范围：整个本地库</span>
    </div>

    <p v-if="error" class="form-error">{{ error }}</p>

    <MainPlayerNotice text="设置后，选手页的交手数据默认以你为视角显示。" />

    <section class="block">
      <SectionHeader
        title="我关注的人"
        :description="followed.length ? '个人收藏，排在最上面；「我」由主玩家决定' : undefined"
        :count="followed.length"
        size="lg"
      />
      <PersonTable :rows="followed" :me-id="me?.id ?? null" :loading="loading" empty="没有匹配的关注对象" @toggle-follow="toggleFollow" />
    </section>

    <section class="block">
      <SectionHeader
        :title="query.trim() ? '搜索结果' : '全部选手'"
        description="按本地库里出现的场次排序"
        :count="others.length"
        size="lg"
      />
      <PersonTable :rows="others" :me-id="me?.id ?? null" :loading="loading" :empty="query.trim() ? '没有找到匹配的选手' : '库里还没有选手，先解析几场对局'" @toggle-follow="toggleFollow" />
      <p v-if="others.length >= LIMIT" class="muted more">只显示前 {{ LIMIT }} 个，输入关键词缩小范围</p>
    </section>
  </div>
</template>

<script setup lang="ts">
import type { PersonDetailDto, PersonSummaryDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';
import { errorMessage, searchPersons, useMe } from '~/composables/usePlayers';

definePageMeta({ layout: 'default' });

const LIMIT = 100;
const toast = useToast();
const { me, refreshMe } = useMe();

const query = ref('');
const followed = ref<PersonSummaryDto[]>([]);
const others = ref<PersonSummaryDto[]>([]);
const loading = ref(false);
const error = ref('');

let seq = 0;
async function load() {
  const my = ++seq;
  loading.value = true;
  error.value = '';
  try {
    const [f, all] = await Promise.all([
      searchPersons(query.value, { followedOnly: true, limit: 50 }),
      searchPersons(query.value, { limit: LIMIT + 50 }),
    ]);
    if (my !== seq) return;
    followed.value = f;
    others.value = all.filter(p => !p.followed).slice(0, LIMIT);
  } catch (e) {
    if (my === seq) error.value = errorMessage(e);
  } finally {
    if (my === seq) loading.value = false;
  }
}

let timer: ReturnType<typeof setTimeout> | undefined;
watch(query, () => {
  clearTimeout(timer);
  timer = setTimeout(load, 200);
});

async function toggleFollow(p: PersonSummaryDto) {
  try {
    await tauriInvoke<PersonDetailDto>('update_person', { id: p.id, patch: { followed: !p.followed } });
    await load(); // 关注只是收藏，不影响「我」
  } catch (e) {
    toast.add({ title: '操作失败', description: errorMessage(e), color: 'error' });
  }
}

onMounted(() => {
  load();
  refreshMe();
});
</script>

<style scoped>
.players-page { display: flex; flex-direction: column; gap: var(--space-5); max-width: 1100px; }
.toolbar { display: flex; align-items: center; gap: var(--space-3); }
.search { width: 360px; }
.hint { font-size: var(--text-xs); }
.block { display: flex; flex-direction: column; }
.more { margin: var(--space-2) 0 0; font-size: var(--text-xs); }
</style>
