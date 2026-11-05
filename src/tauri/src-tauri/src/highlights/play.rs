//! 在 CS2 中播放集锦。
//!
//! 保底方案（默认）：
//! - 生成 `playdemo "<路径>"` 和 `demo_gototick <起点 − 提前秒数>`，前端一键复制；
//! - 「用 CS2 打开 demo」：`steam.exe -applaunch 730 +playdemo "<路径>"`，找不到 steam.exe 时用
//!   `steam://rungameid/730//+playdemo …`。跳转仍要用户在控制台粘贴 `demo_gototick`。
//!
//! demo 已被压缩（`.dem.zst`）时先解压到系统临时目录 `cs-analyst-playback/`（只保留最近 2 个）；
//! 已删除的返回 `deleted`，前端提示不可播放。
//!
//! 实验（设置里默认关闭）：启动参数加 `-netconport <端口>`，连上 CS2 的本地控制台端口后等 demo
//! 加载完再自动发送 `demo_gototick`。CS2 已经开着且端口可连时不再启动，直接发送命令；同一个 demo
//! 只发跳转。Windows 上这个端口能不能用要实测（docs/roadmap.md 6.3）。

use super::{log, repo};
use crate::database::Database;
use parking_lot::Mutex;
use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 回放的 tick 率（CS2 demo 都是 64）。
pub const DEMO_TICK_RATE: i64 = 64;
/// 临时目录里最多保留几个解压出来的 demo（每个 100–500 MB）。
const KEEP_DECOMPRESSED: usize = 2;
/// netcon：等 CS2 打开端口的最长时间。
const NETCON_CONNECT_TIMEOUT: Duration = Duration::from_secs(180);
/// netcon：连上之后最多等多久 demo 加载完（看不到加载完成的日志时按这个时间发送）。
const NETCON_LOAD_FALLBACK: Duration = Duration::from_secs(45);
/// 控制台日志里表示 demo 已经开始播放的标记（不区分大小写）。
const LOADED_MARKERS: &[&str] = &["csgo_game_ui_state_ingame", "signonstate_full", "demo playback", "playing demo"];

static LAST_NETCON_DEMO: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackInfo {
    pub highlight_id: i64,
    pub match_id: String,
    /// ready | deleted | missing
    pub status: String,
    pub message: Option<String>,
    /// 要播放的 .dem（原文件，或解压到临时目录的副本）
    pub demo_path: Option<String>,
    pub decompressed: bool,
    pub start_tick: i64,
    pub goto_tick: i64,
    pub lead_secs: i64,
    pub playdemo_command: Option<String>,
    pub goto_command: String,
    /// 两条一起（分号分隔），适合一次粘贴到控制台
    pub combined_command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub playback: PlaybackInfo,
    /// steam_exe | steam_url | netcon（CS2 已在运行，命令直接发过去了）| unavailable
    pub method: String,
    /// 是否会自动跳转（实验开关打开时）
    pub auto_goto: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NetconEvent<'a> {
    highlight_id: i64,
    /// connecting | connected | loading | sent | failed
    stage: &'a str,
    message: String,
}

