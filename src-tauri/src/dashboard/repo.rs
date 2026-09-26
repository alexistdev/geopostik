use std::sync::LazyLock;

use rusqlite::{Connection, OptionalExtension, named_params, params};

use super::model::{
    DailyTotal, DebtRow, ExpiryRow, LowStockRow, MethodTotal, OpnamePendingRow, RxQueueRow, Tally, TopProduct,
};
use crate::error::AppResult;

// Tanggal dikirim sebagai `YYYY-MM-DD`; rentang waktu selalu `[from, to)` sehingga
// perbandingan teks dengan kolom `YYYY-MM-DD HH:MM:SS` tetap benar.

fn tally(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> AppResult<Tally> {
    Ok(conn.query_row(sql, p, |r| Ok(Tally { count: r.get(0)?, amount: r.get(1)? }))?)
}

// ─── Shift ───────────────────────────────────────────────────────────────────

pub struct OpenShiftRow {
    pub id: i64,
    pub user_id: i64,
    pub opened_by: String,
    pub opened_at: String,
    pub opening_cash: i64,
}

pub fn open_shift(conn: &Connection) -> AppResult<Option<OpenShiftRow>> {
    Ok(conn
        .query_row(
            "SELECT s.id, s.user_id, u.full_name, s.opened_at, s.opening_cash
             FROM shifts s JOIN users u ON u.id = s.user_id
             WHERE s.status = 'OPEN'",
            [],
            |r| {
                Ok(OpenShiftRow {
                    id: r.get(0)?,
                    user_id: r.get(1)?,
                    opened_by: r.get(2)?,
                    opened_at: r.get(3)?,
                    opening_cash: r.get(4)?,
                })
            },
        )
        .optional()?)
}

pub fn shift_sales(conn: &Connection, shift_id: i64) -> AppResult<Tally> {
    tally(
        conn,
        "SELECT count(*), COALESCE(SUM(grand_total), 0) FROM sales WHERE shift_id = ?1 AND status = 'COMPLETED'",
        [shift_id],
    )
}

/// Tunai yang masuk laci selama shift: bagian `CASH` nota yang tidak batal, dikurangi refund retur.
pub fn shift_cash_in(conn: &Connection, shift_id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE((SELECT SUM(p.amount) FROM sale_payments p JOIN sales s ON s.id = p.sale_id
                          WHERE s.shift_id = ?1 AND s.status = 'COMPLETED' AND p.method = 'CASH'), 0)
              - COALESCE((SELECT SUM(refund_total) FROM sale_returns WHERE shift_id = ?1), 0)",
        [shift_id],
        |r| r.get(0),
    )?)
}

pub fn cashier_sales(conn: &Connection, cashier_id: i64, from: &str, to: &str) -> AppResult<Tally> {
    tally(
        conn,
        "SELECT count(*), COALESCE(SUM(grand_total), 0) FROM sales
         WHERE cashier_id = ?1 AND status = 'COMPLETED' AND sold_at >= ?2 AND sold_at < ?3",
        params![cashier_id, from, to],
    )
}

// ─── Penjualan ───────────────────────────────────────────────────────────────

pub fn sales_between(conn: &Connection, from: &str, to: &str) -> AppResult<Tally> {
    tally(
        conn,
        "SELECT count(*), COALESCE(SUM(grand_total), 0) FROM sales
         WHERE status = 'COMPLETED' AND sold_at >= ?1 AND sold_at < ?2",
        [from, to],
    )
}

pub fn prescription_sales_between(conn: &Connection, from: &str, to: &str) -> AppResult<Tally> {
    tally(
        conn,
        "SELECT count(*), COALESCE(SUM(grand_total), 0) FROM sales
         WHERE status = 'COMPLETED' AND sale_type = 'PRESCRIPTION' AND sold_at >= ?1 AND sold_at < ?2",
        [from, to],
    )
}

/// Nota batal, dihitung menurut tanggal pembatalan.
pub fn voids_between(conn: &Connection, from: &str, to: &str) -> AppResult<Tally> {
    tally(
        conn,
        "SELECT count(*), COALESCE(SUM(grand_total), 0) FROM sales
         WHERE status = 'VOID' AND voided_at >= ?1 AND voided_at < ?2",
        [from, to],
    )
}

