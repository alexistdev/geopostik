use std::path::Path;
use std::sync::LazyLock;

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

use crate::error::{AppError, AppResult};

static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/001_init.sql")),
        M::up(include_str!("../../migrations/002_racks_manufacturers.sql")),
        M::up(include_str!("../../migrations/003_master_codes.sql")),
        M::up(include_str!("../../migrations/004_master_soft_delete.sql")).foreign_key_check(),
        M::up(include_str!("../../migrations/005_master_code_immutable.sql")),
        M::up(include_str!("../../migrations/006_barcode_soft_delete.sql")).foreign_key_check(),
        M::up(include_str!("../../migrations/007_master_created_by.sql")),
    ])
});

/// Membuka (atau membuat) database di `path`, lalu menjalankan migration.
pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    prepare(conn)
}

/// Database in-memory untuk test.
#[cfg(test)]
pub fn open_in_memory() -> AppResult<Connection> {
    prepare(Connection::open_in_memory()?)
}

fn prepare(mut conn: Connection) -> AppResult<Connection> {
    conn.pragma_update(None, "busy_timeout", 5000)?;
    // Migration yang membangun ulang tabel butuh foreign key mati; keutuhan relasi diperiksa
    // lewat `foreign_key_check()` pada migration tersebut.
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    MIGRATIONS
        .to_latest(&mut conn)
        .map_err(|e| AppError::Internal(format!("migration gagal: {e}")))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        MIGRATIONS.validate().unwrap();
    }

    #[test]
    fn migration_002_moves_manufacturer_and_rack_text() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        MIGRATIONS.to_version(&mut conn, 1).unwrap();
        conn.execute_batch(
            "INSERT INTO products (code, name, drug_class, base_unit_id, manufacturer, rack_location) VALUES
                 ('P1', 'Obat A', 'FREE', 1, 'Kimia Farma', 'A1'),
                 ('P2', 'Obat B', 'FREE', 1, ' kimia farma ', 'a1'),
                 ('P3', 'Obat C', 'FREE', 1, NULL, '');",
        )
        .unwrap();

        MIGRATIONS.to_latest(&mut conn).unwrap();

        let manufacturers: i64 = conn.query_row("SELECT count(*) FROM manufacturers", [], |r| r.get(0)).unwrap();
        let racks: i64 = conn.query_row("SELECT count(*) FROM racks", [], |r| r.get(0)).unwrap();
        assert_eq!((manufacturers, racks), (1, 1), "nama sama beda huruf besar/kecil digabung");

        let linked: i64 = conn
            .query_row(
                "SELECT count(*) FROM products WHERE manufacturer_id IS NOT NULL AND rack_id IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(linked, 2);
        let empty: (Option<i64>, Option<i64>) = conn
            .query_row("SELECT manufacturer_id, rack_id FROM products WHERE code = 'P3'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(empty, (None, None));
    }

    #[test]
    fn migration_003_gives_existing_masters_a_code() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        MIGRATIONS.to_version(&mut conn, 2).unwrap();
        conn.execute_batch(
            "INSERT INTO categories (name) VALUES ('Analgesik'), ('Vitamin');
             INSERT INTO racks (name) VALUES ('A1');
             INSERT INTO manufacturers (name) VALUES ('Kalbe');",
        )
        .unwrap();

        MIGRATIONS.to_latest(&mut conn).unwrap();

        let codes: Vec<String> = conn
            .prepare("SELECT code FROM categories UNION ALL SELECT code FROM racks UNION ALL SELECT code FROM manufacturers")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(codes, ["KTG0001", "KTG0002", "RAK0001", "PBR0001"]);

        // Kode wajib dan unik.
        assert!(conn.execute("INSERT INTO racks (name) VALUES ('B1')", []).is_err());
        assert!(conn.execute("INSERT INTO racks (code, name) VALUES ('rak0001', 'B1')", []).is_err());
        assert!(conn.execute("UPDATE racks SET code = ''", []).is_err());
    }

    #[test]
    fn migration_004_keeps_data_and_relations() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        MIGRATIONS.to_version(&mut conn, 3).unwrap();
        conn.execute_batch(
            "INSERT INTO users (username, full_name, password_hash) VALUES ('u', 'U', 'x');
             INSERT INTO categories (code, name, margin_bp) VALUES ('KTG0001', 'Analgesik', 2500);
             INSERT INTO racks (code, name) VALUES ('RAK0001', 'A1');
             INSERT INTO manufacturers (code, name) VALUES ('PBR0001', 'Kalbe');
             INSERT INTO products (code, name, drug_class, base_unit_id, category_id, rack_id, manufacturer_id)
                 VALUES ('P1', 'Obat', 'FREE', 1, 1, 1, 1);",
        )
        .unwrap();

        MIGRATIONS.to_latest(&mut conn).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();

        let row: (String, i64, String, String) = conn
            .query_row(
                "SELECT c.name, c.margin_bp, r.code, m.name FROM products p
                 JOIN categories c ON c.id = p.category_id
                 JOIN racks r ON r.id = p.rack_id
                 JOIN manufacturers m ON m.id = p.manufacturer_id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(row, ("Analgesik".into(), 2500, "RAK0001".into(), "Kalbe".into()));

        // Hapus permanen ditolak di semua tabel master.
        for table in ["categories", "racks", "manufacturers", "units", "products", "users"] {
            assert!(conn.execute(&format!("DELETE FROM {table}"), []).is_err(), "{table}");
        }

        // Nama/kode unik hanya di antara data yang belum dihapus.
        assert!(conn.execute("INSERT INTO racks (code, name) VALUES ('RAK0009', 'a1')", []).is_err());
        conn.execute("UPDATE racks SET deleted_at = datetime('now') WHERE id = 1", []).unwrap();
        conn.execute("INSERT INTO racks (code, name) VALUES ('RAK0001', 'A1')", []).unwrap();
    }

    #[test]
    fn migration_006_keeps_existing_barcodes() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        MIGRATIONS.to_version(&mut conn, 5).unwrap();
        conn.execute_batch(
            "INSERT INTO products (code, name, drug_class, base_unit_id) VALUES ('P1', 'Obat', 'FREE', 1);
             INSERT INTO product_units (product_id, unit_id, conversion, sell_price) VALUES (1, 1, 1, 1000);
             INSERT INTO product_barcodes (product_unit_id, barcode) VALUES (1, '8991234567890');",
        )
        .unwrap();

        MIGRATIONS.to_latest(&mut conn).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();

        let (unit, deleted): (i64, Option<String>) = conn
            .query_row(
                "SELECT product_unit_id, deleted_at FROM product_barcodes WHERE barcode = '8991234567890'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((unit, deleted), (1, None));
        assert!(conn.execute("DELETE FROM product_barcodes", []).is_err());
    }

    #[test]
    fn stock_ledger_is_enforced_by_triggers() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO users (id, username, full_name, password_hash) VALUES (1, 'u', 'U', 'x');
             INSERT INTO products (id, code, name, drug_class, base_unit_id) VALUES (1, 'P1', 'Obat', 'FREE', 1);
             INSERT INTO batches (id, product_id, batch_number, expiry_date, unit_cost_x100, source_type)
                 VALUES (1, 1, 'B1', '2030-01-01', 10000, 'OPENING');
             INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id)
                 VALUES (1, 1, 'OPENING', 10, 'stock_opname', 1, 1);",
        )
        .unwrap();

        let qty: i64 = conn
            .query_row("SELECT qty_on_hand_base FROM batches WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(qty, 10);

        // Stok minus ditolak.
        let minus = conn.execute(
            "INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id)
             VALUES (1, 1, 'SALE', -11, 'sale', 1, 1)",
            [],
        );
        assert!(minus.is_err());

        // Kartu stok tidak bisa diubah atau dihapus.
        assert!(conn.execute("UPDATE stock_movements SET qty_change_base = 5", []).is_err());
        assert!(conn.execute("DELETE FROM stock_movements", []).is_err());

        // Pencarian FTS ikut ter-update.
        let found: i64 = conn
            .query_row("SELECT count(*) FROM products_fts WHERE products_fts MATCH 'oba*'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(found, 1);
    }
}