pub fn cs2_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn sanitize(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

pub fn playback_dir() -> PathBuf {
    std::env::temp_dir().join("cs-analyst-playback")
}

/// 解压 `.dem.zst` 到 `dir/<match id>.dem`。已有且大小一致的直接复用。只保留最近的几个。
pub fn decompress_for_playback(zst: &Path, dir: &Path, match_id: &str, original_size: Option<i64>) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let target = dir.join(format!("{}.dem", sanitize(match_id)));
    let reuse = std::fs::metadata(&target)
        .ok()
        .is_some_and(|m| m.len() > 0 && original_size.is_none_or(|s| s as u64 == m.len()));
    if reuse {
        if let Ok(f) = std::fs::File::options().write(true).open(&target) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
    } else {
        let part = dir.join(format!("{}.dem.part", sanitize(match_id)));
        let run = || -> std::io::Result<()> {
            let input = std::io::BufReader::with_capacity(1 << 20, std::fs::File::open(zst)?);
            let mut dec = zstd::stream::read::Decoder::with_buffer(input)?;
            let mut out = std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(&part)?);
            std::io::copy(&mut dec, &mut out)?;
            out.flush()?;
            Ok(())
        };
        if let Err(e) = run() {
            let _ = std::fs::remove_file(&part);
            return Err(format!("解压 demo 失败: {e}"));
        }
        if let Some(s) = original_size {
            let got = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
            if got != s as u64 {
                let _ = std::fs::remove_file(&part);
                return Err(format!("解压后的大小不对（{got} ≠ {s}）"));
            }
        }
        let _ = std::fs::remove_file(&target);
        std::fs::rename(&part, &target).map_err(|e| format!("重命名失败: {e}"))?;
    }
    prune_playback_dir(dir, &target);
    Ok(target)
}

/// 只保留最近修改的 [`KEEP_DECOMPRESSED`] 个 .dem（`keep` 一定保留），删掉残留的 .part。
fn prune_playback_dir(dir: &Path, keep: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut dems: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if name.ends_with(".dem.part") && p != keep {
            // 超过 1 小时的半成品才删（可能正在解压另一场）
            let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|d| d.as_secs() > 3600);
            if old {
                let _ = std::fs::remove_file(&p);
            }
        } else if name.ends_with(".dem") && p != keep {
            if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
                dems.push((t, p));
            }
        }
    }
    dems.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in dems.into_iter().skip(KEEP_DECOMPRESSED.saturating_sub(1)) {
        let _ = std::fs::remove_file(p);
    }
}

pub fn goto_tick(start_tick: i64, round_start: Option<i64>, lead_secs: i64) -> i64 {
    (start_tick - lead_secs * DEMO_TICK_RATE).max(round_start.unwrap_or(0)).max(0)
}

/// 准备播放：找到 demo（必要时解压），算出跳转 tick 和控制台命令。
pub fn prepare(db: &Arc<Mutex<Database>>, id: i64) -> Result<PlaybackInfo, String> {
    let (row, abs, lead) = {
        let db = db.lock();
        let row = repo::get_playback_row(&db, id).map_err(|e| e.to_string())?.ok_or("集锦不存在（可能已重新计算）")?;
        let abs = row.file_path.as_deref().filter(|s| !s.trim().is_empty()).map(|p| db.resolve_path(p));
        let lead = repo::get_settings(&db).map(|s| s.lead_secs).unwrap_or(5);
        (row, abs, lead)
    };
    prepare_with(id, &row, abs.as_deref(), lead, &playback_dir())
}

pub fn prepare_with(id: i64, row: &repo::PlaybackRow, abs: Option<&Path>, lead: i64, tmp: &Path) -> Result<PlaybackInfo, String> {
    let goto = goto_tick(row.start_tick, row.round_start_tick, lead);
    let goto_command = format!("demo_gototick {goto}");
    let mut info = PlaybackInfo {
        highlight_id: id,
        match_id: row.match_id.clone(),
        status: "ready".into(),
        message: None,
        demo_path: None,
        decompressed: false,
        start_tick: row.start_tick,
        goto_tick: goto,
        lead_secs: lead,
        playdemo_command: None,
        goto_command,
        combined_command: None,
    };
    if row.demo_storage == "deleted" {
        info.status = "deleted".into();
        info.message = Some("原始 demo 已按存储策略删除，只保留了分析数据，无法在 CS2 中播放。".into());
        return Ok(info);
    }
    let Some(abs) = abs.filter(|p| p.exists()) else {
        info.status = "missing".into();
        info.message = Some("找不到 demo 文件（可能被移动或删除了）。".into());
        return Ok(info);
    };
    let is_zst = abs.extension().is_some_and(|e| e.eq_ignore_ascii_case("zst"));
    let play = if is_zst {
        info.decompressed = true;
        decompress_for_playback(abs, tmp, &row.match_id, row.demo_original_size)?
    } else {
        abs.to_path_buf()
    };
    let p = cs2_path(&play);
    info.playdemo_command = Some(format!("playdemo \"{p}\""));
    info.combined_command = Some(format!("playdemo \"{p}\"; {}", info.goto_command));
    info.demo_path = Some(p);
    Ok(info)
}

