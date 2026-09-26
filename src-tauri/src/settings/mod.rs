//! Pengaturan aplikasi: key-value dengan nilai JSON.

pub mod commands;

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

pub const PRICE_DEFAULT_MARGIN_BP: &str = "price.default_margin_bp";
pub const PRICE_ROUNDING: &str = "price.rounding";

/// Pengaturan harga jual otomatis.
#[derive(Debug, Clone, Copy)]
pub struct PriceSettings {
    /// Margin bila obat dan kategori tidak punya margin sendiri.
    pub default_margin_bp: i64,
    /// Harga otomatis dibulatkan ke atas ke kelipatan ini (rupiah).
    pub rounding: i64,
}

pub fn price(conn: &Connection) -> AppResult<PriceSettings> {
    Ok(PriceSettings {
        default_margin_bp: get(conn, PRICE_DEFAULT_MARGIN_BP)?.unwrap_or(2_000),
        rounding: get(conn, PRICE_ROUNDING)?.unwrap_or(100),
    })
}

pub const TAX_IS_PKP: &str = "tax.is_pkp";
pub const TAX_PPN_RATE_BP: &str = "tax.ppn_rate_bp";

/// Pengaturan pajak (FLOW.md §6: PKP ya/tidak; PPN pembelian selalu ditangani).
#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaxSettings {
    /// Apotek Pengusaha Kena Pajak: PPN masukan bisa dikreditkan, jadi tidak masuk HPP.
    pub is_pkp: bool,
    /// Tarif PPN default untuk faktur pembelian (basis point, 11% = 1100).
    pub ppn_rate_bp: i64,
}

pub fn tax(conn: &Connection) -> AppResult<TaxSettings> {
    Ok(TaxSettings {
        is_pkp: get(conn, TAX_IS_PKP)?.unwrap_or(false),
        ppn_rate_bp: get(conn, TAX_PPN_RATE_BP)?.unwrap_or(1_100),
    })
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
