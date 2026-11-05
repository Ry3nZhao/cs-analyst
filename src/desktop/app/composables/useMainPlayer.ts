import { listen } from '@tauri-apps/api/event';
import type { MainPlayerDto, MainPlayerSuggestionDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from './useTauri';

/**
 * 主玩家（「我」）。全应用共享一份状态，第一次调用时从后端加载，
 * 之后跟随 `main_player_changed` 事件自动更新（任何地方调用 `set` 都会广播）。
 *
 * ```ts
 * const { steamid, displayName, ready, onChange } = useMainPlayer();
 * await ready;                       // 需要首屏就拿到值时
 * watch(steamid, (id) => load(id));  // 或者
 * onChange((p) => load(p.steamid));  // 返回取消函数；组件卸载时自动取消
 * ```
 */

type Listener = (player: MainPlayerDto) => void;

const EMPTY: MainPlayerDto = { steamid: null, displayName: null, personId: null, matchCount: 0, suggestion: null };

const player = shallowRef<MainPlayerDto>(EMPTY);
const loaded = ref(false);
const listeners = new Set<Listener>();
let readyPromise: Promise<MainPlayerDto> | null = null;

function apply(next: MainPlayerDto) {
  const changed = next.steamid !== player.value.steamid;
  player.value = next;
  loaded.value = true;
  if (changed) {
    for (const cb of listeners) {
      try {
        cb(next);
      } catch (e) {
        console.error('[useMainPlayer] listener failed', e);
      }
    }
  }
}

async function refresh(): Promise<MainPlayerDto> {
  try {
    apply(await tauriInvoke<MainPlayerDto>('get_main_player'));
  } catch {
    loaded.value = true; // 浏览器里直接打开页面（没有 Tauri）时保持空
  }
  return player.value;
}

function init(): Promise<MainPlayerDto> {
  if (!readyPromise) {
    listen<MainPlayerDto>('main_player_changed', (e) => apply(e.payload)).catch(() => {});
    // 新对局解析完：显示名 / 场次 / 候选人可能变了（不会触发 onChange，除非 steamid 变）
    let timer: ReturnType<typeof setTimeout> | null = null;
    listen<{ status: string }>('analysis_updated', (e) => {
      if (e.payload.status !== 'done') return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => void refresh(), 1500);
    }).catch(() => {});
    readyPromise = refresh();
  }
  return readyPromise;
}

export function useMainPlayer() {
  const ready = init();

  /** 设置主玩家。接受 SteamID64、个人主页链接、`[U:1:n]`、`STEAM_X:Y:Z`；null 清除。失败时抛出后端的中文错误。 */
  async function set(steamid: string | null): Promise<MainPlayerDto> {
    const next = await tauriInvoke<MainPlayerDto>('set_main_player', { steamid });
    apply(next);
    return next;
  }

  /** 主玩家变化时回调（不会对当前值立即回调）。在组件 setup 里调用时卸载自动取消。 */
  function onChange(cb: Listener): () => void {
    listeners.add(cb);
    const off = () => listeners.delete(cb);
    if (getCurrentInstance()) onScopeDispose(off);
    return off;
  }

  return {
    /** 十进制字符串；null = 未设置 */
    steamid: computed<string | null>(() => player.value.steamid),
    /** 显示名：手动名 > 最近的游戏名；可能为 null（steamid 不在任何已解析对局里） */
    displayName: computed<string | null>(() => player.value.displayName),
    /** W1-C 的 person id，可用于 `/players/[steamid]` 之外的身份查询 */
    personId: computed<number | null>(() => player.value.personId),
    matchCount: computed<number>(() => player.value.matchCount),
    isSet: computed<boolean>(() => player.value.steamid !== null),
    /** 未设置时后端推测的候选人（不会自动写入） */
    suggestion: computed<MainPlayerSuggestionDto | null>(() => player.value.suggestion),
    /** 完整对象 */
    player: readonly(player),
    /** 是否已经从后端加载过 */
    loaded: readonly(loaded),
    /** 首次加载完成的 Promise */
    ready,
    refresh,
    set,
    clear: () => set(null),
    onChange,
  };
}

/** 候选来源的中文说明 */
export function mainPlayerSourceLabel(source: MainPlayerSuggestionDto['source']): string {
  return {
    valve: '已绑定的 Valve 账号',
    account: '已绑定的平台账号',
    followed: '我关注的人',
    frequent: '已解析对局里出场最多',
  }[source];
}
