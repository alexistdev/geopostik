-- Barcode obat tidak pernah dihapus permanen: barcode yang dilepas dari obat ditandai terhapus.
-- Tabel dibangun ulang karena UNIQUE pada kolom barcode harus diganti index unik parsial (hanya
-- barcode yang belum dihapus), agar barcode yang pernah dilepas boleh dipakai obat lain.
--
-- Dijalankan dengan foreign_keys = OFF (lihat db::prepare) dan diperiksa foreign_key_check.

CREATE TABLE product_barcodes_new (
    id               INTEGER PRIMARY KEY,
    product_unit_id  INTEGER NOT NULL REFERENCES product_units (id),
    barcode          TEXT    NOT NULL CHECK (barcode <> ''),
    deleted_at       TEXT,
    deleted_by       INTEGER REFERENCES users (id),
    created_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

INSERT INTO product_barcodes_new (id, product_unit_id, barcode, created_at)
SELECT id, product_unit_id, barcode, created_at FROM product_barcodes;

DROP TABLE product_barcodes;
ALTER TABLE product_barcodes_new RENAME TO product_barcodes;

CREATE UNIQUE INDEX product_barcodes_barcode ON product_barcodes (barcode) WHERE deleted_at IS NULL;
CREATE INDEX product_barcodes_unit ON product_barcodes (product_unit_id);

CREATE TRIGGER product_barcodes_no_delete BEFORE DELETE ON product_barcodes
BEGIN SELECT RAISE(ABORT, 'product_barcodes: hapus permanen tidak diizinkan, gunakan soft delete'); END;
