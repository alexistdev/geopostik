-- Menu Kasir: shift, penjualan bebas & resep, pembayaran split, void.
--
-- Penjagaan konkurensi (lihat src-tauri/src/sales/mod.rs):
--   * setiap penjualan/void/tutup shift berjalan dalam satu transaksi BEGIN IMMEDIATE;
--   * `client_ref` unik = kunci idempotensi, sehingga klik ganda / kirim ulang tidak membuat
--     nota kedua;
--   * stok hanya berkurang lewat stock_movements (trigger + CHECK qty >= 0 menolak stok minus).

-- ─── Penjualan ───────────────────────────────────────────────────────────────

-- Kunci idempotensi yang dibuat kasir per checkout (UUID). NULL untuk data lama.
ALTER TABLE sales ADD COLUMN client_ref TEXT;
CREATE UNIQUE INDEX sales_client_ref ON sales (client_ref) WHERE client_ref IS NOT NULL;
CREATE INDEX sales_cashier ON sales (cashier_id, sold_at);
CREATE INDEX sales_prescription ON sales (prescription_id) WHERE prescription_id IS NOT NULL;

-- Satu resep hanya boleh dibayar sekali (nota batal tidak dihitung).
CREATE UNIQUE INDEX sales_one_per_prescription ON sales (prescription_id)
WHERE prescription_id IS NOT NULL AND status = 'COMPLETED';

-- Nota tidak pernah dihapus; batal = status VOID.
CREATE TRIGGER sales_no_delete BEFORE DELETE ON sales
BEGIN SELECT RAISE(ABORT, 'sales: hapus tidak diizinkan, gunakan void'); END;

-- Nota yang sudah VOID final; nota COMPLETED hanya boleh berubah menjadi VOID (angka tetap).
CREATE TRIGGER sales_immutable BEFORE UPDATE ON sales
WHEN old.status = 'VOID'
     OR new.status <> 'VOID'
     OR new.grand_total IS NOT old.grand_total
     OR new.subtotal IS NOT old.subtotal
     OR new.shift_id IS NOT old.shift_id
     OR new.number IS NOT old.number
BEGIN SELECT RAISE(ABORT, 'sales: nota tidak bisa diubah selain dibatalkan'); END;

-- Rincian nota tidak pernah berubah.
CREATE TRIGGER sale_items_no_update BEFORE UPDATE ON sale_items
BEGIN SELECT RAISE(ABORT, 'sale_items: tidak bisa diubah'); END;
CREATE TRIGGER sale_items_no_delete BEFORE DELETE ON sale_items
BEGIN SELECT RAISE(ABORT, 'sale_items: tidak bisa dihapus'); END;
CREATE TRIGGER sale_item_batches_no_update BEFORE UPDATE ON sale_item_batches
BEGIN SELECT RAISE(ABORT, 'sale_item_batches: tidak bisa diubah'); END;
CREATE TRIGGER sale_item_batches_no_delete BEFORE DELETE ON sale_item_batches
BEGIN SELECT RAISE(ABORT, 'sale_item_batches: tidak bisa dihapus'); END;
CREATE TRIGGER sale_payments_no_update BEFORE UPDATE ON sale_payments
BEGIN SELECT RAISE(ABORT, 'sale_payments: tidak bisa diubah'); END;
CREATE TRIGGER sale_payments_no_delete BEFORE DELETE ON sale_payments
BEGIN SELECT RAISE(ABORT, 'sale_payments: tidak bisa dihapus'); END;

-- Nota hanya bisa dibuat pada shift yang masih terbuka.
CREATE TRIGGER sales_shift_open BEFORE INSERT ON sales
WHEN (SELECT status FROM shifts WHERE id = new.shift_id) IS NOT 'OPEN'
BEGIN SELECT RAISE(ABORT, 'sales: shift sudah ditutup'); END;

-- ─── Shift ───────────────────────────────────────────────────────────────────

CREATE INDEX shifts_opened ON shifts (opened_at);

CREATE TRIGGER shifts_no_delete BEFORE DELETE ON shifts
BEGIN SELECT RAISE(ABORT, 'shifts: tidak bisa dihapus'); END;

-- Shift yang sudah ditutup final (tidak bisa dibuka lagi atau diubah angkanya).
CREATE TRIGGER shifts_closed_final BEFORE UPDATE ON shifts
WHEN old.status = 'CLOSED'
BEGIN SELECT RAISE(ABORT, 'shifts: shift sudah ditutup'); END;

-- ─── Resep ───────────────────────────────────────────────────────────────────

-- Nota resep yang dibatalkan (void) mengembalikan resep ke SCREENED agar bisa dibayar ulang.
-- Selain itu resep PAID/CANCELLED tetap final.
DROP TRIGGER prescriptions_closed;
CREATE TRIGGER prescriptions_closed BEFORE UPDATE ON prescriptions
WHEN old.status IN ('PAID', 'CANCELLED')
     AND NOT (old.status = 'PAID' AND new.status = 'SCREENED')
BEGIN SELECT RAISE(ABORT, 'prescriptions: resep sudah dibayar atau dibatalkan'); END;
