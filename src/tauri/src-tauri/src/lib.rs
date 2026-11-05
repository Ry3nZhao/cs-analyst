mod aim;
mod analysis;
mod commands;
mod database;
mod identity;
mod onboarding;
mod profile;
mod orchestrator;
mod replay;
mod sidecar;
mod stats;
mod training;
mod watch;
mod storage;
mod highlights;

use analysis::AnalysisQueue;
use database::{repo, Database};
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
        .plugin(tauri_plugin_notification::init())
        .on_window_event(onboarding::on_window_event)
        .setup(|app| {
            let data_root = onboarding::resolve_data_root(app.handle());
            let database = Database::open(data_root).expect("failed to open database");
            repo::sync_data_root_setting(&database).expect("failed to sync data_root setting");
            let _ = repo::reset_interrupted_downloads(&database);
            let db = Arc::new(Mutex::new(database));
            identity::init(db.clone());
            let script = resolve_sidecar_script();
            let log_dir = std::path::PathBuf::from(db.lock().get_data_root()).join("logs");
            let sidecar = Arc::new(SidecarManager::new(script, Some(log_dir)));
            let analysis = AnalysisQueue::new(db.clone());
            analysis.start(app.handle().clone());
            stats::spawn_backfill(db.clone());
            aim::start_backfill(db.clone());
            storage::init(db.clone());
            highlights::start_backfill(db.clone());
            let orchestrator = Arc::new(DownloadOrchestrator::new(db.clone(), sidecar.clone(), analysis.clone()));
            orchestrator.wire_sidecar_events(app.handle().clone());
            orchestrator.sync_adapter_switches();
            sidecar.spawn_supervisor();
            {
                let sidecar = sidecar.clone();
                std::thread::spawn(move || {
                    let _ = sidecar.start();
                });
            }

            app.manage(AppState {
                db: db.clone(),
                sidecar,
                orchestrator: orchestrator.clone(),
                analysis: analysis.clone(),
            });
            onboarding::init(app.handle());

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
            crate::stats::commands::get_match_detail,
            crate::stats::commands::list_player_stats,
            crate::stats::commands::get_round_win_probability,
            identity::commands::search_persons,
            identity::commands::get_person,
            identity::commands::get_head_to_head,
            identity::commands::list_teammates,
            identity::commands::merge_persons,
            identity::commands::undo_merge,
            identity::commands::split_identity,
            identity::commands::update_person,
            identity::commands::rebuild_head_to_head,
            aim::commands::get_aim_stats,
            aim::commands::get_sprays,
            sidecar::commands::get_downloader_status,
            sidecar::commands::restart_downloader,
            sidecar::commands::get_downloader_logs,
            replay::commands::get_replay, replay::commands::get_heatmap_points, replay::commands::get_radar, replay::commands::get_radar_image, replay::commands::get_radar_settings, replay::commands::update_radar_settings,
            training::commands::save_training_result,
            training::commands::list_training_results,
            training::commands::get_training_trials,
            training::commands::delete_training_result,
            training::commands::detect_cs2_sensitivity,
            training::commands::read_cs2_sensitivity,
            onboarding::commands::get_main_player,
            profile::commands::get_profile,
            onboarding::commands::set_main_player,
            onboarding::commands::list_main_player_candidates,
            onboarding::commands::get_onboarding_state,
            onboarding::commands::complete_onboarding,
            onboarding::commands::set_data_root,
            onboarding::commands::get_experience_settings,
            onboarding::commands::update_experience_settings,
            onboarding::commands::get_watch_status,
            onboarding::commands::detect_replays_dir,
            onboarding::commands::send_test_notification,
            storage::commands::get_storage_settings,
            storage::commands::update_storage_settings,
            storage::commands::get_storage_usage,
            storage::commands::list_match_storage,
            storage::commands::set_match_favorite,
            storage::commands::preview_storage_plan,
            storage::commands::apply_storage_plan,
            storage::commands::export_matches,
            storage::commands::create_backup,
            storage::commands::list_backups,
            storage::commands::restore_backup,
            storage::commands::inspect_backup,
            highlights::commands::list_highlights, highlights::commands::get_highlight_settings, highlights::commands::update_highlight_settings, highlights::commands::recompute_highlights, highlights::commands::prepare_highlight_playback, highlights::commands::launch_highlight_in_cs2,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
