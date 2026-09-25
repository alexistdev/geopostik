use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::auth::{Permission, SessionUser};
use crate::error::{AppError, AppResult};
use crate::license::License;

pub struct AppState {
    db: Mutex<Connection>,
    session: Mutex<Option<SessionUser>>,
    license: Mutex<License>,
}

impl AppState {
    pub fn new(conn: Connection, license: License) -> Self {
        Self {
            db: Mutex::new(conn),
            session: Mutex::new(None),
            license: Mutex::new(license),
        }
    }

    pub fn license(&self) -> AppResult<MutexGuard<'_, License>> {
        self.license
            .lock()
            .map_err(|_| AppError::Internal("data license terkunci".into()))
    }

    /// Error bila license tidak aktif (belum diaktifkan, kedaluwarsa, atau jam dimundurkan).
    pub fn require_license(&self) -> AppResult<()> {
        self.license()?.require_active()
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

    /// User yang sedang login dengan license aktif; error bila belum login atau license tidak aktif.
    pub fn current_user(&self) -> AppResult<SessionUser> {
        let user = self.session_user()?;
        self.require_license()?;
        Ok(user)
    }

    /// User yang sedang login dan memiliki `permission`, dengan license aktif.
    pub fn require(&self, permission: Permission) -> AppResult<SessionUser> {
        let user = self.current_user()?;
        has_permission(user, permission)
    }

    /// Seperti `require` tetapi tanpa memeriksa license. Hanya untuk menu Pengaturan License,
    /// yang harus tetap bisa dibuka saat license kedaluwarsa.
    pub fn require_unlicensed(&self, permission: Permission) -> AppResult<SessionUser> {
        has_permission(self.session_user()?, permission)
    }

    fn session_user(&self) -> AppResult<SessionUser> {
        self.session()
            .ok_or_else(|| AppError::Unauthenticated("Silakan login terlebih dahulu".into()))
    }
}

fn has_permission(user: SessionUser, permission: Permission) -> AppResult<SessionUser> {
    if user.permissions.contains(&permission) {
        Ok(user)
    } else {
        Err(AppError::Forbidden)
    }
}
