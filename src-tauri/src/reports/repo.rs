use rusqlite::{Connection, named_params};

use super::model::{
    ReportCashier, ReportCategoryValue, ReportExpiryRow, ReportInventoryRow, ReportInvoiceRow, ReportMethod,
    ReportSlowRow, ReportSupplierRow, SalesReportSummary, SipnapRow,
};
use crate::error::AppResult;

// Tanggal dikirim sebagai `YYYY-MM-DD`; rentang selalu `[from, to)` sehingga perbandingan teks
// dengan kolom `YYYY-MM-DD HH:MM:SS` tetap benar. `:cashier` NULL = semua kasir.

/// Filter nota selesai dalam rentang (alias `s`).
const SALES_IN_RANGE: &str = "s.status = 'COMPLETED' AND s.sold_at >= :from AND s.sold_at < :to
     AND (:cashier IS NULL OR s.cashier_id = :cashier)";

/// HPP batch terpakai (rupiah, dibulatkan) untuk baris `sale_item_batches` alias `b`.
const COST_SUM: &str = "(SUM(b.qty_base * b.unit_cost_x100) + 50) / 100";

// ─── Penjualan ───────────────────────────────────────────────────────────────

pub fn sales_summary(conn: &Connection, from: &str, to: &str, cashier: Option<i64>) -> AppResult<SalesReportSummary> {
    let p = named_params! { ":from": from, ":to": to, ":cashier": cashier };
    let mut s = conn.query_row(
        &format!(
            "SELECT count(*), COALESCE(SUM(subtotal), 0), COALESCE(SUM(discount_total), 0),
                    COALESCE(SUM(tax_total), 0), COALESCE(SUM(rounding), 0), COALESCE(SUM(grand_total), 0),
                    COALESCE(SUM(sale_type = 'PRESCRIPTION'), 0),
                    COALESCE(SUM(CASE WHEN sale_type = 'PRESCRIPTION' THEN grand_total END), 0)
             FROM sales s WHERE {SALES_IN_RANGE}"
        ),
        p,
        |r| {
            Ok(SalesReportSummary {
                count: r.get(0)?,
                subtotal: r.get(1)?,
                discount: r.get(2)?,
                tax: r.get(3)?,
                rounding: r.get(4)?,
                net: r.get(5)?,
                prescription_count: r.get(6)?,
                prescription_amount: r.get(7)?,
                ..Default::default()
            })
        },
    )?;
    (s.void_count, s.void_amount) = conn.query_row(
        "SELECT count(*), COALESCE(SUM(grand_total), 0) FROM sales s
         WHERE s.status = 'VOID' AND s.voided_at >= :from AND s.voided_at < :to
           AND (:cashier IS NULL OR s.cashier_id = :cashier)",
        p,
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(s)
}

/// HPP batch terpakai seluruh nota dalam rentang.
pub fn sales_cost(conn: &Connection, from: &str, to: &str, cashier: Option<i64>) -> AppResult<i64> {
    Ok(conn.query_row(
        &format!(
            "SELECT COALESCE({COST_SUM}, 0)
             FROM sale_item_batches b
             JOIN sale_items i ON i.id = b.sale_item_id
             JOIN sales s ON s.id = i.sale_id
             WHERE {SALES_IN_RANGE}"
        ),
        named_params! { ":from": from, ":to": to, ":cashier": cashier },
        |r| r.get(0),
    )?)
}

pub struct DayRow {
    pub date: String,
    pub count: i64,
    pub amount: i64,
    /// Omzet tanpa PPN.
    pub net_of_tax: i64,
}

/// Total per tanggal; hanya tanggal yang ada penjualannya.
pub fn sales_daily(conn: &Connection, from: &str, to: &str, cashier: Option<i64>) -> AppResult<Vec<DayRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT substr(s.sold_at, 1, 10) AS d, count(*), SUM(s.grand_total), SUM(s.grand_total - s.tax_total)
         FROM sales s WHERE {SALES_IN_RANGE}
         GROUP BY d ORDER BY d"
    ))?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to, ":cashier": cashier }, |r| {
            Ok(DayRow { date: r.get(0)?, count: r.get(1)?, amount: r.get(2)?, net_of_tax: r.get(3)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// HPP per tanggal penjualan.
pub fn cost_daily(conn: &Connection, from: &str, to: &str, cashier: Option<i64>) -> AppResult<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT substr(s.sold_at, 1, 10) AS d, {COST_SUM}
         FROM sale_item_batches b
         JOIN sale_items i ON i.id = b.sale_item_id
         JOIN sales s ON s.id = i.sale_id
         WHERE {SALES_IN_RANGE}
         GROUP BY d"
    ))?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to, ":cashier": cashier }, |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn by_method(conn: &Connection, from: &str, to: &str, cashier: Option<i64>) -> AppResult<Vec<ReportMethod>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT p.method, count(DISTINCT s.id), SUM(p.amount)
         FROM sale_payments p JOIN sales s ON s.id = p.sale_id
         WHERE {SALES_IN_RANGE}
         GROUP BY p.method
         ORDER BY CASE p.method WHEN 'CASH' THEN 0 WHEN 'QRIS' THEN 1 ELSE 2 END"
    ))?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to, ":cashier": cashier }, |r| {
            Ok(ReportMethod { method: r.get(0)?, count: r.get(1)?, amount: r.get(2)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn by_cashier(conn: &Connection, from: &str, to: &str, cashier: Option<i64>) -> AppResult<Vec<ReportCashier>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT u.id, u.full_name, count(*), SUM(s.grand_total) AS amount
         FROM sales s JOIN users u ON u.id = s.cashier_id
         WHERE {SALES_IN_RANGE}
         GROUP BY u.id
         ORDER BY amount DESC, u.full_name"
    ))?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to, ":cashier": cashier }, |r| {
            Ok(ReportCashier { user_id: r.get(0)?, name: r.get(1)?, count: r.get(2)?, amount: r.get(3)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub struct ShiftRow {
    pub id: i64,
    pub user_id: i64,
    pub opened_by: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub status: String,
    pub opening_cash: i64,
    pub expected_cash: Option<i64>,
    pub counted_cash: Option<i64>,
    pub difference: Option<i64>,
    pub count: i64,
    pub amount: i64,
}

/// Shift yang dibuka dalam rentang. Dengan `cashier`: shift milik user itu atau shift tempat ia
/// berjualan, dan penjualan yang dihitung hanya nota miliknya.
pub fn shifts(conn: &Connection, from: &str, to: &str, cashier: Option<i64>) -> AppResult<Vec<ShiftRow>> {
    let mut stmt = conn.prepare(
        "SELECT sh.id, sh.user_id, u.full_name, sh.opened_at, sh.closed_at, sh.status, sh.opening_cash,
                sh.expected_cash, sh.counted_cash, sh.difference,
                (SELECT count(*) FROM sales s WHERE s.shift_id = sh.id AND s.status = 'COMPLETED'
                   AND (:cashier IS NULL OR s.cashier_id = :cashier)),
                (SELECT COALESCE(SUM(grand_total), 0) FROM sales s WHERE s.shift_id = sh.id AND s.status = 'COMPLETED'
                   AND (:cashier IS NULL OR s.cashier_id = :cashier))
         FROM shifts sh JOIN users u ON u.id = sh.user_id
         WHERE sh.opened_at >= :from AND sh.opened_at < :to
           AND (:cashier IS NULL OR sh.user_id = :cashier
                OR EXISTS (SELECT 1 FROM sales s WHERE s.shift_id = sh.id AND s.cashier_id = :cashier))
         ORDER BY sh.opened_at DESC, sh.id DESC",
    )?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to, ":cashier": cashier }, |r| {
            Ok(ShiftRow {
                id: r.get(0)?,
                user_id: r.get(1)?,
                opened_by: r.get(2)?,
                opened_at: r.get(3)?,
                closed_at: r.get(4)?,
                status: r.get(5)?,
                opening_cash: r.get(6)?,
                expected_cash: r.get(7)?,
                counted_cash: r.get(8)?,
                difference: r.get(9)?,
                count: r.get(10)?,
                amount: r.get(11)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Tunai yang masuk laci selama shift: bagian `CASH` nota yang tidak batal, dikurangi refund retur.
/// Rumus yang sama dengan Dashboard, untuk shift yang masih terbuka.
pub fn shift_cash_in(conn: &Connection, shift_id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE((SELECT SUM(p.amount) FROM sale_payments p JOIN sales s ON s.id = p.sale_id
                          WHERE s.shift_id = ?1 AND s.status = 'COMPLETED' AND p.method = 'CASH'), 0)
              - COALESCE((SELECT SUM(refund_total) FROM sale_returns WHERE shift_id = ?1), 0)",
        [shift_id],
        |r| r.get(0),
    )?)
}

// ─── Obat terlaris & slow moving ─────────────────────────────────────────────

pub struct ProductSalesRow {
    pub product_id: i64,
    pub code: String,
    pub name: String,
    pub qty_base: i64,
    pub base_unit_name: String,
    pub sale_count: i64,
    pub amount: i64,
    /// HPP baris penjualan langsung (tanpa komponen racikan).
    pub cost: i64,
}

/// Obat terjual dalam rentang. Komponen racikan ikut dihitung jumlahnya; nilainya ada di baris
/// induk racikan sehingga nilai & HPP-nya tidak dihitung di sini.
pub fn product_sales(conn: &Connection, from: &str, to: &str, limit: i64) -> AppResult<Vec<ProductSalesRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT p.id, p.code, p.name, SUM(i.qty_base), u.name, count(DISTINCT s.id),
                SUM(i.line_total) AS amount,
                COALESCE(SUM(CASE WHEN i.parent_item_id IS NULL THEN
                    (SELECT SUM(b.qty_base * b.unit_cost_x100) FROM sale_item_batches b WHERE b.sale_item_id = i.id)
                END) + 50, 0) / 100
         FROM sale_items i
         JOIN sales s ON s.id = i.sale_id
         JOIN products p ON p.id = i.product_id
         JOIN units u ON u.id = p.base_unit_id
         WHERE {SALES_IN_RANGE} AND i.item_kind = 'PRODUCT'
         GROUP BY p.id
         ORDER BY amount DESC, SUM(i.qty_base) DESC, p.name COLLATE NOCASE
         LIMIT :limit"
    ))?;
    let rows = stmt
        .query_map(
            named_params! { ":from": from, ":to": to, ":cashier": None::<i64>, ":limit": limit },
            |r| {
                Ok(ProductSalesRow {
                    product_id: r.get(0)?,
                    code: r.get(1)?,
                    name: r.get(2)?,
                    qty_base: r.get(3)?,
                    base_unit_name: r.get(4)?,
                    sale_count: r.get(5)?,
                    amount: r.get(6)?,
                    cost: r.get(7)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Obat aktif yang masih ada stok tetapi tidak terjual (termasuk sebagai komponen racikan)
/// dalam rentang; stok terbesar dulu.
pub fn slow_moving(conn: &Connection, from: &str, to: &str, limit: i64) -> AppResult<Vec<ReportSlowRow>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.code, p.name, st.qty, u.name,
                (SELECT max(s.sold_at) FROM sale_items i JOIN sales s ON s.id = i.sale_id
                 WHERE i.product_id = p.id AND s.status = 'COMPLETED'),
                st.value
         FROM products p
         JOIN units u ON u.id = p.base_unit_id
         JOIN (SELECT product_id, SUM(qty_on_hand_base) AS qty,
                      (SUM(qty_on_hand_base * unit_cost_x100) + 50) / 100 AS value
               FROM batches WHERE qty_on_hand_base > 0 GROUP BY product_id) st ON st.product_id = p.id
         WHERE p.deleted_at IS NULL AND p.is_active = 1
           AND NOT EXISTS (SELECT 1 FROM sale_items i JOIN sales s ON s.id = i.sale_id
                           WHERE i.product_id = p.id AND s.status = 'COMPLETED'
                             AND s.sold_at >= :from AND s.sold_at < :to)
         ORDER BY st.value DESC, st.qty DESC, p.name COLLATE NOCASE
         LIMIT :limit",
    )?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to, ":limit": limit }, |r| {
            Ok(ReportSlowRow {
                product_id: r.get(0)?,
                code: r.get(1)?,
                name: r.get(2)?,
                stock_base: r.get(3)?,
                base_unit_name: r.get(4)?,
                last_sold_at: r.get(5)?,
                value: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ─── Nilai persediaan ────────────────────────────────────────────────────────

/// Nilai stok per obat (semua batch ber-stok, termasuk yang sudah ED atau terkunci).
pub fn inventory_rows(conn: &Connection) -> AppResult<Vec<ReportInventoryRow>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.code, p.name, c.name, SUM(b.qty_on_hand_base), u.name, count(*),
                (SUM(b.qty_on_hand_base * b.unit_cost_x100) + 50) / 100 AS value
         FROM batches b
         JOIN products p ON p.id = b.product_id
         JOIN units u ON u.id = p.base_unit_id
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE b.qty_on_hand_base > 0
         GROUP BY p.id
         ORDER BY value DESC, p.name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ReportInventoryRow {
                product_id: r.get(0)?,
                code: r.get(1)?,
                name: r.get(2)?,
                category: r.get(3)?,
                stock_base: r.get(4)?,
                base_unit_name: r.get(5)?,
                batch_count: r.get(6)?,
                value: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn inventory_by_category(conn: &Connection) -> AppResult<Vec<ReportCategoryValue>> {
    let mut stmt = conn.prepare(
        "SELECT c.name, count(DISTINCT p.id), (SUM(b.qty_on_hand_base * b.unit_cost_x100) + 50) / 100 AS value
         FROM batches b
         JOIN products p ON p.id = b.product_id
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE b.qty_on_hand_base > 0
         GROUP BY c.id
         ORDER BY value DESC",
    )?;
    let rows = stmt
        .query_map([], |r| Ok(ReportCategoryValue { category: r.get(0)?, product_count: r.get(1)?, value: r.get(2)? }))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub struct InventoryTotals {
    pub value: i64,
    pub batch_count: i64,
    pub expired_value: i64,
}

pub fn inventory_totals(conn: &Connection, today: &str) -> AppResult<InventoryTotals> {
    Ok(conn.query_row(
        "SELECT COALESCE((SUM(qty_on_hand_base * unit_cost_x100) + 50) / 100, 0), count(*),
                COALESCE((SUM(CASE WHEN expiry_date <= ?1 THEN qty_on_hand_base * unit_cost_x100 END) + 50) / 100, 0)
         FROM batches WHERE qty_on_hand_base > 0",
        [today],
        |r| Ok(InventoryTotals { value: r.get(0)?, batch_count: r.get(1)?, expired_value: r.get(2)? }),
    )?)
}

// ─── ED ──────────────────────────────────────────────────────────────────────

/// Batch ber-stok dengan ED sampai `until`: sudah ED dulu, lalu ED terdekat.
pub fn expiry_rows(conn: &Connection, today: &str, until: &str) -> AppResult<Vec<ReportExpiryRow>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, p.id, p.code, p.name, b.batch_number, b.expiry_date, b.qty_on_hand_base, u.name,
                CAST(julianday(b.expiry_date) - julianday(:today) AS INTEGER), b.is_locked,
                (b.qty_on_hand_base * b.unit_cost_x100 + 50) / 100
         FROM batches b
         JOIN products p ON p.id = b.product_id
         JOIN units u ON u.id = p.base_unit_id
         WHERE b.qty_on_hand_base > 0 AND p.deleted_at IS NULL AND b.expiry_date <= :until
         ORDER BY b.expiry_date, p.name COLLATE NOCASE, b.id",
    )?;
    let rows = stmt
        .query_map(named_params! { ":today": today, ":until": until }, |r| {
            Ok(ReportExpiryRow {
                batch_id: r.get(0)?,
                product_id: r.get(1)?,
                code: r.get(2)?,
                product_name: r.get(3)?,
                batch_number: r.get(4)?,
                expiry_date: r.get(5)?,
                qty_base: r.get(6)?,
                base_unit_name: r.get(7)?,
                days_left: r.get(8)?,
                is_locked: r.get(9)?,
                value: Some(r.get(10)?),
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ─── Pembelian ───────────────────────────────────────────────────────────────

pub struct PurchaseTotals {
    pub count: i64,
    pub subtotal: i64,
    pub discount: i64,
    pub tax: i64,
    pub total: i64,
    pub cash_total: i64,
    pub credit_total: i64,
}

/// Faktur yang diposting dan tidak dibatalkan, menurut tanggal terima.
const POSTED_IN_RANGE: &str = "pu.status = 'POSTED' AND pu.received_date >= :from AND pu.received_date < :to";

pub fn purchase_totals(conn: &Connection, from: &str, to: &str) -> AppResult<PurchaseTotals> {
    Ok(conn.query_row(
        &format!(
            "SELECT count(*), COALESCE(SUM(subtotal), 0), COALESCE(SUM(discount_total), 0),
                    COALESCE(SUM(tax_total), 0), COALESCE(SUM(grand_total), 0),
                    COALESCE(SUM(CASE WHEN payment_type = 'CASH' THEN grand_total END), 0),
                    COALESCE(SUM(CASE WHEN payment_type = 'CREDIT' THEN grand_total END), 0)
             FROM purchases pu WHERE {POSTED_IN_RANGE}"
        ),
        named_params! { ":from": from, ":to": to },
        |r| {
            Ok(PurchaseTotals {
                count: r.get(0)?,
                subtotal: r.get(1)?,
                discount: r.get(2)?,
                tax: r.get(3)?,
                total: r.get(4)?,
                cash_total: r.get(5)?,
                credit_total: r.get(6)?,
            })
        },
    )?)
}

pub fn purchase_by_supplier(conn: &Connection, from: &str, to: &str) -> AppResult<Vec<ReportSupplierRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT sp.id, sp.name, count(*), SUM(pu.grand_total) AS total
         FROM purchases pu JOIN suppliers sp ON sp.id = pu.supplier_id
         WHERE {POSTED_IN_RANGE}
         GROUP BY sp.id
         ORDER BY total DESC, sp.name COLLATE NOCASE"
    ))?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to }, |r| {
            Ok(ReportSupplierRow { supplier_id: r.get(0)?, name: r.get(1)?, count: r.get(2)?, total: r.get(3)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn purchase_invoices(conn: &Connection, from: &str, to: &str, with_debt: bool) -> AppResult<Vec<ReportInvoiceRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT pu.id, pu.number, sp.name, pu.invoice_number, pu.received_date, pu.payment_type, pu.due_date,
                pu.grand_total, CASE WHEN pu.payment_type = 'CREDIT' THEN {} END
         FROM purchases pu JOIN suppliers sp ON sp.id = pu.supplier_id
         WHERE {POSTED_IN_RANGE}
         ORDER BY pu.received_date, pu.id",
        crate::purchasing::OUTSTANDING
    ))?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to }, |r| {
            Ok(ReportInvoiceRow {
                purchase_id: r.get(0)?,
                number: r.get(1)?,
                supplier_name: r.get(2)?,
                invoice_number: r.get(3)?,
                received_date: r.get(4)?,
                payment_type: r.get(5)?,
                due_date: r.get(6)?,
                total: r.get(7)?,
                outstanding: if with_debt { r.get(8)? } else { None },
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ─── SIPNAP ──────────────────────────────────────────────────────────────────

/// Mutasi narkotika & psikotropika dari kartu stok dalam `[from, to)`. Obat yang sudah dihapus
/// tetap tampil bila masih punya stok atau mutasi di bulan itu.
pub fn sipnap(conn: &Connection, from: &str, to: &str) -> AppResult<Vec<SipnapRow>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM (
             SELECT p.id, p.code, p.name, p.drug_class, u.name, p.deleted_at,
                    COALESCE(SUM(CASE WHEN m.created_at < :from THEN m.qty_change_base END), 0) AS opening,
                    COALESCE(SUM(CASE WHEN m.created_at >= :from
                                       AND m.movement_type IN ('OPENING', 'PURCHASE', 'PURCHASE_VOID')
                                      THEN m.qty_change_base END), 0) AS received,
                    -COALESCE(SUM(CASE WHEN m.created_at >= :from
                                        AND m.movement_type IN ('SALE', 'SALE_VOID', 'SALE_RETURN')
                                       THEN m.qty_change_base END), 0) AS sold,
                    -COALESCE(SUM(CASE WHEN m.created_at >= :from
                                        AND m.movement_type IN ('DESTRUCTION', 'SUPPLIER_RETURN')
                                       THEN m.qty_change_base END), 0) AS destroyed,
                    COALESCE(SUM(CASE WHEN m.created_at >= :from AND m.movement_type = 'ADJUSTMENT'
                                      THEN m.qty_change_base END), 0) AS adjusted,
                    COALESCE(SUM(CASE WHEN m.created_at >= :from THEN 1 END), 0) AS moves
             FROM products p
             JOIN units u ON u.id = p.base_unit_id
             LEFT JOIN stock_movements m ON m.product_id = p.id AND m.created_at < :to
             WHERE p.drug_class IN ('NARCOTIC', 'PSYCHOTROPIC')
             GROUP BY p.id
         )
         WHERE deleted_at IS NULL OR opening <> 0 OR moves > 0
         ORDER BY drug_class = 'PSYCHOTROPIC', name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map(named_params! { ":from": from, ":to": to }, |r| {
            let opening: i64 = r.get(6)?;
            let received: i64 = r.get(7)?;
            let sold: i64 = r.get(8)?;
            let destroyed: i64 = r.get(9)?;
            let adjusted: i64 = r.get(10)?;
            Ok(SipnapRow {
                product_id: r.get(0)?,
                code: r.get(1)?,
                name: r.get(2)?,
                drug_class: r.get(3)?,
                base_unit_name: r.get(4)?,
                opening,
                received,
                sold,
                destroyed,
                adjusted,
                closing: opening + received - sold - destroyed + adjusted,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}
