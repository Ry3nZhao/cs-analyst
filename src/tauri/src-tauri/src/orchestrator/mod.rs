use crate::database::models::{BackgroundTask, DiscoveredMatchPayload, MetadataRefreshResult, MetadataRefreshSummary, ParsedDemMetadata};
use crate::analysis::AnalysisQueue;
use crate::database::repo;
use crate::database::Database;
use crate::sidecar::SidecarManager;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;

fn orch_log(message: impl AsRef<str>) {
    eprintln!("[discover] {}", message.as_ref());
}

fn valve_channel_from_metadata(metadata: &Value) -> &'static str {
    match metadata.get("source").and_then(|v| v.as_str()) {
        Some("auth_code") => "authcode",
        _ => "local",
    }
}

fn detect_platform_from_path(path: &Path) -> &'static str {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if is_valve_gcpd_filename(name) {
        "valve"
    } else {
        "imported"
    }
}

fn is_valve_gcpd_filename(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    if !lower.ends_with(".dem") {
        return false;
    }
    let stem = lower.trim_end_matches(".dem");
    let parts: Vec<&str> = stem.split('_').collect();
    // match, gameId, partA, partB, regionId [, gameMode]
    if parts.len() < 5 || parts.len() > 6 || parts[0] != "match" {
        return false;
    }
    parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].chars().all(|c| c.is_ascii_digit())
        && parts[3].chars().all(|c| c.is_ascii_digit())
        && parts[4].chars().all(|c| c.is_ascii_digit())
}

fn move_demo_into_place(source: &Path, dest: &Path) -> Result<(), OrchestratorError> {
    if source == dest {
        return Ok(());
    }
    match fs::rename(source, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(source, dest)
                .map_err(|e| OrchestratorError::Message(format!("移动文件失败: {e}")))?;
            fs::remove_file(source)
                .map_err(|e| OrchestratorError::Message(format!("删除源文件失败: {e}")))?;
            Ok(())
        }
    }
}

fn paths_same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn is_valve_share_match_id(value: &str) -> bool {
    let len = value.len();
    (10..=20).contains(&len) && value.chars().all(|c| c.is_ascii_digit())
}

fn resolve_reclassify_match_id(
    parsed: Option<&str>,
    stored: Option<String>,
) -> Option<String> {
    if let Some(id) = parsed.filter(|s| !s.is_empty()) {
        if is_valve_share_match_id(id) {
            return Some(id.to_string());
        }
    }
    if let Some(id) = stored.as_deref().filter(|s| !s.is_empty()) {
        if is_valve_share_match_id(id) {
            return Some(id.to_string());
        }
    }
    parsed
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or(stored.filter(|s| !s.is_empty()))
}

