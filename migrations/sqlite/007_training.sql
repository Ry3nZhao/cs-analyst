-- W2-F 瞄准训练（docs/training.md）。
-- 全部 IF NOT EXISTS：开发库可能先跑到 7 再补 006，重放本迁移不会出错。

-- 一轮训练（一次「开始 → 结果页」）。
CREATE TABLE IF NOT EXISTS training_sessions (
  id TEXT PRIMARY KEY,
  -- reaction / flick / tracking / micro / strafe / spray
  mode TEXT NOT NULL,
  -- 模式内的变体：压枪练习是武器名（ak47 …），其他模式为空串。
  variant TEXT NOT NULL DEFAULT '',
  -- ISO 8601（UTC），由前端在开始时记录。
  started_at TEXT NOT NULL,
  duration_ms INTEGER NOT NULL,
  -- 该模式的主成绩，口径见 src/desktop/app/training/meta.ts 的 TRAINING_MODES（单位、越高越好还是越低越好）。
  score REAL,
  -- 其他汇总指标（JSON 对象，键名随模式不同）。
  metrics TEXT NOT NULL DEFAULT '{}',
  -- 训练时的设置（JSON）：灵敏度、视场、目标大小、时长等。
  settings TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_training_sessions_mode ON training_sessions(mode, variant, started_at);

-- 每一次尝试：一次反应、一个目标、一次急停、一发子弹……
CREATE TABLE IF NOT EXISTS training_results (
  session_id TEXT NOT NULL REFERENCES training_sessions(id) ON DELETE CASCADE,
  idx INTEGER NOT NULL,
  -- 主数值：反应时间 ms、目标用时 ms、急停时机 ms、子弹误差 °……
  value REAL,
  -- 是否成功（命中 / 急停成功 / 非犯规），不适用时为 NULL。
  ok INTEGER,
  -- 附加字段（JSON），例如压枪每发的 err_v / err_h。
  data TEXT NOT NULL DEFAULT '{}',
  PRIMARY KEY (session_id, idx)
);
