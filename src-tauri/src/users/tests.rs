use rusqlite::Connection;
use serde_json::Value;

use super::model::{UserInput, UserListQuery};
use super::service;
use crate::auth::{AccessSettings, Role, SessionUser, effective_permissions, password};
use crate::db;
use crate::error::AppError;

/// Database dengan satu pemilik (id 1) yang sedang login.
fn setup() -> (Connection, SessionUser) {
    let conn = db::open_in_memory().unwrap();
    conn.execute_batch(&format!(
        "INSERT INTO users (id, username, full_name, password_hash) VALUES (1, 'owner', 'Pemilik', '{}');
         INSERT INTO user_roles (user_id, role) VALUES (1, 'OWNER');",
        password::hash("rahasia123").unwrap()
    ))
    .unwrap();
    let owner = SessionUser {
        id: 1,
        username: "owner".into(),
        full_name: "Pemilik".into(),
        roles: vec![Role::Owner],
        permissions: effective_permissions(&[Role::Owner], AccessSettings::default()),
    };
    (conn, owner)
}

fn input(username: &str, roles: &[Role]) -> UserInput {
    UserInput {
        id: None,
        username: username.into(),
        full_name: format!("User {username}"),
        roles: roles.to_vec(),
        license_number: None,
        password: Some("rahasia123".into()),
        pin: Some("1234".into()),
        is_active: true,
    }
}

fn query() -> UserListQuery {
    UserListQuery { limit: 50, ..Default::default() }
}

/// (aksi, detail JSON) log audit untuk user `id`, urut dari yang terlama.
fn logs(conn: &Connection, id: i64) -> Vec<(String, Value)> {
    conn.prepare("SELECT action, detail FROM audit_logs WHERE entity = 'users' AND entity_id = ?1 ORDER BY id")
        .unwrap()
        .query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
        .map(|r| {
            let (action, detail) = r.unwrap();
            (action, serde_json::from_str(&detail).unwrap())
        })
        .collect()
}

#[test]
fn create_update_and_log_every_change() {
    let (mut conn, owner) = setup();
    let mut kasir = input("Sari", &[Role::Cashier]);
    let created = service::save(&mut conn, &owner, &kasir).unwrap().user;
    assert_eq!(created.username, "sari");
    assert_eq!(created.roles, vec![Role::Cashier]);
    assert!(created.has_pin);
    assert_eq!(created.created_by.as_deref(), Some("owner"));

    // Ubah nama + tambah peran TTK, password & PIN baru.
    kasir.id = Some(created.id);
    kasir.full_name = "Sari Dewi".into();
    kasir.roles = vec![Role::Technician, Role::Cashier];
    kasir.license_number = Some("SIPTTK-9".into());
    kasir.password = Some("barubaru".into());
    kasir.pin = Some("5678".into());
    let updated = service::save(&mut conn, &owner, &kasir).unwrap().user;
    assert_eq!(updated.roles, vec![Role::Technician, Role::Cashier]);
    assert_eq!(updated.license_type.as_deref(), Some("SIPTTK"));

    // Simpan tanpa perubahan & tanpa password/PIN baru tidak dicatat.
    kasir.password = None;
    kasir.pin = Some(String::new());
    service::save(&mut conn, &owner, &kasir).unwrap();

    let log = logs(&conn, created.id);
    let actions: Vec<&str> = log.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(actions, ["CREATE", "UPDATE", "PASSWORD_CHANGE", "PIN_CHANGE"]);
    let update = &log[1].1;
    assert_eq!(update["code"], "sari", "username menjadi kode di log");
    assert_eq!(update["before"]["name"], "User Sari");
    assert_eq!(update["after"]["roles"], "TECHNICIAN, CASHIER");
    // Hash password/PIN tidak pernah masuk log.
    let all: String = log.iter().map(|(_, d)| d.to_string()).collect();
    assert!(!all.contains("argon2"));

    // Password baru berlaku.
    let hash: String = conn.query_row("SELECT password_hash FROM users WHERE id = ?1", [created.id], |r| r.get(0)).unwrap();
    assert!(password::verify("barubaru", &hash));
}

#[test]
fn validation_rules() {
    let (mut conn, owner) = setup();
    let mut bad = input("apt", &[Role::Pharmacist]);
    assert!(matches!(service::save(&mut conn, &owner, &bad), Err(AppError::Validation(_))), "SIPA wajib");
    bad.license_number = Some("SIPA-1".into());
    let apt = service::save(&mut conn, &owner, &bad).unwrap().user;
    assert_eq!(apt.license_type.as_deref(), Some("SIPA"));

    assert!(matches!(service::save(&mut conn, &owner, &input("x", &[Role::Cashier])), Err(AppError::Validation(_))));
    assert!(matches!(service::save(&mut conn, &owner, &input("norole", &[])), Err(AppError::Validation(_))));
    let mut no_pin = input("nopin", &[Role::Cashier]);
    no_pin.pin = None;
    assert!(matches!(service::save(&mut conn, &owner, &no_pin), Err(AppError::Validation(_))));

    // Kasir tidak menyimpan nomor izin.
    let mut kasir = input("kasir", &[Role::Cashier]);
    kasir.license_number = Some("123".into());
    assert_eq!(service::save(&mut conn, &owner, &kasir).unwrap().user.license_number, None);

    assert!(matches!(service::save(&mut conn, &owner, &input("KASIR", &[Role::Cashier])), Err(AppError::Conflict(_))));
}

