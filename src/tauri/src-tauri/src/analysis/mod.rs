//! Background demo analysis: completed downloads/imports → demo-parse → SQLite + replay file.
//!
//! One worker thread drains a de-duplicated FIFO. Parsing happens outside the DB lock; only the
//! final ingest (one transaction) holds it. The frontend is notified via `analysis_updated`.

use crate::database::{analysis as repo, Database};
use parking_lot::{Condvar, Mutex};
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

#[derive(Default)]
struct State {
    queue: VecDeque<String>,
    queued: HashSet<String>,
    running: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisQueueStatus {
    pub queued: usize,
    pub running: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisUpdated<'a> {
    match_id: &'a str,
    status: &'a str,
    error: Option<&'a str>,
    queued: usize,
}

pub struct AnalysisQueue {
    db: Arc<Mutex<Database>>,
    state: Mutex<State>,
    wake: Condvar,
    app: OnceLock<AppHandle>,
}

fn log(msg: impl AsRef<str>) {
    eprintln!("[analysis] {}", msg.as_ref());
}

impl AnalysisQueue {
    pub fn new(db: Arc<Mutex<Database>>) -> Arc<Self> {
        Arc::new(Self { db, state: Mutex::new(State::default()), wake: Condvar::new(), app: OnceLock::new() })
    }

    /// Start the worker thread. Call once from app setup.
    pub fn start(self: &Arc<Self>, app: AppHandle) {
        if self.app.set(app).is_err() {
            return;
        }
        let me = Arc::clone(self);
        std::thread::Builder::new()
            .name("demo-analysis".into())
            .spawn(move || me.worker_loop())
            .expect("spawn analysis worker");
    }

    /// Queue one match. No-op if it is already queued or running.
    pub fn enqueue(&self, match_id: &str) {
        self.enqueue_many(std::iter::once(match_id.to_string()), false);
    }

    /// Queue one match unless it already has a current analysis (download / import / metadata
    /// refresh all pass through here; metadata refresh runs on every startup).
    pub fn enqueue_if_needed(&self, match_id: &str) {
        let needed = repo::needs_analysis(&self.db.lock(), match_id).unwrap_or(true);
        if needed {
            self.enqueue(match_id);
        }
    }

    /// Queue matches; `reset_attempts` clears the failure counter (manual re-analysis).
    pub fn enqueue_many(&self, ids: impl IntoIterator<Item = String>, reset_attempts: bool) -> usize {
        let mut added = vec![];
        {
            let mut st = self.state.lock();
            for id in ids {
                if st.running.as_deref() == Some(id.as_str()) || !st.queued.insert(id.clone()) {
                    continue;
                }
                st.queue.push_back(id.clone());
                added.push(id);
            }
        }
        if added.is_empty() {
            return 0;
        }
        {
            let db = self.db.lock();
            for id in &added {
                let _ = repo::mark_analysis_pending(&db, id, reset_attempts);
            }
        }
        self.wake.notify_one();
        self.emit_queue_changed();
        added.len()
    }

    /// Queue everything that still needs analysis (startup, after metadata refresh).
    pub fn enqueue_backlog(&self) -> usize {
        let ids = {
            let db = self.db.lock();
            repo::list_matches_needing_analysis(&db).unwrap_or_default()
        };
        let n = self.enqueue_many(ids, false);
        if n > 0 {
            log(format!("queued {n} match(es) from backlog"));
        }
        n
    }

    /// Re-analyze the given matches, or every analyzable match when `ids` is `None`.
    pub fn reanalyze(&self, ids: Option<Vec<String>>) -> usize {
        let ids = match ids {
            Some(ids) => ids,
            None => {
                let db = self.db.lock();
                repo::list_analyzable_matches(&db).unwrap_or_default()
            }
        };
        self.enqueue_many(ids, true)
    }

