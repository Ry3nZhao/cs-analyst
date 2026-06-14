mod commands;
mod db;
mod orchestrator;
mod sidecar;

use db::{default_data_root, Database};
use orchestrator::DownloadOrchestrator;
use parking_lot::Mutex;
use sidecar::{resolve_sidecar_script, SidecarManager};
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub sidecar: Arc<SidecarManager>,
    pub orchestrator: Arc<DownloadOrchestrator>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_root = default_data_root();
            let database = Database::open(data_root).expect("failed to open database");
            let db = Arc::new(Mutex::new(database));
            let script = resolve_sidecar_script();
            let sidecar = Arc::new(SidecarManager::new(script));
            let _ = sidecar.start();
            let orchestrator = Arc::new(DownloadOrchestrator::new(db.clone(), sidecar.clone()));

            app.manage(AppState {
                db,
                sidecar,
                orchestrator: orchestrator.clone(),
            });

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Some(state) = handle.try_state::<AppState>() {
                    let _ = state.orchestrator.discover_all(&handle);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::list_users,
            commands::create_user,
            commands::update_user,
            commands::delete_user,
            commands::list_sub_accounts,
            commands::add_sub_account,
            commands::update_sub_account,
            commands::set_sub_account_status,
            commands::list_download_jobs,
            commands::get_settings,
            commands::update_settings,
            commands::list_platform_settings,
            commands::update_platform_settings,
            commands::discover_now,
            commands::toggle_queue_pause,
            commands::retry_download_job,
            commands::cancel_download_job,
            commands::open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
