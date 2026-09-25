-- Master rak dan pabrik: menggantikan kolom teks bebas di products.

CREATE TABLE racks (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE manufacturers (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

ALTER TABLE products ADD COLUMN manufacturer_id INTEGER REFERENCES manufacturers (id);
ALTER TABLE products ADD COLUMN rack_id INTEGER REFERENCES racks (id);

-- Pindahkan isi teks lama ke tabel master.
INSERT OR IGNORE INTO manufacturers (name)
SELECT DISTINCT trim(manufacturer) FROM products
WHERE manufacturer IS NOT NULL AND trim(manufacturer) <> '';

INSERT OR IGNORE INTO racks (name)
SELECT DISTINCT trim(rack_location) FROM products
WHERE rack_location IS NOT NULL AND trim(rack_location) <> '';

UPDATE products
SET manufacturer_id = (SELECT id FROM manufacturers m WHERE m.name = trim(products.manufacturer) COLLATE NOCASE)
WHERE manufacturer IS NOT NULL;

UPDATE products
SET rack_id = (SELECT id FROM racks r WHERE r.name = trim(products.rack_location) COLLATE NOCASE)
WHERE rack_location IS NOT NULL;

ALTER TABLE products DROP COLUMN manufacturer;
ALTER TABLE products DROP COLUMN rack_location;

CREATE INDEX products_rack ON products (rack_id);
CREATE INDEX products_manufacturer ON products (manufacturer_id);
