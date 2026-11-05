<template>
  <!-- 没设置主玩家时的一行提示：引导到设置页 / 引导页；有候选人时可以一键采用。 -->
  <div v-if="loaded && !isSet" class="callout callout-info main-player-notice">
    <span>还没有设置主玩家（「我」）。{{ text }}</span>
    <template v-if="suggestion">
      <span class="mpn-sug">
        可能是：<b>{{ suggestion.displayName ?? suggestion.steamid }}</b>
        <span class="muted">（{{ mainPlayerSourceLabel(suggestion.source) }}）</span>
      </span>
      <button type="button" class="btn btn-sm" :disabled="busy" @click="adopt">设为主玩家</button>
    </template>
    <span class="mpn-links">
      到 <NuxtLink to="/settings">设置</NuxtLink> 或 <NuxtLink to="/welcome">引导页</NuxtLink> 选择
    </span>
  </div>
</template>

<script setup lang="ts">
/**
 * 主玩家未设置提示。主玩家设置后自动隐藏。
 *
 * ```vue
 * <MainPlayerNotice text="设置后这里默认显示你的枪法数据。" />
 * ```
 */
import { mainPlayerSourceLabel } from '~/composables/useMainPlayer';

defineProps<{
  /** 说明这一页设置主玩家后会怎样 */
  text?: string;
}>();

const { isSet, loaded, suggestion, set } = useMainPlayer();
const toast = useToast();
const busy = ref(false);

async function adopt() {
  const s = suggestion.value;
  if (!s) return;
  busy.value = true;
  try {
    await set(s.steamid);
  } catch (e) {
    toast.add({ title: '设置主玩家失败', description: e instanceof Error ? e.message : String(e), color: 'error' });
  } finally {
    busy.value = false;
  }
}
</script>

<style scoped>
.main-player-notice {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1) var(--space-3);
}
.mpn-sug b { color: var(--text); font-weight: 600; }
</style>
