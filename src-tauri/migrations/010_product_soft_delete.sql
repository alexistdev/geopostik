-- Obat bisa dihapus lewat soft delete: ditandai terhapus dan disembunyikan dari daftar dan
-- pencarian, tidak pernah dihapus permanen (DELETE tetap ditolak trigger products_no_delete).
-- Kode obat yang terhapus tetap dicadangkan agar label lama tidak menunjuk ke obat lain.

ALTER TABLE products ADD COLUMN deleted_at TEXT;
ALTER TABLE products ADD COLUMN deleted_by INTEGER REFERENCES users (id);
