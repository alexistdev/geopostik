use rusqlite::{Connection, OptionalExtension, named_params, params};

use super::model::{
    MethodAmount, PaymentDetail, PaymentMethod, PosPrescription, PosProduct, PosTier, PosUnit, SaleBatch, SaleQuery,
    SaleRow,
};
use crate::error::AppResult;
use crate::master::DrugClass;

fn page_bounds(limit: i64, offset: i64) -> (i64, i64) {
    (limit.clamp(1, 200), offset.max(0))
}

// ─── Shift ───────────────────────────────────────────────────────────────────

pub struct ShiftRow {
    pub id: i64,
    pub user_id: i64,
    pub opened_by: String,
    pub opened_at: String,
    pub opening_cash: i64,
    pub status: String,
    pub closed_at: Option<String>,
    pub counted_cash: Option<i64>,
    pub difference: Option<i64>,
}

const SHIFT_SELECT: &str = "
    SELECT s.id, s.user_id, u.full_name, s.opened_at, s.opening_cash, s.status, s.closed_at,
           s.counted_cash, s.difference
    FROM shifts s JOIN users u ON u.id = s.user_id";

fn shift_row(r: &rusqlite::Row) -> rusqlite::Result<ShiftRow> {
    Ok(ShiftRow {
        id: r.get(0)?,
        user_id: r.get(1)?,
        opened_by: r.get(2)?,
        opened_at: r.get(3)?,
        opening_cash: r.get(4)?,
        status: r.get(5)?,
        closed_at: r.get(6)?,
        counted_cash: r.get(7)?,
        difference: r.get(8)?,
    })
}

pub fn open_shift(conn: &Connection) -> AppResult<Option<ShiftRow>> {
    Ok(conn
        .query_row(&format!("{SHIFT_SELECT} WHERE s.status = 'OPEN'"), [], shift_row)
        .optional()?)
}

pub fn get_shift(conn: &Connection, id: i64) -> AppResult<Option<ShiftRow>> {
    Ok(conn
        .query_row(&format!("{SHIFT_SELECT} WHERE s.id = ?1"), [id], shift_row)
        .optional()?)
}

pub fn insert_shift(conn: &Connection, user_id: i64, opening_cash: i64, note: Option<&str>, now: &str) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO shifts (user_id, opened_at, opening_cash, note, status) VALUES (?1, ?2, ?3, ?4, 'OPEN')",
        params![user_id, now, opening_cash, note],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Tutup shift hanya bila masih `OPEN`; mengembalikan jumlah baris yang berubah (0 = sudah ditutup).
pub fn close_shift(
    conn: &Connection,
    id: i64,
    expected: i64,
    counted: i64,
    note: Option<&str>,
    now: &str,
) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE shifts
         SET status = 'CLOSED', closed_at = ?2, expected_cash = ?3, counted_cash = ?4,
             difference = ?4 - ?3, note = COALESCE(?5, note)
         WHERE id = ?1 AND status = 'OPEN'",
        params![id, now, expected, counted, note],
    )?)
}

pub struct ShiftTotals {
    pub sale_count: i64,
    pub sales_total: i64,
    pub void_count: i64,
    pub cash_in: i64,
    pub by_method: Vec<MethodAmount>,
}

