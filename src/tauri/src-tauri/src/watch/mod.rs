//! 监听 CS2 replays 目录（W3-C）：新出现的 `.dem` 等写完后自动**复制**导入，不动 CS2 自己的文件。
//!
//! - 目录：设置里手动指定的优先，否则自动检测 Steam 库（`detect`）。
//! - 只处理监听开始之后新出现 / 改名 / 写入的文件，不扫存量。
//! - 文件大小连续 `STABLE_FOR` 不变、且能以写权限打开（CS2 已关闭句柄）才导入。
//! - 目录暂时不存在（CS2 还没下载过 demo）时每 30 秒检查一次。

pub mod detect;

use crate::onboarding::settings;
use crate::AppState;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const STABLE_FOR: Duration = Duration::from_secs(3);
const TICK: Duration = Duration::from_millis(1000);
const BLOCKED_GRACE: Duration = Duration::from_secs(30);
const RETRY_MISSING_DIR: Duration = Duration::from_secs(30);

fn log(msg: impl AsRef<str>) {
    eprintln!("[watch] {}", msg.as_ref());
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WatchStatus {
    pub enabled: bool,
    /// 实际监听（或等待出现）的目录
    pub dir: Option<String>,
    /// manual | auto | none
    pub source: String,
    /// 自动检测到的目录（不管是否手动指定）
    pub detected_dir: Option<String>,
    pub dir_exists: bool,
    pub watching: bool,
    /// 本次启动以来自动导入的数量
    pub imported_count: u64,
    pub last_imported: Option<String>,
    pub last_error: Option<String>,
}

/// `replay_imported` 事件
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayImported {
    pub path: String,
    /// imported | skipped | error
    pub action: String,
    pub error: Option<String>,
}

pub fn is_demo_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("dem"))
}

/// 等文件写完：记录每个候选文件最近一次大小变化的时间。
#[derive(Default)]
pub struct Pending {
    files: HashMap<PathBuf, (u64, Instant)>,
}

impl Pending {
    pub fn touch(&mut self, path: PathBuf, now: Instant) {
        self.files.insert(path, (u64::MAX, now));
    }

    /// 按当前大小更新；返回大小已经稳定 `STABLE_FOR` 的文件（并从候选里移除）。
    /// `size_of` 返回 None 表示文件已经不见了。
    pub fn take_ready(&mut self, now: Instant, mut size_of: impl FnMut(&Path) -> Option<u64>) -> Vec<PathBuf> {
        let mut ready = Vec::new();
        self.files.retain(|path, (size, since)| match size_of(path) {
            None => false,
            Some(cur) if cur != *size => {
                *size = cur;
                *since = now;
                true
            }
            Some(cur) => {
                if cur > 0 && now.duration_since(*since) >= STABLE_FOR {
                    ready.push(path.clone());
                    false
                } else {
                    true
                }
            }
        });
        ready.sort();
        ready
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// CS2 还在写的话会独占写句柄，这里以写权限打开会失败（不会修改文件）。
fn writable_now(path: &Path) -> bool {
    std::fs::OpenOptions::new().append(true).open(path).is_ok()
}

pub struct ReplayWatcher {
    watcher: Mutex<Option<RecommendedWatcher>>,
    status: Mutex<WatchStatus>,
    tx: Sender<PathBuf>,
}

impl ReplayWatcher {
    pub fn status(&self) -> WatchStatus {
        self.status.lock().clone()
    }

    /// 按当前设置重新建立监听。设置变化后调用。
    pub fn reconfigure(&self, app: &AppHandle) {
        let Some(state) = app.try_state::<AppState>() else { return };
        let s = settings::get(&state.db.lock().conn).unwrap_or_default();
        let detected = detect::detect_replays_dir();
        let manual = s.replays_dir.as_deref().map(PathBuf::from);
        let (dir, source) = match (&manual, &detected) {
            (Some(m), _) => (Some(m.clone()), "manual"),
            (None, Some(d)) => (Some(d.clone()), "auto"),
            (None, None) => (None, "none"),
        };

        // 先停掉旧的
        *self.watcher.lock() = None;

        let mut st = self.status.lock();
        st.enabled = s.watch_replays;
        st.dir = dir.as_ref().map(|d| d.to_string_lossy().to_string());
        st.source = source.into();
        st.detected_dir = detected.as_ref().map(|d| d.to_string_lossy().to_string());
        st.dir_exists = dir.as_ref().is_some_and(|d| d.is_dir());
        st.watching = false;
        st.last_error = None;

        if !s.watch_replays {
            drop(st);
            let _ = app.emit("watch_status", self.status());
            return;
        }
        if let Some(dir) = dir.filter(|d| d.is_dir()) {
            let tx = self.tx.clone();
            let result = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                let Ok(event) = res else { return };
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for p in event.paths {
                        if is_demo_file(&p) {
                            let _ = tx.send(p);
                        }
                    }
                }
            })
            .and_then(|mut w| w.watch(&dir, RecursiveMode::NonRecursive).map(|_| w));
            match result {
                Ok(w) => {
                    log(format!("watching {} ({source})", dir.display()));
                    *self.watcher.lock() = Some(w);
                    st.watching = true;
                }
                Err(e) => {
                    log(format!("watch {} failed: {e}", dir.display()));
                    st.last_error = Some(format!("无法监听目录：{e}"));
                }
            }
        }
        drop(st);
        let _ = app.emit("watch_status", self.status());
    }
}

