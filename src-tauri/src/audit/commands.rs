use tauri::State;

use super::model::{AuditPage, AuditQuery, AuditUser};
use super::repo;
use crate::auth::Permission;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn audit_page(state: State<'_, AppState>, query: AuditQuery) -> AppResult<AuditPage> {
    state.require(Permission::AuditView)?;
    let (rows, total) = repo::page(&*state.db()?, &query)?;
    Ok(AuditPage { rows, total })
}

#[tauri::command]
pub async fn audit_users(state: State<'_, AppState>) -> AppResult<Vec<AuditUser>> {
    state.require(Permission::AuditView)?;
    repo::users(&*state.db()?)
}