    pub fn status(&self) -> AnalysisQueueStatus {
        let st = self.state.lock();
        AnalysisQueueStatus { queued: st.queue.len(), running: st.running.clone() }
    }

    fn worker_loop(self: Arc<Self>) {
        loop {
            let id = {
                let mut st = self.state.lock();
                while st.queue.is_empty() {
                    self.wake.wait(&mut st);
                }
                let id = st.queue.pop_front().unwrap();
                st.queued.remove(&id);
                st.running = Some(id.clone());
                id
            };
            self.run_one(&id);
            self.state.lock().running = None;
            self.emit_queue_changed();
        }
    }

    fn run_one(&self, id: &str) {
        let (path, data_root) = {
            let db = self.db.lock();
            let path = crate::database::repo::get_match_abs_path(&db, id).ok().flatten();
            if path.is_some() {
                let _ = repo::mark_analysis_running(&db, id);
            }
            (path, db.data_root.clone())
        };
        let Some(path) = path else {
            // Match deleted or file detached since it was queued (UPDATE is a no-op if deleted).
            let _ = repo::mark_analysis_failed(&self.db.lock(), id, "没有关联的 demo 文件");
            return;
        };
        self.emit(id, "running", None);

        let started = Instant::now();
        let result = analyze_file(&path, &data_root, id);
        let elapsed_ms = started.elapsed().as_millis() as i64;

        let db = self.db.lock();
        match result {
            Ok((demo, replay_rel)) => {
                match repo::replace_match_analysis(&db, id, &demo, replay_rel.as_deref(), elapsed_ms) {
                    Ok(()) => {
                        crate::identity::on_match_analyzed(&db, id);
                        drop(db);
                        log(format!(
                            "done {id}: {} rounds, {} kills, {} shots in {elapsed_ms} ms",
                            demo.rounds.len(),
                            demo.kills.len(),
                            demo.shots.len()
                        ));
                        self.emit(id, "done", None);
                    }
                    Err(e) => {
                        let msg = format!("写入数据库失败: {e}");
                        let _ = repo::mark_analysis_failed(&db, id, &msg);
                        drop(db);
                        log(format!("failed {id}: {msg}"));
                        self.emit(id, "failed", Some(&msg));
                    }
                }
            }
            Err(msg) => {
                let _ = repo::mark_analysis_failed(&db, id, &msg);
                drop(db);
                log(format!("failed {id}: {msg}"));
                self.emit(id, "failed", Some(&msg));
            }
        }
    }

    fn emit(&self, match_id: &str, status: &str, error: Option<&str>) {
        if let Some(app) = self.app.get() {
            let queued = self.state.lock().queue.len();
            let _ = app.emit("analysis_updated", AnalysisUpdated { match_id, status, error, queued });
        }
    }

    fn emit_queue_changed(&self) {
        if let Some(app) = self.app.get() {
            let _ = app.emit("analysis_queue", self.status());
        }
    }
}

/// Parse a demo and write its replay file. Returns the parsed demo and the replay path relative
/// to `data_root`. Parser panics (malformed demos) are caught and reported as errors.
fn analyze_file(path: &Path, data_root: &Path, id: &str) -> Result<(demo_parse::ParsedDemo, Option<String>), String> {
    if !path.exists() {
        return Err(format!("demo 文件不存在: {}", path.display()));
    }
    let parsed = std::panic::catch_unwind(|| demo_parse::parse_demo_file(path))
        .map_err(|p| {
            let what = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".into());
            format!("解析器崩溃: {what}")
        })?
        .map_err(|e| format!("解析失败: {e}"))?;

    let rel = format!("analysis/{}.replay.zst", sanitize(id));
    let replay_rel = match parsed.replay.write_to(&data_root.join(&rel)) {
        Ok(_) => Some(rel),
        Err(e) => {
            // Replay is optional; keep the event data.
            log(format!("replay write failed {id}: {e}"));
            None
        }
    };
    Ok((parsed, replay_rel))
}

fn sanitize(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
