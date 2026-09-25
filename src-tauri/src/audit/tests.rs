use serde_json::json;

use super::model::AuditQuery;
use super::{Change, Entry, log, log_change, repo};
use crate::db;

fn setup() -> rusqlite::Connection {
    let conn = db::open_in_memory().unwrap();
    conn.execute_batch(
        "INSERT INTO users (id, username, full_name, password_hash) VALUES
             (1, 'budi', 'Budi Santoso', 'x'), (2, 'sari', 'Sari Dewi', 'x');",
    )
    .unwrap();
    conn
}

fn change(user_id: i64, action: &'static str, entity: &'static str, before: Option<serde_json::Value>, after: Option<serde_json::Value>) -> Change<'static> {
    Change { user_id, action, entity, entity_id: 1, before, after, reason: None }
}

fn query() -> AuditQuery {
    AuditQuery { limit: 50, ..Default::default() }
}

#[test]
fn unchanged_snapshot_is_not_logged() {
    let conn = setup();
    let same = json!({ "code": "RAK0001", "name": "A1" });
    log_change(&conn, change(1, super::UPDATE, "racks", Some(same.clone()), Some(same))).unwrap();
    assert_eq!(repo::page(&conn, &query()).unwrap().1, 0);
}

#[test]
fn log_cannot_be_edited_or_deleted() {
    let conn = setup();
    log_change(&conn, change(1, super::CREATE, "racks", None, Some(json!({ "code": "RAK0001", "name": "A1" })))).unwrap();
    assert!(conn.execute("UPDATE audit_logs SET action = 'X'", []).is_err());
    assert!(conn.execute("DELETE FROM audit_logs", []).is_err());
}

#[test]
fn page_filters_by_action_entity_user_and_text() {
    let conn = setup();
    log_change(&conn, change(1, super::CREATE, "racks", None, Some(json!({ "code": "RAK0001", "name": "Kulkas" })))).unwrap();
    log_change(
        &conn,
        change(2, super::UPDATE, "categories", Some(json!({ "name": "Vitamin" })), Some(json!({ "name": "Suplemen" }))),
    )
    .unwrap();
    // Aksi lama berawalan entitas tetap cocok dengan filter aksi.
    log(&conn, Entry { user_id: Some(1), action: "PRODUCT_UPDATE", entity: Some("products"), ..Default::default() }).unwrap();
    log(&conn, Entry { user_id: Some(2), action: "LOGIN", ..Default::default() }).unwrap();

    let (rows, total) = repo::page(&conn, &query()).unwrap();
    assert_eq!(total, 4);
    assert_eq!(rows[0].action, "LOGIN", "terbaru di atas");
    assert_eq!(rows[0].full_name.as_deref(), Some("Sari Dewi"));

    let by = |q: AuditQuery| repo::page(&conn, &q).unwrap().1;
    assert_eq!(by(AuditQuery { action: Some("UPDATE".into()), ..query() }), 2);
    assert_eq!(by(AuditQuery { entity: Some("racks".into()), ..query() }), 1);
    assert_eq!(by(AuditQuery { user_id: Some(2), ..query() }), 2);
    assert_eq!(by(AuditQuery { q: Some("kulkas".into()), ..query() }), 1);
    assert_eq!(by(AuditQuery { q: Some("budi".into()), ..query() }), 2);
    assert_eq!(by(AuditQuery { q: Some("vitamin".into()), ..query() }), 1, "nilai sebelum ikut dicari");

    let today: String = conn.query_row("SELECT date('now', 'localtime')", [], |r| r.get(0)).unwrap();
    assert_eq!(by(AuditQuery { date_from: Some(today.clone()), date_to: Some(today), ..query() }), 4);
    assert_eq!(by(AuditQuery { date_to: Some("2000-01-01".into()), ..query() }), 0);

    let users = repo::users(&conn).unwrap();
    assert_eq!(users.iter().map(|u| u.username.as_str()).collect::<Vec<_>>(), ["budi", "sari"]);
}
