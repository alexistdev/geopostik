-- Kode master (kategori, rak, pabrik) selalu dibuat otomatis oleh sistem dan tidak bisa diubah
-- setelah tersimpan, agar label barcode yang sudah dicetak selalu menunjuk ke data yang sama.

CREATE TRIGGER categories_code_immutable BEFORE UPDATE OF code ON categories
WHEN old.code IS NOT new.code
BEGIN
    SELECT RAISE(ABORT, 'categories.code tidak bisa diubah');
END;

CREATE TRIGGER racks_code_immutable BEFORE UPDATE OF code ON racks
WHEN old.code IS NOT new.code
BEGIN
    SELECT RAISE(ABORT, 'racks.code tidak bisa diubah');
END;

CREATE TRIGGER manufacturers_code_immutable BEFORE UPDATE OF code ON manufacturers
WHEN old.code IS NOT new.code
BEGIN
    SELECT RAISE(ABORT, 'manufacturers.code tidak bisa diubah');
END;
