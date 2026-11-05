mod analysis;
mod commands;
mod database;
mod identity;
mod orchestrator;
mod sidecar;

use analysis::AnalysisQueue;
use database::{repo, resolve_data_root, Database};
use orchestrator::DownloadOrchestrator;
use parking_lot::Mutex;
use sidecar::{resolve_sidecar_script, SidecarManager};
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub sidecar: Arc<SidecarManager>,
    pub orchestrator: Arc<DownloadOrchestrator>,
    pub analysis: Arc<AnalysisQueue>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_root = resolve_data_root(app.handle());
            let database = Database::open(data_root).expect("failed to open database");
            repo::sync_data_root_setting(&database).expect("failed to sync data_root setting");
            let _ = repo::reset_interrupted_downloads(&database);
            let db = Arc::new(Mutex::new(database));
            identity::init(db.clone());
            let script = resolve_sidecar_script();
            let sidecar = Arc::new(SidecarManager::new(script));
            let analysis = AnalysisQueue::new(db.clone());
            analysis.start(app.handle().clone());
            let orchestrator = Arc::new(DownloadOrchestrator::new(db.clone(), sidecar.clone(), analysis.clone()));
            orchestrator.wire_sidecar_events(app.handle().clone());
            let _ = sidecar.start();

            app.manage(AppState {
                db: db.clone(),
                sidecar,
                orchestrator: orchestrator.clone(),
                analysis: analysis.clone(),
            });

            // 补齐历史对局的完整解析（中断 / 失败可重试 / 解析器版本升级）
            {
                let analysis = analysis.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    analysis.enqueue_backlog();
                });
            }

            let run_startup_discover = repo::get_settings(&db.lock())
                .map(|settings| settings.auto_discover_on_startup)
                .unwrap_or(false);
            if run_startup_discover {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    if let Some(state) = handle.try_state::<AppState>() {
                        let _ = state
                            .orchestrator
                            .start_discover_background(handle.clone(), "启动自动发现");
                    }
                });
            }

            let needs_metadata = repo::list_matches_needing_metadata(&db.lock())
                .map(|rows| rows.len())
                .unwrap_or(0);
            if needs_metadata > 0 {
                let handle = app.handle().clone();
                let orchestrator = orchestrator.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let _ = orchestrator.start_refresh_metadata_background(handle);
                });
            }
            // Queue starts paused; user resumes from the downloads page.

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::get_background_task,
            commands::get_background_task_logs,
            commands::list_users,
            commands::create_user,
            commands::update_user,
            commands::delete_user,
            commands::list_sub_accounts,
            commands::add_sub_account,
            commands::update_sub_account,
            commands::update_sub_account_meta,
            commands::delete_sub_account,
            commands::set_sub_account_status,
            commands::list_matches,
            commands::get_settings,
            commands::update_settings,
            commands::list_platform_settings,
            commands::update_platform_settings,
            commands::get_valve_bot_config,
            commands::update_valve_bot_config,
            commands::discover_now,
            commands::toggle_queue_pause,
            commands::retry_download_job,
            commands::cancel_download_job,
            commands::open_path,
            commands::open_data_subdirectory,
            commands::import_demos,
            commands::refresh_match_metadata,
            commands::force_download_now,
            commands::reanalyze_matches,
            commands::get_analysis_status,
            identity::commands::search_persons,
            identity::commands::get_person,
            identity::commands::get_head_to_head,
            identity::commands::list_teammates,
            identity::commands::merge_persons,
            identity::commands::undo_merge,
            identity::commands::split_identity,
            identity::commands::update_person,
            identity::commands::rebuild_head_to_head,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
