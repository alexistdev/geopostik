use tauri::State;

use super::model::{UserInput, UserListQuery, UserListResult, UserRow, UserSaveResult};
use super::service;
use crate::auth::Permission;
use crate::error::AppResult;
use crate::master::BatchResult;
use crate::state::AppState;

// Semua aksi menu Pengguna butuh USER_MANAGE (hanya pemilik).

#[tauri::command]
pub async fn user_list(state: State<'_, AppState>, query: UserListQuery) -> AppResult<UserListResult> {
    state.require(Permission::UserManage)?;
    service::list(&*state.db()?, &query)
}

#[tauri::command]
pub async fn user_get(state: State<'_, AppState>, id: i64) -> AppResult<UserRow> {
    state.require(Permission::UserManage)?;
    service::get(&*state.db()?, id)
}

#[tauri::command]
pub async fn user_save(state: State<'_, AppState>, input: UserInput) -> AppResult<UserSaveResult> {
    let actor = state.require(Permission::UserManage)?;
    let result = service::save(&mut *state.db()?, &actor, &input)?;
    // Akun sendiri diubah (nama/peran) → sesi ikut diperbarui.
    if let Some(session) = &result.session {
        state.set_session(Some(session.clone()));
    }
    Ok(result)
}

#[tauri::command]
pub async fn user_set_active(state: State<'_, AppState>, id: i64, active: bool) -> AppResult<()> {
    let actor = state.require(Permission::UserManage)?;
    service::set_active(&mut *state.db()?, &actor, id, active)
}

#[tauri::command]
pub async fn user_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let actor = state.require(Permission::UserManage)?;
    service::delete(&mut *state.db()?, &actor, id)
}

#[tauri::command]
pub async fn user_set_active_many(state: State<'_, AppState>, ids: Vec<i64>, active: bool) -> AppResult<BatchResult> {
    let actor = state.require(Permission::UserManage)?;
    service::set_active_many(&mut *state.db()?, &actor, &ids, active)
}

#[tauri::command]
pub async fn user_delete_many(state: State<'_, AppState>, ids: Vec<i64>) -> AppResult<BatchResult> {
    let actor = state.require(Permission::UserManage)?;
    service::delete_many(&mut *state.db()?, &actor, &ids)
}
