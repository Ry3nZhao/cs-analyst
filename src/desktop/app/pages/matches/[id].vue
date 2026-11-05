<template>
  <div class="match-detail">
    <div class="crumbs">
      <NuxtLink to="/matches" class="back-link">← 对局</NuxtLink>
    </div>

    <div v-if="loading" class="state-msg muted">加载中…</div>
    <div v-else-if="error" class="callout callout-danger">{{ error }}</div>
    <div v-else-if="!detail" class="callout callout-warning">
      这场对局还没有解析结果。回到 <NuxtLink to="/matches">对局列表</NuxtLink> 解析后再查看。
    </div>

    <template v-else>
      <!-- 头部 -->
      <header class="card match-head">
        <div class="head-meta">
          <div class="map-name">{{ mapLabel(detail.mapName) }}</div>
          <div class="meta-line">
            <span class="tag">{{ platformLabel(detail.platform) }}</span>
            <span v-if="job?.gameMode" class="tag">{{ gameModeLabel(job.gameMode) }}</span>
            <span v-if="detail.playedAt ?? job?.playedAt" class="muted num">{{ fmtDateTime(detail.playedAt ?? job?.playedAt) }}</span>
            <span class="muted num">{{ detail.roundsPlayed }} 回合</span>
            <span v-if="job?.durationSeconds" class="muted num">{{ fmtClock(job.durationSeconds) }}</span>
          </div>
          <div v-if="job?.source" class="source muted truncate" :title="job.source">{{ job.source }}</div>
        </div>

        <div class="scoreline">
          <div v-for="(t, i) in teams" :key="t.team" class="score-team" :class="[`pos-${i}`, { won: t.won === true }]">
            <div class="team-name">
              {{ teamLabel(t.team, mineTeam === t.team) }}
              <SideBadge :side="t.startSide" />
            </div>
            <div class="team-score num">{{ t.score }}</div>
            <div class="team-result">
              <StatusChip v-if="t.won === true" tone="accent" label="胜" />
              <span v-else-if="t.won === false" class="muted">负</span>
              <span v-else class="muted">平</span>
            </div>
          </div>
          <div class="score-sep">:</div>
        </div>

        <div class="head-actions">
          <NuxtLink :to="`/highlights?match=${encodeURIComponent(matchId)}`" class="btn">本场集锦</NuxtLink>
          <NuxtLink :to="`/replay/${matchId}`" class="btn">2D 回放</NuxtLink>
        </div>
      </header>

      <MainPlayerNotice text="设置后会在计分板和时间线上标出你和你的队伍。" />

      <!-- 计分板 -->
      <section>
        <MatchScoreboard :detail="detail" :mine="mySteamIds" :mine-team="mineTeam" />
      </section>

      <!-- 回合时间线 -->
      <section class="card card-pad">
        <MatchRoundTimeline :detail="detail" :mine-team="mineTeam" />
      </section>

      <!-- 回合胜率 / Swing -->
      <section class="card card-pad">
        <MatchWinProbability :detail="detail" :mine-team="mineTeam" />
      </section>

      <!-- 经济曲线 -->
      <section class="card card-pad">
        <MatchEconomyChart :detail="detail" :mine-team="mineTeam" />
      </section>

      <!-- 矩阵 -->
      <section class="card card-pad">
        <MatchMatrices :detail="detail" :mine="mySteamIds" :mine-team="mineTeam" />
      </section>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { MatchDetailDto, MatchDto, TeamId } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';
import { fmtClock, fmtDateTime, mapLabel, platformLabel, teamLabel, useMySteamIds } from '~/composables/useMatchDetail';

definePageMeta({ layout: 'default' });

const route = useRoute();
const matchId = computed(() => String(route.params.id));
const mySteamIds = useMySteamIds();

const detail = ref<MatchDetailDto | null>(null);
const job = ref<MatchDto | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);

function gameModeLabel(mode: string) {
  return ({ casual: '休闲', competitive: '竞技', wingman: '双人', premier: 'Premier', deathmatch: '死亡竞赛' } as Record<string, string>)[mode] ?? mode;
}

const teams = computed(() => detail.value?.teams ?? []);
const mineTeam = computed<TeamId | null>(() => {
  const d = detail.value;
  if (!d) return null;
  const me = d.scoreboard.find(p => mySteamIds.value.has(p.steamid));
  return me?.team ?? null;
});

async function load() {
  loading.value = true;
  error.value = null;
  try {
    const [d, jobs] = await Promise.all([
      tauriInvoke<MatchDetailDto | null>('get_match_detail', { matchId: matchId.value }),
      tauriInvoke<MatchDto[]>('list_matches').catch(() => [] as MatchDto[]),
    ]);
    detail.value = d;
    job.value = jobs.find(j => j.id === matchId.value) ?? null;
  } catch (e) {
    error.value = `加载失败：${e instanceof Error ? e.message : String(e)}`;
  } finally {
    loading.value = false;
  }
}

watch(matchId, load, { immediate: true });
</script>

<style scoped>
.match-detail { display: flex; flex-direction: column; gap: var(--space-4); padding-bottom: var(--space-8); }
.crumbs { font-size: var(--text-sm); }
.back-link { color: var(--text-3); text-decoration: none; }
.back-link:hover { color: var(--text); }
.state-msg { padding: var(--space-8); text-align: center; }

.match-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  gap: var(--space-6);
  padding: var(--space-4) var(--space-5);
}
.head-meta { display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; }
.map-name { font-size: var(--text-xl); font-weight: 600; color: var(--text); }
.meta-line { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); font-size: var(--text-sm); }
.source { font-size: var(--text-xs); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.scoreline { position: relative; display: flex; align-items: center; gap: var(--space-8); }
.score-team { display: flex; flex-direction: column; align-items: center; gap: var(--space-1); min-width: 96px; }
.pos-0 { order: 0; }
.score-sep { order: 1; font-size: var(--text-2xl); color: var(--text-disabled); }
.pos-1 { order: 2; }
.team-name { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-sm); color: var(--text-2); }
.team-score { font-size: 40px; font-weight: 600; line-height: 1; color: var(--text-3); }
.score-team.won .team-score { color: var(--accent); }
.score-team.won .team-name { color: var(--text); }
.team-result { height: 20px; font-size: var(--text-xs); display: flex; align-items: center; }

.head-actions { display: flex; justify-content: flex-end; }
.head-actions .btn { text-decoration: none; }
</style>
