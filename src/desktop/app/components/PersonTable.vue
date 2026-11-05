<template>
  <div class="table-wrap">
    <table class="data-table">
      <thead>
        <tr>
          <th>选手</th>
          <th>SteamID</th>
          <th>标签</th>
          <th class="num">场次</th>
          <th>最近出现</th>
          <th class="actions"></th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="p in rows" :key="p.id" class="clickable" @click="navigateTo(playerPath(p))">
          <td class="strong">
            <span class="name">{{ p.displayName }}</span>
            <StatusChip v-if="p.id === meId" tone="accent" label="我" class="me-chip" />
            <span v-if="p.note" class="muted note truncate">{{ p.note }}</span>
          </td>
          <td class="muted mono sid">
            {{ p.steamids[0] ?? '—' }}<span v-if="p.steamids.length > 1" class="dim"> +{{ p.steamids.length - 1 }}</span>
          </td>
          <td>
            <span v-for="t in p.tags" :key="t" class="tag">{{ t }}</span>
            <span v-if="!p.tags.length" class="dim">—</span>
          </td>
          <td class="num">{{ p.matchCount }}</td>
          <td class="muted" :title="fmtDate(p.lastSeenAt)">{{ fmtAgo(p.lastSeenAt) }}</td>
          <td class="actions">
            <button type="button" class="btn btn-sm" @click.stop="emit('toggle-follow', p)">
              {{ p.followed ? '取消关注' : '关注' }}
            </button>
          </td>
        </tr>
        <tr v-if="rows.length === 0">
          <td colspan="6" class="empty">{{ loading ? '加载中…' : empty }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import type { PersonSummaryDto } from '@cs-analyst/shared-types';
import { fmtAgo, fmtDate, playerPath } from '~/composables/usePlayers';

defineProps<{ rows: PersonSummaryDto[]; meId: number | null; loading?: boolean; empty: string }>();
const emit = defineEmits<{ 'toggle-follow': [p: PersonSummaryDto] }>();
</script>

<style scoped>
.name { margin-right: var(--space-2); }
.me-chip { margin-right: var(--space-2); vertical-align: middle; }
.note { font-weight: 400; font-size: var(--text-xs); max-width: 220px; display: inline-block; vertical-align: middle; }
.sid { font-size: var(--text-xs); }
.tag + .tag { margin-left: 4px; }
</style>
