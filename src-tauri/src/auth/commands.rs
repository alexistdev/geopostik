use tauri::State;

use super::model::{AppStatus, LoginInput, SessionUser, SetupInput};
use super::service;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn app_status(state: State<'_, AppState>) -> AppResult<AppStatus> {
    let conn = state.db()?;
    Ok(AppStatus {
        needs_setup: service::needs_setup(&conn)?,
        session: state.session(),
    })
}

#[tauri::command]
pub async fn setup_owner(state: State<'_, AppState>, input: SetupInput) -> AppResult<SessionUser> {
    let mut conn = state.db()?;
    let user = service::setup_owner(&mut conn, &input)?;
    state.set_session(Some(user.clone()));
    Ok(user)
}

#[tauri::command]
pub async fn login(state: State<'_, AppState>, input: LoginInput) -> AppResult<SessionUser> {
    let conn = state.db()?;
    let user = service::login(&conn, &input)?;
    state.set_session(Some(user.clone()));
    Ok(user)
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> AppResult<()> {
    state.set_session(None);
    Ok(())
}
