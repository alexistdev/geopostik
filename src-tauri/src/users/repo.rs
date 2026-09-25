use rusqlite::{Connection, OptionalExtension, named_params, params};

use super::model::{UserListQuery, UserRow};
use crate::auth::Role;
use crate::error::AppResult;

/// Kolom `UserRow` kecuali peran (diambil terpisah per user).
const USER_COLUMNS: &str = "u.id, u.username, u.full_name, u.license_type, u.license_number,
    u.pin_hash IS NOT NULL, u.is_active, u.created_at,
    (SELECT c.username FROM users c WHERE c.id = u.created_by),
    (SELECT max(a.created_at) FROM audit_logs a WHERE a.user_id = u.id AND a.action = 'LOGIN')";

const USER_FILTER: &str = "u.deleted_at IS NULL
      AND (:like IS NULL OR u.username LIKE :like ESCAPE '\\' OR u.full_name LIKE :like ESCAPE '\\'
           OR u.license_number LIKE :like ESCAPE '\\')
      AND (:role IS NULL OR EXISTS (SELECT 1 FROM user_roles r WHERE r.user_id = u.id AND r.role = :role))
      AND (:include_inactive = 1 OR u.is_active = 1)";

fn user_row(r: &rusqlite::Row) -> rusqlite::Result<UserRow> {
    Ok(UserRow {
        id: r.get(0)?,
        username: r.get(1)?,
        full_name: r.get(2)?,
        roles: Vec::new(),
        license_type: r.get(3)?,
        license_number: r.get(4)?,
        has_pin: r.get(5)?,
        is_active: r.get(6)?,
        created_at: r.get(7)?,
        created_by: r.get(8)?,
        last_login_at: r.get(9)?,
    })
}

fn with_roles(conn: &Connection, mut row: UserRow) -> AppResult<UserRow> {
    row.roles = roles_of(conn, row.id)?;
    Ok(row)
}

/// Pola LIKE `%teks%` dengan karakter khusus LIKE di-escape; `None` bila teks kosong.
fn like_pattern(q: Option<&str>) -> Option<String> {
    let q = q.map(str::trim).filter(|q| !q.is_empty())?;
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    Some(format!("%{escaped}%"))
}

pub fn list(conn: &Connection, q: &UserListQuery) -> AppResult<(Vec<UserRow>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let role = q.role.map(Role::as_str);
    let (limit, offset) = (q.limit.clamp(1, 500), q.offset.max(0));
    let total: i64 = conn.query_row(
        &format!("SELECT count(*) FROM users u WHERE {USER_FILTER}"),
        named_params! { ":like": like, ":role": role, ":include_inactive": q.include_inactive },
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {USER_COLUMNS} FROM users u WHERE {USER_FILTER}
         ORDER BY u.full_name COLLATE NOCASE, u.id LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(
            named_params! {
                ":like": like, ":role": role, ":include_inactive": q.include_inactive,
                ":limit": limit, ":offset": offset,
            },
            user_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let rows = rows.into_iter().map(|r| with_roles(conn, r)).collect::<AppResult<_>>()?;
    Ok((rows, total))
}

/// User yang belum dihapus.
pub fn get(conn: &Connection, id: i64) -> AppResult<Option<UserRow>> {
    let row = conn
        .query_row(
            &format!("SELECT {USER_COLUMNS} FROM users u WHERE u.id = ?1 AND u.deleted_at IS NULL"),
            [id],
            user_row,
        )
        .optional()?;
    row.map(|r| with_roles(conn, r)).transpose()
}

pub fn roles_of(conn: &Connection, user_id: i64) -> AppResult<Vec<Role>> {
    let mut stmt = conn.prepare("SELECT role FROM user_roles WHERE user_id = ?1 ORDER BY role")?;
    let roles = stmt
        .query_map([user_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut roles = roles.iter().map(|r| r.parse()).collect::<AppResult<Vec<Role>>>()?;
    roles.sort();
    Ok(roles)
}

/// Username dipakai user lain, termasuk user yang sudah dihapus (username tetap dicadangkan).
pub fn username_taken(conn: &Connection, username: &str, except_id: Option<i64>) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM users WHERE username = ?1 AND id IS NOT ?2)",
        params![username, except_id],
        |r| r.get(0),
    )?)
}

/// Jumlah pemilik aktif selain `except_id`.
pub fn other_active_owners(conn: &Connection, except_id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM users u JOIN user_roles r ON r.user_id = u.id AND r.role = 'OWNER'
         WHERE u.id <> ?1 AND u.is_active = 1 AND u.deleted_at IS NULL",
        [except_id],
        |r| r.get(0),
    )?)
}

pub struct UserFields<'a> {
    pub username: &'a str,
    pub full_name: &'a str,
    pub license_type: Option<&'a str>,
    pub license_number: Option<&'a str>,
    pub is_active: bool,
}

pub fn insert(conn: &Connection, f: &UserFields<'_>, password_hash: &str, pin_hash: &str, created_by: i64) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO users (username, full_name, password_hash, pin_hash, license_type, license_number, is_active, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![f.username, f.full_name, password_hash, pin_hash, f.license_type, f.license_number, f.is_active, created_by],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, f: &UserFields<'_>) -> AppResult<()> {
    conn.execute(
        "UPDATE users SET username = ?2, full_name = ?3, license_type = ?4, license_number = ?5, is_active = ?6,
                          updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, f.username, f.full_name, f.license_type, f.license_number, f.is_active],
    )?;
    Ok(())
}

pub fn set_password(conn: &Connection, id: i64, hash: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE users SET password_hash = ?2, updated_at = datetime('now', 'localtime') WHERE id = ?1",
        params![id, hash],
    )?;
    Ok(())
}

pub fn set_pin(conn: &Connection, id: i64, hash: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE users SET pin_hash = ?2, updated_at = datetime('now', 'localtime') WHERE id = ?1",
        params![id, hash],
    )?;
    Ok(())
}

/// Ganti seluruh peran user (tabel `user_roles` tidak menyimpan riwayat; perubahan dicatat di log).
pub fn set_roles(conn: &Connection, user_id: i64, roles: &[Role]) -> AppResult<()> {
    conn.execute("DELETE FROM user_roles WHERE user_id = ?1", [user_id])?;
    for role in roles {
        conn.execute(
            "INSERT INTO user_roles (user_id, role) VALUES (?1, ?2)",
            params![user_id, role.as_str()],
        )?;
    }
    Ok(())
}

pub fn set_active(conn: &Connection, id: i64, active: bool) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE users SET is_active = ?2, updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, active],
    )?)
}

/// Soft delete: user ditandai terhapus, disembunyikan, dan tidak bisa login lagi.
pub fn soft_delete(conn: &Connection, id: i64, deleted_by: i64) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE users SET deleted_at = datetime('now', 'localtime'), deleted_by = ?2, is_active = 0,
                          updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, deleted_by],
    )?)
}
