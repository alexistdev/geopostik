use tauri::State;

use super::TaxSettings;
use crate::audit::{self, Change};
use crate::auth::Permission;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[tauri::command]
pub async fn tax_settings_get(state: State<'_, AppState>) -> AppResult<TaxSettings> {
    state.current_user()?;
    super::tax(&*state.db()?)
}

/// PKP dan tarif PPN default. Berlaku untuk faktur yang diposting setelahnya; faktur lama tetap
/// memakai snapshot saat diposting.
#[tauri::command]
pub async fn tax_settings_save(state: State<'_, AppState>, input: TaxSettings) -> AppResult<TaxSettings> {
    let user = state.require(Permission::SettingsManage)?;
    if !(0..=10_000).contains(&input.ppn_rate_bp) {
        return Err(AppError::Validation("Tarif PPN harus 0–100%".into()));
    }
    let conn = state.db()?;
    let tx = conn.unchecked_transaction()?;
    let before = super::tax(&tx)?;
    super::set(&tx, super::TAX_IS_PKP, &input.is_pkp)?;
    super::set(&tx, super::TAX_PPN_RATE_BP, &input.ppn_rate_bp)?;
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: audit::UPDATE,
            entity: "settings",
            entity_id: 0,
            before: Some(snapshot(before)),
            after: Some(snapshot(input)),
            reason: None,
        },
    )?;
    tx.commit()?;
    Ok(input)
}

fn snapshot(t: TaxSettings) -> serde_json::Value {
    serde_json::json!({ "name": "Pajak", "isPkp": t.is_pkp, "ppnRateBp": t.ppn_rate_bp })
}
