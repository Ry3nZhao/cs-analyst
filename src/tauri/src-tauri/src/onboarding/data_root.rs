//! 数据目录的选择。
//!
//! 数据库本身在数据目录里，所以「用哪个目录」不能存进数据库：
//! - 环境变量 `CS_ANALYST_DATA_ROOT` 优先（测试、便携使用）；
//! - 否则读默认目录（`%LOCALAPPDATA%\com.cs-analyst.app`）下的 `data_root.txt`；
//! - 都没有就用默认目录。
//!
//! 改目录后需要重启应用；旧目录里的数据不会自动搬走。

use std::path::{Path, PathBuf};

pub const ENV_VAR: &str = "CS_ANALYST_DATA_ROOT";
const POINTER_FILE: &str = "data_root.txt";

pub fn env_override() -> Option<PathBuf> {
    std::env::var(ENV_VAR).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).map(PathBuf::from)
}

pub fn default_root(app: &tauri::AppHandle) -> PathBuf {
    crate::database::resolve_data_root(app)
}

fn read_pointer(default: &Path) -> Option<PathBuf> {
    let s = std::fs::read_to_string(default.join(POINTER_FILE)).ok()?;
    let p = s.trim();
    (!p.is_empty()).then(|| PathBuf::from(p))
}

/// 应用启动时用来打开数据库的目录。
pub fn resolve(app: &tauri::AppHandle) -> PathBuf {
    if let Some(p) = env_override() {
        return p;
    }
    let default = default_root(app);
    read_pointer(&default).unwrap_or(default)
}

/// 写入（或清除）自定义目录。`None` = 恢复默认。
pub fn write_pointer(app: &tauri::AppHandle, target: Option<&Path>) -> Result<(), String> {
    if env_override().is_some() {
        return Err(format!("数据目录由环境变量 {ENV_VAR} 指定，不能在应用里修改"));
    }
    let default = default_root(app);
    std::fs::create_dir_all(&default).map_err(|e| e.to_string())?;
    let pointer = default.join(POINTER_FILE);
    match target {
        Some(dir) if dir != default => {
            std::fs::create_dir_all(dir).map_err(|e| format!("无法创建目录：{e}"))?;
            // 能写才算可用
            let probe = dir.join(".cs-analyst-write-test");
            std::fs::write(&probe, b"ok").map_err(|e| format!("目录不可写：{e}"))?;
            let _ = std::fs::remove_file(probe);
            std::fs::write(pointer, dir.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
        }
        _ => {
            if pointer.exists() {
                std::fs::remove_file(pointer).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }
}
