//! 上手体验（W3-C，docs/roadmap.md 第 5 节）。
//!
//! - `main_player`：主玩家（「我」）读写与候选推测
//! - `settings`：托盘 / 通知 / 监听目录开关，向导是否走过
//! - `data_root`：数据目录选择（指针文件 / 环境变量）
//! - `tray`：托盘菜单、关闭到托盘
//! - `notify`：新对局解析完成的系统通知
//! - `commands`：Tauri 命令
//!
//! 监听 CS2 replays 目录在 `crate::watch`。

pub mod commands;
pub mod data_root;
pub mod main_player;
pub mod notify;
pub mod settings;
pub mod tray;

#[cfg(test)]
mod tests;

pub use data_root::resolve as resolve_data_root;
pub use tray::on_window_event;

/// 在 `setup` 里、`app.manage(AppState)` 之后调用。
pub fn init(app: &tauri::AppHandle) {
    if let Err(e) = tray::init(app) {
        eprintln!("[onboarding] tray init failed: {e}");
    }
    notify::start(app);
    crate::watch::start(app);
}
