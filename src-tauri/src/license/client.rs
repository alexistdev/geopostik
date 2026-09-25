//! Panggilan ke API GeoLicense. Hanya dipakai saat aktivasi dan validasi ulang.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{PORTAL_URL, PRODUCT_SKU};

const ACTIVATE_URL: &str = "https://geolicense.my.id/api/v1/licenses/activate";

/// Hasil aktivasi dari server.
#[derive(Debug)]
pub struct Activation {
    pub expires_at: DateTime<Utc>,
    pub max_seats: i64,
    pub used_seats: i64,
    /// Jam server (header `Date`), untuk mendeteksi jam komputer yang salah.
    pub server_time: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivateRequest<'a> {
    license_key: &'a str,
    machine_id: &'a str,
    product_sku: &'a str,
    os_info: String,
}

/// Amplop `{status, messages, payload}`; error validasi Laravel memakai `message`.
#[derive(Deserialize)]
struct Envelope {
    status: Option<bool>,
    messages: Option<Vec<String>>,
    message: Option<String>,
    payload: Option<ActivatePayload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivatePayload {
    license_expires_at: DateTime<Utc>,
    used_seats: i64,
    max_seats: i64,
}

/// Mengaktifkan (atau memvalidasi ulang) license di komputer ini. Aktivasi ulang di komputer
/// yang sama tidak memakai seat baru. Error berupa pesan siap tampil.
pub fn activate(license_key: &str, machine_id: &str) -> Result<Activation, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .into();
    let request = ActivateRequest {
        license_key,
        machine_id,
        product_sku: PRODUCT_SKU,
        os_info: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    };
    let mut response = agent
        .post(ACTIVATE_URL)
        .header("Accept", "application/json")
        .send_json(&request)
        .map_err(|e| {
            tracing::warn!(error = %e, "server license tidak dapat dihubungi");
            "Tidak dapat terhubung ke server license. Pastikan komputer terhubung ke internet, lalu coba lagi."
                .to_string()
        })?;

    let status = response.status().as_u16();
    let server_time = response
        .headers()
        .get("date")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| DateTime::parse_from_rfc2822(v).ok())
        .map(|t| t.with_timezone(&Utc));
    let envelope: Option<Envelope> = response.body_mut().read_json().ok();

    match envelope {
        Some(Envelope { status: Some(true), payload: Some(p), .. }) if status == 200 => Ok(Activation {
            expires_at: p.license_expires_at,
            max_seats: p.max_seats,
            used_seats: p.used_seats,
            server_time,
        }),
        other => {
            let server_message = other
                .and_then(|e| e.messages.and_then(|m| m.into_iter().next()).or(e.message))
                .unwrap_or_default();
            tracing::warn!(status, message = %server_message, "aktivasi license ditolak");
            Err(rejection_message(status, &server_message))
        }
    }
}

fn rejection_message(status: u16, server_message: &str) -> String {
    match status {
        404 => "License key tidak ditemukan. Periksa kembali license key Anda.".into(),
        402 => format!("Masa aktif license sudah habis. Perpanjang license di {PORTAL_URL}"),
        429 => "Jumlah komputer untuk license ini sudah penuh.".into(),
        422 => "Format license key tidak valid.".into(),
        403 if server_message.contains("not valid for product") => "License ini bukan untuk GeoPOSTik.".into(),
        403 if server_message.contains("not active") => {
            "License tidak aktif (dibatalkan atau belum dibayar).".into()
        }
        500..=599 => format!("Server license sedang bermasalah (HTTP {status}). Coba lagi nanti."),
        _ => format!("License ditolak server (HTTP {status}): {server_message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_messages_are_translated() {
        assert!(rejection_message(404, "License not found: X").contains("tidak ditemukan"));
        assert!(rejection_message(429, "").contains("penuh"));
        assert!(rejection_message(403, "License X is not valid for product: GEOPOS").contains("bukan untuk"));
        assert!(rejection_message(403, "License is not active: X").contains("tidak aktif"));
        assert!(rejection_message(503, "").contains("bermasalah"));
    }

    /// Aktivasi sungguhan ke server. Jalankan manual:
    /// `GEOLICENSE_TEST_KEY=GEOLIC-... cargo test live_activation -- --ignored --nocapture`
    #[test]
    #[ignore = "butuh internet dan license key sungguhan"]
    fn live_activation() {
        let key = std::env::var("GEOLICENSE_TEST_KEY").expect("isi GEOLICENSE_TEST_KEY");
        let result = activate(&key, &super::super::machine::id());
        println!("{result:?}");
        assert!(result.is_ok());
        assert!(activate("GEOLIC-00000000-00000000", "GP-TEST").unwrap_err().contains("tidak ditemukan"));
    }
}
