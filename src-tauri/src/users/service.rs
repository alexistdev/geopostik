use std::collections::{BTreeSet, HashSet};

use rusqlite::{Connection, TransactionBehavior};
use serde_json::{Value, json};

use super::model::{UserInput, UserListQuery, UserListResult, UserRow, UserSaveResult};
use super::repo::{self, UserFields};
use crate::audit::{self, Change, Entry};
use crate::auth::{self, Role, SessionUser, password};
use crate::error::{AppError, AppResult};
use crate::master::BatchResult;

const ENTITY: &str = "users";

pub fn list(conn: &Connection, query: &UserListQuery) -> AppResult<UserListResult> {
    let (rows, total) = repo::list(conn, query)?;
    Ok(UserListResult { rows, total })
}

pub fn get(conn: &Connection, id: i64) -> AppResult<UserRow> {
    repo::get(conn, id)?.ok_or_else(not_found)
}

fn not_found() -> AppError {
    AppError::NotFound("Pengguna tidak ditemukan".into())
}

/// Tambah atau ubah pengguna beserta perannya. Password dan PIN wajib untuk pengguna baru; saat
/// mengubah, kosongkan agar tidak diganti. Setiap perubahan (termasuk ganti password/PIN, tanpa
/// nilainya) dicatat di log audit.
pub fn save(conn: &mut Connection, actor: &SessionUser, input: &UserInput) -> AppResult<UserSaveResult> {
    let full_name = auth::required(&input.full_name, "Nama lengkap")?;
    let username = auth::validate_username(&input.username)?;
    let roles: Vec<Role> = input.roles.iter().copied().collect::<BTreeSet<_>>().into_iter().collect();
    if roles.is_empty() {
        return Err(AppError::Validation("Pilih minimal satu peran".into()));
    }
    let license_number = input.license_number.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let license_type = if roles.contains(&Role::Pharmacist) {
        if license_number.is_none() {
            return Err(AppError::Validation("Nomor SIPA wajib diisi untuk apoteker".into()));
        }
        Some("SIPA")
    } else if roles.contains(&Role::Technician) {
        license_number.map(|_| "SIPTTK")
    } else {
        None
    };
    // Nomor izin hanya disimpan untuk apoteker/TTK.
    let license_number = license_type.and(license_number);

    let new_password = secret(&input.password);
    let new_pin = secret(&input.pin);
    if input.id.is_none() {
        if new_password.is_none() {
            return Err(AppError::Validation("Password wajib diisi".into()));
        }
        if new_pin.is_none() {
            return Err(AppError::Validation("PIN wajib diisi".into()));
        }
    }
    if let Some(p) = new_password {
        auth::validate_password(p)?;
    }
    if let Some(p) = new_pin {
        auth::validate_pin(p)?;
    }
    // Hash di luar transaksi: argon2 sengaja lambat.
    let password_hash = new_password.map(password::hash).transpose()?;
    let pin_hash = new_pin.map(password::hash).transpose()?;

    let fields = UserFields {
        username: &username,
        full_name,
        license_type,
        license_number,
        is_active: input.is_active,
    };

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if repo::username_taken(&tx, &username, input.id)? {
        return Err(AppError::Conflict(format!(
            "Username {username} sudah dipakai (termasuk oleh pengguna yang sudah dihapus)"
        )));
    }
    let id = match input.id {
        None => {
            let (Some(password_hash), Some(pin_hash)) = (&password_hash, &pin_hash) else {
                unreachable!("password & PIN sudah divalidasi wajib");
            };
            let id = repo::insert(&tx, &fields, password_hash, pin_hash, actor.id)?;
            repo::set_roles(&tx, id, &roles)?;
            audit::log_change(
                &tx,
                Change {
                    user_id: actor.id,
                    action: audit::CREATE,
                    entity: ENTITY,
                    entity_id: id,
                    before: None,
                    after: Some(snapshot(&tx, id)?),
                    reason: None,
                },
            )?;
            id
        }
        Some(id) => {
            let current = repo::get(&tx, id)?.ok_or_else(not_found)?;
            if id == actor.id {
                if !input.is_active {
                    return Err(AppError::Conflict("Tidak bisa menonaktifkan akun sendiri".into()));
                }
                if current.roles.contains(&Role::Owner) && !roles.contains(&Role::Owner) {
                    return Err(AppError::Conflict("Tidak bisa melepas peran Pemilik dari akun sendiri".into()));
                }
            }
            let stays_owner = input.is_active && roles.contains(&Role::Owner);
            if !stays_owner {
                ensure_other_owner(&tx, &current)?;
            }

            let before = snapshot(&tx, id)?;
            repo::update(&tx, id, &fields)?;
            repo::set_roles(&tx, id, &roles)?;
            let action = match (current.is_active, input.is_active) {
                (false, true) => audit::ACTIVATE,
                (true, false) => audit::DEACTIVATE,
                _ => audit::UPDATE,
            };
            audit::log_change(
                &tx,
                Change {
                    user_id: actor.id,
                    action,
                    entity: ENTITY,
                    entity_id: id,
                    before: Some(before),
                    after: Some(snapshot(&tx, id)?),
                    reason: None,
                },
            )?;
            if let Some(hash) = &password_hash {
                repo::set_password(&tx, id, hash)?;
                log_secret_change(&tx, actor, id, audit::PASSWORD_CHANGE, &username, full_name)?;
            }
            if let Some(hash) = &pin_hash {
                repo::set_pin(&tx, id, hash)?;
                log_secret_change(&tx, actor, id, audit::PIN_CHANGE, &username, full_name)?;
            }
            id
        }
    };
    tx.commit()?;

    let user = get(conn, id)?;
    let session = match id == actor.id {
        true => Some(auth::session_for(conn, id, user.username.clone(), user.full_name.clone())?),
        false => None,
    };
    Ok(UserSaveResult { user, session })
}

