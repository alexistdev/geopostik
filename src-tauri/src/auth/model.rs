use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Permission, Role};
use crate::license::LicenseStatus;

/// User yang sedang login beserta hak efektifnya.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionUser {
    pub id: i64,
    pub username: String,
    pub full_name: String,
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
}

/// Status aplikasi saat dibuka: license aktif? perlu setup awal? sudah ada sesi?
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppStatus {
    pub license: LicenseStatus,
    pub needs_setup: bool,
    pub session: Option<SessionUser>,
}

/// Setup awal: membuat akun pemilik pertama.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SetupInput {
    pub pharmacy_name: String,
    pub full_name: String,
    pub username: String,
    pub password: String,
    pub pin: String,
    /// Pemilik juga apoteker → diberi peran PHARMACIST.
    pub also_pharmacist: bool,
    /// Nomor SIPA, wajib bila `also_pharmacist`.
    pub license_number: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PharmacyProfile {
    pub name: String,
}
