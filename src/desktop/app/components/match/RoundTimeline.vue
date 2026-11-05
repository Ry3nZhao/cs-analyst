<template>
  <div class="timeline">
    <SectionHeader title="回合时间线" :count="detail.rounds.length" description="点击回合查看击杀流水。经济：P 手枪局 · E ECO · F 强起 · B 全起" size="lg" />

    <div class="strip-wrap">
      <div class="strip">
        <div class="labels">
          <div class="cell-h">回合</div>
          <div class="lane lbl">{{ teamLabel('A', mineTeam === 'A') }}</div>
          <div class="reason-row lbl">原因</div>
          <div class="lane lbl">{{ teamLabel('B', mineTeam === 'B') }}</div>
          <div class="econ lbl">{{ mineTeam === 'A' ? '我方' : 'A' }} 经济</div>
          <div class="econ lbl">{{ mineTeam === 'B' ? '我方' : 'B' }} 经济</div>
          <div class="econ lbl">击杀</div>
        </div>
        <button
          v-for="(r, i) in detail.rounds"
          :key="r.number"
          class="round-col"
          :class="{ active: selected === r.number, half: i > 0 && detail.rounds[i - 1]!.sideA !== r.sideA }"
          :title="`第 ${r.number} 回合 · ${reasonLabel(r.reason)} · ${r.scoreA}:${r.scoreB}`"
          @click="selected = selected === r.number ? null : r.number"
        >
          <div class="cell-h num">{{ r.number }}</div>
          <div class="lane">
            <SideBadge v-if="r.winnerTeam === 'A'" :side="r.winnerSide" variant="solid" />
            <span v-else class="lose-dot" />
          </div>
          <div class="reason-row"><MatchReasonIcon :reason="r.reason" /></div>
          <div class="lane">
            <SideBadge v-if="r.winnerTeam === 'B'" :side="r.winnerSide" variant="solid" />
            <span v-else class="lose-dot" />
          </div>
          <div class="econ num" :class="buyClass(r.number, 'A')">{{ buyShort(r.number, 'A') }}</div>
          <div class="econ num" :class="buyClass(r.number, 'B')">{{ buyShort(r.number, 'B') }}</div>
          <div class="econ num kills">{{ killsOf(r.number, 'A') }}<span class="dim">:</span>{{ killsOf(r.number, 'B') }}</div>
        </button>
      </div>
    </div>

    <!-- 选中回合 -->
    <div v-if="round" class="round-detail">
      <div class="rd-head">
        <span class="rd-title">第 {{ round.number }} 回合</span>
        <span class="rd-score num">{{ round.scoreA }} : {{ round.scoreB }}</span>
        <span class="rd-win">
          <SideBadge :side="round.winnerSide" variant="solid" />
          {{ round.winnerTeam ? teamLabel(round.winnerTeam, mineTeam === round.winnerTeam) : '—' }} 获胜
        </span>
        <span class="rd-reason"><MatchReasonIcon :reason="round.reason" /> {{ reasonLabel(round.reason) }}</span>
        <span class="muted num">时长 {{ fmtClock(round.durationSecs) }}</span>
        <span v-if="round.bombPlanted" class="tag">已下包</span>
        <span v-if="round.mvp" class="muted">MVP <NuxtLink :to="`/players/${round.mvp}`" class="plink">{{ nameOf(round.mvp) }}</NuxtLink></span>
      </div>

      <div class="rd-econ">
        <div v-for="t in (['A', 'B'] as const)" :key="t" class="rd-econ-item">
          <SideBadge :side="sideOf(t, round.sideA)" />
          <span class="strong">{{ teamLabel(t, mineTeam === t) }}</span>
          <span class="muted">{{ econOf(round.number, t) ? BUY_LABEL[econOf(round.number, t)!.buyType] : '—' }}</span>
          <span class="num">装备 {{ fmtMoney(econOf(round.number, t)?.equipValue) }}</span>
          <span class="num muted">余额 {{ fmtMoney(econOf(round.number, t)?.balance) }}</span>
          <span class="num muted">存活 {{ econOf(round.number, t)?.survivors ?? '—' }}</span>
        </div>
      </div>

      <table class="data-table compact feed">
        <thead>
          <tr>
            <th class="num col-time">时间</th>
            <th>击杀者</th>
            <th>武器</th>
            <th>被击杀</th>
            <th>标记</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(k, i) in round.kills" :key="i" :class="{ after: k.afterRoundEnd }">
            <td class="num muted col-time">{{ fmtClock(k.timeSecs) }}</td>
            <td>
              <span v-if="k.attacker && k.kind !== 'world' && k.kind !== 'suicide'" class="who">
                <SideBadge :side="k.attackerSide" variant="bar" />
                <NuxtLink :to="`/players/${k.attacker}`" class="plink" :class="{ me: mine.has(k.attacker) }">{{ nameOf(k.attacker) }}</NuxtLink>
                <span v-if="k.assister" class="muted assist">+ {{ nameOf(k.assister) }}</span>
              </span>
              <span v-else class="dim">—</span>
            </td>
            <td class="weapon">{{ weaponLabel(k.weapon) }}<span v-if="k.headshot" class="hs" title="爆头">HS</span></td>
            <td>
              <span v-if="k.victim" class="who">
                <SideBadge :side="k.victimSide" variant="bar" />
                <NuxtLink :to="`/players/${k.victim}`" class="plink" :class="{ me: mine.has(k.victim) }">{{ nameOf(k.victim) }}</NuxtLink>
              </span>
            </td>
            <td class="marks">
              <span v-if="k.opening" class="tag">首杀</span>
              <span v-if="k.trade" class="tag">补枪</span>
              <span v-if="k.traded" class="tag">被补回</span>
              <span v-if="k.assistedFlash" class="tag">闪光助攻</span>
              <StatusChip v-if="k.kind === 'team_kill'" tone="danger" label="误杀队友" />
              <span v-if="k.kind === 'suicide' || k.kind === 'world'" class="tag">自杀 / 环境</span>
              <span v-if="k.afterRoundEnd" class="muted">回合结束后</span>
            </td>
          </tr>
          <tr v-if="round.kills.length === 0"><td colspan="5" class="empty">本回合没有击杀</td></tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { BuyType, MatchDetailDto, SteamId, TeamId, TeamRoundStatsDto } from '@cs-analyst/shared-types';