pub fn start(app: &AppHandle) {
    let (tx, rx) = channel::<PathBuf>();
    let watcher = Arc::new(ReplayWatcher { watcher: Mutex::new(None), status: Mutex::new(WatchStatus::default()), tx });
    app.manage(watcher.clone());
    watcher.reconfigure(app);
    let handle = app.clone();
    std::thread::Builder::new()
        .name("replay-watch".into())
        .spawn(move || run(handle, watcher, rx))
        .expect("spawn replay watcher");
}

fn run(app: AppHandle, watcher: Arc<ReplayWatcher>, rx: Receiver<PathBuf>) {
    let mut pending = Pending::default();
    // (路径, 大小) 已处理过的不再导入：导入完成后 CS2 或杀毒软件再碰一下文件也不会重复
    let mut handled: HashSet<(PathBuf, u64)> = HashSet::new();
    // 一直不能以写权限打开（例如目录只读）的文件，稳定足够久后也照样导入
    let mut blocked_since: HashMap<PathBuf, Instant> = HashMap::new();
    let mut last_dir_check = Instant::now();
    loop {
        match rx.recv_timeout(TICK) {
            Ok(path) => pending.touch(path, Instant::now()),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        while let Ok(path) = rx.try_recv() {
            pending.touch(path, Instant::now());
        }

        // 目录还没出现（或被删掉后重建）：定期重试
        if last_dir_check.elapsed() >= RETRY_MISSING_DIR {
            last_dir_check = Instant::now();
            let st = watcher.status();
            let exists_now = st.dir.as_deref().is_some_and(|d| Path::new(d).is_dir());
            if st.enabled && (!st.watching && exists_now || st.watching && !exists_now) {
                watcher.reconfigure(&app);
            }
        }

        if pending.is_empty() {
            continue;
        }
        let ready = pending.take_ready(Instant::now(), |p| std::fs::metadata(p).ok().map(|m| m.len()));
        for path in ready {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            if handled.contains(&(path.clone(), size)) {
                continue;
            }
            if !writable_now(&path) {
                let since = *blocked_since.entry(path.clone()).or_insert_with(Instant::now);
                if since.elapsed() < BLOCKED_GRACE {
                    // 还被占用：放回去再等
                    pending.touch(path, Instant::now());
                    continue;
                }
            }
            blocked_since.remove(&path);
            if !watcher.status().enabled {
                continue;
            }
            handled.insert((path.clone(), size));
            import_one(&app, &watcher, &path);
        }
    }
}

fn import_one(app: &AppHandle, watcher: &ReplayWatcher, path: &Path) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let path_str = path.to_string_lossy().to_string();
    log(format!("import (copy) {path_str}"));
    let result = state.orchestrator.import_demo_opts(app, &path_str, true);
    let event = match result {
        Ok(action) => ReplayImported { path: path_str.clone(), action, error: None },
        Err(e) => ReplayImported { path: path_str.clone(), action: "error".into(), error: Some(e.to_string()) },
    };
    log(format!("{} {}", event.action, path_str));
    {
        let mut st = watcher.status.lock();
        match event.action.as_str() {
            "imported" => {
                // 元数据解析（sidecar）失败时导入不会排队解析；这里兜底把未解析的对局排上
                state.analysis.enqueue_backlog();
                st.imported_count += 1;
                st.last_imported = Some(path_str);
            }
            "error" => st.last_error = event.error.clone(),
            _ => {}
        }
    }
    let _ = app.emit("replay_imported", &event);
    let _ = app.emit("watch_status", watcher.status());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_extension() {
        assert!(is_demo_file(Path::new("C:/x/match730_0031.dem")));
        assert!(is_demo_file(Path::new("C:/x/A.DEM")));
        assert!(!is_demo_file(Path::new("C:/x/a.dem.tmp")));
        assert!(!is_demo_file(Path::new("C:/x/a.dem.bz2")));
    }

    #[test]
    fn waits_until_size_is_stable() {
        let t0 = Instant::now();
        let p = PathBuf::from("a.dem");
        let mut pending = Pending::default();
        pending.touch(p.clone(), t0);
        let mut size = 100u64;
        // 第一次看到大小：记录，不导入
        assert!(pending.take_ready(t0, |_| Some(size)).is_empty());
        // 还在长
        size = 200;
        assert!(pending.take_ready(t0 + Duration::from_secs(2), |_| Some(size)).is_empty());
        // 不到 3 秒
        assert!(pending.take_ready(t0 + Duration::from_secs(4), |_| Some(size)).is_empty());
        // 稳定 3 秒 → 导入，并移出候选
        assert_eq!(pending.take_ready(t0 + Duration::from_secs(5), |_| Some(size)), vec![p.clone()]);
        assert!(pending.is_empty());
    }

    #[test]
    fn drops_vanished_and_ignores_empty() {
        let t0 = Instant::now();
        let mut pending = Pending::default();
        pending.touch(PathBuf::from("gone.dem"), t0);
        pending.touch(PathBuf::from("empty.dem"), t0);
        let size_of = |p: &Path| if p.ends_with("gone.dem") { None } else { Some(0) };
        assert!(pending.take_ready(t0, size_of).is_empty());
        assert!(pending.take_ready(t0 + Duration::from_secs(10), size_of).is_empty());
        assert!(!pending.is_empty(), "0 字节的文件继续等");
    }
}
