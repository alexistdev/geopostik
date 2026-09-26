-- Menu Pembelian: master supplier, faktur penerimaan barang (batch + ED + PPN → HPP per batch),
-- batal faktur, dan hutang supplier (pembayaran).
--
-- Faktur: DRAFT (boleh setengah jadi) → POSTED (stok bertambah, batch dibuat) → VOID (batal).
-- Faktur tidak pernah dihapus. Dijalankan dengan foreign_keys = OFF (lihat db::prepare) dan
-- diperiksa foreign_key_check.

-- ─── Supplier ────────────────────────────────────────────────────────────────
-- Mengikuti pola master lain: kode otomatis (SUP0001) yang tidak bisa diubah, soft delete, pembuat.

ALTER TABLE suppliers ADD COLUMN code TEXT;
ALTER TABLE suppliers ADD COLUMN deleted_at TEXT;
ALTER TABLE suppliers ADD COLUMN deleted_by INTEGER REFERENCES users (id);
ALTER TABLE suppliers ADD COLUMN created_by INTEGER REFERENCES users (id);

UPDATE suppliers SET code = 'SUP' || printf('%04d', id);

CREATE UNIQUE INDEX suppliers_code ON suppliers (code COLLATE NOCASE) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX suppliers_name ON suppliers (name COLLATE NOCASE) WHERE deleted_at IS NULL;

CREATE TRIGGER suppliers_code_required BEFORE INSERT ON suppliers
WHEN new.code IS NULL OR new.code = ''
BEGIN SELECT RAISE(ABORT, 'suppliers.code is required'); END;

CREATE TRIGGER suppliers_code_immutable BEFORE UPDATE OF code ON suppliers
WHEN old.code IS NOT new.code
BEGIN SELECT RAISE(ABORT, 'suppliers.code tidak bisa diubah'); END;

-- ─── Faktur pembelian ────────────────────────────────────────────────────────
-- Dibangun ulang karena UNIQUE (supplier_id, invoice_number) harus diganti index unik parsial:
-- faktur yang dibatalkan (misal salah input) boleh diinput ulang dengan nomor yang sama.

CREATE TABLE purchases_new (
    id              INTEGER PRIMARY KEY,
    number          TEXT    NOT NULL UNIQUE,
    supplier_id     INTEGER NOT NULL REFERENCES suppliers (id),
    invoice_number  TEXT    NOT NULL CHECK (invoice_number <> ''),
    invoice_date    TEXT    NOT NULL,
    received_date   TEXT    NOT NULL,
    due_date        TEXT,
    payment_type    TEXT    NOT NULL CHECK (payment_type IN ('CASH', 'CREDIT')),
    tax_mode        TEXT    NOT NULL CHECK (tax_mode IN ('INCLUDED', 'EXCLUDED', 'NONE')),
    tax_rate_bp     INTEGER NOT NULL DEFAULT 0 CHECK (tax_rate_bp BETWEEN 0 AND 10000),
    -- Σ qty × harga faktur (sebelum diskon).
    subtotal        INTEGER NOT NULL DEFAULT 0,
    -- Diskon faktur tingkat header (rupiah), dialokasikan proporsional ke baris saat hitung HPP.
    extra_discount  INTEGER NOT NULL DEFAULT 0 CHECK (extra_discount >= 0),
    -- Diskon baris + diskon faktur.
    discount_total  INTEGER NOT NULL DEFAULT 0,
    tax_total       INTEGER NOT NULL DEFAULT 0,
    grand_total     INTEGER NOT NULL DEFAULT 0,
    -- Snapshot saat posting: 1 = PPN masuk HPP (apotek non-PKP).
    tax_in_cost     INTEGER CHECK (tax_in_cost IN (0, 1)),
    status          TEXT    NOT NULL DEFAULT 'DRAFT' CHECK (status IN ('DRAFT', 'POSTED', 'VOID')),
    note            TEXT,
    created_by      INTEGER NOT NULL REFERENCES users (id),
    posted_at       TEXT,
    posted_by       INTEGER REFERENCES users (id),
    voided_at       TEXT,
    voided_by       INTEGER REFERENCES users (id),
    void_reason     TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    CHECK (payment_type = 'CASH' OR due_date IS NOT NULL)
);

INSERT INTO purchases_new (id, number, supplier_id, invoice_number, invoice_date, received_date, due_date,
                           payment_type, tax_mode, tax_rate_bp, subtotal, discount_total, tax_total, grand_total,
                           status, note, created_by, posted_at, created_at, updated_at)
SELECT id, number, supplier_id, invoice_number, invoice_date, received_date, due_date,
       payment_type, tax_mode, tax_rate_bp, subtotal, discount_total, tax_total, grand_total,
       status, note, created_by, posted_at, created_at, updated_at
FROM purchases;

DROP TABLE purchases;
ALTER TABLE purchases_new RENAME TO purchases;

CREATE UNIQUE INDEX purchases_invoice ON purchases (supplier_id, invoice_number COLLATE NOCASE)
WHERE status <> 'VOID';
CREATE INDEX purchases_due_credit ON purchases (due_date) WHERE payment_type = 'CREDIT';
CREATE INDEX purchases_status ON purchases (status, id);
CREATE INDEX purchases_received ON purchases (received_date);

