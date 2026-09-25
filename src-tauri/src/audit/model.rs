use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Query menu Sistem › Log. Semua filter opsional.
#[derive(Debug, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditQuery {
    /// Cari username, nama user, atau isi detail (kode, nama, nilai sebelum/sesudah).
    pub q: Option<String>,
    /// Nama tabel, misal `products`.
    pub entity: Option<String>,
    /// Aksi, misal `UPDATE`. Juga cocok dengan aksi lama berawalan entitas (`PRODUCT_UPDATE`).
    pub action: Option<String>,
    pub user_id: Option<i64>,
    /// Tanggal awal `YYYY-MM-DD` (inklusif).
    pub date_from: Option<String>,
    /// Tanggal akhir `YYYY-MM-DD` (inklusif).
    pub date_to: Option<String>,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditRow {
    pub id: i64,
    pub created_at: String,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub full_name: Option<String>,
    /// Nama user yang mengotorisasi (misal lewat PIN), bila ada.
    pub authorized_by: Option<String>,
    pub action: String,
    pub entity: Option<String>,
    pub entity_id: Option<i64>,
    /// JSON mentah. Untuk perubahan data: `{ code, name, before, after }`.
    pub detail: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditPage {
    pub rows: Vec<AuditRow>,
    pub total: i64,
}

/// User yang pernah tercatat di log, untuk pilihan filter.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditUser {
    pub id: i64,
    pub username: String,
    pub full_name: String,
}
