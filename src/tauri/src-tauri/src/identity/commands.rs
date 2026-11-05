//! 身份与对位的 Tauri 命令。JSON 结构见 src/shared-types/src/identity.ts。

use super::{h2h, manage, query as q};
use crate::AppState;
use tauri::State;

type R<T> = Result<T, String>;

fn e(err: crate::database::DbError) -> String {
    err.to_string()
}

#[tauri::command]
pub fn search_persons(
    state: State<'_, AppState>,
    query: Option<String>,
    followed_only: Option<bool>,
    limit: Option<i64>,
) -> R<Vec<q::PersonSummary>> {
    let db = state.db.lock();
    q::search_persons(&db.conn, query.as_deref(), followed_only.unwrap_or(false), limit.unwrap_or(50).clamp(1, 500))
        .map_err(e)
}

/// 传入已被合并的 id 时返回合并后的人（`person.id` 与传入值不同）
#[tauri::command]
pub fn get_person(state: State<'_, AppState>, id: i64) -> R<q::PersonDetail> {
    let db = state.db.lock();
    let id = q::resolve(&db.conn, id).map_err(e)?;
    q::get_person(&db.conn, id).map_err(e)
}

#[tauri::command]
pub fn get_head_to_head(state: State<'_, AppState>, a: i64, b: i64) -> R<q::HeadToHead> {
    q::get_head_to_head(&state.db.lock().conn, a, b).map_err(e)
}

#[tauri::command]
pub fn list_teammates(
    state: State<'_, AppState>,
    person: i64,
    min_matches: Option<i64>,
    limit: Option<i64>,
) -> R<q::TeammatesDto> {
    q::list_teammates(&state.db.lock().conn, person, min_matches.unwrap_or(2), limit.unwrap_or(50).clamp(1, 500))
        .map_err(e)
}

/// 把 source 并进 target，返回合并后的 target
#[tauri::command]
pub fn merge_persons(state: State<'_, AppState>, target: i64, source: i64) -> R<q::PersonDetail> {
    let db = state.db.lock();
    manage::merge_persons(&db.conn, target, source).map_err(e)?;
    q::get_person(&db.conn, target).map_err(e)
}

/// 撤销合并，返回恢复出来的人
#[tauri::command]
pub fn undo_merge(state: State<'_, AppState>, merge_id: i64) -> R<q::PersonDetail> {
    let db = state.db.lock();
    let source = manage::undo_merge(&db.conn, merge_id).map_err(e)?;
    q::get_person(&db.conn, source).map_err(e)
}

/// 把身份拆出来（挂到 `toPerson`，不传则新建一个人），返回身份现在所属的人
#[tauri::command]
pub fn split_identity(state: State<'_, AppState>, identity_id: i64, to_person: Option<i64>) -> R<q::PersonDetail> {
    let db = state.db.lock();
    let p = manage::split_identity(&db.conn, identity_id, to_person).map_err(e)?;
    q::get_person(&db.conn, p).map_err(e)
}

/// 改显示名 / 备注 / 标签 / 关注
#[tauri::command]
pub fn update_person(state: State<'_, AppState>, id: i64, patch: manage::PersonPatch) -> R<q::PersonDetail> {
    let db = state.db.lock();
    manage::update_person(&db.conn, id, &patch).map_err(e)?;
    q::get_person(&db.conn, id).map_err(e)
}

/// 全量重建对位数据，返回处理的对局数
#[tauri::command]
pub fn rebuild_head_to_head(state: State<'_, AppState>) -> R<usize> {
    h2h::rebuild_all(&state.db.lock().conn).map_err(e)
}
