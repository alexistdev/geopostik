use chrono::Local;
use tauri::State;

use super::model::Dashboard;
use super::service;
use crate::error::AppResult;
use crate::state::AppState;

/// Semua user yang login boleh membuka dashboard; isinya difilter per hak di service.
#[tauri::command]
pub async fn dashboard_get(state: State<'_, AppState>) -> AppResult<Dashboard> {
    let user = state.current_user()?;
    service::get(&*state.db()?, &user, Local::now().date_naive())
}
