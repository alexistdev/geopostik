use rusqlite::{Connection, TransactionBehavior};
use serde_json::json;

use super::model::{LoginInput, PharmacyProfile, SessionUser, SetupInput};
use super::repo::{self, NewUser};
use super::{Role, effective_permissions, password};
use crate::audit;
use crate::error::{AppError, AppResult};
use crate::settings;

pub fn needs_setup(conn: &Connection) -> AppResult<bool> {
    Ok(repo::count_users(conn)? == 0)
}

/// Setup awal: membuat akun pemilik pertama. Hanya bisa bila belum ada user sama sekali.
pub fn setup_owner(conn: &mut Connection, input: &SetupInput) -> AppResult<SessionUser> {
    let pharmacy_name = required(&input.pharmacy_name, "Nama apotek")?;
    let full_name = required(&input.full_name, "Nama lengkap")?;
    let username = validate_username(&input.username)?;
    validate_password(&input.password)?;
    validate_pin(&input.pin)?;
    let license_number = match input.also_pharmacist {
        true => Some(required(input.license_number.as_deref().unwrap_or(""), "Nomor SIPA")?),
        false => None,
    };

    let password_hash = password::hash(&input.password)?;
    let pin_hash = password::hash(&input.pin)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if repo::count_users(&tx)? > 0 {
        return Err(AppError::Conflict("Setup awal sudah pernah dilakukan".into()));
    }

    let user_id = repo::insert_user(
        &tx,
        &NewUser {
            username: &username,
            full_name,
            password_hash: &password_hash,
            pin_hash: Some(&pin_hash),
            license_type: license_number.map(|_| "SIPA"),
            license_number,
        },
    )?;
    repo::insert_role(&tx, user_id, Role::Owner)?;
    if input.also_pharmacist {
        repo::insert_role(&tx, user_id, Role::Pharmacist)?;
    }
    settings::set(&tx, settings::PHARMACY_PROFILE, &PharmacyProfile { name: pharmacy_name.to_owned() })?;
    audit::log(
        &tx,
        audit::Entry {
            user_id: Some(user_id),
            action: "INITIAL_SETUP",
            entity: Some("users"),
            entity_id: Some(user_id),
            detail: Some(json!({ "username": username, "alsoPharmacist": input.also_pharmacist })),
            ..Default::default()
        },
    )?;
    tx.commit()?;

    session_for(conn, user_id, username, full_name.to_owned())
}

pub fn login(conn: &Connection, input: &LoginInput) -> AppResult<SessionUser> {
    let invalid = || AppError::Unauthenticated("Username atau password salah".into());

    let row = repo::find_by_username(conn, input.username.trim())?.ok_or_else(invalid)?;
    if !password::verify(&input.password, &row.password_hash) {
        return Err(invalid());
    }
    if !row.is_active {
        return Err(AppError::Unauthenticated("Akun ini sudah dinonaktifkan".into()));
    }

    audit::log(
        conn,
        audit::Entry {
            user_id: Some(row.id),
            action: "LOGIN",
            ..Default::default()
        },
    )?;
    session_for(conn, row.id, row.username, row.full_name)
}

fn session_for(conn: &Connection, id: i64, username: String, full_name: String) -> AppResult<SessionUser> {
    let roles = repo::roles_of(conn, id)?;
    let permissions = effective_permissions(&roles, settings::access(conn)?);
    Ok(SessionUser {
        id,
        username,
        full_name,
        roles,
        permissions,
    })
}

fn required<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::Validation(format!("{label} wajib diisi")));
    }
    Ok(v)
}

fn validate_username(username: &str) -> AppResult<String> {
    let u = username.trim().to_lowercase();
    let valid_chars = u.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
    if !(3..=32).contains(&u.len()) || !valid_chars {
        return Err(AppError::Validation(
            "Username 3–32 karakter, hanya huruf, angka, titik, dan garis bawah".into(),
        ));
    }
    Ok(u)
}

fn validate_password(password: &str) -> AppResult<()> {
    if password.chars().count() < 6 {
        return Err(AppError::Validation("Password minimal 6 karakter".into()));
    }
    Ok(())
}

fn validate_pin(pin: &str) -> AppResult<()> {
    if !(4..=6).contains(&pin.len()) || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Validation("PIN harus 4–6 digit angka".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Permission;
    use crate::db;

    fn setup_input() -> SetupInput {
        SetupInput {
            pharmacy_name: "Apotek Sehat".into(),
            full_name: "Budi".into(),
            username: "Budi".into(),
            password: "rahasia123".into(),
            pin: "1234".into(),
            also_pharmacist: false,
            license_number: None,
        }
    }

    #[test]
    fn setup_then_login() {
        let mut conn = db::open_in_memory().unwrap();
        assert!(needs_setup(&conn).unwrap());

        let owner = setup_owner(&mut conn, &setup_input()).unwrap();
        assert_eq!(owner.username, "budi");
        assert_eq!(owner.roles, vec![Role::Owner]);
        assert!(!needs_setup(&conn).unwrap());

        let session = login(
            &conn,
            &LoginInput {
                username: "budi".into(),
                password: "rahasia123".into(),
            },
        )
        .unwrap();
        assert!(session.permissions.contains(&Permission::UserManage));

        let wrong = login(
            &conn,
            &LoginInput {
                username: "budi".into(),
                password: "salah".into(),
            },
        );
        assert!(matches!(wrong, Err(AppError::Unauthenticated(_))));
    }

    #[test]
    fn setup_only_once() {
        let mut conn = db::open_in_memory().unwrap();
        setup_owner(&mut conn, &setup_input()).unwrap();
        let again = setup_owner(&mut conn, &setup_input());
        assert!(matches!(again, Err(AppError::Conflict(_))));
    }

    #[test]
    fn owner_pharmacist_requires_license() {
        let mut conn = db::open_in_memory().unwrap();
        let mut input = setup_input();
        input.also_pharmacist = true;
        assert!(matches!(setup_owner(&mut conn, &input), Err(AppError::Validation(_))));

        input.license_number = Some("SIPA-123".into());
        let owner = setup_owner(&mut conn, &input).unwrap();
        assert_eq!(owner.roles, vec![Role::Owner, Role::Pharmacist]);
        assert!(owner.permissions.contains(&Permission::SellHardDrug));
    }

    #[test]
    fn rejects_bad_pin() {
        let mut conn = db::open_in_memory().unwrap();
        let mut input = setup_input();
        input.pin = "12a4".into();
        assert!(matches!(setup_owner(&mut conn, &input), Err(AppError::Validation(_))));
    }
}
