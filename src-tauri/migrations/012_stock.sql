-- Menu Stok: stok per batch, kartu stok, stok opname (stok awal & berkala).

-- Alasan batch dikunci (recall, rusak, menunggu pemusnahan), tampil di daftar batch.
ALTER TABLE batches ADD COLUMN lock_reason TEXT;

-- Satu batch hanya boleh muncul sekali dalam satu opname.
CREATE UNIQUE INDEX stock_opname_items_batch ON stock_opname_items (opname_id, batch_id)
WHERE batch_id IS NOT NULL;

CREATE INDEX stock_opnames_status ON stock_opnames (status, id);
CREATE INDEX batches_product_qty ON batches (product_id, qty_on_hand_base);

-- Dokumen opname tidak pernah dihapus; batal = status CANCELLED.
CREATE TRIGGER stock_opnames_no_delete BEFORE DELETE ON stock_opnames
BEGIN SELECT RAISE(ABORT, 'stock_opnames: hapus permanen tidak diizinkan, gunakan batal'); END;

-- Baris opname hanya boleh diubah/dihapus selama dokumennya masih DRAFT.
CREATE TRIGGER stock_opname_items_draft_update BEFORE UPDATE ON stock_opname_items
WHEN (SELECT status FROM stock_opnames WHERE id = old.opname_id) <> 'DRAFT'
     AND NOT (old.batch_id IS NULL AND new.batch_id IS NOT NULL
              AND (SELECT status FROM stock_opnames WHERE id = old.opname_id) = 'SUBMITTED')
BEGIN SELECT RAISE(ABORT, 'stock_opname_items: opname sudah tidak DRAFT'); END;

CREATE TRIGGER stock_opname_items_draft_delete BEFORE DELETE ON stock_opname_items
WHEN (SELECT status FROM stock_opnames WHERE id = old.opname_id) <> 'DRAFT'
BEGIN SELECT RAISE(ABORT, 'stock_opname_items: opname sudah tidak DRAFT'); END;

CREATE TRIGGER stock_opname_items_draft_insert BEFORE INSERT ON stock_opname_items
WHEN (SELECT status FROM stock_opnames WHERE id = new.opname_id) <> 'DRAFT'
BEGIN SELECT RAISE(ABORT, 'stock_opname_items: opname sudah tidak DRAFT'); END;
