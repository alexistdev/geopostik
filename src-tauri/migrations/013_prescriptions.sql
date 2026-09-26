-- Menu Resep: master dokter & pasien, resep berikut item & racikan, dan skrining apoteker.
--
-- Resep diinput (TTK/apoteker/pemilik) → divalidasi apoteker (skrining) → dibayar di kasir.
-- Sebelum dibayar, obat resep disimpan di `prescription_items` (belum mengurangi stok). Saat
-- dibayar, item disalin ke `sale_items` (lewat `sales.prescription_id`) dan stok berkurang FEFO.

-- ─── Dokter & pasien (tabel `customers`) ─────────────────────────────────────
-- Mengikuti pola master lain: kode otomatis yang tidak bisa diubah, soft delete, pembuat.
-- Kedua tabel belum pernah punya layar isi, tetapi data lama (bila ada) tetap diberi kode.

ALTER TABLE doctors ADD COLUMN code TEXT;
ALTER TABLE doctors ADD COLUMN deleted_at TEXT;
ALTER TABLE doctors ADD COLUMN deleted_by INTEGER REFERENCES users (id);
ALTER TABLE doctors ADD COLUMN created_by INTEGER REFERENCES users (id);

ALTER TABLE customers ADD COLUMN code TEXT;
ALTER TABLE customers ADD COLUMN deleted_at TEXT;
ALTER TABLE customers ADD COLUMN deleted_by INTEGER REFERENCES users (id);
ALTER TABLE customers ADD COLUMN created_by INTEGER REFERENCES users (id);

UPDATE doctors SET code = 'DOK' || printf('%04d', id);
UPDATE customers SET code = 'PSN' || printf('%04d', id);

CREATE UNIQUE INDEX doctors_code ON doctors (code COLLATE NOCASE) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX customers_code ON customers (code COLLATE NOCASE) WHERE deleted_at IS NULL;
CREATE INDEX doctors_name ON doctors (name COLLATE NOCASE);
CREATE INDEX customers_name ON customers (name COLLATE NOCASE);

CREATE TRIGGER doctors_code_required BEFORE INSERT ON doctors
WHEN new.code IS NULL OR new.code = ''
BEGIN SELECT RAISE(ABORT, 'doctors.code is required'); END;

CREATE TRIGGER customers_code_required BEFORE INSERT ON customers
WHEN new.code IS NULL OR new.code = ''
BEGIN SELECT RAISE(ABORT, 'customers.code is required'); END;

CREATE TRIGGER doctors_code_immutable BEFORE UPDATE OF code ON doctors
WHEN old.code IS NOT new.code
BEGIN SELECT RAISE(ABORT, 'doctors.code tidak bisa diubah'); END;

CREATE TRIGGER customers_code_immutable BEFORE UPDATE OF code ON customers
WHEN old.code IS NOT new.code
BEGIN SELECT RAISE(ABORT, 'customers.code tidak bisa diubah'); END;

-- ─── Resep ───────────────────────────────────────────────────────────────────

-- Nomor internal (RSP-2609-0001), dibedakan dari nomor resep yang ditulis dokter.
ALTER TABLE prescriptions ADD COLUMN number TEXT;
ALTER TABLE prescriptions ADD COLUMN status TEXT NOT NULL DEFAULT 'DRAFT'
    CHECK (status IN ('DRAFT', 'SCREENED', 'PAID', 'CANCELLED'));
ALTER TABLE prescriptions ADD COLUMN screening_note TEXT;
ALTER TABLE prescriptions ADD COLUMN created_by INTEGER REFERENCES users (id);
ALTER TABLE prescriptions ADD COLUMN cancelled_at TEXT;
ALTER TABLE prescriptions ADD COLUMN cancelled_by INTEGER REFERENCES users (id);
ALTER TABLE prescriptions ADD COLUMN cancel_reason TEXT;

CREATE UNIQUE INDEX prescriptions_number ON prescriptions (number);
CREATE INDEX prescriptions_status ON prescriptions (status, id);
CREATE INDEX prescriptions_date ON prescriptions (prescription_date);
CREATE INDEX prescriptions_doctor ON prescriptions (doctor_id);
CREATE INDEX prescriptions_customer ON prescriptions (customer_id);

