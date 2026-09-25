use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::auth::{Role, SessionUser};

#[derive(Debug, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserListQuery {
    /// Cari username, nama lengkap, atau nomor SIPA/SIPTTK.
    pub q: Option<String>,
    pub role: Option<Role>,
    pub include_inactive: bool,
    pub offset: i64,
    pub limit: i64,
}

/// Satu pengguna untuk tabel dan form. Hash password/PIN tidak pernah dikirim ke frontend.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub full_name: String,
    pub roles: Vec<Role>,
    /// `SIPA` (apoteker) / `SIPTTK` (TTK).
    pub license_type: Option<String>,
    pub license_number: Option<String>,
    pub has_pin: bool,
    pub is_active: bool,
    pub created_at: String,
    /// Username pembuat; `None` untuk pemilik dari setup awal.
    pub created_by: Option<String>,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserListResult {
    pub rows: Vec<UserRow>,
    pub total: i64,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserInput {
    pub id: Option<i64>,
    pub username: String,
    pub full_name: String,
    pub roles: Vec<Role>,
    /// Nomor SIPA (apoteker) atau SIPTTK (TTK).
    pub license_number: Option<String>,
    /// Wajib untuk user baru; kosong saat ubah = tidak diganti.
    pub password: Option<String>,
    /// Wajib untuk user baru; kosong saat ubah = tidak diganti.
    pub pin: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserSaveResult {
    pub user: UserRow,
    /// Diisi bila yang diubah adalah akun yang sedang login (peran/nama bisa berubah).
    pub session: Option<SessionUser>,
}
