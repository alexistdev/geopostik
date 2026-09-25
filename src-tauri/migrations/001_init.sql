-- GeoPOSTik: skema awal. Lihat docs/SCHEMA.md.
-- Konvensi: uang = INTEGER rupiah, HPP per satuan terkecil = *_x100, persen = *_bp,
-- qty stok = *_base (satuan terkecil), waktu = TEXT waktu lokal.

-- ─── 1. Pengguna, pengaturan, audit ──────────────────────────────────────────

CREATE TABLE users (
    id              INTEGER PRIMARY KEY,
    username        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    full_name       TEXT    NOT NULL,
    password_hash   TEXT    NOT NULL,
    pin_hash        TEXT,
    license_type    TEXT    CHECK (license_type IN ('SIPA', 'SIPTTK')),
    license_number  TEXT,
    is_active       INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE user_roles (
    user_id  INTEGER NOT NULL REFERENCES users (id),
    role     TEXT    NOT NULL CHECK (role IN ('OWNER', 'PHARMACIST', 'TECHNICIAN', 'CASHIER')),
    PRIMARY KEY (user_id, role)
) WITHOUT ROWID;

CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
) WITHOUT ROWID;

CREATE TABLE audit_logs (
    id             INTEGER PRIMARY KEY,
    user_id        INTEGER REFERENCES users (id),
    authorized_by  INTEGER REFERENCES users (id),
    action         TEXT    NOT NULL,
    entity         TEXT,
    entity_id      INTEGER,
    detail         TEXT,
    reason         TEXT,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TRIGGER audit_logs_no_update BEFORE UPDATE ON audit_logs
BEGIN
    SELECT RAISE(ABORT, 'audit_logs is append-only');
END;

CREATE TRIGGER audit_logs_no_delete BEFORE DELETE ON audit_logs
BEGIN
    SELECT RAISE(ABORT, 'audit_logs is append-only');
END;

CREATE TABLE doc_sequences (
    prefix   TEXT    NOT NULL,
    period   TEXT    NOT NULL,
    last_no  INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (prefix, period)
) WITHOUT ROWID;

-- ─── 2. Master data ──────────────────────────────────────────────────────────

CREATE TABLE categories (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    margin_bp   INTEGER CHECK (margin_bp >= 0),
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE units (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE products (
    id              INTEGER PRIMARY KEY,
    code            TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    name            TEXT    NOT NULL,
    generic_name    TEXT,
    manufacturer    TEXT,
    category_id     INTEGER REFERENCES categories (id),
    drug_class      TEXT    NOT NULL
                    CHECK (drug_class IN ('FREE', 'LIMITED_FREE', 'HARD', 'PSYCHOTROPIC', 'NARCOTIC')),
    is_owa          INTEGER NOT NULL DEFAULT 0 CHECK (is_owa IN (0, 1)),
    base_unit_id    INTEGER NOT NULL REFERENCES units (id),
    min_stock_base  INTEGER NOT NULL DEFAULT 0 CHECK (min_stock_base >= 0),
    rack_location   TEXT,
    margin_bp       INTEGER CHECK (margin_bp >= 0),
    last_cost_x100  INTEGER CHECK (last_cost_x100 >= 0),
    is_active       INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    CHECK (is_owa = 0 OR drug_class = 'HARD')
);

CREATE INDEX products_name ON products (name);

-- Pencarian instan nama/generik/kode di kasir.
CREATE VIRTUAL TABLE products_fts USING fts5 (
    code, name, generic_name,
    content = 'products', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2',
    prefix = '2 3'
);

CREATE TRIGGER products_fts_ai AFTER INSERT ON products
BEGIN
    INSERT INTO products_fts (rowid, code, name, generic_name)
    VALUES (new.id, new.code, new.name, new.generic_name);
END;

CREATE TRIGGER products_fts_ad AFTER DELETE ON products
BEGIN
    INSERT INTO products_fts (products_fts, rowid, code, name, generic_name)
    VALUES ('delete', old.id, old.code, old.name, old.generic_name);
END;

CREATE TRIGGER products_fts_au AFTER UPDATE OF code, name, generic_name ON products
BEGIN
    INSERT INTO products_fts (products_fts, rowid, code, name, generic_name)
    VALUES ('delete', old.id, old.code, old.name, old.generic_name);
    INSERT INTO products_fts (rowid, code, name, generic_name)
    VALUES (new.id, new.code, new.name, new.generic_name);
END;

CREATE TABLE product_units (
    id               INTEGER PRIMARY KEY,
    product_id       INTEGER NOT NULL REFERENCES products (id),
    unit_id          INTEGER NOT NULL REFERENCES units (id),
    conversion       INTEGER NOT NULL CHECK (conversion >= 1),
    sell_price       INTEGER NOT NULL CHECK (sell_price >= 0),
    price_mode       TEXT    NOT NULL DEFAULT 'AUTO' CHECK (price_mode IN ('AUTO', 'MANUAL')),
    is_default_sale  INTEGER NOT NULL DEFAULT 0 CHECK (is_default_sale IN (0, 1)),
    is_active        INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE (product_id, unit_id)
);

CREATE TABLE price_tiers (
    id               INTEGER PRIMARY KEY,
    product_unit_id  INTEGER NOT NULL REFERENCES product_units (id),
    min_qty          INTEGER NOT NULL CHECK (min_qty > 1),
    price_mode       TEXT    NOT NULL DEFAULT 'MANUAL' CHECK (price_mode IN ('AUTO', 'MANUAL')),
    margin_bp        INTEGER CHECK (margin_bp >= 0),
    price            INTEGER NOT NULL CHECK (price >= 0),
    is_active        INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE (product_unit_id, min_qty),
    CHECK (price_mode = 'MANUAL' OR margin_bp IS NOT NULL)
);

CREATE TABLE product_barcodes (
    id               INTEGER PRIMARY KEY,
    product_unit_id  INTEGER NOT NULL REFERENCES product_units (id),
    barcode          TEXT    NOT NULL UNIQUE,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE suppliers (
    id                 INTEGER PRIMARY KEY,
    name               TEXT    NOT NULL,
    address            TEXT,
    phone              TEXT,
    npwp               TEXT,
    payment_term_days  INTEGER NOT NULL DEFAULT 0 CHECK (payment_term_days >= 0),
    is_active          INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at         TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at         TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE doctors (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    sip_number  TEXT,
    specialty   TEXT,
    address     TEXT,
    phone       TEXT,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE customers (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    gender      TEXT    CHECK (gender IN ('M', 'F')),
    birth_date  TEXT,
    address     TEXT,
    phone       TEXT,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- ─── 3. Stok ─────────────────────────────────────────────────────────────────

-- Satu baris = satu lot penerimaan. Stok hanya diubah oleh trigger stock_movements.
CREATE TABLE batches (
    id                INTEGER PRIMARY KEY,
    product_id        INTEGER NOT NULL REFERENCES products (id),
    batch_number      TEXT    NOT NULL,
    expiry_date       TEXT    NOT NULL,
    unit_cost_x100    INTEGER NOT NULL CHECK (unit_cost_x100 >= 0),
    qty_on_hand_base  INTEGER NOT NULL DEFAULT 0 CHECK (qty_on_hand_base >= 0),
    is_locked         INTEGER NOT NULL DEFAULT 0 CHECK (is_locked IN (0, 1)),
    source_type       TEXT    NOT NULL CHECK (source_type IN ('OPENING', 'PURCHASE', 'ADJUSTMENT')),
    source_id         INTEGER,
    created_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX batches_fefo ON batches (product_id, expiry_date);

-- Kartu stok: append-only.
CREATE TABLE stock_movements (
    id               INTEGER PRIMARY KEY,
    batch_id         INTEGER NOT NULL REFERENCES batches (id),
    product_id       INTEGER NOT NULL REFERENCES products (id),
    movement_type    TEXT    NOT NULL CHECK (movement_type IN (
                         'OPENING', 'PURCHASE', 'PURCHASE_VOID', 'SALE', 'SALE_VOID',
                         'SALE_RETURN', 'SUPPLIER_RETURN', 'ADJUSTMENT', 'DESTRUCTION')),
    qty_change_base  INTEGER NOT NULL CHECK (qty_change_base <> 0),
    ref_type         TEXT    NOT NULL,
    ref_id           INTEGER NOT NULL,
    ref_line_id      INTEGER,
    user_id          INTEGER NOT NULL REFERENCES users (id),
    note             TEXT,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX stock_movements_product ON stock_movements (product_id, id);
CREATE INDEX stock_movements_batch ON stock_movements (batch_id, id);
CREATE INDEX stock_movements_ref ON stock_movements (ref_type, ref_id);

CREATE TRIGGER stock_movements_check_product BEFORE INSERT ON stock_movements
WHEN (SELECT product_id FROM batches WHERE id = new.batch_id) IS NOT new.product_id
BEGIN
    SELECT RAISE(ABORT, 'stock_movements.product_id does not match batch');
END;

-- CHECK (qty_on_hand_base >= 0) di batches membuat stok minus menggagalkan seluruh transaksi.
CREATE TRIGGER stock_movements_apply AFTER INSERT ON stock_movements
BEGIN
    UPDATE batches
    SET qty_on_hand_base = qty_on_hand_base + new.qty_change_base,
        updated_at = datetime('now', 'localtime')
    WHERE id = new.batch_id;
END;

CREATE TRIGGER stock_movements_no_update BEFORE UPDATE ON stock_movements
BEGIN
    SELECT RAISE(ABORT, 'stock_movements is append-only');
END;

CREATE TRIGGER stock_movements_no_delete BEFORE DELETE ON stock_movements
BEGIN
    SELECT RAISE(ABORT, 'stock_movements is append-only');
END;

CREATE TABLE stock_opnames (
    id            INTEGER PRIMARY KEY,
    number        TEXT    NOT NULL UNIQUE,
    opname_type   TEXT    NOT NULL CHECK (opname_type IN ('OPENING', 'PERIODIC')),
    scope_note    TEXT,
    status        TEXT    NOT NULL DEFAULT 'DRAFT'
                  CHECK (status IN ('DRAFT', 'SUBMITTED', 'APPROVED', 'CANCELLED')),
    created_by    INTEGER NOT NULL REFERENCES users (id),
    approved_by   INTEGER REFERENCES users (id),
    approved_at   TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE stock_opname_items (
    id                 INTEGER PRIMARY KEY,
    opname_id          INTEGER NOT NULL REFERENCES stock_opnames (id),
    product_id         INTEGER NOT NULL REFERENCES products (id),
    batch_id           INTEGER REFERENCES batches (id),
    batch_number       TEXT,
    expiry_date        TEXT,
    unit_cost_x100     INTEGER CHECK (unit_cost_x100 >= 0),
    system_qty_base    INTEGER NOT NULL DEFAULT 0,
    physical_qty_base  INTEGER CHECK (physical_qty_base >= 0),
    note               TEXT,
    created_at         TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at         TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    CHECK (batch_id IS NOT NULL
           OR (batch_number IS NOT NULL AND expiry_date IS NOT NULL AND unit_cost_x100 IS NOT NULL))
);

CREATE INDEX stock_opname_items_opname ON stock_opname_items (opname_id);

CREATE TABLE destructions (
    id                INTEGER PRIMARY KEY,
    number            TEXT    NOT NULL UNIQUE,
    destruction_date  TEXT    NOT NULL,
    witnesses         TEXT,
    note              TEXT,
    status            TEXT    NOT NULL DEFAULT 'DRAFT' CHECK (status IN ('DRAFT', 'APPROVED', 'CANCELLED')),
    created_by        INTEGER NOT NULL REFERENCES users (id),
    approved_by       INTEGER REFERENCES users (id),
    created_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at        TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE destruction_items (
    id              INTEGER PRIMARY KEY,
    destruction_id  INTEGER NOT NULL REFERENCES destructions (id),
    batch_id        INTEGER NOT NULL REFERENCES batches (id),
    qty_base        INTEGER NOT NULL CHECK (qty_base > 0),
    created_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- ─── 4. Pembelian ────────────────────────────────────────────────────────────

CREATE TABLE purchases (
    id              INTEGER PRIMARY KEY,
    number          TEXT    NOT NULL UNIQUE,
    supplier_id     INTEGER NOT NULL REFERENCES suppliers (id),
    invoice_number  TEXT    NOT NULL,
    invoice_date    TEXT    NOT NULL,
    received_date   TEXT    NOT NULL,
    due_date        TEXT,
    payment_type    TEXT    NOT NULL CHECK (payment_type IN ('CASH', 'CREDIT')),
    tax_mode        TEXT    NOT NULL CHECK (tax_mode IN ('INCLUDED', 'EXCLUDED', 'NONE')),
    tax_rate_bp     INTEGER NOT NULL DEFAULT 0 CHECK (tax_rate_bp >= 0),
    subtotal        INTEGER NOT NULL DEFAULT 0,
    discount_total  INTEGER NOT NULL DEFAULT 0,
    tax_total       INTEGER NOT NULL DEFAULT 0,
    grand_total     INTEGER NOT NULL DEFAULT 0,
    status          TEXT    NOT NULL DEFAULT 'DRAFT' CHECK (status IN ('DRAFT', 'POSTED', 'VOID')),
    note            TEXT,
    created_by      INTEGER NOT NULL REFERENCES users (id),
    posted_at       TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE (supplier_id, invoice_number),
    CHECK (payment_type = 'CASH' OR due_date IS NOT NULL)
);

CREATE INDEX purchases_due_credit ON purchases (due_date) WHERE payment_type = 'CREDIT';

CREATE TABLE purchase_items (
    id               INTEGER PRIMARY KEY,
    purchase_id      INTEGER NOT NULL REFERENCES purchases (id),
    product_id       INTEGER NOT NULL REFERENCES products (id),
    product_unit_id  INTEGER NOT NULL REFERENCES product_units (id),
    qty              INTEGER NOT NULL CHECK (qty > 0),
    conversion       INTEGER NOT NULL CHECK (conversion >= 1),
    unit_price       INTEGER NOT NULL CHECK (unit_price >= 0),
    discount1_bp     INTEGER NOT NULL DEFAULT 0 CHECK (discount1_bp BETWEEN 0 AND 10000),
    discount2_bp     INTEGER NOT NULL DEFAULT 0 CHECK (discount2_bp BETWEEN 0 AND 10000),
    line_total       INTEGER NOT NULL CHECK (line_total >= 0),
    batch_number     TEXT    NOT NULL,
    expiry_date      TEXT    NOT NULL,
    unit_cost_x100   INTEGER CHECK (unit_cost_x100 >= 0),
    batch_id         INTEGER REFERENCES batches (id),
    created_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX purchase_items_purchase ON purchase_items (purchase_id);

CREATE TABLE supplier_payments (
    id            INTEGER PRIMARY KEY,
    supplier_id   INTEGER NOT NULL REFERENCES suppliers (id),
    purchase_id   INTEGER NOT NULL REFERENCES purchases (id),
    payment_date  TEXT    NOT NULL,
    amount        INTEGER NOT NULL CHECK (amount > 0),
    method        TEXT    NOT NULL CHECK (method IN ('CASH', 'TRANSFER', 'GIRO')),
    note          TEXT,
    created_by    INTEGER NOT NULL REFERENCES users (id),
    created_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE INDEX supplier_payments_purchase ON supplier_payments (purchase_id);

-- ─── 5. Shift dan penjualan ──────────────────────────────────────────────────

CREATE TABLE shifts (
    id             INTEGER PRIMARY KEY,
    user_id        INTEGER NOT NULL REFERENCES users (id),
    opened_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    closed_at      TEXT,
    opening_cash   INTEGER NOT NULL CHECK (opening_cash >= 0),
    expected_cash  INTEGER,
    counted_cash   INTEGER CHECK (counted_cash >= 0),
    difference     INTEGER,
    note           TEXT,
    status         TEXT    NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN', 'CLOSED'))
);

CREATE UNIQUE INDEX shifts_one_open ON shifts (status) WHERE status = 'OPEN';

CREATE TABLE prescriptions (
    id                   INTEGER PRIMARY KEY,
    prescription_number  TEXT    NOT NULL,
    prescription_date    TEXT    NOT NULL,
    doctor_id            INTEGER NOT NULL REFERENCES doctors (id),
    customer_id          INTEGER REFERENCES customers (id),
    patient_name         TEXT    NOT NULL,
    patient_age          TEXT,
    patient_address      TEXT,
    screened_by          INTEGER REFERENCES users (id),
    screened_at          TEXT,
    note                 TEXT,
    created_at           TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    updated_at           TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE sales (
    id               INTEGER PRIMARY KEY,
    number           TEXT    NOT NULL UNIQUE,
    shift_id         INTEGER NOT NULL REFERENCES shifts (id),
    cashier_id       INTEGER NOT NULL REFERENCES users (id),
    customer_id      INTEGER REFERENCES customers (id),
    prescription_id  INTEGER REFERENCES prescriptions (id),
    sale_type        TEXT    NOT NULL CHECK (sale_type IN ('OTC', 'PRESCRIPTION')),
    sold_at          TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    subtotal         INTEGER NOT NULL,
    discount_total   INTEGER NOT NULL DEFAULT 0,
    tax_total        INTEGER NOT NULL DEFAULT 0,
    rounding         INTEGER NOT NULL DEFAULT 0,
    grand_total      INTEGER NOT NULL CHECK (grand_total >= 0),
    status           TEXT    NOT NULL DEFAULT 'COMPLETED' CHECK (status IN ('COMPLETED', 'VOID')),
    voided_at        TEXT,
    voided_by        INTEGER REFERENCES users (id),
    void_reason      TEXT,
    CHECK (sale_type = 'OTC' OR prescription_id IS NOT NULL),
    CHECK (status = 'COMPLETED' OR (voided_at IS NOT NULL AND voided_by IS NOT NULL AND void_reason IS NOT NULL))
);

CREATE INDEX sales_sold_at ON sales (sold_at);
CREATE INDEX sales_shift ON sales (shift_id);

CREATE TABLE sale_items (
    id                 INTEGER PRIMARY KEY,
    sale_id            INTEGER NOT NULL REFERENCES sales (id),
    line_no            INTEGER NOT NULL,
    item_kind          TEXT    NOT NULL CHECK (item_kind IN ('PRODUCT', 'COMPOUND', 'SERVICE')),
    parent_item_id     INTEGER REFERENCES sale_items (id),
    product_id         INTEGER REFERENCES products (id),
    product_unit_id    INTEGER REFERENCES product_units (id),
    description        TEXT    NOT NULL,
    qty                INTEGER NOT NULL CHECK (qty > 0),
    conversion         INTEGER NOT NULL DEFAULT 1 CHECK (conversion >= 1),
    qty_base           INTEGER NOT NULL CHECK (qty_base > 0),
    unit_price         INTEGER NOT NULL CHECK (unit_price >= 0),
    price_tier_id      INTEGER REFERENCES price_tiers (id),
    discount_amount    INTEGER NOT NULL DEFAULT 0 CHECK (discount_amount >= 0),
    line_total         INTEGER NOT NULL CHECK (line_total >= 0),
    usage_instruction  TEXT,
    authorized_by      INTEGER REFERENCES users (id),
    CHECK (item_kind <> 'PRODUCT' OR (product_id IS NOT NULL AND product_unit_id IS NOT NULL))
);

CREATE INDEX sale_items_sale ON sale_items (sale_id);

CREATE TABLE sale_item_batches (
    id              INTEGER PRIMARY KEY,
    sale_item_id    INTEGER NOT NULL REFERENCES sale_items (id),
    batch_id        INTEGER NOT NULL REFERENCES batches (id),
    qty_base        INTEGER NOT NULL CHECK (qty_base > 0),
    unit_cost_x100  INTEGER NOT NULL CHECK (unit_cost_x100 >= 0)
);

CREATE INDEX sale_item_batches_item ON sale_item_batches (sale_item_id);
CREATE INDEX sale_item_batches_batch ON sale_item_batches (batch_id);

CREATE TABLE sale_payments (
    id             INTEGER PRIMARY KEY,
    sale_id        INTEGER NOT NULL REFERENCES sales (id),
    method         TEXT    NOT NULL CHECK (method IN ('CASH', 'QRIS', 'DEBIT')),
    amount         INTEGER NOT NULL CHECK (amount > 0),
    tendered       INTEGER CHECK (tendered >= 0),
    change_amount  INTEGER CHECK (change_amount >= 0),
    reference      TEXT,
    CHECK (method = 'CASH' OR (tendered IS NULL AND change_amount IS NULL))
);

CREATE INDEX sale_payments_sale ON sale_payments (sale_id);
CREATE UNIQUE INDEX sale_payments_one_cash ON sale_payments (sale_id) WHERE method = 'CASH';

-- ─── 6. Retur ────────────────────────────────────────────────────────────────

CREATE TABLE sale_returns (
    id             INTEGER PRIMARY KEY,
    number         TEXT    NOT NULL UNIQUE,
    sale_id        INTEGER NOT NULL REFERENCES sales (id),
    shift_id       INTEGER NOT NULL REFERENCES shifts (id),
    returned_at    TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    reason         TEXT    NOT NULL,
    refund_total   INTEGER NOT NULL CHECK (refund_total >= 0),
    created_by     INTEGER NOT NULL REFERENCES users (id),
    authorized_by  INTEGER NOT NULL REFERENCES users (id)
);

CREATE TABLE sale_return_items (
    id                  INTEGER PRIMARY KEY,
    sale_return_id      INTEGER NOT NULL REFERENCES sale_returns (id),
    sale_item_batch_id  INTEGER NOT NULL REFERENCES sale_item_batches (id),
    qty_base            INTEGER NOT NULL CHECK (qty_base > 0),
    refund_amount       INTEGER NOT NULL CHECK (refund_amount >= 0)
);

CREATE INDEX sale_return_items_sib ON sale_return_items (sale_item_batch_id);

CREATE TABLE supplier_returns (
    id           INTEGER PRIMARY KEY,
    number       TEXT    NOT NULL UNIQUE,
    supplier_id  INTEGER NOT NULL REFERENCES suppliers (id),
    purchase_id  INTEGER REFERENCES purchases (id),
    return_date  TEXT    NOT NULL,
    settlement   TEXT    NOT NULL CHECK (settlement IN ('DEBT_CUT', 'REPLACEMENT', 'REFUND')),
    total        INTEGER NOT NULL CHECK (total >= 0),
    note         TEXT,
    created_by   INTEGER NOT NULL REFERENCES users (id),
    created_at   TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

CREATE TABLE supplier_return_items (
    id                  INTEGER PRIMARY KEY,
    supplier_return_id  INTEGER NOT NULL REFERENCES supplier_returns (id),
    batch_id            INTEGER NOT NULL REFERENCES batches (id),
    qty_base            INTEGER NOT NULL CHECK (qty_base > 0),
    amount              INTEGER NOT NULL CHECK (amount >= 0)
);

-- ─── Data awal ───────────────────────────────────────────────────────────────

INSERT INTO units (name) VALUES
    ('Tablet'), ('Kaplet'), ('Kapsul'), ('Strip'), ('Blister'), ('Box'), ('Botol'),
    ('Tube'), ('Sachet'), ('Ampul'), ('Vial'), ('Pcs'), ('Pot'), ('Pak');
