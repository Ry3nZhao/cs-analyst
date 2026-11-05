-- W3-D 存储与数据：收藏、原始 demo 的存储状态、存储策略设置。

-- 收藏的对局永不被自动清理
ALTER TABLE matches ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0;
-- 原始 demo 的存储状态：raw = 原始 .dem；compressed = .dem.zst；deleted = 已删除，只留数据
ALTER TABLE matches ADD COLUMN demo_storage TEXT NOT NULL DEFAULT 'raw'
  CHECK (demo_storage IN ('raw', 'compressed', 'deleted'));
-- 压缩 / 删除前原始 .dem 的字节数，以及状态变化时间
ALTER TABLE matches ADD COLUMN demo_original_size INTEGER;
ALTER TABLE matches ADD COLUMN demo_storage_at TEXT;

CREATE INDEX IF NOT EXISTS idx_matches_favorite ON matches(favorite) WHERE favorite = 1;

CREATE TABLE IF NOT EXISTS storage_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  -- 解析成功后对原始 demo 的处理：keep / compress / delete
  demo_policy TEXT NOT NULL DEFAULT 'keep' CHECK (demo_policy IN ('keep', 'compress', 'delete')),
  -- 启动时按规则自动清理原始 demo（默认关闭，开启前界面会先预览）
  auto_cleanup INTEGER NOT NULL DEFAULT 0,
  -- 清理规则：对局早于 N 天 / demo 总占用超过 N GB；NULL 表示不启用该规则
  cleanup_max_age_days INTEGER,
  cleanup_max_total_gb REAL,
  last_cleanup_at TEXT,
  last_cleanup_summary TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO storage_settings (id) SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM storage_settings WHERE id = 1);