CREATE TRIGGER prescriptions_number_required BEFORE INSERT ON prescriptions
WHEN new.number IS NULL OR new.number = ''
BEGIN SELECT RAISE(ABORT, 'prescriptions.number is required'); END;

CREATE TRIGGER prescriptions_number_immutable BEFORE UPDATE OF number ON prescriptions
WHEN old.number IS NOT new.number
BEGIN SELECT RAISE(ABORT, 'prescriptions.number tidak bisa diubah'); END;

-- Resep adalah dokumen transaksi: tidak pernah dihapus, batal = status CANCELLED.
CREATE TRIGGER prescriptions_no_delete BEFORE DELETE ON prescriptions
BEGIN SELECT RAISE(ABORT, 'prescriptions: hapus permanen tidak diizinkan, gunakan batal'); END;

-- Resep yang sudah dibayar atau dibatalkan tidak bisa diubah lagi.
CREATE TRIGGER prescriptions_closed BEFORE UPDATE ON prescriptions
WHEN old.status IN ('PAID', 'CANCELLED')
BEGIN SELECT RAISE(ABORT, 'prescriptions: resep sudah dibayar atau dibatalkan'); END;

-- Item resep sebelum dibayar. Bentuknya sama dengan `sale_items`:
--   PRODUCT  obat biasa (atau komponen racikan bila parent_item_id terisi)
--   COMPOUND racikan; nilainya = jumlah komponennya
--   SERVICE  jasa racik, embalase, dll.
-- Harga adalah perkiraan saat input (sudah harga tier); harga final dihitung ulang saat dibayar.
CREATE TABLE prescription_items (
    id                 INTEGER PRIMARY KEY,
    prescription_id    INTEGER NOT NULL REFERENCES prescriptions (id),
    line_no            INTEGER NOT NULL,
    item_kind          TEXT    NOT NULL CHECK (item_kind IN ('PRODUCT', 'COMPOUND', 'SERVICE')),
    parent_item_id     INTEGER REFERENCES prescription_items (id),
    product_id         INTEGER REFERENCES products (id),
    product_unit_id    INTEGER REFERENCES product_units (id),
    description        TEXT    NOT NULL,
    compound_form      TEXT    CHECK (compound_form IN ('POWDER', 'CAPSULE', 'OINTMENT', 'LIQUID', 'OTHER')),
    qty                INTEGER NOT NULL CHECK (qty > 0),
    conversion         INTEGER NOT NULL DEFAULT 1 CHECK (conversion >= 1),
    qty_base           INTEGER NOT NULL CHECK (qty_base > 0),
    unit_price         INTEGER NOT NULL CHECK (unit_price >= 0),
    price_tier_id      INTEGER REFERENCES price_tiers (id),
    line_total         INTEGER NOT NULL CHECK (line_total >= 0),
    usage_instruction  TEXT,
    created_at         TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    CHECK (item_kind <> 'PRODUCT' OR (product_id IS NOT NULL AND product_unit_id IS NOT NULL)),
    CHECK (item_kind = 'PRODUCT' OR (product_id IS NULL AND product_unit_id IS NULL)),
    CHECK (item_kind = 'COMPOUND' OR compound_form IS NULL),
    CHECK (item_kind = 'PRODUCT' OR parent_item_id IS NULL)
);

CREATE INDEX prescription_items_prescription ON prescription_items (prescription_id, line_no);
CREATE INDEX prescription_items_product ON prescription_items (product_id);

-- Item hanya boleh diganti selama resep belum dibayar/dibatalkan.
CREATE TRIGGER prescription_items_open_insert BEFORE INSERT ON prescription_items
WHEN (SELECT status FROM prescriptions WHERE id = new.prescription_id) IN ('PAID', 'CANCELLED')
BEGIN SELECT RAISE(ABORT, 'prescription_items: resep sudah dibayar atau dibatalkan'); END;

CREATE TRIGGER prescription_items_open_delete BEFORE DELETE ON prescription_items
WHEN (SELECT status FROM prescriptions WHERE id = old.prescription_id) IN ('PAID', 'CANCELLED')
BEGIN SELECT RAISE(ABORT, 'prescription_items: resep sudah dibayar atau dibatalkan'); END;

CREATE TRIGGER prescription_items_no_update BEFORE UPDATE ON prescription_items
BEGIN SELECT RAISE(ABORT, 'prescription_items: ganti item dengan hapus lalu tambah'); END;
