//! Stok: stok per batch + ED, kartu stok (append-only), stok opname (stok awal & berkala).

pub mod commands;
mod model;
mod repo;
mod service;

// Dipakai menu Pembelian (batch & kartu stok saat posting / batal faktur).
pub(crate) use repo::{NewBatch, NewMovement, insert_batch, insert_movement};
pub(crate) use service::NEAR_EXPIRY_DAYS;

#[cfg(test)]
mod tests;
