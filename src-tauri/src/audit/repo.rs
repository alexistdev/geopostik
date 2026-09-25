use rusqlite::{Connection, named_params};

use super::model::{AuditQuery, AuditRow, AuditUser};
use crate::error::AppResult;

const FILTER: &str = "(:entity IS NULL OR a.entity = :entity)
      AND (:action IS NULL OR a.action = :action OR a.action LIKE '%\\_' || :action ESCAPE '\\')
      AND (:user_id IS NULL OR a.user_id = :user_id)
      AND (:date_from IS NULL OR a.created_at >= :date_from)
      AND (:date_to IS NULL OR a.created_at < date(:date_to, '+1 day'))
      AND (:like IS NULL OR u.username LIKE :like ESCAPE '\\' OR u.full_name LIKE :like ESCAPE '\\'
           OR a.detail LIKE :like ESCAPE '\\' OR a.reason LIKE :like ESCAPE '\\')";

/// Pola LIKE `%teks%` dengan karakter khusus LIKE di-escape; `None` bila teks kosong.
fn like_pattern(q: Option<&str>) -> Option<String> {
    let q = q.map(str::trim).filter(|q| !q.is_empty())?;
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    Some(format!("%{escaped}%"))
}

fn blank_to_none(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

pub fn page(conn: &Connection, q: &AuditQuery) -> AppResult<(Vec<AuditRow>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let entity = blank_to_none(&q.entity);
    let action = blank_to_none(&q.action);
    let date_from = blank_to_none(&q.date_from);
    let date_to = blank_to_none(&q.date_to);
    let (limit, offset) = (q.limit.clamp(1, 500), q.offset.max(0));

    let total: i64 = conn.query_row(
        &format!("SELECT count(*) FROM audit_logs a LEFT JOIN users u ON u.id = a.user_id WHERE {FILTER}"),
        named_params! {
            ":entity": entity, ":action": action, ":user_id": q.user_id,
            ":date_from": date_from, ":date_to": date_to, ":like": like,
        },
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT a.id, a.created_at, a.user_id, u.username, u.full_name,
                (SELECT au.full_name FROM users au WHERE au.id = a.authorized_by),
                a.action, a.entity, a.entity_id, a.detail, a.reason
         FROM audit_logs a LEFT JOIN users u ON u.id = a.user_id
         WHERE {FILTER}
         ORDER BY a.id DESC LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(
            named_params! {
                ":entity": entity, ":action": action, ":user_id": q.user_id,
                ":date_from": date_from, ":date_to": date_to, ":like": like,
                ":limit": limit, ":offset": offset,
            },
            |r| {
                Ok(AuditRow {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    user_id: r.get(2)?,
                    username: r.get(3)?,
                    full_name: r.get(4)?,
                    authorized_by: r.get(5)?,
                    action: r.get(6)?,
                    entity: r.get(7)?,
                    entity_id: r.get(8)?,
                    detail: r.get(9)?,
                    reason: r.get(10)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn users(conn: &Connection) -> AppResult<Vec<AuditUser>> {
    let mut stmt = conn.prepare(
        "SELECT id, username, full_name FROM users
         WHERE id IN (SELECT DISTINCT user_id FROM audit_logs)
         ORDER BY full_name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |r| Ok(AuditUser { id: r.get(0)?, username: r.get(1)?, full_name: r.get(2)? }))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}
