//! 自动检测 CS2 的 `game/csgo/replays` 目录：注册表里的 Steam 安装路径 → libraryfolders.vdf → 每个库。

use std::path::{Path, PathBuf};

const CS2_DIR: &str = "steamapps/common/Counter-Strike Global Offensive";
const REPLAYS_SUBDIR: &str = "game/csgo/replays";

/// 从 libraryfolders.vdf 里取出所有 `"path"` 值（反转义 `\\`）。
pub fn parse_library_paths(vdf: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in vdf.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("\"path\"") else { continue };
        let rest = rest.trim();
        let Some(inner) = rest.strip_prefix('"').and_then(|r| r.strip_suffix('"')) else { continue };
        let path = inner.replace("\\\\", "\\");
        if !path.is_empty() {
            out.push(PathBuf::from(path));
        }
    }
    out
}

#[cfg(windows)]
fn steam_roots_from_registry() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let mut out = Vec::new();
    if let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
        if let Ok(p) = k.get_value::<String, _>("SteamPath") {
            out.push(PathBuf::from(p.replace('/', "\\")));
        }
    }
    for sub in ["SOFTWARE\\WOW6432Node\\Valve\\Steam", "SOFTWARE\\Valve\\Steam"] {
        if let Ok(k) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(sub) {
            if let Ok(p) = k.get_value::<String, _>("InstallPath") {
                out.push(PathBuf::from(p));
            }
        }
    }
    out
}

#[cfg(not(windows))]
fn steam_roots_from_registry() -> Vec<PathBuf> {
    Vec::new()
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots = steam_roots_from_registry();
    roots.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));
    roots.push(PathBuf::from("C:\\Program Files\\Steam"));
    let mut seen = Vec::<String>::new();
    roots.retain(|r| {
        let key = r.to_string_lossy().to_lowercase().replace('/', "\\");
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    roots
}

/// 某个 Steam 库下 CS2 的 replays 目录（不检查是否存在）。
pub fn replays_dir_in_library(lib: &Path) -> PathBuf {
    let parts = CS2_DIR.split('/').chain(REPLAYS_SUBDIR.split('/'));
    parts.fold(lib.to_path_buf(), |p, s| p.join(s))
}

/// 找到 CS2 安装位置下的 replays 目录。优先返回已存在的 replays 目录；
/// CS2 装了但还没下载过 demo（replays 不存在）时也返回它，由监听线程等目录出现。
pub fn detect_replays_dir() -> Option<PathBuf> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in steam_roots() {
        let vdf = root.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            libraries.extend(parse_library_paths(&text));
        }
        libraries.push(root);
    }
    let mut installed: Option<PathBuf> = None;
    for lib in libraries {
        let replays = replays_dir_in_library(&lib);
        if replays.is_dir() {
            return Some(replays);
        }
        if installed.is_none() && replays.parent().is_some_and(|csgo| csgo.is_dir()) {
            installed = Some(replays);
        }
    }
    installed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_library_folders() {
        let vdf = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"228980"		"1234"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"apps"
		{
			"730"		"60000000000"
		}
	}
}"#;
        let libs = parse_library_paths(vdf);
        assert_eq!(libs, vec![PathBuf::from("C:\\Program Files (x86)\\Steam"), PathBuf::from("D:\\SteamLibrary")]);
        assert_eq!(
            replays_dir_in_library(&libs[1]),
            PathBuf::from("D:\\SteamLibrary\\steamapps\\common\\Counter-Strike Global Offensive\\game\\csgo\\replays")
        );
    }
}
