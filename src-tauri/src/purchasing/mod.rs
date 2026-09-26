//! Pembelian: master supplier, faktur penerimaan barang (batch + ED + PPN → HPP per batch),
//! batal faktur, dan hutang supplier. Lihat docs/FLOW.md bagian D dan docs/SCHEMA.md bagian 4.
//!
//! Faktur disimpan sebagai DRAFT (boleh setengah jadi, stok belum berubah), lalu diposting:
//! satu transaksi `BEGIN IMMEDIATE` menghitung HPP per baris, membuat satu batch per baris,
//! mencatat kartu stok `PURCHASE`, dan (opsional) memperbarui HPP acuan + harga jual `AUTO`.
//! Faktur yang sudah diposting hanya bisa dibatalkan selama stok batch-nya belum dipakai
//! (movement `PURCHASE_VOID`); bila sudah terjual, gunakan retur ke supplier.

mod calc;
pub mod commands;
mod model;
mod repo;
mod service;

#[cfg(test)]
mod tests;

/// Sisa hutang satu faktur kredit (alias tabel `pu`): total − pembayaran yang tidak dibatalkan −
/// retur ke supplier yang memotong hutang (SCHEMA.md §4). Dipakai juga oleh Dashboard.
pub const OUTSTANDING: &str = "(pu.grand_total
    - COALESCE((SELECT SUM(amount) FROM supplier_payments x
                WHERE x.purchase_id = pu.id AND x.voided_at IS NULL), 0)
    - COALESCE((SELECT SUM(total) FROM supplier_returns x
                WHERE x.purchase_id = pu.id AND x.settlement = 'DEBT_CUT'), 0))";
