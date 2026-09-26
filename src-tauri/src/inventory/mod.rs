//! Stok: stok per batch + ED, kartu stok (append-only), stok opname (stok awal & berkala).

pub mod commands;
mod model;
mod repo;
mod service;

#[cfg(test)]
mod tests;