CREATE TRIGGER purchases_no_delete BEFORE DELETE ON purchases
BEGIN SELECT RAISE(ABORT, 'purchases: hapus tidak diizinkan, gunakan batal'); END;

CREATE TRIGGER purchases_number_immutable BEFORE UPDATE OF number ON purchases
WHEN old.number IS NOT new.number
BEGIN SELECT RAISE(ABORT, 'purchases.number tidak bisa diubah'); END;

-- Faktur POSTED hanya boleh berubah menjadi VOID tanpa mengubah angka; VOID final.
CREATE TRIGGER purchases_posted_immutable BEFORE UPDATE ON purchases
WHEN old.status = 'VOID'
     OR (old.status = 'POSTED'
         AND (new.status <> 'VOID'
              OR new.grand_total IS NOT old.grand_total
              OR new.subtotal IS NOT old.subtotal
              OR new.tax_total IS NOT old.tax_total
              OR new.supplier_id IS NOT old.supplier_id
              OR new.invoice_number IS NOT old.invoice_number
              OR new.payment_type IS NOT old.payment_type
              OR new.due_date IS NOT old.due_date))
BEGIN SELECT RAISE(ABORT, 'purchases: faktur sudah diposting atau dibatalkan'); END;

-- ─── Baris faktur ────────────────────────────────────────────────────────────

ALTER TABLE purchase_items ADD COLUMN line_no INTEGER NOT NULL DEFAULT 0;
-- Bonus dari supplier (satuan beli, tanpa harga). Ikut menambah stok; HPP baris dibagi ke
-- qty + bonus.
ALTER TABLE purchase_items ADD COLUMN bonus_qty INTEGER NOT NULL DEFAULT 0 CHECK (bonus_qty >= 0);

CREATE INDEX purchase_items_product ON purchase_items (product_id);

-- Baris hanya bisa ditambah/diubah/dihapus selama faktur masih DRAFT (termasuk pengisian HPP dan
-- batch saat posting, yang dilakukan sebelum status berubah menjadi POSTED).
CREATE TRIGGER purchase_items_draft_insert BEFORE INSERT ON purchase_items
WHEN (SELECT status FROM purchases WHERE id = new.purchase_id) IS NOT 'DRAFT'
BEGIN SELECT RAISE(ABORT, 'purchase_items: faktur sudah diposting atau dibatalkan'); END;

CREATE TRIGGER purchase_items_draft_update BEFORE UPDATE ON purchase_items
WHEN (SELECT status FROM purchases WHERE id = old.purchase_id) IS NOT 'DRAFT'
BEGIN SELECT RAISE(ABORT, 'purchase_items: faktur sudah diposting atau dibatalkan'); END;

CREATE TRIGGER purchase_items_draft_delete BEFORE DELETE ON purchase_items
WHEN (SELECT status FROM purchases WHERE id = old.purchase_id) IS NOT 'DRAFT'
BEGIN SELECT RAISE(ABORT, 'purchase_items: faktur sudah diposting atau dibatalkan'); END;

-- ─── Pembayaran hutang ───────────────────────────────────────────────────────
-- Tidak pernah dihapus atau diubah; pembayaran yang salah dibatalkan (void) dengan alasan.

ALTER TABLE supplier_payments ADD COLUMN number TEXT;
ALTER TABLE supplier_payments ADD COLUMN reference TEXT;
ALTER TABLE supplier_payments ADD COLUMN voided_at TEXT;
ALTER TABLE supplier_payments ADD COLUMN voided_by INTEGER REFERENCES users (id);
ALTER TABLE supplier_payments ADD COLUMN void_reason TEXT;

UPDATE supplier_payments SET number = 'BH-LAMA-' || printf('%04d', id) WHERE number IS NULL;

CREATE UNIQUE INDEX supplier_payments_number ON supplier_payments (number);

CREATE TRIGGER supplier_payments_no_delete BEFORE DELETE ON supplier_payments
BEGIN SELECT RAISE(ABORT, 'supplier_payments: hapus tidak diizinkan, gunakan batal'); END;

-- Satu-satunya perubahan: pembayaran aktif dibatalkan (angka tetap).
CREATE TRIGGER supplier_payments_immutable BEFORE UPDATE ON supplier_payments
WHEN old.voided_at IS NOT NULL
     OR new.voided_at IS NULL
     OR new.amount IS NOT old.amount
     OR new.purchase_id IS NOT old.purchase_id
     OR new.payment_date IS NOT old.payment_date
     OR new.method IS NOT old.method
BEGIN SELECT RAISE(ABORT, 'supplier_payments: pembayaran tidak bisa diubah selain dibatalkan'); END;

-- Pembayaran hanya untuk faktur kredit yang sudah diposting.
CREATE TRIGGER supplier_payments_posted_credit BEFORE INSERT ON supplier_payments
WHEN (SELECT status || '/' || payment_type FROM purchases WHERE id = new.purchase_id) IS NOT 'POSTED/CREDIT'
BEGIN SELECT RAISE(ABORT, 'supplier_payments: hanya untuk faktur kredit yang sudah diposting'); END;