import { BUY_LABEL, BUY_SHORT, fmtClock, fmtMoney, reasonLabel, sideOf, teamLabel, useMySteamIds, weaponLabel } from '~/composables/useMatchDetail';

const props = defineProps<{ detail: MatchDetailDto; mineTeam: TeamId | null }>();
const mine = useMySteamIds();

const selected = ref<number | null>(null);
const round = computed(() => props.detail.rounds.find(r => r.number === selected.value) ?? null);

const econMap = computed(() => {
  const m = new Map<string, TeamRoundStatsDto>();
  for (const e of props.detail.economy) m.set(`${e.round}:${e.team}`, e);
  return m;
});
const names = computed(() => new Map(props.detail.scoreboard.map(p => [p.steamid, p.name || p.steamid])));

function econOf(round: number, team: TeamId) { return econMap.value.get(`${round}:${team}`); }
function buyShort(round: number, team: TeamId) { const b = econOf(round, team)?.buyType; return b ? BUY_SHORT[b] : '—'; }
function buyClass(round: number, team: TeamId) {
  const b: BuyType | undefined = econOf(round, team)?.buyType;
  return b === 'full' ? 'buy-full' : b === 'pistol' ? 'buy-pistol' : 'buy-low';
}
function killsOf(round: number, team: TeamId) { return econOf(round, team)?.kills ?? 0; }
function nameOf(id: SteamId | null) { return id ? (names.value.get(id) ?? id) : '—'; }
</script>

<style scoped>
.strip-wrap { overflow-x: auto; padding-bottom: var(--space-1); }
.strip { display: flex; align-items: stretch; width: max-content; }

.labels, .round-col { display: flex; flex-direction: column; }
.labels { position: sticky; left: 0; z-index: 1; width: 64px; background: var(--surface); }
.lbl { justify-content: flex-start !important; font-size: var(--text-xs); color: var(--text-3); white-space: nowrap; }

.cell-h, .lane, .reason-row, .econ { display: flex; align-items: center; justify-content: center; }
.cell-h { height: 20px; font-size: var(--text-xs); color: var(--text-3); }
.lane { height: 26px; }
.reason-row { height: 20px; color: var(--text-3); }
.econ { height: 18px; font-size: var(--text-xs); }

.round-col {
  width: 34px; padding: 2px 0;
  border: 1px solid transparent; border-radius: var(--radius-sm);
  background: transparent; color: inherit; cursor: pointer; font: inherit;
}
.round-col:hover { background: var(--surface-hover); }
.round-col:hover .reason-row { color: var(--text); }
.round-col.active { background: var(--surface-active); border-color: var(--accent-line); }
.round-col.half { margin-left: 10px; position: relative; }
.round-col.half::before {
  content: ''; position: absolute; left: -6px; top: 4px; bottom: 4px;
  border-left: 1px dashed var(--border-strong);
}
.lose-dot { width: 4px; height: 4px; border-radius: 50%; background: var(--border); }

.buy-full { color: var(--text); }
.buy-pistol { color: var(--text-2); }
.buy-low { color: var(--text-disabled); }
.kills { color: var(--text-3); letter-spacing: -0.02em; }

.round-detail { margin-top: var(--space-4); padding-top: var(--space-4); border-top: 1px solid var(--border-subtle); display: flex; flex-direction: column; gap: var(--space-3); }
.rd-head { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4); font-size: var(--text-sm); }
.rd-title { font-weight: 600; color: var(--text); font-size: var(--text-md); }
.rd-score { color: var(--text-2); }
.rd-win, .rd-reason { display: inline-flex; align-items: center; gap: var(--space-2); color: var(--text-2); }
.rd-econ { display: flex; flex-wrap: wrap; gap: var(--space-6); font-size: var(--text-sm); }
.rd-econ-item { display: flex; align-items: center; gap: var(--space-2); }
.strong { color: var(--text); font-weight: 500; }

.feed .col-time { width: 56px; }
.who { display: inline-flex; align-items: center; gap: var(--space-2); }
.plink { color: var(--text); text-decoration: none; }
.plink:hover { text-decoration: underline; text-underline-offset: 2px; }
.plink.me { color: var(--accent); }
.assist { font-size: var(--text-xs); }
.weapon { color: var(--text-2); white-space: nowrap; }
.hs { margin-left: 6px; font-size: 10px; font-weight: 600; color: var(--text-3); border: 1px solid var(--border); border-radius: 3px; padding: 0 3px; }
.marks { white-space: nowrap; }
.marks > * + * { margin-left: var(--space-1); }
tr.after td { opacity: 0.55; }
</style>
