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

#[cfg(test)]
mod tests;
