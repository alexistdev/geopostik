use std::path::Path;
use std::sync::LazyLock;

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

use crate::error::{AppError, AppResult};

static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![M::up(include_str!("../../migrations/001_init.sql"))])
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
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    MIGRATIONS
        .to_latest(&mut conn)
        .map_err(|e| AppError::Internal(format!("migration gagal: {e}")))?;
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
