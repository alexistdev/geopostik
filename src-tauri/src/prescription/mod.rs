//! Resep: master dokter & pasien, input resep (obat biasa, racikan, jasa), dan skrining apoteker.
//! Lihat docs/FLOW.md bagian F. Pembayaran & pengurangan stok dilakukan di Kasir.

pub mod commands;
mod model;
pub(crate) mod pricing;
mod repo;
mod service;

#[cfg(test)]
mod tests;
