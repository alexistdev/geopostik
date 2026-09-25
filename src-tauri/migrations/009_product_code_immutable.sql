-- Kode obat selalu dibuat otomatis oleh sistem dan tidak bisa diubah setelah tersimpan,
-- sama seperti kode master data, agar label barcode yang sudah dicetak tetap valid.

CREATE TRIGGER products_code_immutable BEFORE UPDATE OF code ON products
WHEN old.code IS NOT new.code
BEGIN
    SELECT RAISE(ABORT, 'products.code tidak bisa diubah');
END;
