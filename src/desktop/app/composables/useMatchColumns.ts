import type { MatchDto } from '@cs-analyst/shared-types';

export type MatchColumnKey =
  | 'fileName'
  | 'mapName'
  | 'durationSeconds'
  | 'source'
  | 'status'
  | 'analysis'
  | 'roundsPlayed'
  | 'progress'
  | 'playedAt'
  | 'gameMode'
  | 'gameType'
  | 'matchId'
  | 'shareCode'
  | 'filePath'
  | 'demoFileStamp'
  | 'networkProtocol'
  | 'serverName'
  | 'clientName'
  | 'gameDirectory'
  | 'fullpacketsVersion'
  | 'allowClientsideEntities'
  | 'demoHeaderField9'
  | 'demoHeaderField10'
  | 'demoVersionName'
  | 'demoVersionGuid'
  | 'playbackTime'
  | 'playbackTicks'
  | 'playbackFrames'
  | 'demGameInfo';

export interface MatchColumnDef {
  key: MatchColumnKey;
  label: string;
  defaultVisible: boolean;
  className?: string;
}

export const MATCH_COLUMNS: MatchColumnDef[] = [
  { key: 'fileName', label: '文件名', defaultVisible: true },
  { key: 'mapName', label: '地图', defaultVisible: true },
  { key: 'durationSeconds', label: '持续时间', defaultVisible: true, className: 'num' },
  { key: 'source', label: '来源', defaultVisible: true },
  { key: 'status', label: '状态', defaultVisible: false },
  { key: 'analysis', label: '解析', defaultVisible: true },
  { key: 'roundsPlayed', label: '回合', defaultVisible: true, className: 'num' },
  { key: 'progress', label: '进度', defaultVisible: false, className: 'col-prog' },
  { key: 'playedAt', label: '时间', defaultVisible: false },
  { key: 'gameMode', label: '模式', defaultVisible: false },
  { key: 'gameType', label: '类型', defaultVisible: false, className: 'num' },
  { key: 'matchId', label: 'Match ID', defaultVisible: false },
  { key: 'shareCode', label: 'Share Code', defaultVisible: false },
  { key: 'filePath', label: '路径', defaultVisible: false },
  { key: 'demoFileStamp', label: 'Demo Stamp', defaultVisible: false },
  { key: 'networkProtocol', label: '网络协议', defaultVisible: false, className: 'num' },
  { key: 'serverName', label: '服务器', defaultVisible: false },
  { key: 'clientName', label: '客户端', defaultVisible: false },
  { key: 'gameDirectory', label: '游戏目录', defaultVisible: false },
  { key: 'fullpacketsVersion', label: 'Fullpackets', defaultVisible: false, className: 'num' },
  { key: 'allowClientsideEntities', label: '客户端实体', defaultVisible: false },
  { key: 'demoHeaderField9', label: 'Header #9', defaultVisible: false, className: 'num' },
  { key: 'demoHeaderField10', label: 'Header #10', defaultVisible: false },
  { key: 'demoVersionName', label: 'Demo 版本', defaultVisible: false },
  { key: 'demoVersionGuid', label: 'Demo GUID', defaultVisible: false },
  { key: 'playbackTime', label: '播放时长(s)', defaultVisible: false, className: 'num' },
  { key: 'playbackTicks', label: 'Ticks', defaultVisible: false, className: 'num' },
  { key: 'playbackFrames', label: 'Frames', defaultVisible: false, className: 'num' },
  { key: 'demGameInfo', label: 'Game Info', defaultVisible: false },
];

const STORAGE_KEY = 'matches-visible-columns';

export function defaultVisibleColumnKeys(): MatchColumnKey[] {
  return MATCH_COLUMNS.filter(c => c.defaultVisible).map(c => c.key);
}

