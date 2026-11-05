-- W3-C 上手体验：主玩家、托盘、通知、监听 CS2 replays 目录、首次启动向导。

-- 主玩家（「我」）。NULL = 未设置
ALTER TABLE app_settings ADD COLUMN main_player_steamid INTEGER;
-- 关闭窗口时最小化到托盘
ALTER TABLE app_settings ADD COLUMN close_to_tray INTEGER NOT NULL DEFAULT 1;
-- 新对局解析完成时弹系统通知
ALTER TABLE app_settings ADD COLUMN notify_new_match INTEGER NOT NULL DEFAULT 1;
-- 监听 CS2 replays 目录，新出现的 .dem 自动复制导入
ALTER TABLE app_settings ADD COLUMN watch_replays INTEGER NOT NULL DEFAULT 1;
-- 手动指定的 replays 目录；NULL = 自动检测 Steam 库
ALTER TABLE app_settings ADD COLUMN replays_dir TEXT;
-- 首次启动向导完成（或跳过）的时间；NULL = 还没走过
ALTER TABLE app_settings ADD COLUMN onboarding_completed_at TEXT;

-- 已经弹过「新对局已解析」通知的对局。重新解析不会再弹。
CREATE TABLE IF NOT EXISTS match_notifications (
  match_id TEXT PRIMARY KEY REFERENCES matches(id) ON DELETE CASCADE,
  notified_at TEXT NOT NULL DEFAULT (datetime('now'))
);
-- 存量对局视为已通知，避免升级后回填 / 重新解析时刷屏
INSERT OR IGNORE INTO match_notifications (match_id) SELECT id FROM matches;

-- 已有对局或账号的老用户不再进入首次启动向导
UPDATE app_settings SET onboarding_completed_at = datetime('now')
 WHERE id = 1
   AND (EXISTS (SELECT 1 FROM matches) OR EXISTS (SELECT 1 FROM sub_accounts));
