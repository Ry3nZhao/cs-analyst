/**
 * 瞄准训练（W2-F）。命令在 `src/tauri/src-tauri/src/training/`，表在迁移 007，说明见 docs/training.md。
 */

export type TrainingMode = 'reaction' | 'flick' | 'tracking' | 'micro' | 'strafe' | 'spray';

/** `list_training_results` 的时间范围 */
export type TrainingRange = '7d' | '30d' | '90d' | '365d' | 'all';

export interface TrainingModeMeta {
  mode: TrainingMode;
  label: string;
  /** 一句话说明 */
  description: string;
  /** 主成绩（`score` 列）的名称、单位和方向 */
  scoreLabel: string;
  scoreUnit: string;
  higherIsBetter: boolean;
  /** 是否需要锁定鼠标（Pointer Lock） */
  pointerLock: boolean;
}

/** 各模式的元数据（运行时常量）在前端 `src/desktop/app/training/meta.ts` 的 TRAINING_MODES。 */

/** 一次尝试（`training_results` 一行） */
export interface TrainingTrial {
  value?: number | null;
  ok?: boolean | null;
  data?: Record<string, unknown>;
}

/** `save_training_result({ result })` 的入参 */
export interface TrainingResultInput {
  mode: TrainingMode;
  /** 压枪练习的武器名，其他模式不传 */
  variant?: string | null;
  /** ISO 8601 */
  startedAt: string;
  durationMs: number;
  score: number | null;
  metrics: Record<string, number | string | null>;
  settings: TrainingSettingsSnapshot & Record<string, unknown>;
  trials: TrainingTrial[];
}

/** 每次训练都会记下的输入设置 */
export interface TrainingSettingsSnapshot {
  sensitivity: number;
  /** 每个鼠标计数转多少度 = sensitivity × m_yaw */
  degPerCount: number;
  /** 训练区域的水平视场（度） */
  hfovDeg: number;
  /** 是否拿到了 Pointer Lock */
  pointerLock: boolean;
}

/** `list_training_results({ mode, range?, variant? })` 返回的一行，按开始时间升序 */
export interface TrainingSessionDto {
  id: string;
  mode: TrainingMode;
  variant: string;
  startedAt: string;
  durationMs: number;
  score: number | null;
  metrics: Record<string, number | string | null>;
  settings: Record<string, unknown>;
  trialCount: number;
}

/** `get_training_trials({ sessionId })` */
export interface TrainingTrialDto {
  idx: number;
  value: number | null;
  ok: boolean | null;
  data: Record<string, unknown>;
}

/** `detect_cs2_sensitivity()` 里的一个配置文件 */
export interface Cs2SensitivityCandidate {
  path: string;
  accountId: string | null;
  /** SteamID64，十进制字符串 */
  steamid: string | null;
  sensitivity: number | null;
  modifiedAt: string | null;
  error: string | null;
}

export interface Cs2SensitivityDetect {
  steamPath: string | null;
  /** 按修改时间从新到旧 */
  candidates: Cs2SensitivityCandidate[];
}