export function useMatchColumns() {
  const visibleKeys = ref<MatchColumnKey[]>(loadVisibleColumns());

  const visibleColumns = computed(() =>
    MATCH_COLUMNS.filter(c => visibleKeys.value.includes(c.key)),
  );

  function loadVisibleColumns(): MatchColumnKey[] {
    if (!import.meta.client) return defaultVisibleColumnKeys();
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return defaultVisibleColumnKeys();
      const parsed = JSON.parse(raw) as MatchColumnKey[];
      const valid = new Set(MATCH_COLUMNS.map(c => c.key));
      const filtered = parsed.filter(k => valid.has(k));
      return filtered.length > 0 ? filtered : defaultVisibleColumnKeys();
    } catch {
      return defaultVisibleColumnKeys();
    }
  }

  function saveVisibleColumns(keys: MatchColumnKey[]) {
    visibleKeys.value = keys;
    if (import.meta.client) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(keys));
    }
  }

  function toggleColumn(key: MatchColumnKey) {
    const set = new Set(visibleKeys.value);
    if (set.has(key)) {
      if (set.size <= 1) return;
      set.delete(key);
    } else {
      set.add(key);
    }
    const ordered = MATCH_COLUMNS.map(c => c.key).filter(k => set.has(k));
    saveVisibleColumns(ordered);
  }

  function resetColumns() {
    saveVisibleColumns(defaultVisibleColumnKeys());
  }

  return { visibleKeys, visibleColumns, toggleColumn, resetColumns, MATCH_COLUMNS };
}

export function formatMatchCell(key: MatchColumnKey, job: MatchDto, helpers: {
  fmtTime: (v?: string | null) => string;
  fmtDuration: (v?: number | null) => string;
  gameModeLabel: (v?: string | null) => string;
  gameTypeLabel: (v?: number | null) => string;
  statusLabel: (v: string) => string;
}): string {
  switch (key) {
    case 'fileName': return job.fileName ?? '—';
    case 'mapName': return job.mapName ?? '—';
    case 'durationSeconds': return helpers.fmtDuration(job.durationSeconds);
    case 'source': return job.source;
    case 'roundsPlayed': return job.roundsPlayed != null ? String(job.roundsPlayed) : '—';
    case 'playedAt': return helpers.fmtTime(job.playedAt);
    case 'gameMode': return helpers.gameModeLabel(job.gameMode);
    case 'gameType': return helpers.gameTypeLabel(job.gameType);
    case 'matchId': return job.matchId;
    case 'shareCode': return job.shareCode ?? '—';
    case 'filePath': return job.filePath ?? '—';
    case 'demoFileStamp': return job.demoFileStamp ?? '—';
    case 'networkProtocol': return job.networkProtocol != null ? String(job.networkProtocol) : '—';
    case 'serverName': return job.serverName ?? '—';
    case 'clientName': return job.clientName ?? '—';
    case 'gameDirectory': return job.gameDirectory ?? '—';
    case 'fullpacketsVersion': return job.fullpacketsVersion != null ? String(job.fullpacketsVersion) : '—';
    case 'allowClientsideEntities':
      return job.allowClientsideEntities == null ? '—' : (job.allowClientsideEntities ? '是' : '否');
    case 'demoHeaderField9': return job.demoHeaderField9 != null ? String(job.demoHeaderField9) : '—';
    case 'demoHeaderField10': return job.demoHeaderField10 ?? '—';
    case 'demoVersionName': return job.demoVersionName ?? '—';
    case 'demoVersionGuid': return job.demoVersionGuid ?? '—';
    case 'playbackTime': return job.playbackTime != null ? job.playbackTime.toFixed(1) : '—';
    case 'playbackTicks': return job.playbackTicks != null ? String(job.playbackTicks) : '—';
    case 'playbackFrames': return job.playbackFrames != null ? String(job.playbackFrames) : '—';
    case 'demGameInfo':
      return job.demGameInfo ? JSON.stringify(job.demGameInfo) : '—';
    default: return '—';
  }
}
