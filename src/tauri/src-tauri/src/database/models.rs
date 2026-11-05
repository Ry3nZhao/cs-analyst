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
pub struct Match {
    pub id: String,
    pub status: String,
    pub progress: f64,
    pub platform: String,
    pub match_id: String,
    /// Computed: platform only (same as `platform`)
    pub source: String,
    pub map_name: Option<String>,
    pub played_at: Option<String>,
    pub valve_channel: Option<String>,
    pub duration_seconds: Option<i64>,
    pub game_mode: Option<String>,
    pub game_type: Option<i64>,
    pub share_code: Option<String>,
    pub checksum: Option<String>,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub file_size: Option<i64>,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub demo_url: Option<String>,
    // CDemoFileHeader 1-12 (5=map_name above)
    pub demo_file_stamp: Option<String>,
    pub network_protocol: Option<i64>,
    pub server_name: Option<String>,
    pub client_name: Option<String>,
    pub game_directory: Option<String>,
    pub fullpackets_version: Option<i64>,
    pub allow_clientside_entities: Option<bool>,
    pub demo_header_field_9: Option<i64>,
    pub demo_header_field_10: Option<String>,
    pub demo_version_name: Option<String>,
    pub demo_version_guid: Option<String>,
    // CDemoFileInfo 1-4 (1=duration_seconds + playback_time)
    pub playback_time: Option<f64>,
    pub playback_ticks: Option<i64>,
    pub playback_frames: Option<i64>,
    pub dem_game_info: Option<serde_json::Value>,
    // Full demo analysis (migration 002)
    pub analysis_status: Option<String>,
    pub analysis_error: Option<String>,
    pub analyzed_at: Option<String>,
    pub analysis_ms: Option<i64>,
    pub rounds_played: Option<i64>,
}

/// Kept for backwards compat with old serialized field name
pub type DownloadJob = Match;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub path: String,
    pub action: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub imported: usize,
    pub skipped: usize,
    pub results: Vec<ImportResult>,
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
    pub background_task: Option<BackgroundTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundTask {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub status: String,
    pub progress: f64,
    pub current: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValveBotConfig {
    pub username: Option<String>,
    pub password_set: bool,
    pub secret_set: bool,
    pub steam_api_key: Option<String>,
    pub cron_schedule: String,
    pub run_on_startup: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValveBotConfigUpdate {
    pub username: Option<String>,
    pub password: Option<String>,
    pub secret: Option<String>,
    pub steam_api_key: Option<String>,
    pub cron_schedule: Option<String>,
    pub run_on_startup: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ValveBotSecrets {
    pub username: Option<String>,
    pub password: Option<String>,
    pub secret: Option<String>,
    pub steam_api_key: Option<String>,
    pub refresh_token: Option<String>,
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

#[derive(Debug, Clone)]
pub struct ParsedDemMetadata {
    pub match_id: Option<String>,
    pub map_name: Option<String>,
    pub duration_seconds: Option<i64>,
    pub game_mode: Option<String>,
    pub game_type: Option<i64>,
    pub played_at: Option<String>,
    pub checksum: Option<String>,
    pub is_valid: bool,
    pub demo_file_stamp: Option<String>,
    pub network_protocol: Option<i64>,
    pub server_name: Option<String>,
    pub client_name: Option<String>,
    pub game_directory: Option<String>,
    pub fullpackets_version: Option<i64>,
    pub allow_clientside_entities: Option<bool>,
    pub demo_header_field_9: Option<i64>,
    pub demo_header_field_10: Option<String>,
    pub demo_version_name: Option<String>,
    pub demo_version_guid: Option<String>,
    pub playback_time: Option<f64>,
    pub playback_ticks: Option<i64>,
    pub playback_frames: Option<i64>,
    pub dem_game_info: Option<serde_json::Value>,
    pub detected_platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRefreshResult {
    pub id: String,
    pub file_path: String,
    pub action: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRefreshSummary {
    pub updated: usize,
    pub merged: usize,
    pub skipped: usize,
    pub failed: usize,
    pub results: Vec<MetadataRefreshResult>,
}
