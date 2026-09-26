//! Kasir: shift, penjualan bebas & resep (FEFO per batch), pembayaran split, void.
//! Lihat docs/FLOW.md bagian B, E, F, I dan docs/SCHEMA.md bagian 5 & 10.
//!
//! # Konkurensi — bagaimana race condition & deadlock dicegah
//!
//! 1. **Satu penulis dalam proses.** Semua command memakai satu koneksi SQLite di balik
//!    `Mutex` (`AppState::db`). Command kasir memegang mutex itu selama satu transaksi penuh,
//!    jadi dua klik "Bayar", atau "Bayar" dan "Tutup shift", tidak pernah berjalan bersamaan.
//! 2. **Urutan kunci tetap.** Command selalu mengambil sesi & license lebih dulu
//!    (`state.require(..)`, lalu dilepas), baru kemudian `state.db()`. Tidak ada kode yang meminta
//!    sesi/license sambil memegang kunci DB, sehingga tidak ada siklus tunggu (deadlock).
//!    Verifikasi PIN (argon2, lambat) dikerjakan sebelum transaksi tulis dibuka.
//! 3. **Satu transaksi `BEGIN IMMEDIATE` per aksi** (`db::write_tx`). Kunci tulis SQLite diambil
//!    di awal, jadi baca stok FEFO → kurangi stok → simpan nota adalah satu langkah atomik,
//!    juga terhadap proses lain yang membuka file DB yang sama (backup, alat luar). Gagal di
//!    langkah mana pun = rollback total: tidak ada nota setengah jadi atau stok yang hilang.
//! 4. **Stok tidak bisa minus.** Stok hanya berubah lewat `stock_movements`; trigger mengubah
//!    `batches.qty_on_hand_base` dan `CHECK (>= 0)` menggagalkan transaksi bila minus. Setelah
//!    alokasi, ringkasan tiap batch yang tersentuh dicocokkan dengan jumlah kartu stoknya.
//! 5. **Idempotensi.** Setiap checkout membawa `client_ref` (UUID) yang unik di database.
//!    Kiriman ulang (klik ganda, retry) mengembalikan nota yang sama, bukan nota kedua.
//! 6. **Perubahan status bersyarat.** Void (`WHERE status = 'COMPLETED'`), tutup shift
//!    (`WHERE status = 'OPEN'`), dan bayar resep (`WHERE status = 'SCREENED'`) memeriksa jumlah
//!    baris yang berubah, sehingga aksi yang sama tidak bisa terjadi dua kali.
//! 7. **Harga dihitung ulang di server.** Total dari layar hanya dipakai sebagai pembanding;
//!    bila harga/tier berubah sejak keranjang disusun, transaksi ditolak (bukan diam-diam beda).

pub mod commands;
mod model;
mod repo;
mod service;

#[cfg(test)]
mod tests;
