use rusqlite::{Connection, OptionalExtension, params};

use super::Role;
use crate::error::AppResult;

pub struct NewUser<'a> {
    pub username: &'a str,
    pub full_name: &'a str,
    pub password_hash: &'a str,
    pub pin_hash: Option<&'a str>,
    pub license_type: Option<&'a str>,
    pub license_number: Option<&'a str>,
}

pub struct LoginRow {
    pub id: i64,
    pub username: String,
    pub full_name: String,
    pub password_hash: String,
    pub is_active: bool,
}

pub fn count_users(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?)
}

pub fn insert_user(conn: &Connection, u: &NewUser<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO users (username, full_name, password_hash, pin_hash, license_type, license_number)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![u.username, u.full_name, u.password_hash, u.pin_hash, u.license_type, u.license_number],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_role(conn: &Connection, user_id: i64, role: Role) -> AppResult<()> {
    conn.execute(
        "INSERT INTO user_roles (user_id, role) VALUES (?1, ?2)",
        params![user_id, role.as_str()],
    )?;
    Ok(())
}

pub fn find_by_username(conn: &Connection, username: &str) -> AppResult<Option<LoginRow>> {
    Ok(conn
        .query_row(
            // User yang sudah dihapus (soft delete) dianggap tidak ada.
            "SELECT id, username, full_name, password_hash, is_active FROM users
             WHERE username = ?1 AND deleted_at IS NULL",
            [username],
            |r| {
                Ok(LoginRow {
                    id: r.get(0)?,
                    username: r.get(1)?,
                    full_name: r.get(2)?,
                    password_hash: r.get(3)?,
                    is_active: r.get(4)?,
                })
            },
        )
        .optional()?)
}

/// User aktif (belum dihapus) yang punya PIN, kandidat otorisasi PIN.
pub fn pin_holders(conn: &Connection) -> AppResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, pin_hash FROM users
         WHERE pin_hash IS NOT NULL AND is_active = 1 AND deleted_at IS NULL
         ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn roles_of(conn: &Connection, user_id: i64) -> AppResult<Vec<Role>> {
    let mut stmt = conn.prepare("SELECT role FROM user_roles WHERE user_id = ?1 ORDER BY role")?;
    let roles = stmt
        .query_map([user_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    roles.iter().map(|r| r.parse()).collect()
}
