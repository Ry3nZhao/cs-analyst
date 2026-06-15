use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub display_name: String,
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAccount {
    pub id: String,
    pub user_id: String,
    pub platform: String,
    pub external_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub status: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJob {
    pub job_id: String,
    pub status: String,
    pub progress: f64,
    pub platform: String,
    pub match_id: String,
    pub map_name: Option<String>,
    pub played_at: Option<String>,
    pub valve_channel: Option<String>,
    pub user_id: String,
    pub user_name: String,
    pub account_name: String,
    pub account_id: String,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub demo_url: Option<String>,
    pub share_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub data_root: String,
    pub global_max_concurrent: i32,
    pub auto_discover_on_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformSettings {
    pub platform: String,
    pub enabled: bool,
    pub auto_discover: bool,
    pub discover_interval_min: i32,
    pub max_concurrent_jobs: i32,
    pub auto_retry_failed: bool,
    pub max_auto_attempts: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub sidecar_running: bool,
    pub orchestrator_running: bool,
    pub queue_paused: bool,
    pub data_root: String,
    pub last_discover_minutes_ago: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredMatchPayload {
    pub match_id: String,
    pub share_code: Option<String>,
    pub demo_url: Option<String>,
    pub map_name: Option<String>,
    pub played_at: Option<String>,
    pub valve_channel: Option<String>,
    pub raw_payload: serde_json::Value,
}
