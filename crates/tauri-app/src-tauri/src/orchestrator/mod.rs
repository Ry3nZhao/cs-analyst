use crate::db::models::DiscoveredMatchPayload;
use crate::db::repo;
use crate::db::Database;
use crate::sidecar::SidecarManager;
use parking_lot::Mutex;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("db: {0}")]
    Db(#[from] crate::db::DbError),
    #[error("sidecar: {0}")]
    Sidecar(#[from] crate::sidecar::SidecarError),
    #[error("{0}")]
    Message(String),
}

pub struct DownloadOrchestrator {
    db: Arc<Mutex<Database>>,
    sidecar: Arc<SidecarManager>,
    queue_paused: Mutex<bool>,
    last_discover: Mutex<Option<std::time::Instant>>,
}

impl DownloadOrchestrator {
    pub fn new(db: Arc<Mutex<Database>>, sidecar: Arc<SidecarManager>) -> Self {
        Self {
            db,
            sidecar,
            queue_paused: Mutex::new(false),
            last_discover: Mutex::new(None),
        }
    }

    pub fn is_paused(&self) -> bool {
        *self.queue_paused.lock()
    }

    pub fn toggle_pause(&self) -> bool {
        let mut p = self.queue_paused.lock();
        *p = !*p;
        *p
    }

    pub fn discover_all(&self, app: &AppHandle) -> Result<(), OrchestratorError> {
        let platforms = ["valve", "renown", "5eplay"];
        for platform in platforms {
            self.discover_platform(platform, app)?;
            {
                let db = self.db.lock();
                repo::requeue_failed_jobs(&db, platform)?;
            }
        }
        *self.last_discover.lock() = Some(std::time::Instant::now());
        let _ = app.emit("jobs_updated", ());
        Ok(())
    }

    fn discover_platform(&self, platform: &str, app: &AppHandle) -> Result<(), OrchestratorError> {
        let accounts = {
            let db = self.db.lock();
            let settings = repo::list_platform_settings(&db)?
                .into_iter()
                .find(|p| p.platform == platform);
            if settings.map(|s| s.enabled).unwrap_or(false) == false {
                return Ok(());
            }
            repo::list_active_sub_accounts(&db, platform)?
        };

        for account in accounts {
            let params = json!({
                "platform": account.platform,
                "subAccountId": account.id,
                "externalId": account.external_id,
                "metadata": account.metadata,
                "valveChannel": account.metadata.get("source").and_then(|v| {
                    if v == "auth_code" { Some("authcode") } else { Some("local") }
                }),
            });

            let result = self.sidecar.call("discover", params)?;
            let matches: Vec<DiscoveredMatchPayload> =
                serde_json::from_value(result).unwrap_or_default();

            for m in matches {
                self.ingest_match(&account.id, &account.platform, &m, app)?;
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
                    return Ok(());
                }
            }
        }

        if repo::match_exists(&db, platform, &m.match_id)? {
            return Ok(());
        }

        let discovered_id = repo::upsert_discovered_match(&db, sub_account_id, platform, m)?;
        let job_id = repo::ensure_download_job(&db, &discovered_id)?;

        if platform == "valve" {
            if let Some(ref code) = m.share_code {
                if repo::share_code_exists(&db, code)? {
                    repo::mark_job_skipped(&db, &job_id)?;
                    let _ = app.emit("jobs_updated", ());
                    return Ok(());
                }
            }
        }

        drop(db);
        let _ = app.emit("jobs_updated", ());
        self.process_queue(app)?;
        Ok(())
    }

    pub fn process_queue(&self, app: &AppHandle) -> Result<(), OrchestratorError> {
        if self.is_paused() {
            return Ok(());
        }

        let settings = {
            let db = self.db.lock();
            repo::get_settings(&db)?
        };

        let jobs = {
            let db = self.db.lock();
            repo::fetch_queued_jobs(&db, settings.global_max_concurrent)?
        };

        for (job_id, platform, match_id, demo_url, external_id, share_code, valve_channel, sub_account_id) in jobs {
            self.run_download(
                app,
                &job_id,
                &platform,
                &match_id,
                demo_url.as_deref(),
                &external_id,
                share_code.as_deref(),
                valve_channel.as_deref(),
                &sub_account_id,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn run_download(
        &self,
        app: &AppHandle,
        job_id: &str,
        platform: &str,
        match_id: &str,
        demo_url: Option<&str>,
        external_id: &str,
        share_code: Option<&str>,
        valve_channel: Option<&str>,
        sub_account_id: &str,
    ) -> Result<(), OrchestratorError> {
        let output_path = {
            let db = self.db.lock();
            let dir = db.ensure_demo_dir(platform, external_id)?;
            let file_name = format!("{match_id}.dem");
            dir.join(&file_name)
        };

        {
            let db = self.db.lock();
            repo::update_job_status(&db, job_id, "downloading", Some(0.0), None)?;
        }
        let _ = app.emit("job_progress", json!({ "jobId": job_id, "progress": 0.1 }));

        let download_params = json!({
            "jobId": job_id,
            "platform": platform,
            "outputPath": output_path.to_string_lossy(),
            "demoUrl": demo_url.unwrap_or(""),
            "matchId": match_id,
            "valveChannel": valve_channel,
            "shareCode": share_code,
        });

        match self.sidecar.call("download", download_params) {
            Ok(_) => {
                let meta = fs::metadata(&output_path).ok();
                let file_size = meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
                let file_name = output_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("{match_id}.dem"));
                let rel = {
                    let db = self.db.lock();
                    let rel = db.relative_path(&output_path);
                    repo::insert_demo(
                        &db,
                        platform,
                        sub_account_id,
                        match_id,
                        share_code,
                        &rel,
                        &file_name,
                        file_size,
                        valve_channel,
                    )?;
                    repo::update_job_status(&db, job_id, "completed", Some(1.0), None)?;
                    rel
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