#[test]
fn soft_delete_hides_user_blocks_login_and_reserves_username() {
    let (mut conn, owner) = setup();
    let sari = service::save(&mut conn, &owner, &input("sari", &[Role::Cashier])).unwrap().user;
    service::delete(&mut conn, &owner, sari.id).unwrap();

    assert_eq!(service::list(&conn, &UserListQuery { include_inactive: true, ..query() }).unwrap().total, 1);
    assert!(matches!(service::get(&conn, sari.id), Err(AppError::NotFound(_))));
    // Tidak pernah dihapus permanen.
    let (deleted_at, deleted_by): (Option<String>, Option<i64>) = conn
        .query_row("SELECT deleted_at, deleted_by FROM users WHERE id = ?1", [sari.id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert!(deleted_at.is_some());
    assert_eq!(deleted_by, Some(owner.id));

    let login = crate::auth::login_for_test(&conn, "sari", "rahasia123");
    assert!(matches!(login, Err(AppError::Unauthenticated(_))));
    assert!(matches!(service::save(&mut conn, &owner, &input("sari", &[Role::Cashier])), Err(AppError::Conflict(_))));

    let log = logs(&conn, sari.id);
    assert_eq!(log.last().unwrap().0, "DELETE");
    assert_eq!(log.last().unwrap().1["before"]["username"], "sari");
}

#[test]
fn cannot_lock_out_self_or_last_owner() {
    let (mut conn, owner) = setup();
    assert!(matches!(service::delete(&mut conn, &owner, owner.id), Err(AppError::Conflict(_))));
    assert!(matches!(service::set_active(&mut conn, &owner, owner.id, false), Err(AppError::Conflict(_))));
    let mut me = input("owner", &[Role::Pharmacist]);
    me.id = Some(owner.id);
    me.license_number = Some("SIPA-1".into());
    assert!(matches!(service::save(&mut conn, &owner, &me), Err(AppError::Conflict(_))), "lepas peran pemilik sendiri");

    // Menambah peran apoteker ke akun sendiri boleh, dan sesi ikut diperbarui.
    me.roles = vec![Role::Owner, Role::Pharmacist];
    let session = service::save(&mut conn, &owner, &me).unwrap().session.unwrap();
    assert_eq!(session.roles, vec![Role::Owner, Role::Pharmacist]);

    // Pemilik lain bisa dinonaktifkan selama masih ada pemilik aktif.
    let second = service::save(&mut conn, &owner, &input("owner2", &[Role::Owner])).unwrap().user;
    service::set_active(&mut conn, &owner, second.id, false).unwrap();

    // Pemilik aktif terakhir (dilihat dari sesi pemilik kedua) tidak bisa dihapus.
    service::set_active(&mut conn, &owner, second.id, true).unwrap();
    let second_session = SessionUser { id: second.id, ..owner.clone() };
    service::delete(&mut conn, &second_session, owner.id).unwrap();
    assert!(matches!(service::delete(&mut conn, &owner, second.id), Err(AppError::Conflict(_))), "sisa satu pemilik");
}

#[test]
fn batch_actions_skip_rejected_users() {
    let (mut conn, owner) = setup();
    let a = service::save(&mut conn, &owner, &input("a.kasir", &[Role::Cashier])).unwrap().user;
    let b = service::save(&mut conn, &owner, &input("b.kasir", &[Role::Cashier])).unwrap().user;

    let result = service::set_active_many(&mut conn, &owner, &[a.id, b.id, owner.id, a.id], false).unwrap();
    assert_eq!(result.done, 2);
    assert_eq!(result.skipped.len(), 1, "akun sendiri dilewati");
    assert_eq!(service::list(&conn, &query()).unwrap().total, 1, "hanya pemilik yang aktif");

    // Sudah nonaktif → tidak dihitung dan tidak dicatat lagi.
    assert_eq!(service::set_active_many(&mut conn, &owner, &[a.id], false).unwrap().done, 0);
    assert_eq!(logs(&conn, a.id).len(), 2);

    let result = service::delete_many(&mut conn, &owner, &[a.id, b.id, 999]).unwrap();
    assert_eq!(result.done, 2);
    assert_eq!(result.skipped.len(), 1);
}

#[test]
fn list_filters_by_text_and_role() {
    let (mut conn, owner) = setup();
    let mut apt = input("apt.rina", &[Role::Pharmacist]);
    apt.license_number = Some("SIPA-777".into());
    service::save(&mut conn, &owner, &apt).unwrap();
    service::save(&mut conn, &owner, &input("kasir1", &[Role::Cashier])).unwrap();

    let by = |q: UserListQuery| service::list(&conn, &q).unwrap();
    assert_eq!(by(query()).total, 3);
    assert_eq!(by(UserListQuery { role: Some(Role::Pharmacist), ..query() }).rows[0].username, "apt.rina");
    assert_eq!(by(UserListQuery { q: Some("777".into()), ..query() }).total, 1);
    assert_eq!(by(UserListQuery { q: Some("kasir".into()), ..query() }).total, 1);
    assert_eq!(by(UserListQuery { q: Some("_".into()), ..query() }).total, 0, "garis bawah bukan wildcard");
}
