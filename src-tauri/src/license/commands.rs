use chrono::Utc;
use tauri::State;

use super::{LicenseStatus, client, normalize_key};
use crate::auth::Permission;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

// Status & validasi ulang tidak butuh login: status dipakai sebelum setup/login, dan validasi
// ulang hanya memperbarui license yang sudah terpasang (kasir pun boleh menekannya).

#[tauri::command]
pub async fn license_status(state: State<'_, AppState>) -> AppResult<LicenseStatus> {
    Ok(state.license()?.check())
}

/// Aktivasi pertama (tanpa login), atau mengganti license key (khusus pemilik).
#[tauri::command]
pub async fn license_activate(state: State<'_, AppState>, license_key: String) -> AppResult<LicenseStatus> {
    let key = normalize_key(&license_key)?;
    if state.license()?.is_installed() {
        state.require_unlicensed(Permission::SettingsManage)?;
    }
    activate(&state, key).await
}

/// Memvalidasi ulang license yang terpasang ke server (butuh internet).
#[tauri::command]
pub async fn license_revalidate(state: State<'_, AppState>) -> AppResult<LicenseStatus> {
    let key = state
        .license()?
        .license_key()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Validation("Belum ada license yang terpasang".into()))?;
    activate(&state, key).await
}

async fn activate(state: &AppState, key: String) -> AppResult<LicenseStatus> {
    // Panggilan jaringan dilakukan tanpa mengunci license, agar command lain tidak ikut menunggu.
    let machine_id = state.license()?.machine_id().to_owned();
    let request_key = key.clone();
    let activation = tauri::async_runtime::spawn_blocking(move || client::activate(&request_key, &machine_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .map_err(AppError::Validation)?;

    let mut license = state.license()?;
    license.apply(key, activation, Utc::now())?;
    Ok(license.check())
}
