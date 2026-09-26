//! Master data: kategori, satuan, obat, harga, tier, barcode.

pub mod commands;
mod model;
pub(crate) mod pricing;
mod repo;
mod service;

pub use model::BatchResult;
// Dipakai menu Resep (master dokter & pasien, golongan obat).
pub(crate) use model::{DrugClass, MasterPageQuery};
pub(crate) use repo::like_pattern;
// Dipakai menu Pembelian: harga AUTO dihitung ulang dari HPP faktur.
pub(crate) use service::recalc_auto_prices;

#[cfg(test)]
mod tests;
