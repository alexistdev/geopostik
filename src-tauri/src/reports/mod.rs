//! Laporan: penjualan, obat terlaris & slow moving, nilai persediaan, ED, pembelian, dan
//! narkotika/psikotropika (SIPNAP). Lihat docs/FLOW.md bagian J.
//!
//! Seperti Dashboard, data sensitif difilter di sini: HPP, laba, dan nilai persediaan hanya
//! dikirim untuk `VIEW_COST`; tanpa `REPORT_SALES` laporan penjualan hanya berisi nota sendiri.

pub mod commands;
mod model;
mod repo;
mod service;

#[cfg(test)]
mod tests;
