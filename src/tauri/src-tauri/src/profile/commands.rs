//! Profile command. JSON shape mirrored in `src/shared-types/src/profile.ts`.

use super::{build_profile, Profile, ProfileOptions};
use crate::AppState;
use tauri::State;

/// Personal profile of `steamid` (and the person's other steam identities).
#[tauri::command]
pub fn get_profile(
    state: State<'_, AppState>,
    steamid: String,
    options: Option<ProfileOptions>,
) -> std::result::Result<Profile, String> {
    let sid: u64 = steamid.trim().parse().map_err(|_| format!("无效的 steamid: {steamid}"))?;
    build_profile(&state.db.lock(), sid, &options.unwrap_or_default()).map_err(|e| e.to_string())
}
