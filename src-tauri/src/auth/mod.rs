//! Login, sesi, peran, dan hak akses.

pub mod commands;
mod model;
pub(crate) mod password;
mod permission;
mod repo;
mod service;

pub use model::SessionUser;
pub(crate) use service::{required, session_for, validate_password, validate_pin, validate_username};
pub use permission::{AccessSettings, Permission, Role, effective_permissions};

/// Login tanpa lewat command Tauri, untuk test modul lain.
#[cfg(test)]
pub(crate) fn login_for_test(conn: &rusqlite::Connection, username: &str, password: &str) -> crate::error::AppResult<SessionUser> {
    service::login(conn, &model::LoginInput { username: username.into(), password: password.into() })
}