enum MetadataReclassifyOutcome {
    None,
    Reclassified { id: String },
    Merged { target_id: String },
}

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("db: {0}")]
    Db(#[from] crate::database::DbError),
    #[error("sidecar: {0}")]
    Sidecar(#[from] crate::sidecar::SidecarError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub struct DownloadOrchestrator {
    db: Arc<Mutex<Database>>,
    sidecar: Arc<SidecarManager>,
    analysis: Arc<AnalysisQueue>,
    queue_paused: Mutex<bool>,
    last_discover: Mutex<Option<std::time::Instant>>,
    active_task: Mutex<Option<BackgroundTask>>,
    task_logs: Mutex<VecDeque<String>>,
}

impl DownloadOrchestrator {
    pub fn new(db: Arc<Mutex<Database>>, sidecar: Arc<SidecarManager>, analysis: Arc<AnalysisQueue>) -> Self {
        Self {
            db,
            sidecar,
            analysis,
            queue_paused: Mutex::new(true),
            last_discover: Mutex::new(None),
            active_task: Mutex::new(None),
            task_logs: Mutex::new(VecDeque::new()),
        }
    }

    pub fn wire_sidecar_events(self: &Arc<Self>, app: AppHandle) {
        let orchestrator = Arc::clone(self);
        self.sidecar.set_event_handler(Arc::new(move |event| {
            orchestrator.handle_sidecar_event(&app, event);
        }));
    }

    fn handle_sidecar_event(&self, app: &AppHandle, event: Value) {
        let Some(event_type) = event.get("event").and_then(|v| v.as_str()) else {
            return;
        };

        match event_type {
            "resolve.progress" => {
                let Some(job_id) = event.get("jobId").and_then(|v| v.as_str()) else {
                    return;
                };
                let phase = event.get("phase").and_then(|v| v.as_str()).unwrap_or("");
                let db = self.db.lock();
                let result = match phase {
                    "resolved" => {
                        let demo_url = event.get("demoUrl").and_then(|v| v.as_str()).unwrap_or("");
                        if demo_url.is_empty() {
                            repo::mark_match_expired(&db, job_id, "URL_EXPIRED")
                        } else {
                            repo::update_match_resolve_result(
                                &db,
                                job_id,
                                demo_url,
                                event.get("playedAt").and_then(|v| v.as_str()),
                                event.get("mapName").and_then(|v| v.as_str()),
                            )
                        }
                    }
                    "failed" => {
                        let error = event
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("resolve failed");
                        repo::mark_match_expired(&db, job_id, error)
                    }
                    _ => Ok(()),
                };
                drop(db);
                if result.is_ok() && phase != "started" {
                    let _ = app.emit("jobs_updated", ());
                }
            }
            "download.progress" => {
                let Some(job_id) = event.get("jobId").and_then(|v| v.as_str()) else {
                    return;
                };
                let phase = event.get("phase").and_then(|v| v.as_str()).unwrap_or("");
                let progress = event.get("progress").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let db = self.db.lock();
                let result = match phase {
                    "downloading" => {
                        let _ = repo::update_job_status(&db, job_id, "downloading", Some(progress), None);
                        repo::update_job_progress(&db, job_id, progress)
                    }
                    "extracting" => repo::update_job_progress(&db, job_id, progress),
                    "done" => repo::update_job_progress(&db, job_id, 1.0),
                    _ => Ok(()),
                };
                drop(db);
                if result.is_ok() {
                    let _ = app.emit(
                        "job_progress",
                        json!({ "jobId": job_id, "progress": progress }),
                    );
                    let _ = app.emit("jobs_updated", ());
                }
            }
            _ => {}
        }
    }

    fn resolve_valve_authcode_urls(&self, app: &AppHandle) -> Result<(), OrchestratorError> {
        let pending = {
            let db = self.db.lock();
            repo::list_valve_authcode_pending_resolve(&db)?
        };
        if pending.is_empty() {
            return Ok(());
        }

        let bot = {
            let db = self.db.lock();
            repo::get_valve_bot_secrets(&db)?
        };
        let username = bot
            .username
            .clone()
            .filter(|u| !u.is_empty())
            .ok_or_else(|| OrchestratorError::Message("Valve bot 未配置".into()))?;

        let job_sub: HashMap<String, Option<String>> = pending
            .iter()
            .map(|(id, _, _, sub_id)| (id.clone(), sub_id.clone()))
            .collect();

        let items: Vec<Value> = pending
            .iter()
            .map(|(id, match_id, share_code, _)| {
                json!({
                    "jobId": id,
                    "matchId": match_id,
                    "shareCode": share_code,
                })
            })
            .collect();

        self.append_log(
            app,
            format!("resolve_valve_authcode_urls: {} item(s)", items.len()),
        );

        let params = json!({
            "bot": {
                "username": username,
                "password": bot.password,
                "secret": bot.secret,
                "refreshToken": bot.refresh_token,
            },
            "items": items,
        });

        let result = self.sidecar.call("resolve_valve_authcode_urls", params)?;

        if let Some(token) = result.get("refreshToken").and_then(|v| v.as_str()) {
            if !token.is_empty() {
                let db = self.db.lock();
                repo::update_valve_bot_refresh_token(&db, token)?;
            }
        }

        if let Some(resolved) = result.get("resolved").and_then(|v| v.as_array()) {
            let mut last_share_by_sub: HashMap<String, String> = HashMap::new();
            for item in resolved {
                let job_id = item.get("jobId").and_then(|v| v.as_str()).unwrap_or("");
                let share_code = item.get("shareCode").and_then(|v| v.as_str()).unwrap_or("");
                let has_url = item.get("demoUrl").and_then(|v| v.as_str()).is_some_and(|u| !u.is_empty());
                let has_error = item.get("error").and_then(|v| v.as_str()).is_some();
                if !has_url || has_error || job_id.is_empty() || share_code.is_empty() {
                    continue;
                }
                if let Some(sub_id) = job_sub.get(job_id).and_then(|s| s.clone()) {
                        last_share_by_sub.insert(sub_id, share_code.to_string());
                    }
            }
            let db = self.db.lock();
            for (sub_id, share_code) in last_share_by_sub {
                repo::upsert_sync_cursor(&db, &sub_id, "share_code", &share_code)?;
            }
        }

        let _ = app.emit("jobs_updated", ());
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        *self.queue_paused.lock()
    }

    pub fn toggle_pause(&self) -> bool {
        let mut p = self.queue_paused.lock();
        *p = !*p;
        *p
    }

    pub fn active_task(&self) -> Option<BackgroundTask> {
        self.active_task.lock().clone()
    }

    pub fn task_logs(&self) -> Vec<String> {
        self.task_logs.lock().iter().cloned().collect()
    }

    fn append_log(&self, app: &AppHandle, message: impl AsRef<str>) {
        let message = message.as_ref().to_string();
        orch_log(&message);
        {
            let mut logs = self.task_logs.lock();
            let line = format!(
                "{} {message}",
                chrono::Local::now().format("%H:%M:%S")
            );
            logs.push_back(line.clone());
            while logs.len() > 400 {
                logs.pop_front();
            }
            let _ = app.emit("background_task_log", line);
        }
    }

    fn set_task(
        &self,
        app: &AppHandle,
        kind: &str,
        label: &str,
        status: &str,
        progress: f64,
        current: Option<String>,
    ) {
        let task = BackgroundTask {
            id: kind.to_string(),
            kind: kind.to_string(),
            label: label.to_string(),
            status: status.to_string(),
            progress: progress.clamp(0.0, 1.0),
            current,
        };
        *self.active_task.lock() = Some(task.clone());
        let _ = app.emit("background_task_updated", task);
    }

    fn clear_task(&self, app: &AppHandle) {
        *self.active_task.lock() = None;
        let _ = app.emit("background_task_updated", serde_json::Value::Null);
    }

    pub fn start_discover_background(self: &Arc<Self>, app: AppHandle, label: &'static str) -> bool {
        if self.active_task.lock().is_some() {
            return false;
        }
        let orchestrator = Arc::clone(self);
        orchestrator.set_task(&app, "discover", label, "running", 0.0, Some("准备开始".into()));
        std::thread::spawn(move || {
            if let Err(err) = orchestrator.discover_all(&app) {
                orchestrator.append_log(&app, format!("{label}失败: {err}"));
                orchestrator.set_task(&app, "discover", label, "failed", 1.0, Some(err.to_string()));
            } else {
                orchestrator.set_task(&app, "discover", label, "completed", 1.0, Some("完成".into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(800));
            orchestrator.clear_task(&app);
        });
        true
    }

    pub fn start_retry_background(self: &Arc<Self>, job_id: String, app: AppHandle) -> bool {
        if self.active_task.lock().is_some() {
            return false;
        }
        let orchestrator = Arc::clone(self);
        orchestrator.set_task(&app, "download", "重试下载", "running", 0.0, Some(job_id.clone()));
        std::thread::spawn(move || {
            if let Err(err) = orchestrator.retry_job(&job_id, &app) {
                orchestrator.append_log(&app, format!("重试任务 {job_id} 失败: {err}"));
                orchestrator.set_task(&app, "download", "重试下载", "failed", 1.0, Some(err.to_string()));
            } else {
                orchestrator.set_task(&app, "download", "重试下载", "completed", 1.0, Some("完成".into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(800));
            orchestrator.clear_task(&app);
        });
        true
    }

    pub fn start_import_background(self: &Arc<Self>, file_paths: Vec<String>, app: AppHandle) -> bool {
        if self.active_task.lock().is_some() {
            return false;
        }
        let orchestrator = Arc::clone(self);
        let total = file_paths.len();
        orchestrator.set_task(&app, "import", "导入对局", "running", 0.0, Some(format!("共 {total} 个文件")));
        std::thread::spawn(move || {
            use crate::database::models::{ImportResult, ImportSummary};
            let mut imported = 0usize;
            let mut skipped = 0usize;
            let mut results = Vec::new();

            for (i, path) in file_paths.iter().enumerate() {
                let short_name = path.split(['/', '\\']).last().unwrap_or(path.as_str()).to_string();
                orchestrator.set_task(
                    &app, "import", "导入对局", "running",
                    i as f64 / total as f64,
                    Some(format!("{}/{} {}", i + 1, total, short_name)),
                );
                match orchestrator.import_demo(&app, path) {
                    Ok(action) => {
                        if action == "skipped" { skipped += 1; } else { imported += 1; }
                        results.push(ImportResult { path: path.clone(), action, error: None });
                    }
                    Err(e) => {
                        results.push(ImportResult { path: path.clone(), action: "error".into(), error: Some(e.to_string()) });
                    }
                }
            }

            let summary = ImportSummary { imported, skipped, results };
            let _ = app.emit("import_completed", &summary);
            orchestrator.set_task(&app, "import", "导入对局", "completed", 1.0,
                Some(format!("已导入 {imported} 个，跳过 {skipped} 个")));
            std::thread::sleep(std::time::Duration::from_millis(800));
            orchestrator.clear_task(&app);
        });
        true
    }

    pub fn start_refresh_metadata_background(self: &Arc<Self>, app: AppHandle) -> bool {
        if self.active_task.lock().is_some() {
            return false;
        }
        let orchestrator = Arc::clone(self);
        orchestrator.set_task(&app, "metadata", "更新元数据", "running", 0.0, Some("准备开始".into()));
        std::thread::spawn(move || {
            match orchestrator.refresh_all_metadata(&app) {
                Ok(summary) => {
                    let _ = app.emit("metadata_refresh_completed", &summary);
                    // 元数据刷新可能新增 / 合并对局，补齐尚未解析的
                    orchestrator.analysis.enqueue_backlog();
                    orchestrator.set_task(
                        &app,
                        "metadata",
                        "更新元数据",
                        "completed",
                        1.0,
                        Some(format!(
                            "已更新 {} 个，合并 {} 个，跳过 {} 个，失败 {} 个",
                            summary.updated, summary.merged, summary.skipped, summary.failed
                        )),
                    );
                }
                Err(err) => {
                    orchestrator.append_log(&app, format!("更新元数据失败: {err}"));
                    orchestrator.set_task(
                        &app,
                        "metadata",
                        "更新元数据",
                        "failed",
                        1.0,
                        Some(err.to_string()),
                    );
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(800));
            orchestrator.clear_task(&app);
        });
        true
    }

    fn parse_dem_metadata(&self, abs_path: &Path) -> Result<ParsedDemMetadata, OrchestratorError> {
        let header: serde_json::Value = self
            .sidecar
            .call(
                "analyze_dem",
                json!({ "filePath": abs_path.to_string_lossy() }),
            )
            .map_err(|e| OrchestratorError::Message(e.to_string()))?;

        fn opt_str(v: &serde_json::Value, key: &str) -> Option<String> {
            v.get(key)
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        }
        fn opt_i64(v: &serde_json::Value, key: &str) -> Option<i64> {
            v.get(key).and_then(|x| x.as_i64())
        }
        fn opt_f64(v: &serde_json::Value, key: &str) -> Option<f64> {
            v.get(key).and_then(|x| x.as_f64())
        }
        fn opt_bool(v: &serde_json::Value, key: &str) -> Option<bool> {
            v.get(key).and_then(|x| x.as_bool())
        }

        let checksum = opt_str(&header, "sha256");
        let is_valid = header.get("isValid").and_then(|v| v.as_bool()).unwrap_or(false);
        let dem_game_info = header.get("demGameInfo").cloned().filter(|v| !v.is_null());

        Ok(ParsedDemMetadata {
            match_id: opt_str(&header, "matchId"),
            map_name: opt_str(&header, "mapName"),
            duration_seconds: opt_i64(&header, "durationSeconds"),
            game_mode: opt_str(&header, "gameMode"),
            game_type: opt_i64(&header, "gameType"),
            played_at: opt_str(&header, "playedAt"),
            checksum,
            is_valid,
            demo_file_stamp: opt_str(&header, "demoFileStamp"),
            network_protocol: opt_i64(&header, "networkProtocol"),
            server_name: opt_str(&header, "serverName"),
            client_name: opt_str(&header, "clientName"),
            game_directory: opt_str(&header, "gameDirectory"),
            fullpackets_version: opt_i64(&header, "fullpacketsVersion"),
            allow_clientside_entities: opt_bool(&header, "allowClientsideEntities"),
            demo_header_field_9: opt_i64(&header, "demoHeaderField9"),
            demo_header_field_10: opt_str(&header, "demoHeaderField10"),
            demo_version_name: opt_str(&header, "demoVersionName"),
            demo_version_guid: opt_str(&header, "demoVersionGuid"),
            playback_time: opt_f64(&header, "playbackTime"),
            playback_ticks: opt_i64(&header, "playbackTicks"),
            playback_frames: opt_i64(&header, "playbackFrames"),
            dem_game_info,
            detected_platform: opt_str(&header, "detectedPlatform"),
        })
    }

    fn apply_dem_metadata(
        &self,
        app: &AppHandle,
        match_db_id: &str,
        abs_path: &Path,
        overwrite: bool,
    ) -> Result<bool, OrchestratorError> {
        let file_size = fs::metadata(abs_path).map(|m| m.len() as i64).unwrap_or(0);
        let file_name = abs_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown.dem".into());
        let rel_path = {
            let db = self.db.lock();
            db.relative_path(abs_path)
        };

        {
            let db = self.db.lock();
            repo::update_match_file(
                &db,
                match_db_id,
                &rel_path,
                &file_name,
                file_size,
                None,
            )?;
        }

        let metadata = match self.parse_dem_metadata(abs_path) {
            Ok(meta) => meta,
            Err(err) => {
                self.append_log(app, format!("metadata parse failed match={match_db_id}: {err}"));
                return Ok(false);
            }
        };

        if !metadata.is_valid {
            self.append_log(app, format!("metadata invalid match={match_db_id}"));
            return Ok(false);
        }

        {
            let db = self.db.lock();
            repo::update_match_dem_metadata(
                &db,
                match_db_id,
                &metadata,
                &rel_path,
                &file_name,
                file_size,
                overwrite,
            )?;
        }
        // 文件头有效 → 排队做完整解析（下载完成、导入、元数据刷新都会走到这里；已是最新解析的跳过）
        self.analysis.enqueue_if_needed(match_db_id);
        Ok(true)
    }

    fn maybe_reclassify_or_merge_imported(
        &self,
        app: &AppHandle,
        current_id: &str,
        abs_path: &Path,
        metadata: &ParsedDemMetadata,
    ) -> Result<MetadataReclassifyOutcome, OrchestratorError> {
        let db = self.db.lock();
        let current_platform = repo::get_match_platform(&db, current_id)?
            .unwrap_or_default();
        if current_platform != "imported" {
            return Ok(MetadataReclassifyOutcome::None);
        }

        let detected = metadata
            .detected_platform
            .as_deref()
            .unwrap_or_else(|| detect_platform_from_path(abs_path));
        if detected != "valve" {
            return Ok(MetadataReclassifyOutcome::None);
        }

        let stored_match_id = repo::get_match_stored_match_id(&db, current_id)?;
        let match_id = match resolve_reclassify_match_id(
            metadata.match_id.as_deref(),
            stored_match_id,
        ) {
            Some(id) => id,
            None => {
                self.append_log(
                    app,
                    format!("metadata reclassify skip match={current_id}: no match_id"),
                );
                return Ok(MetadataReclassifyOutcome::None);
            }
        };

        let merge_match_id = is_valve_share_match_id(&match_id).then(|| match_id.as_str());

        if let Some(target_id) = merge_match_id.and_then(|mid| {
            repo::find_match_id_by_platform_excluding(&db, "valve", mid, current_id)
                .ok()
                .flatten()
        }) {
            drop(db);
            let file_name = abs_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("{match_id}.dem"));
            let file_size = fs::metadata(abs_path).map(|m| m.len() as i64).unwrap_or(0);
            let checksum = metadata.checksum.as_deref();

            let (rel_path, _dest_abs) = {
                let db = self.db.lock();
                let dest_dir = db.ensure_demo_dir("valve")?;
                let dest_path = dest_dir.join(&file_name);
                if dest_path.exists() && !paths_same_file(abs_path, &dest_path) {
                    let accepts = repo::target_accepts_imported_file(&db, &target_id)?;
                    if !accepts {
                        self.append_log(
                            app,
                            format!(
                                "metadata merge skip source={current_id} target={target_id}: target already has file"
                            ),
                        );
                        return Ok(MetadataReclassifyOutcome::None);
                    }
                }
                if !paths_same_file(abs_path, &dest_path) {
                    move_demo_into_place(abs_path, &dest_path)?;
                }
                let rel = db.relative_path(&dest_path);
                (rel, dest_path)
            };

            {
                let db = self.db.lock();
                if repo::target_accepts_imported_file(&db, &target_id)? {
                    repo::attach_file_to_match(
                        &db,
                        &target_id,
                        &rel_path,
                        &file_name,
                        file_size,
                        checksum,
                    )?;
                }
                repo::delete_match_by_id(&db, current_id)?;
            }

            self.append_log(
                app,
                format!("metadata merged source={current_id} -> target={target_id} match={match_id}"),
            );
            return Ok(MetadataReclassifyOutcome::Merged { target_id });
        }

        drop(db);

        let file_name = abs_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{match_id}.dem"));
        let file_size = fs::metadata(abs_path).map(|m| m.len() as i64).unwrap_or(0);
        let checksum = metadata.checksum.clone();

        let rel_path = {
            let db = self.db.lock();
            let dest_dir = db.ensure_demo_dir("valve")?;
            let dest_path = dest_dir.join(&file_name);
            if dest_path.exists() && !paths_same_file(abs_path, &dest_path) {
                self.append_log(
                    app,
                    format!("metadata reclassify skip match={current_id}: dest exists"),
                );
                return Ok(MetadataReclassifyOutcome::None);
            }
            if !paths_same_file(abs_path, &dest_path) {
                move_demo_into_place(abs_path, &dest_path)?;
            }
            let rel = db.relative_path(&dest_path);
            repo::reclassify_match_to_valve(
                &db,
                current_id,
                &match_id,
                &rel,
                &file_name,
                file_size,
                checksum.as_deref(),
            )?;
            rel
        };

        self.append_log(
            app,
            format!("metadata reclassified match={current_id} -> valve match={match_id} path={rel_path}"),
        );
        Ok(MetadataReclassifyOutcome::Reclassified {
            id: current_id.to_string(),
        })
    }

    pub fn refresh_all_metadata(&self, app: &AppHandle) -> Result<MetadataRefreshSummary, OrchestratorError> {
        let matches = {
            let db = self.db.lock();
            repo::list_completed_matches_with_files(&db)?
        };
        let total = matches.len();
        let mut updated = 0usize;
        let mut merged = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;
        let mut results = Vec::new();

        for (index, (id, rel_path)) in matches.into_iter().enumerate() {
            self.set_task(
                app,
                "metadata",
                "更新元数据",
                "running",
                index as f64 / total.max(1) as f64,
                Some(format!("{}/{} {}", index + 1, total, rel_path)),
            );

            let abs_path = {
                let db = self.db.lock();
                db.resolve_path(&rel_path)
            };

            if !abs_path.exists() {
                failed += 1;
                results.push(MetadataRefreshResult {
                    id: id.clone(),
                    file_path: rel_path.clone(),
                    action: "error".into(),
                    error: Some("文件不存在".into()),
                });
                continue;
            }

            let metadata = match self.parse_dem_metadata(&abs_path) {
                Ok(m) => m,
                Err(err) => {
                    failed += 1;
                    results.push(MetadataRefreshResult {
                        id: id.clone(),
                        file_path: rel_path.clone(),
                        action: "error".into(),
                        error: Some(err.to_string()),
                    });
                    continue;
                }
            };

            let apply_id = match self.maybe_reclassify_or_merge_imported(app, &id, &abs_path, &metadata) {
                Ok(MetadataReclassifyOutcome::Merged { target_id }) => {
                    merged += 1;
                    let target_path = {
                        let db = self.db.lock();
                        repo::get_match_abs_path(&db, &target_id)?
                            .unwrap_or_else(|| abs_path.clone())
                    };
                    match self.apply_dem_metadata(app, &target_id, &target_path, true) {
                        Ok(true) => {
                            results.push(MetadataRefreshResult {
                                id: target_id,
                                file_path: rel_path.clone(),
                                action: "merged".into(),
                                error: None,
                            });
                        }
                        Ok(false) => {
                            skipped += 1;
                            results.push(MetadataRefreshResult {
                                id: target_id,
                                file_path: rel_path.clone(),
                                action: "skipped".into(),
                                error: None,
                            });
                        }
                        Err(err) => {
                            failed += 1;
                            results.push(MetadataRefreshResult {
                                id: target_id,
                                file_path: rel_path.clone(),
                                action: "error".into(),
                                error: Some(err.to_string()),
                            });
                        }
                    }
                    continue;
                }
                Ok(MetadataReclassifyOutcome::Reclassified { id: rid }) => rid,
                Ok(MetadataReclassifyOutcome::None) => id,
                Err(err) => {
                    failed += 1;
                    results.push(MetadataRefreshResult {
                        id: id.clone(),
                        file_path: rel_path.clone(),
                        action: "error".into(),
                        error: Some(err.to_string()),
                    });
                    continue;
                }
            };

            let apply_path = {
                let db = self.db.lock();
                repo::get_match_abs_path(&db, &apply_id)?
                    .unwrap_or(abs_path)
            };

            match self.apply_dem_metadata(app, &apply_id, &apply_path, true) {
                Ok(true) => {
                    updated += 1;
                    results.push(MetadataRefreshResult {
                        id: apply_id,
                        file_path: rel_path,
                        action: "updated".into(),
                        error: None,
                    });
                }
                Ok(false) => {
                    skipped += 1;
                    results.push(MetadataRefreshResult {
                        id: apply_id,
                        file_path: rel_path,
                        action: "skipped".into(),
                        error: None,
                    });
                }
                Err(err) => {
                    failed += 1;
                    results.push(MetadataRefreshResult {
                        id: apply_id,
                        file_path: rel_path,
                        action: "error".into(),
                        error: Some(err.to_string()),
                    });
                }
            }
        }

        let _ = app.emit("jobs_updated", ());
        Ok(MetadataRefreshSummary {
            updated,
            merged,
            skipped,
            failed,
            results,
        })
    }

    pub fn start_queue_background(self: &Arc<Self>, app: AppHandle, label: &'static str) -> bool {
        if self.active_task.lock().is_some() {
            return false;
        }
        let orchestrator = Arc::clone(self);
        orchestrator.set_task(&app, "download", label, "running", 0.0, Some("检查队列".into()));
        std::thread::spawn(move || {
            if let Err(err) = orchestrator.process_queue(&app) {
                orchestrator.append_log(&app, format!("{label}失败: {err}"));
                orchestrator.set_task(&app, "download", label, "failed", 1.0, Some(err.to_string()));
            } else {
                orchestrator.set_task(&app, "download", label, "completed", 1.0, Some("完成".into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(800));
            orchestrator.clear_task(&app);
        });
        true
    }

    pub fn discover_all(&self, app: &AppHandle) -> Result<(), OrchestratorError> {
        self.append_log(app, "discover_all: start");
        let platforms = ["valve", "renown", "5eplay"];
        for (index, platform) in platforms.iter().enumerate() {
            self.set_task(
                app,
                "discover",
                "发现 Demo",
                "running",
                index as f64 / platforms.len() as f64,
                Some(format!("检查平台 {platform}")),
            );
            if let Err(err) = self.discover_platform(platform, app) {
                self.append_log(app, format!("platform {platform} FAILED: {err}"));
            }
            {
                let db = self.db.lock();
                repo::requeue_failed_jobs(&db, platform)?;
            }
            let _ = app.emit("jobs_updated", ());
            // Start downloading queued jobs for this platform before moving to the next
            self.process_queue(app)?;
        }
        *self.last_discover.lock() = Some(std::time::Instant::now());
        self.append_log(app, "discover_all: done");
        Ok(())
    }

    fn discover_platform(&self, platform: &str, app: &AppHandle) -> Result<(), OrchestratorError> {
        let (accounts, bot_secrets) = {
            let db = self.db.lock();
            let settings = repo::list_platform_settings(&db)?
                .into_iter()
                .find(|p| p.platform == platform);
            if settings.map(|s| s.enabled).unwrap_or(false) == false {
                self.append_log(app, format!("platform {platform}: skipped (disabled)"));
                return Ok(());
            }
            let accounts = repo::list_active_sub_accounts(&db, platform)?;
            let bot_secrets = if platform == "valve" {
                repo::get_valve_bot_secrets(&db).ok()
            } else {
                None
            };
            (accounts, bot_secrets)
        };

        self.append_log(app, format!(
            "platform {platform}: {} active account(s)",
            accounts.len()
        ));

        if platform == "valve" {
            if let Some(ref bot) = bot_secrets {
                self.append_log(app, format!(
                    "valve bot: username={:?} password={} api_key={}",
                    bot.username,
                    bot.password.as_ref().is_some_and(|p| !p.is_empty()),
                    bot.steam_api_key.as_ref().is_some_and(|k| !k.is_empty()),
                ));
            } else {
                self.append_log(app, "valve bot: not configured");
            }
        }

        let total = accounts.len().max(1);
        for (account_index, account) in accounts.into_iter().enumerate() {
            let mut metadata = account.metadata.clone();
            let channel = valve_channel_from_metadata(&metadata);
            if account.platform == "valve" && channel == "local" {
                self.append_log(
                    app,
                    format!(
                        "skip valve local account {}: local login discovery is not implemented",
                        account.external_id
                    ),
                );
                continue;
            }
            if account.platform == "valve" {
                if let Some(ref bot) = bot_secrets {
                    if let Some(ref key) = bot.steam_api_key {
                        if !key.is_empty() {
                            metadata["steam_api_key"] = json!(key);
                        }
                    }
                }
                if let Some(cursor) = repo::get_sync_cursor(&self.db.lock(), &account.id, "share_code")? {
                    metadata["last_share_code"] = json!(cursor);
                }
            }

            let params = json!({
                "platform": account.platform,
                "subAccountId": account.id,
                "externalId": account.external_id,
                "metadata": metadata,
                "valveChannel": channel,
            });

            self.set_task(
                app,
                "discover",
                "发现 Demo",
                "running",
                account_index as f64 / total as f64,
                Some(format!("{} {}", account.display_name, account.external_id)),
            );
            self.append_log(app, format!(
                "discover account {} ({}) channel={channel}",
                account.display_name, account.external_id
            ));

            let result = match self.sidecar.call("discover", params) {
                Ok(value) => value,
                Err(err) => {
                    self.append_log(app, format!(
                        "discover account {} FAILED: {err}",
                        account.external_id
                    ));
                    continue;
                }
            };

            let matches: Vec<DiscoveredMatchPayload> =
                serde_json::from_value(result).unwrap_or_default();

            self.append_log(app, format!(
                "discover account {} -> {} match(es)",
                account.external_id,
                matches.len()
            ));

            for m in matches {
                self.ingest_match(&account.id, &account.platform, &m, app)?;
            }
        }
        if platform == "valve" {
            if let Err(err) = self.resolve_valve_authcode_urls(app) {
                self.append_log(app, format!("resolve valve urls FAILED: {err}"));
            }
        }
        Ok(())
    }

    fn ingest_match(
        &self,
        sub_account_id: &str,
        platform: &str,
        m: &DiscoveredMatchPayload,
        app: &AppHandle,
    ) -> Result<(), OrchestratorError> {
        let db = self.db.lock();

        if platform == "valve" {
            if let Some(ref code) = m.share_code {
                if repo::share_code_exists(&db, code)? {
                    self.append_log(app, format!("skip duplicate valve share code for match {}", m.match_id));
                    return Ok(());
                }
            }
        }

        if repo::match_exists(&db, platform, &m.match_id)? {
            return Ok(());
        }

        let match_db_id = repo::upsert_discovered_match(&db, sub_account_id, platform, m)?;
        self.append_log(app, format!(
            "new match platform={platform} match_id={} db_id={match_db_id}",
            m.match_id
        ));

        drop(db);
        let _ = app.emit("jobs_updated", ());
        Ok(())
    }

    pub fn import_demo(
        self: &Arc<Self>,
        app: &AppHandle,
        file_path: &str,
    ) -> Result<String, OrchestratorError> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(OrchestratorError::Message(format!("文件不存在: {file_path}")));
        }

        let magic = {
            use std::io::Read;
            let mut f = std::fs::File::open(path)?;
            let mut buf = [0u8; 8];
            f.read_exact(&mut buf).map_err(|e| OrchestratorError::Message(e.to_string()))?;
            buf
        };
        if &magic != b"PBDEMS2\0" {
            return Err(OrchestratorError::Message("不是有效的 CS2 demo 文件".into()));
        }

        let parsed = self.parse_dem_metadata(path).ok();
        let platform = parsed
            .as_ref()
            .and_then(|m| m.detected_platform.as_deref())
            .filter(|p| *p == "valve" || *p == "imported")
            .unwrap_or_else(|| detect_platform_from_path(path));
        let valve_channel = if platform == "valve" {
            Some("local")
        } else {
            None
        };
        let checksum = parsed.as_ref().and_then(|m| m.checksum.clone());
        let effective_match_id = if platform == "valve" {
            let Some(match_id) = parsed
                .as_ref()
                .and_then(|m| m.match_id.clone())
                .filter(|s| !s.is_empty())
            else {
                self.append_log(
                    app,
                    format!("import skip {}: valve demo missing match_id", path.display()),
                );
                return Ok("skipped".to_string());
            };
            match_id
        } else {
            parsed
                .as_ref()
                .and_then(|m| m.match_id.clone())
                .or_else(|| checksum.clone())
                .unwrap_or_else(|| {
                    path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "imported".into())
                })
        };

        {
            let db = self.db.lock();
            if repo::should_skip_import(
                &db,
                platform,
                &effective_match_id,
                checksum.as_deref(),
            )
            .map_err(|e| OrchestratorError::Message(e.to_string()))? {
                return Ok("skipped".to_string());
            }
        }

        let dest_dir = {
            let db = self.db.lock();
            db.ensure_demo_dir(platform)?
        };
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{effective_match_id}.dem"));
        let dest_path = dest_dir.join(&file_name);

        if dest_path.exists() && !paths_same_file(path, &dest_path) {
            return Ok("skipped".to_string());
        }

        if !paths_same_file(path, &dest_path) {
            move_demo_into_place(path, &dest_path)?;
        }

        let rel_path = {
            let db = self.db.lock();
            db.relative_path(&dest_path)
        };
        let file_size = std::fs::metadata(&dest_path).map(|m| m.len() as i64).unwrap_or(0);

        let match_db_id = {
            let db = self.db.lock();
            repo::insert_imported_match(
                &db,
                platform,
                &effective_match_id,
                parsed.as_ref().and_then(|m| m.map_name.as_deref()),
                parsed.as_ref().and_then(|m| m.played_at.as_deref()),
                parsed.as_ref().and_then(|m| m.duration_seconds),
                parsed.as_ref().and_then(|m| m.game_mode.as_deref()),
                parsed.as_ref().and_then(|m| m.game_type),
                valve_channel,
                &rel_path,
                &file_name,
                file_size,
                checksum.as_deref(),
            )
            .map_err(|e| OrchestratorError::Message(e.to_string()))?
        };

        let _ = self.apply_dem_metadata(app, &match_db_id, &dest_path, false);

        let _ = app.emit("jobs_updated", ());
        Ok("imported".to_string())
    }

    pub fn process_queue(&self, app: &AppHandle) -> Result<(), OrchestratorError> {
        if self.is_paused() {
            self.append_log(app, "process_queue: skipped because queue is paused");
            return Ok(());
        }

        let mut completed_in_run = 0usize;
        if let Err(err) = self.resolve_valve_authcode_urls(app) {
            self.append_log(app, format!("resolve valve urls FAILED: {err}"));
        }
        loop {
            if self.is_paused() {
                self.append_log(app, "process_queue: paused while processing");
                break;
            }

            let (settings, remaining) = {
                let db = self.db.lock();
                (repo::get_settings(&db)?, repo::count_queued_jobs(&db)?)
            };
            if remaining == 0 {
                if completed_in_run == 0 {
                    self.append_log(app, "process_queue: no queued jobs");
                }
                break;
            }

            let jobs = {
                let db = self.db.lock();
                repo::fetch_queued_jobs(&db, settings.global_max_concurrent)?
            };
            if jobs.is_empty() {
                break;
            }

            self.append_log(app, format!("process_queue: {} queued job(s), processing {}", remaining, jobs.len()));

            let batch_size = jobs.len();
            let progress = (completed_in_run as f64 / (completed_in_run + remaining) as f64).clamp(0.0, 0.95);
            self.set_task(
                app,
                "download",
                "下载 Demo",
                "running",
                progress,
                Some(format!("并发下载 {batch_size} 个任务")),
            );

            let mut failed_in_batch = 0usize;
            let mut spawned_in_batch = 0usize;
            std::thread::scope(|scope| {
                let mut handles = Vec::with_capacity(batch_size);
                for (id, platform, match_id, demo_url, share_code, valve_channel) in jobs {
                    if repo::is_job_cancelled(&self.db.lock(), &id).unwrap_or(false) {
                        continue;
                    }
                    if platform == "valve"
                        && valve_channel.as_deref() == Some("authcode")
                        && demo_url.as_ref().is_none_or(|u| u.is_empty())
                    {
                        // 不能留在 queued，否则 count_queued_jobs 永远不为 0，本循环死转
                        self.append_log(
                            app,
                            format!("job {id}: valve authcode missing demo_url after resolve, marking failed"),
                        );
                        let db = self.db.lock();
                        let _ = repo::update_job_status(
                            &db,
                            &id,
                            "failed",
                            None,
                            Some("缺少 demo URL：解析未完成或需要重新发现"),
                        );
                        continue;
                    }
                    spawned_in_batch += 1;
                    let app = app.clone();
                    handles.push(scope.spawn(move || {
                        self.run_download(
                            &app,
                            &id,
                            &platform,
                            &match_id,
                            demo_url.as_deref(),
                            share_code.as_deref(),
                            valve_channel.as_deref(),
                        )
                    }));
                }

                for handle in handles {
                    match handle.join() {
                        Ok(Ok(())) => {
                            completed_in_run += 1;
                        }
                        Ok(Err(err)) => {
                            failed_in_batch += 1;
                            self.append_log(app, format!("download worker failed: {err}"));
                        }
                        Err(_) => {
                            failed_in_batch += 1;
                            self.append_log(app, "download worker panicked");
                        }
                    }
                }
            });
            if failed_in_batch > 0 {
                self.append_log(app, format!("process_queue: {failed_in_batch} job(s) failed in this batch, continuing"));
            }
            if spawned_in_batch == 0 {
                let now_remaining = {
                    let db = self.db.lock();
                    repo::count_queued_jobs(&db)?
                };
                if now_remaining >= remaining {
                    self.append_log(
                        app,
                        "process_queue: batch made no progress, stopping to avoid busy loop",
                    );
                    break;
                }
            }
        }
        Ok(())
    }

    fn run_download(
        &self,
        app: &AppHandle,
        job_id: &str,
        platform: &str,
        match_id: &str,
        demo_url: Option<&str>,
        share_code: Option<&str>,
        valve_channel: Option<&str>,
    ) -> Result<(), OrchestratorError> {
        let output_path = {
            let db = self.db.lock();
            let dir = db.ensure_demo_dir(platform)?;
            let file_name = format!("{match_id}.dem");
            dir.join(&file_name)
        };

        {
            let db = self.db.lock();
            repo::update_job_status(&db, job_id, "downloading", Some(0.0), None)?;
        }
        let _ = app.emit("job_progress", json!({ "jobId": job_id, "progress": 0.1 }));

        self.append_log(app, format!(
            "download start job={job_id} platform={platform} match={match_id} channel={} share={}",
            valve_channel.unwrap_or("-"),
            share_code.unwrap_or("-"),
        ));

        if platform == "valve"
            && valve_channel != Some("local")
            && demo_url.map(str::is_empty).unwrap_or(true)
        {
            return Err(OrchestratorError::Message(
                "Valve demo URL missing; run resolve first".into(),
            ));
        }

        let download_params = json!({
            "jobId": job_id,
            "platform": platform,
            "outputPath": output_path.to_string_lossy(),
            "demoUrl": demo_url.unwrap_or(""),
            "matchId": match_id,
            "valveChannel": valve_channel,
        });

        match self.sidecar.call("download", download_params) {
            Ok(_) => {
                if repo::is_job_cancelled(&self.db.lock(), job_id)? {
                    self.append_log(app, format!("download finished after cancel job={job_id}; keeping skipped"));
                    let _ = fs::remove_file(&output_path);
                    let _ = app.emit("jobs_updated", ());
                    return Ok(());
                }
                self.append_log(app, format!("download OK job={job_id} -> {}", output_path.display()));
                {
                    let db = self.db.lock();
                    repo::update_job_status(&db, job_id, "completed", Some(1.0), None)?;
                }
                let _ = self.apply_dem_metadata(app, job_id, &output_path, false);
                let rel = {
                    let db = self.db.lock();
                    db.relative_path(&output_path)
                };
                let _ = app.emit("job_progress", json!({ "jobId": job_id, "progress": 1.0 }));
                let _ = app.emit("jobs_updated", json!({ "filePath": rel }));
            }
            Err(e) => {
                let msg = e.to_string();
                let status = if msg.contains("URL_EXPIRED") {
                    "expired"
                } else {
                    "failed"
                };
                self.append_log(app, format!("download {status} job={job_id}: {msg}"));
                let db = self.db.lock();
                repo::update_job_status(&db, job_id, status, None, Some(&msg))?;
                let _ = app.emit("jobs_updated", ());
            }
        }
        Ok(())
    }

    pub fn retry_job(&self, job_id: &str, app: &AppHandle) -> Result<(), OrchestratorError> {
        let db = self.db.lock();
        repo::retry_job(&db, job_id)?;
        drop(db);
        let _ = app.emit("jobs_updated", ());
        self.process_queue(app)
    }

    pub fn cancel_job(&self, job_id: &str, app: &AppHandle) -> Result<(), OrchestratorError> {
        let db = self.db.lock();
        repo::cancel_job(&db, job_id)?;
        drop(db);
        let _ = app.emit("jobs_updated", ());
        Ok(())
    }

    pub fn minutes_since_discover(&self) -> Option<i64> {
        self.last_discover
            .lock()
            .as_ref()
            .map(|t| t.elapsed().as_secs() as i64 / 60)
    }
}
