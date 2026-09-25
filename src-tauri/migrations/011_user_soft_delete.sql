-- Menu Pengguna: user bisa dihapus lewat soft delete (ditandai terhapus dan disembunyikan dari
-- daftar, tidak bisa login lagi), tidak pernah dihapus permanen (DELETE tetap ditolak trigger
-- users_no_delete) agar riwayat transaksi dan log audit tetap menunjuk ke user yang benar.
-- Username user yang terhapus tetap dicadangkan (UNIQUE) agar log lama tidak tertukar orang.
-- `created_by` mencatat pemilik yang membuat akun; akun pemilik pertama (setup awal) NULL.

ALTER TABLE users ADD COLUMN deleted_at TEXT;
ALTER TABLE users ADD COLUMN deleted_by INTEGER REFERENCES users (id);
ALTER TABLE users ADD COLUMN created_by INTEGER REFERENCES users (id);
