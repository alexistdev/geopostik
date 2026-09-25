-- Kode untuk master data (kategori, rak, pabrik), bisa dicetak sebagai label barcode.
-- Format kode: huruf besar, angka, dan tanda hubung (aman untuk barcode Code 128).

ALTER TABLE categories ADD COLUMN code TEXT;
ALTER TABLE racks ADD COLUMN code TEXT;
ALTER TABLE manufacturers ADD COLUMN code TEXT;

-- Data yang sudah ada diberi kode otomatis.
UPDATE categories SET code = 'KTG' || printf('%04d', id);
UPDATE racks SET code = 'RAK' || printf('%04d', id);
UPDATE manufacturers SET code = 'PBR' || printf('%04d', id);

CREATE UNIQUE INDEX categories_code ON categories (code COLLATE NOCASE);
CREATE UNIQUE INDEX racks_code ON racks (code COLLATE NOCASE);
CREATE UNIQUE INDEX manufacturers_code ON manufacturers (code COLLATE NOCASE);

-- ALTER TABLE tidak bisa menambah NOT NULL tanpa default; dijaga dengan trigger.
CREATE TRIGGER categories_code_required BEFORE INSERT ON categories
WHEN new.code IS NULL OR new.code = ''
BEGIN
    SELECT RAISE(ABORT, 'categories.code is required');
END;

CREATE TRIGGER racks_code_required BEFORE INSERT ON racks
WHEN new.code IS NULL OR new.code = ''
BEGIN
    SELECT RAISE(ABORT, 'racks.code is required');
END;

CREATE TRIGGER manufacturers_code_required BEFORE INSERT ON manufacturers
WHEN new.code IS NULL OR new.code = ''
BEGIN
    SELECT RAISE(ABORT, 'manufacturers.code is required');
END;

CREATE TRIGGER categories_code_not_empty BEFORE UPDATE OF code ON categories
WHEN new.code IS NULL OR new.code = ''
BEGIN
    SELECT RAISE(ABORT, 'categories.code is required');
END;

CREATE TRIGGER racks_code_not_empty BEFORE UPDATE OF code ON racks
WHEN new.code IS NULL OR new.code = ''
BEGIN
    SELECT RAISE(ABORT, 'racks.code is required');
END;

CREATE TRIGGER manufacturers_code_not_empty BEFORE UPDATE OF code ON manufacturers
WHEN new.code IS NULL OR new.code = ''
BEGIN
    SELECT RAISE(ABORT, 'manufacturers.code is required');
END;