fn secret(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|v| !v.is_empty())
}

fn log_secret_change(tx: &Connection, actor: &SessionUser, id: i64, action: &str, username: &str, full_name: &str) -> AppResult<()> {
    audit::log(
        tx,
        Entry {
            user_id: Some(actor.id),
            action,
            entity: Some(ENTITY),
            entity_id: Some(id),
            detail: Some(json!({ "code": username, "name": full_name })),
            ..Default::default()
        },
    )
}

/// Harus selalu ada minimal satu pemilik aktif, agar menu Pengguna & Pengaturan tetap bisa dibuka.
fn ensure_other_owner(conn: &Connection, user: &UserRow) -> AppResult<()> {
    if user.is_active && user.roles.contains(&Role::Owner) && repo::other_active_owners(conn, user.id)? == 0 {
        return Err(AppError::Conflict(format!(
            "{} adalah satu-satunya pemilik aktif. Tambahkan pemilik lain terlebih dahulu.",
            user.full_name
        )));
    }
    Ok(())
}

/// Nonaktif / hapus akun sendiri atau pemilik aktif terakhir ditolak.
fn ensure_can_disable(conn: &Connection, actor: &SessionUser, user: &UserRow, verb: &str) -> AppResult<()> {
    if user.id == actor.id {
        return Err(AppError::Conflict(format!("Tidak bisa {verb} akun sendiri")));
    }
    ensure_other_owner(conn, user)
}

pub fn set_active(conn: &mut Connection, actor: &SessionUser, id: i64, active: bool) -> AppResult<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    set_active_in(&tx, actor, id, active)?;
    tx.commit()?;
    Ok(())
}

/// `Ok(false)` bila status sudah sesuai (tidak ada perubahan, tidak dicatat).
fn set_active_in(tx: &Connection, actor: &SessionUser, id: i64, active: bool) -> AppResult<bool> {
    let user = repo::get(tx, id)?.ok_or_else(not_found)?;
    if user.is_active == active {
        return Ok(false);
    }
    if !active {
        ensure_can_disable(tx, actor, &user, "menonaktifkan")?;
    }
    let before = snapshot(tx, id)?;
    repo::set_active(tx, id, active)?;
    audit::log_change(
        tx,
        Change {
            user_id: actor.id,
            action: if active { audit::ACTIVATE } else { audit::DEACTIVATE },
            entity: ENTITY,
            entity_id: id,
            before: Some(before),
            after: Some(snapshot(tx, id)?),
            reason: None,
        },
    )?;
    Ok(true)
}

/// Aktifkan/nonaktifkan banyak pengguna dalam satu transaksi. Pengguna yang ditolak (akun sendiri,
/// pemilik terakhir) dilewati dan disebutkan di `skipped`.
pub fn set_active_many(conn: &mut Connection, actor: &SessionUser, ids: &[i64], active: bool) -> AppResult<BatchResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut result = BatchResult::default();
    for id in unique(ids) {
        match set_active_in(&tx, actor, id, active) {
            Ok(changed) => result.done += i64::from(changed),
            Err(AppError::Conflict(m) | AppError::NotFound(m)) => result.skipped.push(m),
            Err(e) => return Err(e),
        }
    }
    tx.commit()?;
    Ok(result)
}

/// Hapus pengguna (soft delete): disembunyikan dan tidak bisa login lagi, tetapi tidak pernah
/// dihapus permanen sehingga transaksi dan log yang menunjuk ke user ini tetap utuh.
pub fn delete(conn: &mut Connection, actor: &SessionUser, id: i64) -> AppResult<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    delete_in(&tx, actor, id)?;
    tx.commit()?;
    Ok(())
}

fn delete_in(tx: &Connection, actor: &SessionUser, id: i64) -> AppResult<()> {
    let user = repo::get(tx, id)?.ok_or_else(not_found)?;
    ensure_can_disable(tx, actor, &user, "menghapus")?;
    let before = snapshot(tx, id)?;
    repo::soft_delete(tx, id, actor.id)?;
    audit::log_change(
        tx,
        Change {
            user_id: actor.id,
            action: audit::DELETE,
            entity: ENTITY,
            entity_id: id,
            before: Some(before),
            after: None,
            reason: None,
        },
    )
}

pub fn delete_many(conn: &mut Connection, actor: &SessionUser, ids: &[i64]) -> AppResult<BatchResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut result = BatchResult::default();
    for id in unique(ids) {
        match delete_in(&tx, actor, id) {
            Ok(()) => result.done += 1,
            Err(AppError::Conflict(m) | AppError::NotFound(m)) => result.skipped.push(m),
            Err(e) => return Err(e),
        }
    }
    tx.commit()?;
    Ok(result)
}

fn unique(ids: &[i64]) -> Vec<i64> {
    let mut seen = HashSet::new();
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

/// Isi pengguna untuk log audit (tanpa hash password/PIN). Peran ditulis sebagai kode dipisah koma;
/// diterjemahkan di menu Log.
fn snapshot(conn: &Connection, id: i64) -> AppResult<Value> {
    let u = get(conn, id)?;
    let roles: Vec<&str> = u.roles.iter().map(|r| r.as_str()).collect();
    Ok(json!({
        "username": u.username,
        "name": u.full_name,
        "roles": roles.join(", "),
        "licenseType": u.license_type,
        "licenseNumber": u.license_number,
        "isActive": u.is_active,
    }))
}
