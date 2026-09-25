//! File `license.json`: isi license + tanda tangan HMAC-SHA256.
//!
//! Tanda tangan mencegah masa aktif diubah manual. Isi yang ditandatangani disimpan sebagai
//! string JSON apa adanya agar hasil verifikasi tidak bergantung pada urutan field.

use std::path::Path;

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use super::LicenseData;
use crate::error::{AppError, AppResult};

const SECRET: &[u8] = b"GeoPOSTik/license/v1/7c1e4b9a-2f3d-4e8b-a6d5-91f0c3b2e847";

#[derive(Serialize, Deserialize)]
struct Envelope {
    data: String,
    signature: String,
}

/// `None` bila file tidak ada, rusak, tanda tangannya salah, atau dibuat di komputer lain.
pub fn read(path: &Path, machine_id: &str) -> Option<LicenseData> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(error = %e, "gagal membaca file license");
            return None;
        }
    };
    let parsed = serde_json::from_str::<Envelope>(&raw).ok().and_then(|env| {
        let signature = hex_decode(&env.signature)?;
        mac().chain_update(env.data.as_bytes()).verify_slice(&signature).ok()?;
        serde_json::from_str::<LicenseData>(&env.data).ok()
    });
    match parsed {
        Some(data) if data.machine_id == machine_id => Some(data),
        Some(_) => {
            tracing::warn!("file license milik komputer lain, diabaikan");
            None
        }
        None => {
            tracing::warn!("file license rusak atau telah diubah, diabaikan");
            None
        }
    }
}

pub fn write(path: &Path, data: &LicenseData) -> AppResult<()> {
    let data = serde_json::to_string(data).map_err(|e| AppError::Internal(e.to_string()))?;
    let signature = hex_encode(&mac().chain_update(data.as_bytes()).finalize().into_bytes());
    let json = serde_json::to_string_pretty(&Envelope { data, signature })
        .map_err(|e| AppError::Internal(e.to_string()))?;
    // Tulis ke file sementara lalu rename, agar file tidak setengah jadi bila listrik mati.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| AppError::Internal(format!("gagal menyimpan file license: {e}")))
}

fn mac() -> Hmac<Sha256> {
    Hmac::new_from_slice(SECRET).expect("HMAC menerima kunci berapa pun panjangnya")
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::*;

    #[test]
    fn rejects_tampered_and_foreign_files() {
        let dir = std::env::temp_dir().join(format!("geopostik-license-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("license.json");
        let now = Utc::now();
        let data = LicenseData {
            license_key: "GEOLIC-AAAA-BBBB".into(),
            product_sku: "GEOPOS".into(),
            machine_id: "GP-TEST".into(),
            max_seats: 1,
            used_seats: 1,
            expires_at: now + Duration::days(30),
            activated_at: now,
            validated_at: now,
            last_seen_at: now,
        };

        assert_eq!(read(&path, "GP-TEST"), None, "belum ada file");
        write(&path, &data).unwrap();
        assert_eq!(read(&path, "GP-TEST"), Some(data.clone()));
        assert_eq!(read(&path, "GP-LAIN"), None, "file disalin ke komputer lain");

        // Masa aktif diubah manual → tanda tangan tidak cocok.
        let raw = std::fs::read_to_string(&path).unwrap();
        let year = data.expires_at.format("%Y").to_string();
        let tampered = raw.replacen(&year, "2099", 1);
        assert_ne!(raw, tampered);
        std::fs::write(&path, tampered).unwrap();
        assert_eq!(read(&path, "GP-TEST"), None);

        std::fs::write(&path, "bukan json").unwrap();
        assert_eq!(read(&path, "GP-TEST"), None);

        std::fs::remove_dir_all(dir).unwrap();
    }
}