pub fn shift_totals(conn: &Connection, shift_id: i64) -> AppResult<ShiftTotals> {
    let (sale_count, sales_total, void_count) = conn.query_row(
        "SELECT COALESCE(SUM(status = 'COMPLETED'), 0),
                COALESCE(SUM(CASE WHEN status = 'COMPLETED' THEN grand_total END), 0),
                COALESCE(SUM(status = 'VOID'), 0)
         FROM sales WHERE shift_id = ?1",
        [shift_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut stmt = conn.prepare(
        "SELECT p.method, SUM(p.amount) FROM sale_payments p JOIN sales s ON s.id = p.sale_id
         WHERE s.shift_id = ?1 AND s.status = 'COMPLETED'
         GROUP BY p.method
         ORDER BY CASE p.method WHEN 'CASH' THEN 0 WHEN 'QRIS' THEN 1 ELSE 2 END",
    )?;
    let by_method: Vec<MethodAmount> = stmt
        .query_map([shift_id], |r| Ok(MethodAmount { method: r.get(0)?, amount: r.get(1)? }))?
        .collect::<Result<_, _>>()?;
    let cash_in = by_method.iter().filter(|m| m.method == PaymentMethod::Cash).map(|m| m.amount).sum();
    Ok(ShiftTotals { sale_count, sales_total, void_count, cash_in, by_method })
}

// ─── Pencarian obat ──────────────────────────────────────────────────────────

/// Query FTS5 prefix per kata: "para 500" → `"para"* "500"*`.
fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .map(|t| t.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// Obat aktif yang cocok dengan barcode persis, atau nama/generik/kode.
/// Barcode persis diutamakan agar scan langsung memilih obat dan satuannya.
pub fn search_products(conn: &Connection, text: &str, today: &str, limit: i64) -> AppResult<Vec<PosProduct>> {
    let barcode: Option<(i64, i64)> = conn
        .query_row(
            "SELECT pu.product_id, pu.id FROM product_barcodes b
             JOIN product_units pu ON pu.id = b.product_unit_id
             JOIN products p ON p.id = pu.product_id
             WHERE b.barcode = ?1 AND b.deleted_at IS NULL AND pu.is_active = 1
               AND p.is_active = 1 AND p.deleted_at IS NULL",
            [text],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;

    let ids: Vec<i64> = match barcode {
        Some((pid, _)) => vec![pid],
        None => {
            let fts = fts_query(text);
            let mut stmt = conn.prepare(
                "SELECT p.id FROM products p
                 WHERE p.is_active = 1 AND p.deleted_at IS NULL
                   AND (upper(p.code) = upper(:text)
                        OR (:fts IS NOT NULL AND p.id IN (SELECT rowid FROM products_fts WHERE products_fts MATCH :fts)))
                 ORDER BY upper(p.code) = upper(:text) DESC, p.name COLLATE NOCASE
                 LIMIT :limit",
            )?;
            stmt.query_map(named_params! { ":text": text, ":fts": fts, ":limit": limit }, |r| r.get(0))?
                .collect::<Result<_, _>>()?
        }
    };

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(mut p) = pos_product(conn, id, today)? {
            p.matched_unit_id = barcode.map(|(_, unit)| unit);
            out.push(p);
        }
    }
    Ok(out)
}

fn pos_product(conn: &Connection, id: i64, today: &str) -> AppResult<Option<PosProduct>> {
    let head = conn
        .query_row(
            "SELECT p.id, p.code, p.name, p.generic_name, p.drug_class, p.is_owa, bu.name,
                    COALESCE((SELECT SUM(qty_on_hand_base) FROM batches b
                              WHERE b.product_id = p.id AND b.qty_on_hand_base > 0 AND b.is_locked = 0
                                AND b.expiry_date > ?2), 0)
             FROM products p JOIN units bu ON bu.id = p.base_unit_id
             WHERE p.id = ?1",
            params![id, today],
            |r| {
                Ok(PosProduct {
                    product_id: r.get(0)?,
                    code: r.get(1)?,
                    name: r.get(2)?,
                    generic_name: r.get(3)?,
                    drug_class: r.get(4)?,
                    is_owa: r.get(5)?,
                    base_unit_name: r.get(6)?,
                    sellable_base: r.get(7)?,
                    units: Vec::new(),
                    matched_unit_id: None,
                })
            },
        )
        .optional()?;
    let Some(mut product) = head else { return Ok(None) };

    let mut stmt = conn.prepare(
        "SELECT pu.id, un.name, pu.conversion, pu.sell_price, pu.is_default_sale
         FROM product_units pu JOIN units un ON un.id = pu.unit_id
         WHERE pu.product_id = ?1 AND pu.is_active = 1
         ORDER BY pu.conversion, pu.id",
    )?;
    let mut units: Vec<PosUnit> = stmt
        .query_map([id], |r| {
            Ok(PosUnit {
                product_unit_id: r.get(0)?,
                unit_name: r.get(1)?,
                conversion: r.get(2)?,
                sell_price: r.get(3)?,
                is_default: r.get(4)?,
                tiers: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut tiers = conn.prepare(
        "SELECT min_qty, price FROM price_tiers WHERE product_unit_id = ?1 AND is_active = 1 ORDER BY min_qty",
    )?;
    for u in &mut units {
        u.tiers = tiers
            .query_map([u.product_unit_id], |r| Ok(PosTier { min_qty: r.get(0)?, price: r.get(1)? }))?
            .collect::<Result<_, _>>()?;
    }
    product.units = units;
    Ok(Some(product))
}

// ─── Penjualan ───────────────────────────────────────────────────────────────

/// Nota yang pernah disimpan dengan kunci idempotensi ini: (id, kasir).
pub fn sale_by_client_ref(conn: &Connection, client_ref: &str) -> AppResult<Option<(i64, i64)>> {
    Ok(conn
        .query_row("SELECT id, cashier_id FROM sales WHERE client_ref = ?1", [client_ref], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?)
}

pub struct SaleUnit {
    pub product_id: i64,
    pub product_name: String,
    pub unit_name: String,
    pub conversion: i64,
    pub drug_class: DrugClass,
    /// Obat & satuannya aktif dan belum dihapus.
    pub usable: bool,
}

pub fn sale_unit(conn: &Connection, product_unit_id: i64) -> AppResult<Option<SaleUnit>> {
    Ok(conn
        .query_row(
            "SELECT p.id, p.name, un.name, pu.conversion, p.drug_class,
                    p.is_active = 1 AND p.deleted_at IS NULL AND pu.is_active = 1
             FROM product_units pu
             JOIN products p ON p.id = pu.product_id
             JOIN units un ON un.id = pu.unit_id
             WHERE pu.id = ?1",
            [product_unit_id],
            |r| {
                Ok(SaleUnit {
                    product_id: r.get(0)?,
                    product_name: r.get(1)?,
                    unit_name: r.get(2)?,
                    conversion: r.get(3)?,
                    drug_class: r.get(4)?,
                    usable: r.get(5)?,
                })
            },
        )
        .optional()?)
}

pub struct FefoBatch {
    pub id: i64,
    pub qty_on_hand: i64,
    pub unit_cost_x100: i64,
}

/// Batch yang bisa dijual, ED terdekat dulu (docs/SCHEMA.md "Query FEFO").
/// Dibaca di dalam transaksi IMMEDIATE sehingga angkanya tidak bisa berubah sampai commit.
pub fn fefo_batches(conn: &Connection, product_id: i64, today: &str) -> AppResult<Vec<FefoBatch>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, qty_on_hand_base, unit_cost_x100 FROM batches
         WHERE product_id = ?1 AND qty_on_hand_base > 0 AND is_locked = 0 AND expiry_date > ?2
         ORDER BY expiry_date, id",
    )?;
    let rows = stmt
        .query_map(params![product_id, today], |r| {
            Ok(FefoBatch { id: r.get(0)?, qty_on_hand: r.get(1)?, unit_cost_x100: r.get(2)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub struct NewSale<'a> {
    pub number: &'a str,
    pub client_ref: &'a str,
    pub shift_id: i64,
    pub cashier_id: i64,
    pub customer_id: Option<i64>,
    pub prescription_id: Option<i64>,
    pub sale_type: &'a str,
    pub sold_at: &'a str,
    pub subtotal: i64,
    pub discount_total: i64,
    pub rounding: i64,
    pub grand_total: i64,
}

pub fn insert_sale(conn: &Connection, s: &NewSale<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO sales (number, client_ref, shift_id, cashier_id, customer_id, prescription_id, sale_type,
                            sold_at, subtotal, discount_total, tax_total, rounding, grand_total, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12, 'COMPLETED')",
        params![
            s.number, s.client_ref, s.shift_id, s.cashier_id, s.customer_id, s.prescription_id, s.sale_type,
            s.sold_at, s.subtotal, s.discount_total, s.rounding, s.grand_total
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub struct NewItem<'a> {
    pub sale_id: i64,
    pub line_no: i64,
    pub kind: &'a str,
    pub parent_item_id: Option<i64>,
    pub product_id: Option<i64>,
    pub product_unit_id: Option<i64>,
    pub description: &'a str,
    pub qty: i64,
    pub conversion: i64,
    pub unit_price: i64,
    pub price_tier_id: Option<i64>,
    pub discount_amount: i64,
    pub line_total: i64,
    pub usage_instruction: Option<&'a str>,
    pub authorized_by: Option<i64>,
}

pub fn insert_item(conn: &Connection, i: &NewItem<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO sale_items (sale_id, line_no, item_kind, parent_item_id, product_id, product_unit_id,
                                 description, qty, conversion, qty_base, unit_price, price_tier_id,
                                 discount_amount, line_total, usage_instruction, authorized_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8 * ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            i.sale_id, i.line_no, i.kind, i.parent_item_id, i.product_id, i.product_unit_id, i.description,
            i.qty, i.conversion, i.unit_price, i.price_tier_id, i.discount_amount, i.line_total,
            i.usage_instruction, i.authorized_by
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_item_batch(conn: &Connection, sale_item_id: i64, batch_id: i64, qty_base: i64, cost_x100: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sale_item_batches (sale_item_id, batch_id, qty_base, unit_cost_x100) VALUES (?1, ?2, ?3, ?4)",
        params![sale_item_id, batch_id, qty_base, cost_x100],
    )?;
    Ok(())
}

/// Baris kartu stok. Trigger `stock_movements_apply` mengubah `batches.qty_on_hand_base`; bila hasilnya
/// minus, `CHECK (qty_on_hand_base >= 0)` menggagalkan statement dan seluruh transaksi di-rollback.
#[allow(clippy::too_many_arguments)]
pub fn insert_movement(
    conn: &Connection,
    batch_id: i64,
    product_id: i64,
    movement_type: &str,
    qty_change: i64,
    sale_id: i64,
    sale_item_id: i64,
    user_id: i64,
    at: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id,
                                      ref_line_id, user_id, created_at)
         VALUES (?1, ?2, ?3, ?4, 'sale', ?5, ?6, ?7, ?8)",
        params![batch_id, product_id, movement_type, qty_change, sale_id, sale_item_id, user_id, at],
    )?;
    Ok(())
}

pub fn insert_payment(conn: &Connection, sale_id: i64, p: &PaymentDetail) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sale_payments (sale_id, method, amount, tendered, change_amount, reference)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![sale_id, p.method, p.amount, p.tendered, p.change_amount, p.reference],
    )?;
    Ok(())
}

/// Batch yang ringkasan stoknya tidak sama dengan jumlah kartu stoknya (seharusnya selalu kosong).
pub fn inconsistent_batches(conn: &Connection, batch_ids: &[i64]) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare_cached(
        "SELECT b.qty_on_hand_base = COALESCE((SELECT SUM(qty_change_base) FROM stock_movements m
                                                WHERE m.batch_id = b.id), 0)
         FROM batches b WHERE b.id = ?1",
    )?;
    let mut bad = Vec::new();
    for &id in batch_ids {
        let ok: bool = stmt.query_row([id], |r| r.get(0))?;
        if !ok {
            bad.push(id);
        }
    }
    Ok(bad)
}

// ─── Baca nota ───────────────────────────────────────────────────────────────

pub struct SaleHeader {
    pub id: i64,
    pub number: String,
    pub shift_id: i64,
    pub cashier_id: i64,
    pub cashier_name: String,
    pub sold_at: String,
    pub sale_type: String,
    pub prescription_id: Option<i64>,
    pub prescription_number: Option<String>,
    pub patient_name: Option<String>,
    pub subtotal: i64,
    pub discount_total: i64,
    pub rounding: i64,
    pub grand_total: i64,
    pub status: String,
    pub voided_at: Option<String>,
    pub voided_by: Option<String>,
    pub void_reason: Option<String>,
}

pub fn sale_header(conn: &Connection, id: i64) -> AppResult<Option<SaleHeader>> {
    Ok(conn
        .query_row(
            "SELECT s.id, s.number, s.shift_id, s.cashier_id, c.full_name, s.sold_at, s.sale_type, s.prescription_id,
                    r.number, r.patient_name, s.subtotal, s.discount_total, s.rounding, s.grand_total, s.status,
                    s.voided_at, v.full_name, s.void_reason
             FROM sales s
             JOIN users c ON c.id = s.cashier_id
             LEFT JOIN users v ON v.id = s.voided_by
             LEFT JOIN prescriptions r ON r.id = s.prescription_id
             WHERE s.id = ?1",
            [id],
            |r| {
                Ok(SaleHeader {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    shift_id: r.get(2)?,
                    cashier_id: r.get(3)?,
                    cashier_name: r.get(4)?,
                    sold_at: r.get(5)?,
                    sale_type: r.get(6)?,
                    prescription_id: r.get(7)?,
                    prescription_number: r.get(8)?,
                    patient_name: r.get(9)?,
                    subtotal: r.get(10)?,
                    discount_total: r.get(11)?,
                    rounding: r.get(12)?,
                    grand_total: r.get(13)?,
                    status: r.get(14)?,
                    voided_at: r.get(15)?,
                    voided_by: r.get(16)?,
                    void_reason: r.get(17)?,
                })
            },
        )
        .optional()?)
}

pub struct ItemRow {
    pub id: i64,
    pub line_no: i64,
    pub kind: String,
    pub parent_item_id: Option<i64>,
    pub description: String,
    pub unit_name: Option<String>,
    pub qty: i64,
    pub unit_price: i64,
    pub tier_min_qty: Option<i64>,
    pub discount_amount: i64,
    pub line_total: i64,
    pub usage_instruction: Option<String>,
}

pub fn sale_items(conn: &Connection, sale_id: i64) -> AppResult<Vec<ItemRow>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.line_no, i.item_kind, i.parent_item_id, i.description, un.name, i.qty, i.unit_price,
                t.min_qty, i.discount_amount, i.line_total, i.usage_instruction
         FROM sale_items i
         LEFT JOIN product_units pu ON pu.id = i.product_unit_id
         LEFT JOIN units un ON un.id = pu.unit_id
         LEFT JOIN price_tiers t ON t.id = i.price_tier_id
         WHERE i.sale_id = ?1
         ORDER BY i.line_no, i.id",
    )?;
    let rows = stmt
        .query_map([sale_id], |r| {
            Ok(ItemRow {
                id: r.get(0)?,
                line_no: r.get(1)?,
                kind: r.get(2)?,
                parent_item_id: r.get(3)?,
                description: r.get(4)?,
                unit_name: r.get(5)?,
                qty: r.get(6)?,
                unit_price: r.get(7)?,
                tier_min_qty: r.get(8)?,
                discount_amount: r.get(9)?,
                line_total: r.get(10)?,
                usage_instruction: r.get(11)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn item_batches(conn: &Connection, sale_item_id: i64) -> AppResult<Vec<SaleBatch>> {
    let mut stmt = conn.prepare_cached(
        "SELECT b.batch_number, b.expiry_date, x.qty_base FROM sale_item_batches x
         JOIN batches b ON b.id = x.batch_id
         WHERE x.sale_item_id = ?1 ORDER BY x.id",
    )?;
    let rows = stmt
        .query_map([sale_item_id], |r| {
            Ok(SaleBatch { batch_number: r.get(0)?, expiry_date: r.get(1)?, qty_base: r.get(2)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn payments(conn: &Connection, sale_id: i64) -> AppResult<Vec<PaymentDetail>> {
    let mut stmt = conn.prepare(
        "SELECT method, amount, tendered, change_amount, reference FROM sale_payments
         WHERE sale_id = ?1
         ORDER BY CASE method WHEN 'CASH' THEN 0 WHEN 'QRIS' THEN 1 ELSE 2 END, id",
    )?;
    let rows = stmt
        .query_map([sale_id], |r| {
            Ok(PaymentDetail {
                method: r.get(0)?,
                amount: r.get(1)?,
                tendered: r.get(2)?,
                change_amount: r.get(3)?,
                reference: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Daftar nota satu shift. `cashier_id` diisi untuk user yang hanya boleh melihat nota sendiri.
pub fn page_sales(conn: &Connection, shift_id: i64, cashier_id: Option<i64>, q: &SaleQuery) -> AppResult<(Vec<SaleRow>, i64)> {
    let text = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{s}%"));
    let filter = "FROM sales s JOIN users c ON c.id = s.cashier_id
                  WHERE s.shift_id = :shift AND (:cashier IS NULL OR s.cashier_id = :cashier)
                    AND (:text IS NULL OR s.number LIKE :text)";
    let total: i64 = conn.query_row(
        &format!("SELECT count(*) {filter}"),
        named_params! { ":shift": shift_id, ":cashier": cashier_id, ":text": text },
        |r| r.get(0),
    )?;
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.number, s.sold_at, c.full_name, s.sale_type,
                (SELECT count(*) FROM sale_items i WHERE i.sale_id = s.id AND i.parent_item_id IS NULL),
                s.grand_total, s.status,
                COALESCE((SELECT group_concat(method, '+') FROM
                            (SELECT method FROM sale_payments p WHERE p.sale_id = s.id
                             ORDER BY CASE method WHEN 'CASH' THEN 0 WHEN 'QRIS' THEN 1 ELSE 2 END)), '')
         {filter}
         ORDER BY s.id DESC
         LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(
            named_params! {
                ":shift": shift_id, ":cashier": cashier_id, ":text": text, ":limit": limit, ":offset": offset,
            },
            |r| {
                Ok(SaleRow {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    sold_at: r.get(2)?,
                    cashier_name: r.get(3)?,
                    sale_type: r.get(4)?,
                    item_count: r.get(5)?,
                    grand_total: r.get(6)?,
                    status: r.get(7)?,
                    methods: r.get(8)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

// ─── Void ────────────────────────────────────────────────────────────────────

/// Tandai batal hanya bila masih `COMPLETED`; 0 = sudah dibatalkan oleh permintaan lain.
pub fn mark_void(conn: &Connection, sale_id: i64, user_id: i64, reason: &str, now: &str) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE sales SET status = 'VOID', voided_at = ?3, voided_by = ?2, void_reason = ?4
         WHERE id = ?1 AND status = 'COMPLETED'",
        params![sale_id, user_id, now, reason],
    )?)
}

pub struct UsedBatch {
    pub sale_item_id: i64,
    pub product_id: i64,
    pub batch_id: i64,
    pub qty_base: i64,
}

pub fn used_batches(conn: &Connection, sale_id: i64) -> AppResult<Vec<UsedBatch>> {
    let mut stmt = conn.prepare(
        "SELECT x.sale_item_id, i.product_id, x.batch_id, x.qty_base
         FROM sale_item_batches x JOIN sale_items i ON i.id = x.sale_item_id
         WHERE i.sale_id = ?1 ORDER BY x.id",
    )?;
    let rows = stmt
        .query_map([sale_id], |r| {
            Ok(UsedBatch { sale_item_id: r.get(0)?, product_id: r.get(1)?, batch_id: r.get(2)?, qty_base: r.get(3)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn return_count(conn: &Connection, sale_id: i64) -> AppResult<i64> {
    Ok(conn.query_row("SELECT count(*) FROM sale_returns WHERE sale_id = ?1", [sale_id], |r| r.get(0))?)
}

// ─── Resep ───────────────────────────────────────────────────────────────────

pub fn ready_prescriptions(conn: &Connection) -> AppResult<Vec<PosPrescription>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.number, r.prescription_number, r.prescription_date, r.patient_name, d.name, r.screened_at,
                (SELECT count(*) FROM prescription_items i WHERE i.prescription_id = r.id AND i.parent_item_id IS NULL)
         FROM prescriptions r JOIN doctors d ON d.id = r.doctor_id
         WHERE r.status = 'SCREENED'
         ORDER BY r.screened_at, r.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(PosPrescription {
                id: r.get(0)?,
                number: r.get(1)?,
                prescription_number: r.get(2)?,
                prescription_date: r.get(3)?,
                patient_name: r.get(4)?,
                doctor_name: r.get(5)?,
                screened_at: r.get(6)?,
                item_count: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn count_ready_prescriptions(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT count(*) FROM prescriptions WHERE status = 'SCREENED'", [], |r| r.get(0))?)
}

pub struct RxHeader {
    pub id: i64,
    pub number: String,
    pub prescription_number: String,
    pub patient_name: String,
    pub doctor_name: String,
    pub customer_id: Option<i64>,
    pub status: String,
}

pub fn rx_header(conn: &Connection, id: i64) -> AppResult<Option<RxHeader>> {
    Ok(conn
        .query_row(
            "SELECT r.id, r.number, r.prescription_number, r.patient_name, d.name, r.customer_id, r.status
             FROM prescriptions r JOIN doctors d ON d.id = r.doctor_id WHERE r.id = ?1",
            [id],
            |r| {
                Ok(RxHeader {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    prescription_number: r.get(2)?,
                    patient_name: r.get(3)?,
                    doctor_name: r.get(4)?,
                    customer_id: r.get(5)?,
                    status: r.get(6)?,
                })
            },
        )
        .optional()?)
}

pub struct RxItem {
    pub id: i64,
    pub kind: String,
    pub parent_item_id: Option<i64>,
    pub product_unit_id: Option<i64>,
    pub description: String,
    pub qty: i64,
    pub unit_price: i64,
    pub usage_instruction: Option<String>,
}

pub fn rx_items(conn: &Connection, prescription_id: i64) -> AppResult<Vec<RxItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, item_kind, parent_item_id, product_unit_id, description, qty, unit_price, usage_instruction
         FROM prescription_items WHERE prescription_id = ?1 ORDER BY line_no, id",
    )?;
    let rows = stmt
        .query_map([prescription_id], |r| {
            Ok(RxItem {
                id: r.get(0)?,
                kind: r.get(1)?,
                parent_item_id: r.get(2)?,
                product_unit_id: r.get(3)?,
                description: r.get(4)?,
                qty: r.get(5)?,
                unit_price: r.get(6)?,
                usage_instruction: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// `SCREENED` → `PAID` hanya bila status masih `SCREENED` (0 = sudah dibayar/diubah).
pub fn mark_prescription_paid(conn: &Connection, id: i64) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE prescriptions SET status = 'PAID', updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND status = 'SCREENED'",
        [id],
    )?)
}

/// Nota resep dibatalkan: resep kembali menunggu dibayar.
pub fn reopen_prescription(conn: &Connection, id: i64) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE prescriptions SET status = 'SCREENED', updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND status = 'PAID'",
        [id],
    )?)
}