// ---------------------------------------------------------------------------------------------
// 启动 CS2
// ---------------------------------------------------------------------------------------------

#[cfg(windows)]
fn steam_exe() -> Option<PathBuf> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let k = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam").ok()?;
    if let Ok(exe) = k.get_value::<String, _>("SteamExe") {
        let p = PathBuf::from(exe.replace('/', "\\"));
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(root) = k.get_value::<String, _>("SteamPath") {
        let p = PathBuf::from(root.replace('/', "\\")).join("steam.exe");
        if p.exists() {
            return Some(p);
        }
    }
    let p = PathBuf::from("C:\\Program Files (x86)\\Steam\\steam.exe");
    p.exists().then_some(p)
}

#[cfg(not(windows))]
fn steam_exe() -> Option<PathBuf> {
    None
}

/// CS2 的启动参数（不含 `-applaunch 730`）。
pub fn launch_args(demo: &str, netcon_port: Option<i64>, extra: &str) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(port) = netcon_port {
        args.push("-netconport".into());
        args.push(port.to_string());
    }
    args.extend(extra.split_whitespace().map(str::to_string));
    args.push("+playdemo".into());
    args.push(demo.to_string());
    args
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/:+".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `steam://rungameid/730//<参数>`（参数之间是空格，整体 URL 编码）。
pub fn steam_url(args: &[String]) -> String {
    let joined = args
        .iter()
        .map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() })
        .collect::<Vec<_>>()
        .join(" ");
    format!("steam://rungameid/730//{}", percent_encode(&joined))
}

fn netcon_addr(port: i64) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port as u16))
}

fn emit(app: Option<&AppHandle>, id: i64, stage: &str, message: impl Into<String>) {
    let message = message.into();
    log(format!("netcon #{id} {stage}: {message}"));
    if let Some(app) = app {
        let _ = app.emit("highlight_netcon", NetconEvent { highlight_id: id, stage, message });
    }
}

fn send_line(s: &mut TcpStream, cmd: &str) -> std::io::Result<()> {
    s.write_all(format!("{cmd}\n").as_bytes())?;
    s.flush()
}

/// 等 demo 加载完（看控制台日志里的标记，最多 [`NETCON_LOAD_FALLBACK`]），然后发 `demo_gototick`。
fn wait_loaded_and_goto(mut s: TcpStream, app: Option<&AppHandle>, id: i64, goto: &str) {
    emit(app, id, "loading", "已连上 CS2 控制台端口，等待 demo 加载");
    let started = Instant::now();
    let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
    if let Ok(clone) = s.try_clone() {
        let mut reader = BufReader::new(clone);
        let mut line = String::new();
        while started.elapsed() < NETCON_LOAD_FALLBACK {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // 连接关闭
                Ok(_) => {
                    let l = line.to_ascii_lowercase();
                    if LOADED_MARKERS.iter().any(|m| l.contains(m)) {
                        break;
                    }
                }
                Err(_) => continue, // 读超时：继续等
            }
        }
    }
    // 加载完到能跳转之间留一点余量
    std::thread::sleep(Duration::from_secs(3));
    match send_line(&mut s, goto) {
        Ok(()) => emit(app, id, "sent", format!("已发送 {goto}")),
        Err(e) => emit(app, id, "failed", format!("发送失败: {e}")),
    }
}

