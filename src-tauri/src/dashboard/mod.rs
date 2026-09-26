//! Dashboard: ringkasan harian yang isinya menyesuaikan hak akses user.
//!
//! Setiap panel hanya diisi bila user punya hak terkait; panel lain dikirim `null` sehingga
//! data yang tidak dibutuhkan (terutama omzet, HPP, laba, hutang) tidak pernah sampai ke
//! frontend. Lihat docs/FLOW.md bagian 1 (matriks hak) dan H (dashboard harian).

pub mod commands;
mod model;
mod repo;
mod service;

#[cfg(test)]
mod tests;
