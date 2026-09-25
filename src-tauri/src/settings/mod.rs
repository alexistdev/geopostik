//! Pengaturan aplikasi: key-value dengan nilai JSON.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::auth::AccessSettings;
use crate::error::{AppError, AppResult};

pub const PHARMACY_PROFILE: &str = "pharmacy.profile";
pub const ACCESS_PHARMACIST_CAN_VIEW_COST: &str = "access.pharmacist_can_view_cost";
pub const ACCESS_PHARMACIST_CAN_EDIT_PRICE: &str = "access.pharmacist_can_edit_price";

pub fn get<T: DeserializeOwned>(conn: &Connection, key: &str) -> AppResult<Option<T>> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()?;
    raw.map(|v| {
        serde_json::from_str(&v)
            .map_err(|e| AppError::Internal(format!("pengaturan '{key}' rusak: {e}")))
    })
    .transpose()
}

pub fn set<T: Serialize>(conn: &Connection, key: &str, value: &T) -> AppResult<()> {
    let json = serde_json::to_string(value).map_err(|e| AppError::Internal(e.to_string()))?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value,
                                         updated_at = datetime('now', 'localtime')",
        params![key, json],
    )?;
    Ok(())
}

pub fn access(conn: &Connection) -> AppResult<AccessSettings> {
    let default = AccessSettings::default();
    Ok(AccessSettings {
        pharmacist_can_view_cost: get(conn, ACCESS_PHARMACIST_CAN_VIEW_COST)?
            .unwrap_or(default.pharmacist_can_view_cost),
        pharmacist_can_edit_price: get(conn, ACCESS_PHARMACIST_CAN_EDIT_PRICE)?
            .unwrap_or(default.pharmacist_can_edit_price),
    })
}
