-- Master data tidak pernah dihapus permanen.
--
-- 1. Kategori, rak, pabrik mendapat kolom soft delete (deleted_at, deleted_by). Tabel dibangun
--    ulang karena UNIQUE pada kolom name harus diganti index unik parsial (hanya data yang belum
--    dihapus), agar nama/kode data yang sudah dihapus boleh dipakai lagi.
-- 2. DELETE ditolak oleh trigger di semua tabel master.
--
-- Dijalankan dengan foreign_keys = OFF (lihat db::prepare) dan diperiksa foreign_key_check.

-- ─── Kategori ────────────────────────────────────────────────────────────────

CREATE TABLE categories_new (
    id          INTEGER PRIMARY KEY,
    code        TEXT    NOT NULL CHECK (code <> ''),
    name        TEXT    NOT NULL COLLATE NOCASE,
    margin_bp   INTEGER CHECK (margin_bp >= 0),
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    deleted_at  TEXT,
    deleted_by  INTEGER REFERENCES users (id),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

INSERT INTO categories_new (id, code, name, margin_bp, is_active, created_at, updated_at)
SELECT id, code, name, margin_bp, is_active, created_at, updated_at FROM categories;

DROP TABLE categories;
ALTER TABLE categories_new RENAME TO categories;

CREATE UNIQUE INDEX categories_code ON categories (code COLLATE NOCASE) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX categories_name ON categories (name COLLATE NOCASE) WHERE deleted_at IS NULL;

-- ─── Rak ─────────────────────────────────────────────────────────────────────

CREATE TABLE racks_new (
    id          INTEGER PRIMARY KEY,
    code        TEXT    NOT NULL CHECK (code <> ''),
    name        TEXT    NOT NULL COLLATE NOCASE,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    deleted_at  TEXT,
    deleted_by  INTEGER REFERENCES users (id),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

INSERT INTO racks_new (id, code, name, is_active, created_at, updated_at)
SELECT id, code, name, is_active, created_at, updated_at FROM racks;

DROP TABLE racks;
ALTER TABLE racks_new RENAME TO racks;

CREATE UNIQUE INDEX racks_code ON racks (code COLLATE NOCASE) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX racks_name ON racks (name COLLATE NOCASE) WHERE deleted_at IS NULL;

-- ─── Pabrik ──────────────────────────────────────────────────────────────────

CREATE TABLE manufacturers_new (
    id          INTEGER PRIMARY KEY,
    code        TEXT    NOT NULL CHECK (code <> ''),
    name        TEXT    NOT NULL COLLATE NOCASE,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    deleted_at  TEXT,
    deleted_by  INTEGER REFERENCES users (id),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

INSERT INTO manufacturers_new (id, code, name, is_active, created_at, updated_at)
SELECT id, code, name, is_active, created_at, updated_at FROM manufacturers;

DROP TABLE manufacturers;
ALTER TABLE manufacturers_new RENAME TO manufacturers;

CREATE UNIQUE INDEX manufacturers_code ON manufacturers (code COLLATE NOCASE) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX manufacturers_name ON manufacturers (name COLLATE NOCASE) WHERE deleted_at IS NULL;

-- ─── Tolak DELETE permanen di semua tabel master ─────────────────────────────

CREATE TRIGGER categories_no_delete BEFORE DELETE ON categories
BEGIN SELECT RAISE(ABORT, 'categories: hapus permanen tidak diizinkan, gunakan soft delete'); END;

CREATE TRIGGER racks_no_delete BEFORE DELETE ON racks
BEGIN SELECT RAISE(ABORT, 'racks: hapus permanen tidak diizinkan, gunakan soft delete'); END;

CREATE TRIGGER manufacturers_no_delete BEFORE DELETE ON manufacturers
BEGIN SELECT RAISE(ABORT, 'manufacturers: hapus permanen tidak diizinkan, gunakan soft delete'); END;

CREATE TRIGGER units_no_delete BEFORE DELETE ON units
BEGIN SELECT RAISE(ABORT, 'units: hapus permanen tidak diizinkan'); END;

CREATE TRIGGER products_no_delete BEFORE DELETE ON products
BEGIN SELECT RAISE(ABORT, 'products: hapus permanen tidak diizinkan, gunakan nonaktif'); END;

CREATE TRIGGER product_units_no_delete BEFORE DELETE ON product_units
BEGIN SELECT RAISE(ABORT, 'product_units: hapus permanen tidak diizinkan, gunakan nonaktif'); END;

CREATE TRIGGER price_tiers_no_delete BEFORE DELETE ON price_tiers
BEGIN SELECT RAISE(ABORT, 'price_tiers: hapus permanen tidak diizinkan, gunakan nonaktif'); END;

CREATE TRIGGER users_no_delete BEFORE DELETE ON users
BEGIN SELECT RAISE(ABORT, 'users: hapus permanen tidak diizinkan, gunakan nonaktif'); END;

CREATE TRIGGER suppliers_no_delete BEFORE DELETE ON suppliers
BEGIN SELECT RAISE(ABORT, 'suppliers: hapus permanen tidak diizinkan, gunakan nonaktif'); END;

CREATE TRIGGER doctors_no_delete BEFORE DELETE ON doctors
BEGIN SELECT RAISE(ABORT, 'doctors: hapus permanen tidak diizinkan, gunakan nonaktif'); END;

CREATE TRIGGER customers_no_delete BEFORE DELETE ON customers
BEGIN SELECT RAISE(ABORT, 'customers: hapus permanen tidak diizinkan, gunakan nonaktif'); END;
