//! 样例 demo 回归测试的本机配置。样例目录和主玩家 steamid 只从环境变量读取，不写进仓库：
//!
//! - `CS_ANALYST_SAMPLE_DEMOS`：样例目录（含 `Official/`、`Platforms/`、`Profs/`）
//! - `CS_ANALYST_SAMPLE_STEAMID`：样例里固定数字所对应的主玩家
//!
//! 本机放在 `src/tauri/src-tauri/.cargo/config.toml` 的 `[env]` 里（已 gitignore），见 docs/development.md。
//! 没有设置时样例测试自动跳过。

use std::path::PathBuf;

/// 样例目录；未设置时返回空路径，拼出来的文件都不存在，相关测试就会跳过。
pub fn dir() -> PathBuf {
    std::env::var_os("CS_ANALYST_SAMPLE_DEMOS").map(PathBuf::from).unwrap_or_default()
}

/// 主玩家 steamid（字符串形式）；编译期读取，未设置时为 "0"。
pub const PLAYER_STR: &str = match option_env!("CS_ANALYST_SAMPLE_STEAMID") {
    Some(s) => s,
    None => "0",
};

/// 主玩家 steamid。
pub const PLAYER: u64 = parse_u64(PLAYER_STR);

const fn parse_u64(s: &str) -> u64 {
    let b = s.as_bytes();
    let mut i = 0;
    let mut v: u64 = 0;
    while i < b.len() {
        let c = b[i];
        if c >= b'0' && c <= b'9' {
            v = v * 10 + (c - b'0') as u64;
        }
        i += 1;
    }
    v
}