pub fn by_method_between(conn: &Connection, from: &str, to: &str) -> AppResult<Vec<MethodTotal>> {
    let mut stmt = conn.prepare(
        "SELECT p.method, SUM(p.amount) FROM sale_payments p JOIN sales s ON s.id = p.sale_id
         WHERE s.status = 'COMPLETED' AND s.sold_at >= ?1 AND s.sold_at < ?2
         GROUP BY p.method
         ORDER BY CASE p.method WHEN 'CASH' THEN 0 WHEN 'QRIS' THEN 1 ELSE 2 END",
    )?;
    let rows = stmt
        .query_map([from, to], |r| Ok(MethodTotal { method: r.get(0)?, amount: r.get(1)? }))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Total per tanggal; hanya tanggal yang ada penjualannya.
pub fn daily_between(conn: &Connection, from: &str, to: &str) -> AppResult<Vec<DailyTotal>> {
    let mut stmt = conn.prepare(
        "SELECT substr(sold_at, 1, 10) AS d, count(*), SUM(grand_total) FROM sales
         WHERE status = 'COMPLETED' AND sold_at >= ?1 AND sold_at < ?2
         GROUP BY d ORDER BY d",
    )?;
    let rows = stmt
        .query_map([from, to], |r| Ok(DailyTotal { date: r.get(0)?, count: r.get(1)?, amount: r.get(2)? }))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Obat terlaris menurut nilai. Baris obat biasa dihitung dari `line_total`; komponen racikan
/// (nilainya ada di baris induk) ikut dihitung jumlahnya saja.
pub fn top_products_between(conn: &Connection, from: &str, to: &str, limit: i64) -> AppResult<Vec<TopProduct>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, SUM(i.qty_base), u.name, SUM(i.line_total) AS amount
         FROM sale_items i
         JOIN sales s ON s.id = i.sale_id
         JOIN products p ON p.id = i.product_id
         JOIN units u ON u.id = p.base_unit_id
         WHERE s.status = 'COMPLETED' AND s.sold_at >= ?1 AND s.sold_at < ?2 AND i.item_kind = 'PRODUCT'
         GROUP BY p.id
         ORDER BY amount DESC, SUM(i.qty_base) DESC, p.name
         LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(params![from, to, limit], |r| {
            Ok(TopProduct {
                product_id: r.get(0)?,
                name: r.get(1)?,
                qty_base: r.get(2)?,
                base_unit_name: r.get(3)?,
                amount: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Laba kotor = penjualan tanpa PPN − HPP batch yang terpakai (dibulatkan ke rupiah).
pub fn gross_profit_between(conn: &Connection, from: &str, to: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE((SELECT SUM(grand_total - tax_total) FROM sales
                          WHERE status = 'COMPLETED' AND sold_at >= ?1 AND sold_at < ?2), 0)
              - COALESCE((SELECT (SUM(b.qty_base * b.unit_cost_x100) + 50) / 100
                          FROM sale_item_batches b
                          JOIN sale_items i ON i.id = b.sale_item_id
                          JOIN sales s ON s.id = i.sale_id
                          WHERE s.status = 'COMPLETED' AND s.sold_at >= ?1 AND s.sold_at < ?2), 0)",
        [from, to],
        |r| r.get(0),
    )?)
}

// ─── Stok ────────────────────────────────────────────────────────────────────

/// Stok per obat aktif (0 bila tidak punya batch ber-stok).
const PRODUCT_STOCK: &str = "
    SELECT p.id, p.code, p.name, p.min_stock_base, u.name AS unit_name,
           COALESCE((SELECT SUM(qty_on_hand_base) FROM batches b WHERE b.product_id = p.id), 0) AS stock
    FROM products p JOIN units u ON u.id = p.base_unit_id
    WHERE p.deleted_at IS NULL AND p.is_active = 1";

pub struct StockCounts {
    pub low: i64,
    pub empty: i64,
    pub near_expiry: i64,
    pub expired: i64,
}

pub fn stock_counts(conn: &Connection, today: &str, near: &str) -> AppResult<StockCounts> {
    let (low, empty) = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(min_stock_base > 0 AND stock < min_stock_base), 0),
                    COALESCE(SUM(stock = 0), 0)
             FROM ({PRODUCT_STOCK})"
        ),
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (near_expiry, expired) = conn.query_row(
        "SELECT COALESCE(SUM(b.expiry_date > :today AND b.expiry_date <= :near), 0),
                COALESCE(SUM(b.expiry_date <= :today), 0)
         FROM batches b JOIN products p ON p.id = b.product_id
         WHERE b.qty_on_hand_base > 0 AND p.deleted_at IS NULL",
        named_params! { ":today": today, ":near": near },
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(StockCounts { low, empty, near_expiry, expired })
}

pub fn expiring(conn: &Connection, today: &str, near: &str, limit: i64) -> AppResult<Vec<ExpiryRow>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, p.id, p.name, b.batch_number, b.expiry_date, b.qty_on_hand_base, u.name,
                CAST(julianday(b.expiry_date) - julianday(:today) AS INTEGER), b.is_locked
         FROM batches b
         JOIN products p ON p.id = b.product_id
         JOIN units u ON u.id = p.base_unit_id
         WHERE b.qty_on_hand_base > 0 AND p.deleted_at IS NULL AND b.expiry_date <= :near
         ORDER BY b.expiry_date, p.name COLLATE NOCASE, b.id
         LIMIT :limit",
    )?;
    let rows = stmt
        .query_map(named_params! { ":today": today, ":near": near, ":limit": limit }, |r| {
            Ok(ExpiryRow {
                batch_id: r.get(0)?,
                product_id: r.get(1)?,
                product_name: r.get(2)?,
                batch_number: r.get(3)?,
                expiry_date: r.get(4)?,
                qty_base: r.get(5)?,
                base_unit_name: r.get(6)?,
                days_left: r.get(7)?,
                is_locked: r.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Obat di bawah stok minimal, yang paling kritis (stok ÷ minimal terkecil) dulu.
pub fn low_stock(conn: &Connection, limit: i64) -> AppResult<Vec<LowStockRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT id, code, name, stock, min_stock_base, unit_name FROM ({PRODUCT_STOCK})
         WHERE min_stock_base > 0 AND stock < min_stock_base
         ORDER BY CAST(stock AS REAL) / min_stock_base, name COLLATE NOCASE
         LIMIT ?1"
    ))?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(LowStockRow {
                product_id: r.get(0)?,
                code: r.get(1)?,
                name: r.get(2)?,
                stock_base: r.get(3)?,
                min_stock_base: r.get(4)?,
                base_unit_name: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn inventory_value(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE((SUM(qty_on_hand_base * unit_cost_x100) + 50) / 100, 0)
         FROM batches WHERE qty_on_hand_base > 0",
        [],
        |r| r.get(0),
    )?)
}

// ─── Resep ───────────────────────────────────────────────────────────────────

pub fn prescription_count(conn: &Connection, status: &str) -> AppResult<i64> {
    Ok(conn.query_row("SELECT count(*) FROM prescriptions WHERE status = ?1", [status], |r| r.get(0))?)
}

/// Antrian resep berstatus `status`, yang paling lama menunggu dulu.
pub fn prescription_queue(conn: &Connection, status: &str, limit: i64) -> AppResult<Vec<RxQueueRow>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.number, r.prescription_date, r.patient_name, d.name,
                (SELECT count(*) FROM prescription_items i
                 WHERE i.prescription_id = r.id AND i.parent_item_id IS NULL),
                r.created_at
         FROM prescriptions r JOIN doctors d ON d.id = r.doctor_id
         WHERE r.status = ?1
         ORDER BY r.updated_at, r.id
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![status, limit], |r| {
            Ok(RxQueueRow {
                id: r.get(0)?,
                number: r.get(1)?,
                prescription_date: r.get(2)?,
                patient_name: r.get(3)?,
                doctor_name: r.get(4)?,
                item_count: r.get(5)?,
                created_at: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ─── Opname ──────────────────────────────────────────────────────────────────

pub fn opname_count(conn: &Connection, status: &str) -> AppResult<i64> {
    Ok(conn.query_row("SELECT count(*) FROM stock_opnames WHERE status = ?1", [status], |r| r.get(0))?)
}

pub fn opname_pending(conn: &Connection, limit: i64) -> AppResult<Vec<OpnamePendingRow>> {
    let mut stmt = conn.prepare(
        "SELECT o.id, o.number, o.opname_type, o.status, o.scope_note, u.full_name,
                (SELECT count(*) FROM stock_opname_items i WHERE i.opname_id = o.id), o.created_at
         FROM stock_opnames o LEFT JOIN users u ON u.id = o.created_by
         WHERE o.status IN ('DRAFT', 'SUBMITTED')
         ORDER BY o.status = 'DRAFT', o.id
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(OpnamePendingRow {
                id: r.get(0)?,
                number: r.get(1)?,
                opname_type: r.get(2)?,
                status: r.get(3)?,
                scope_note: r.get(4)?,
                created_by: r.get(5)?,
                item_count: r.get(6)?,
                created_at: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ─── Hutang ──────────────────────────────────────────────────────────────────

/// Sisa hutang per faktur kredit: total − pembayaran − retur yang memotong hutang (SCHEMA.md §4).
/// Rumusnya sama dengan menu Pembelian › Hutang (`purchasing::OUTSTANDING`).
static DEBTS: LazyLock<String> = LazyLock::new(|| {
    format!(
        "SELECT * FROM (
            SELECT pu.id, pu.number, sp.name AS supplier_name, pu.invoice_number, pu.due_date,
                   {} AS outstanding
            FROM purchases pu JOIN suppliers sp ON sp.id = pu.supplier_id
            WHERE pu.payment_type = 'CREDIT' AND pu.status = 'POSTED'
        ) WHERE outstanding > 0",
        crate::purchasing::OUTSTANDING
    )
});

pub struct DebtTotals {
    pub outstanding: Tally,
    pub overdue: Tally,
    pub due_soon: Tally,
}

pub fn debt_totals(conn: &Connection, today: &str, soon: &str) -> AppResult<DebtTotals> {
    let debts = &*DEBTS;
    Ok(conn.query_row(
        &format!(
            "SELECT count(*), COALESCE(SUM(outstanding), 0),
                    COALESCE(SUM(due_date < :today), 0),
                    COALESCE(SUM(CASE WHEN due_date < :today THEN outstanding END), 0),
                    COALESCE(SUM(due_date >= :today AND due_date <= :soon), 0),
                    COALESCE(SUM(CASE WHEN due_date >= :today AND due_date <= :soon THEN outstanding END), 0)
             FROM ({debts})"
        ),
        named_params! { ":today": today, ":soon": soon },
        |r| {
            Ok(DebtTotals {
                outstanding: Tally { count: r.get(0)?, amount: r.get(1)? },
                overdue: Tally { count: r.get(2)?, amount: r.get(3)? },
                due_soon: Tally { count: r.get(4)?, amount: r.get(5)? },
            })
        },
    )?)
}

pub fn debts_upcoming(conn: &Connection, today: &str, limit: i64) -> AppResult<Vec<DebtRow>> {
    let debts = &*DEBTS;
    let mut stmt = conn.prepare(&format!(
        "SELECT id, number, supplier_name, invoice_number, due_date, outstanding,
                CAST(julianday(due_date) - julianday(:today) AS INTEGER)
         FROM ({debts})
         ORDER BY due_date, id
         LIMIT :limit"
    ))?;
    let rows = stmt
        .query_map(named_params! { ":today": today, ":limit": limit }, |r| {
            Ok(DebtRow {
                purchase_id: r.get(0)?,
                number: r.get(1)?,
                supplier_name: r.get(2)?,
                invoice_number: r.get(3)?,
                due_date: r.get(4)?,
                outstanding: r.get(5)?,
                days_left: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}
