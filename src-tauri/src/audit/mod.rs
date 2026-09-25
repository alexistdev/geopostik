//! Log audit (append-only) untuk aksi sensitif dan setiap perubahan data master.
//! Dibaca lewat menu Sistem › Log (hak AUDIT_VIEW).

pub mod commands;
mod model;
mod repo;

#[cfg(test)]
mod tests;

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::error::AppResult;

/// Aksi pada data. Aksi lama (sebelum menu Log) memakai nama berawalan entitas, misal `PRODUCT_CREATE`.
pub const CREATE: &str = "CREATE";
pub const UPDATE: &str = "UPDATE";
pub const DELETE: &str = "DELETE";
pub const ACTIVATE: &str = "ACTIVATE";
pub const DEACTIVATE: &str = "DEACTIVATE";
pub const PRICE_CHANGE: &str = "PRICE_CHANGE";
/// Harga otomatis dihitung ulang karena data lain berubah (misal margin kategori).
pub const PRICE_RECALC: &str = "PRICE_RECALC";
/// Password / PIN user diganti (nilainya tidak pernah dicatat).
pub const PASSWORD_CHANGE: &str = "PASSWORD_CHANGE";
pub const PIN_CHANGE: &str = "PIN_CHANGE";

#[derive(Default)]
pub struct Entry<'a> {
    pub user_id: Option<i64>,
    pub authorized_by: Option<i64>,
    pub action: &'a str,
    pub entity: Option<&'a str>,
    pub entity_id: Option<i64>,
    pub detail: Option<serde_json::Value>,
    pub reason: Option<&'a str>,
}

pub fn log(conn: &Connection, entry: Entry<'_>) -> AppResult<()> {
    conn.execute(
        "INSERT INTO audit_logs (user_id, authorized_by, action, entity, entity_id, detail, reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entry.user_id,
            entry.authorized_by,
            entry.action,
            entry.entity,
            entry.entity_id,
            entry.detail.map(|d| d.to_string()),
            entry.reason,
        ],
    )?;
    Ok(())
}

/// Perubahan satu data, dicatat sebagai snapshot lengkap sebelum dan sesudah.
pub struct Change<'a> {
    pub user_id: i64,
    pub action: &'a str,
    pub entity: &'a str,
    pub entity_id: i64,
    /// `None` untuk data baru.
    pub before: Option<Value>,
    /// `None` untuk data yang dihapus.
    pub after: Option<Value>,
    pub reason: Option<&'a str>,
}

/// Catat perubahan dengan detail `{ code, name, before, after }`. Kode & nama diambil dari snapshot
/// (untuk user, `username` menjadi kode) agar log tetap terbaca walau datanya kelak diubah atau
/// dihapus. Simpan tanpa perubahan apa pun (snapshot sama persis) tidak dicatat.
pub fn log_change(conn: &Connection, change: Change<'_>) -> AppResult<()> {
    if change.before.is_some() && change.before == change.after {
        return Ok(());
    }
    let current = change.after.as_ref().or(change.before.as_ref());
    let field = |key: &str| current.and_then(|s| s.get(key)).cloned().unwrap_or(Value::Null);
    let code = match field("code") {
        Value::Null => field("username"),
        code => code,
    };
    log(
        conn,
        Entry {
            user_id: Some(change.user_id),
            action: change.action,
            entity: Some(change.entity),
            entity_id: Some(change.entity_id),
            detail: Some(json!({
                "code": code,
                "name": field("name"),
                "before": change.before,
                "after": change.after,
            })),
            reason: change.reason,
            ..Default::default()
        },
    )
}
