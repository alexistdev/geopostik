//! License GeoPOSTik dari server GeoLicense (https://geolicense.my.id).
//!
//! Aplikasi berjalan offline, jadi server hanya dihubungi saat aktivasi pertama dan saat
//! validasi ulang. Hasil aktivasi disimpan di file `license.json` di folder data, ditandatangani
//! HMAC dan terikat ke komputer ini. Setelah masa aktif di file lewat (atau jam komputer
//! dimundurkan), aplikasi terkunci sampai license divalidasi ulang lewat internet.

mod client;
pub mod commands;
mod file;
mod machine;

use std::path::PathBuf;

use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

pub use client::Activation;

/// Tempat pembeli mendapatkan license.
pub const PORTAL_URL: &str = "https://geolicense.my.id/";
/// SKU produk GeoPOSTik di server GeoLicense.
pub const PRODUCT_SKU: &str = "GEOPOS";
/// Jam komputer boleh mundur sebanyak ini dari pemakaian terakhir (misal sinkronisasi jam).
const CLOCK_TOLERANCE: Duration = Duration::hours(1);
/// Waktu pemakaian terakhir di file cukup diperbarui tiap jam.
const TOUCH_INTERVAL: Duration = Duration::hours(1);

/// Isi file license (bagian yang ditandatangani).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseData {
    pub license_key: String,
    pub product_sku: String,
    pub machine_id: String,
    pub max_seats: i64,
    pub used_seats: i64,
    /// Akhir masa aktif license menurut server.
    pub expires_at: DateTime<Utc>,
    /// Aktivasi pertama license ini di komputer ini.
    pub activated_at: DateTime<Utc>,
    /// Validasi terakhir ke server.
    pub validated_at: DateTime<Utc>,
    /// Waktu pemakaian terakhir, untuk mendeteksi jam komputer yang dimundurkan.
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum LicenseState {
    /// Belum pernah diaktifkan di komputer ini: aplikasi belum bisa dipakai sama sekali.
    NotActivated,
    Active,
    /// Masa aktif habis: terkunci sampai divalidasi ulang.
    Expired,
    /// Jam komputer lebih mundur dari pemakaian terakhir.
    ClockRollback,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LicenseStatus {
    pub state: LicenseState,
    /// Alasan license tidak aktif, siap ditampilkan.
    pub message: Option<String>,
    pub license_key: Option<String>,
    /// Waktu lokal "YYYY-MM-DD HH:MM:SS".
    pub expires_at: Option<String>,
    pub activated_at: Option<String>,
    pub validated_at: Option<String>,
    /// Sisa hari masa aktif (0 bila habis hari ini).
    pub days_left: Option<i64>,
    pub max_seats: Option<i64>,
    pub used_seats: Option<i64>,
    pub machine_id: String,
    pub portal_url: String,
}

/// License yang terpasang di komputer ini.
pub struct License {
    path: PathBuf,
    machine_id: String,
    data: Option<LicenseData>,
}

impl License {
    /// Membaca `license.json`. File yang rusak, diubah, atau dari komputer lain dianggap tidak ada.
    pub fn load(path: PathBuf) -> Self {
        let machine_id = machine::id();
        let data = file::read(&path, &machine_id);
        Self { path, machine_id, data }
    }

    pub fn machine_id(&self) -> &str {
        &self.machine_id
    }

    /// Sudah pernah diaktifkan di komputer ini (walau mungkin sudah kedaluwarsa).
    pub fn is_installed(&self) -> bool {
        self.data.is_some()
    }

    pub fn license_key(&self) -> Option<&str> {
        self.data.as_ref().map(|d| d.license_key.as_str())
    }

    /// Status saat ini. Sekalian mencatat waktu pemakaian terakhir ke file.
    pub fn check(&mut self) -> LicenseStatus {
        let now = Utc::now();
        let state = evaluate(self.data.as_ref(), now).0;
        if let Some(data) = &mut self.data
            && state != LicenseState::ClockRollback
            && now - data.last_seen_at >= TOUCH_INTERVAL
        {
            data.last_seen_at = now;
            if let Err(e) = file::write(&self.path, data) {
                tracing::warn!(error = %e, "gagal memperbarui file license");
            }
        }
        self.status(now)
    }

    /// Error bila license tidak aktif.
    pub fn require_active(&mut self) -> AppResult<()> {
        let status = self.check();
        match status.state {
            LicenseState::Active => Ok(()),
            _ => Err(AppError::LicenseRequired(status.message.unwrap_or_default())),
        }
    }

    /// Menyimpan hasil aktivasi/validasi ulang dari server ke file.
    pub fn apply(&mut self, license_key: String, activation: Activation, now: DateTime<Utc>) -> AppResult<()> {
        let activated_at = self
            .data
            .as_ref()
            .filter(|d| d.license_key == license_key)
            .map_or(now, |d| d.activated_at);
        // Jam server dipakai bila lebih maju, agar jam komputer yang mundur tetap ketahuan.
        let seen = activation.server_time.map_or(now, |t| t.max(now));
        let data = LicenseData {
            license_key,
            product_sku: PRODUCT_SKU.into(),
            machine_id: self.machine_id.clone(),
            max_seats: activation.max_seats,
            used_seats: activation.used_seats,
            expires_at: activation.expires_at,
            activated_at,
            validated_at: seen,
            last_seen_at: seen,
        };
        file::write(&self.path, &data)?;
        tracing::info!(key = %data.license_key, expires_at = %data.expires_at, "license diaktifkan");
        self.data = Some(data);
        Ok(())
    }

    fn status(&self, now: DateTime<Utc>) -> LicenseStatus {
        let (state, message) = evaluate(self.data.as_ref(), now);
        let d = self.data.as_ref();
        LicenseStatus {
            state,
            message,
            license_key: d.map(|d| d.license_key.clone()),
            expires_at: d.map(|d| local(d.expires_at)),
            activated_at: d.map(|d| local(d.activated_at)),
            validated_at: d.map(|d| local(d.validated_at)),
            days_left: d.map(|d| (d.expires_at - now).num_days().max(0)),
            max_seats: d.map(|d| d.max_seats),
            used_seats: d.map(|d| d.used_seats),
            machine_id: self.machine_id.clone(),
            portal_url: PORTAL_URL.into(),
        }
    }
}

fn local(t: DateTime<Utc>) -> String {
    t.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()
}

fn evaluate(data: Option<&LicenseData>, now: DateTime<Utc>) -> (LicenseState, Option<String>) {
    let Some(d) = data else {
        return (
            LicenseState::NotActivated,
            Some("Aplikasi belum diaktifkan. Masukkan license key.".into()),
        );
    };
    if now + CLOCK_TOLERANCE < d.last_seen_at {
        return (
            LicenseState::ClockRollback,
            Some(format!(
                "Jam komputer lebih mundur dari pemakaian terakhir ({}). \
                 Perbaiki tanggal & jam komputer, lalu hubungkan ke internet dan validasi ulang license.",
                &local(d.last_seen_at)[..16]
            )),
        );
    }
    if now >= d.expires_at {
        return (
            LicenseState::Expired,
            Some(format!(
                "Masa aktif license habis pada {}. Hubungkan komputer ke internet lalu validasi ulang license.",
                &local(d.expires_at)[..16]
            )),
        );
    }
    (LicenseState::Active, None)
}

/// Merapikan license key yang diketik/ditempel pengguna.
pub fn normalize_key(key: &str) -> AppResult<String> {
    let key = key.trim().to_uppercase();
    if key.is_empty() {
        return Err(AppError::Validation("License key wajib diisi".into()));
    }
    if key.len() > 100 || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(AppError::Validation("Format license key tidak valid".into()));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(now: DateTime<Utc>) -> LicenseData {
        LicenseData {
            license_key: "GEOLIC-AAAA-BBBB".into(),
            product_sku: PRODUCT_SKU.into(),
            machine_id: "GP-TEST".into(),
            max_seats: 1,
            used_seats: 1,
            expires_at: now + Duration::days(30),
            activated_at: now,
            validated_at: now,
            last_seen_at: now,
        }
    }

    fn activation(expires_at: DateTime<Utc>, server_time: Option<DateTime<Utc>>) -> Activation {
        Activation { expires_at, max_seats: 2, used_seats: 1, server_time }
    }

    #[test]
    fn evaluate_states() {
        let now = Utc::now();
        assert_eq!(evaluate(None, now).0, LicenseState::NotActivated);

        let d = data(now);
        assert_eq!(evaluate(Some(&d), now), (LicenseState::Active, None));
        assert_eq!(evaluate(Some(&d), now + Duration::days(29)).0, LicenseState::Active);
        assert_eq!(evaluate(Some(&d), now + Duration::days(30)).0, LicenseState::Expired);

        // Jam mundur sedikit masih ditoleransi, mundur jauh dikunci.
        assert_eq!(evaluate(Some(&d), now - Duration::minutes(30)).0, LicenseState::Active);
        assert_eq!(evaluate(Some(&d), now - Duration::days(2)).0, LicenseState::ClockRollback);
    }

    #[test]
    fn apply_writes_file_and_keeps_first_activation() {
        let dir = std::env::temp_dir().join(format!("geopostik-license-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("license.json");
        let now = Utc::now();

        let mut license = License { path: path.clone(), machine_id: "GP-TEST".into(), data: None };
        assert!(!license.is_installed());
        license
            .apply("GEOLIC-AAAA-BBBB".into(), activation(now + Duration::days(30), None), now)
            .unwrap();
        assert_eq!(license.check().state, LicenseState::Active);

        // Validasi ulang: masa aktif diperpanjang, tanggal aktivasi pertama tetap.
        let later = now + Duration::days(40);
        license
            .apply("GEOLIC-AAAA-BBBB".into(), activation(later + Duration::days(30), None), later)
            .unwrap();
        let saved = file::read(&path, "GP-TEST").unwrap();
        assert_eq!(saved.activated_at, now);
        assert_eq!(saved.expires_at, later + Duration::days(30));

        // Jam server yang lebih maju dipakai sebagai waktu pemakaian terakhir.
        let server = now + Duration::days(3);
        license
            .apply("GEOLIC-AAAA-BBBB".into(), activation(now + Duration::days(30), Some(server)), now)
            .unwrap();
        assert_eq!(license.check().state, LicenseState::ClockRollback);

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn normalize_key_cleans_input() {
        assert_eq!(normalize_key("  geolic-fbdb5965-8346d7c4 ").unwrap(), "GEOLIC-FBDB5965-8346D7C4");
        assert!(normalize_key(" ").is_err());
        assert!(normalize_key("GEOLIC FBDB").is_err());
    }
}
