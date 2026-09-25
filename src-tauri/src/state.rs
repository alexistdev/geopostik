use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::auth::{Permission, SessionUser};
use crate::error::{AppError, AppResult};

pub struct AppState {
    db: Mutex<Connection>,
    session: Mutex<Option<SessionUser>>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Mutex::new(conn),
            session: Mutex::new(None),
        }
    }

    pub fn db(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| AppError::Internal("koneksi database terkunci".into()))
    }

    pub fn session(&self) -> Option<SessionUser> {
        self.session.lock().ok().and_then(|s| s.clone())
    }

    pub fn set_session(&self, user: Option<SessionUser>) {
        if let Ok(mut s) = self.session.lock() {
            *s = user;
        }
    }

    /// User yang sedang login; error bila belum login.
    pub fn current_user(&self) -> AppResult<SessionUser> {
        self.session()
            .ok_or_else(|| AppError::Unauthenticated("Silakan login terlebih dahulu".into()))
    }

    /// User yang sedang login dan memiliki `permission`.
    pub fn require(&self, permission: Permission) -> AppResult<SessionUser> {
        let user = self.current_user()?;
        if user.permissions.contains(&permission) {
            Ok(user)
        } else {
            Err(AppError::Forbidden)
        }
    }
}
