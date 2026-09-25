use tauri::State;

use super::model::{AppStatus, LoginInput, SessionUser, SetupInput};
use super::service;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[tauri::command]
pub async fn app_status(state: State<'_, AppState>) -> AppResult<AppStatus> {
    let license = state.license()?.check();
    let conn = state.db()?;
    Ok(AppStatus {
        license,
        needs_setup: service::needs_setup(&conn)?,
        session: state.session(),
    })
}

#[tauri::command]
pub async fn setup_owner(state: State<'_, AppState>, input: SetupInput) -> AppResult<SessionUser> {
    state.require_license()?;
    let mut conn = state.db()?;
    let user = service::setup_owner(&mut conn, &input)?;
    state.set_session(Some(user.clone()));
    Ok(user)
}

#[tauri::command]
pub async fn login(state: State<'_, AppState>, input: LoginInput) -> AppResult<SessionUser> {
    // License kedaluwarsa tetap boleh login, agar pemilik bisa memvalidasi ulang di Pengaturan.
    if !state.license()?.is_installed() {
        return Err(AppError::LicenseRequired("Aplikasi belum diaktifkan. Masukkan license key.".into()));
    }
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
