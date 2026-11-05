import { listen } from '@tauri-apps/api/event';
import type { MatchStorageInfoDto } from '@cs-analyst/shared-types';
import { listenJobsUpdated, tauriInvoke } from '~/composables/useTauri';

/**
 * 对局的收藏标记与原始 demo 存储状态（W3-D）。
 * `list_matches` 不变，这里按 id 单独拉取后合并；挂载时自动加载，下载 / 恢复后自动刷新。
 */
export function useMatchStorage() {
  const byId = ref<Record<string, MatchStorageInfoDto>>({});
  const toast = useToast();

  async function loadStorage() {
    try {
      const rows = await tauriInvoke<MatchStorageInfoDto[]>('list_match_storage');
      byId.value = Object.fromEntries(rows.map(r => [r.id, r]));
    } catch {
      byId.value = {};
    }
  }

  function isFavorite(id: string) {
    return byId.value[id]?.favorite ?? false;
  }

  async function toggleFavorite(id: string) {
    const next = !isFavorite(id);
    try {
      await tauriInvoke('set_match_favorite', { ids: [id], favorite: next });
      const cur = byId.value[id];
      if (cur) byId.value = { ...byId.value, [id]: { ...cur, favorite: next } };
      else await loadStorage();
    } catch (error) {
      toast.add({ title: '操作失败', description: error instanceof Error ? error.message : String(error), color: 'error' });
    }
  }

  onMounted(() => {
    loadStorage();
    const u1 = listenJobsUpdated(loadStorage);
    let u2: (() => void) | null = null;
    listen('storage_restored', loadStorage).then(fn => { u2 = fn; });
    onUnmounted(() => { u1(); u2?.(); });
  });

  return { storageById: byId, isFavorite, toggleFavorite, loadStorage };
}
