//! ID komputer untuk mengikat license ke satu komputer.

use sha2::{Digest, Sha256};

use super::file::hex_encode;

/// ID stabil dari ID mesin milik sistem operasi (MachineGuid di Windows), di-hash agar
/// ID aslinya tidak terkirim ke server.
pub fn id() -> String {
    let raw = machine_uid::get().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "gagal membaca ID mesin, memakai nama komputer");
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".into())
    });
    let hash = Sha256::digest(format!("geopostik:{raw}").as_bytes());
    format!("GP-{}", hex_encode(&hash[..16]).to_uppercase())
}