/// 打开 CS2 播放这条集锦。
pub fn launch(db: &Arc<Mutex<Database>>, app: Option<AppHandle>, id: i64) -> Result<LaunchResult, String> {
    let info = prepare(db, id)?;
    let settings = repo::get_settings(&db.lock()).map_err(|e| e.to_string())?;
    let Some(demo) = info.demo_path.clone().filter(|_| info.status == "ready") else {
        let message = info.message.clone().unwrap_or_else(|| "demo 不可用".into());
        return Ok(LaunchResult { playback: info, method: "unavailable".into(), auto_goto: false, message });
    };
    let netcon = settings.netcon_enabled.then_some(settings.netcon_port);

    // CS2 已在运行且 netcon 端口可连：直接发命令
    if let Some(port) = netcon {
        if let Ok(mut s) = TcpStream::connect_timeout(&netcon_addr(port), Duration::from_millis(400)) {
            let same = LAST_NETCON_DEMO.lock().as_deref() == Some(demo.as_str());
            if same {
                send_line(&mut s, &info.goto_command).map_err(|e| format!("发送失败: {e}"))?;
                emit(app.as_ref(), id, "sent", format!("已发送 {}", info.goto_command));
                return Ok(LaunchResult {
                    playback: info,
                    method: "netcon".into(),
                    auto_goto: true,
                    message: "CS2 正在播放这个 demo，已直接跳转。".into(),
                });
            }
            let cmd = info.playdemo_command.clone().unwrap_or_default();
            send_line(&mut s, &cmd).map_err(|e| format!("发送失败: {e}"))?;
            *LAST_NETCON_DEMO.lock() = Some(demo.clone());
            let goto = info.goto_command.clone();
            std::thread::spawn(move || wait_loaded_and_goto(s, app.as_ref(), id, &goto));
            return Ok(LaunchResult {
                playback: info,
                method: "netcon".into(),
                auto_goto: true,
                message: "CS2 已在运行，已通过控制台端口打开 demo，加载完会自动跳转。".into(),
            });
        }
    }

    let args = launch_args(&demo, netcon, &settings.extra_launch_args);
    let method = match steam_exe() {
        Some(exe) => {
            let mut cmd = std::process::Command::new(&exe);
            cmd.arg("-applaunch").arg("730").args(&args);
            cmd.spawn().map_err(|e| format!("启动 Steam 失败: {e}"))?;
            log(format!("launch #{id}: {} -applaunch 730 {}", exe.display(), args.join(" ")));
            "steam_exe"
        }
        None => {
            let url = steam_url(&args);
            open::that(&url).map_err(|e| format!("打开 {url} 失败: {e}"))?;
            log(format!("launch #{id}: {url}"));
            "steam_url"
        }
    };

    if let Some(port) = netcon {
        *LAST_NETCON_DEMO.lock() = Some(demo.clone());
        let goto = info.goto_command.clone();
        std::thread::spawn(move || {
            emit(app.as_ref(), id, "connecting", format!("等待 CS2 打开控制台端口 {port}"));
            let started = Instant::now();
            while started.elapsed() < NETCON_CONNECT_TIMEOUT {
                if let Ok(s) = TcpStream::connect_timeout(&netcon_addr(port), Duration::from_millis(500)) {
                    emit(app.as_ref(), id, "connected", format!("已连接 127.0.0.1:{port}"));
                    wait_loaded_and_goto(s, app.as_ref(), id, &goto);
                    return;
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            emit(app.as_ref(), id, "failed", format!("{} 秒内没能连上端口 {port}，请手动粘贴跳转命令", NETCON_CONNECT_TIMEOUT.as_secs()));
        });
    }

    let message = if netcon.is_some() {
        "已通过 Steam 启动 CS2（实验：加载完成后会尝试自动跳转）。".to_string()
    } else {
        format!("已通过 Steam 启动 CS2。demo 加载后，在控制台（~）粘贴：{}", info.goto_command)
    };
    Ok(LaunchResult { playback: info, method: method.into(), auto_goto: netcon.is_some(), message })
}
