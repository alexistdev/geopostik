//! Log audit (append-only) untuk aksi sensitif.

use rusqlite::{Connection, params};

use crate::error::AppResult;

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
