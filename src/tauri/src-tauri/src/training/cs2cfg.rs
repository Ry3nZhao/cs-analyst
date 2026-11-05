//! 读取本机 CS2 配置里的鼠标灵敏度。
//!
//! CS2 把每个 Steam 账号的 convar 存在
//! `<Steam>/userdata/<accountid>/730/local/cfg/cs2_user_convars_0_slot0.vcfg`，
//! 是 KeyValues 文本，里面有一行 `"sensitivity"		"1.25"`。

use serde::Serialize;
use std::path::{Path, PathBuf};

const CFG_REL: [&str; 4] = ["730", "local", "cfg", "cs2_user_convars_0_slot0.vcfg"];

fn cfg_path(account_dir: &Path) -> PathBuf {
    CFG_REL.iter().fold(account_dir.to_path_buf(), |p, c| p.join(c))
}
/// SteamID64 = 账号 ID + 这个基数。
const STEAMID64_BASE: u64 = 76_561_197_960_265_728;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cs2SensitivityCandidate {
    pub path: String,
    /// userdata 下的目录名（32 位账号 ID）。
    pub account_id: Option<String>,
    /// 对应的 SteamID64（十进制字符串）。
    pub steamid: Option<String>,
    pub sensitivity: Option<f64>,
    /// 文件修改时间（RFC 3339）。
    pub modified_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cs2SensitivityDetect {
    pub steam_path: Option<String>,
    /// 按修改时间从新到旧排列。
    pub candidates: Vec<Cs2SensitivityCandidate>,
}

/// 从 vcfg（`"sensitivity"  "1.25"`）或 cfg（`sensitivity 1.25`、`sensitivity "1.25"`）文本里取
/// `sensitivity`。只认完整的键名（不会匹配到 `zoom_sensitivity_ratio`）；有多行时最后一行生效，和游戏执行
/// cfg 的结果一致。
pub fn parse_sensitivity(text: &str) -> Option<f64> {
    let mut found = None;
    for line in text.lines() {
        let line = line.split("//").next().unwrap_or("");
        for stmt in line.split(';') {
            let mut parts = stmt.split(|c: char| c == '"' || c.is_whitespace()).filter(|s| !s.is_empty());
            if parts.next() == Some("sensitivity") {
                if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                    if v.is_finite() && v > 0.0 {
                        found = Some(v);
                    }
                }
            }
        }
    }
    found
}

pub fn read_file(path: &Path) -> Result<f64, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取失败: {e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    parse_sensitivity(&text).ok_or_else(|| "文件里没有 sensitivity".to_string())
}

/// Steam 安装目录：先查注册表，再试常见位置。
pub fn steam_path() -> Option<PathBuf> {
    if let Some(p) = steam_path_from_registry() {
        if p.join("userdata").is_dir() {
            return Some(p);
        }
    }
    [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.join("userdata").is_dir())
}

#[cfg(windows)]
fn steam_path_from_registry() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    parse_reg_query(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(windows))]
fn steam_path_from_registry() -> Option<PathBuf> {
    None
}

fn parse_reg_query(out: &str) -> Option<PathBuf> {
    out.lines().find_map(|l| {
        let l = l.trim();
        let rest = l.strip_prefix("SteamPath")?.trim_start();
        let value = rest.strip_prefix("REG_SZ")?.trim();
        (!value.is_empty()).then(|| PathBuf::from(value.replace('/', "\\")))
    })
}

/// 列出 Steam 目录下所有账号的 CS2 配置。
pub fn detect_in(steam: &Path) -> Vec<Cs2SensitivityCandidate> {
    let Ok(entries) = std::fs::read_dir(steam.join("userdata")) else {
        return Vec::new();
    };
    let mut out: Vec<(std::time::SystemTime, Cs2SensitivityCandidate)> = entries
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let file = cfg_path(&dir);
            let meta = std::fs::metadata(&file).ok()?;
            let account_id = dir.file_name().map(|n| n.to_string_lossy().to_string());
            let steamid = account_id.as_deref().and_then(|a| a.parse::<u64>().ok()).filter(|a| *a > 0).map(|a| (a + STEAMID64_BASE).to_string());
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            let (sensitivity, error) = match read_file(&file) {
                Ok(v) => (Some(v), None),
                Err(e) => (None, Some(e)),
            };
            Some((
                modified,
                Cs2SensitivityCandidate {
                    path: file.to_string_lossy().to_string(),
                    account_id,
                    steamid,
                    sensitivity,
                    modified_at: Some(chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339()),
                    error,
                },
            ))
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|(_, c)| c).collect()
}

pub fn detect() -> Cs2SensitivityDetect {
    let steam = steam_path();
    let candidates = steam.as_deref().map(detect_in).unwrap_or_default();
    Cs2SensitivityDetect { steam_path: steam.map(|p| p.to_string_lossy().to_string()), candidates }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\"config\"\n{\n\t\"convars\"\n\t{\n\t\t\"zoom_sensitivity_ratio\"\t\t\"0.818933\"\n\t\t\"sensitivity\"\t\t\"1.25\"\n\t\t\"m_yaw\"\t\t\"0.022\"\n\t}\n}\n";

    #[test]
    fn parses_sensitivity_line() {
        assert_eq!(parse_sensitivity(SAMPLE), Some(1.25));
        assert_eq!(parse_sensitivity("\"zoom_sensitivity_ratio\" \"0.8\""), None);
        assert_eq!(parse_sensitivity("\"sensitivity\" \"abc\""), None);
        assert_eq!(parse_sensitivity("\"sensitivity\" \"-1\""), None);
        assert_eq!(parse_sensitivity("// sensitivity 9\nsensitivity 2.1; m_yaw 0.022\nsensitivity \"1.6\" // x"), Some(1.6));
    }

    #[test]
    fn parses_reg_output() {
        let out = "\r\nHKEY_CURRENT_USER\\Software\\Valve\\Steam\r\n    SteamPath    REG_SZ    c:/program files (x86)/steam\r\n\r\n";
        assert_eq!(parse_reg_query(out), Some(PathBuf::from(r"c:\program files (x86)\steam")));
        assert_eq!(parse_reg_query("ERROR: not found"), None);
    }

    #[test]
    fn detects_userdata_configs() {
        let root = std::env::temp_dir().join(format!("cs-analyst-steam-test-{}", uuid::Uuid::new_v4()));
        let cfg = cfg_path(&root.join("userdata").join("39734314"));
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(&cfg, SAMPLE).unwrap();
        std::fs::create_dir_all(root.join("userdata").join("0")).unwrap(); // 没有 CS2 配置的目录被忽略
        let c = detect_in(&root);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].sensitivity, Some(1.25));
        assert_eq!(c[0].account_id.as_deref(), Some("39734314"));
        assert_eq!(c[0].steamid.as_deref(), Some("76561198000000042"));
        let _ = std::fs::remove_dir_all(root);
    }
}
