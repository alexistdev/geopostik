use rusqlite::{Connection, OptionalExtension, named_params, params};

use super::OUTSTANDING;
use super::model::{
    DebtFilter, DebtQuery, DebtSummary, PurchaseItemDetail, PurchasePaymentType, PurchaseProduct, PurchaseQuery,
    PurchaseRow, PurchaseStatus, PurchaseUnit, Supplier, SupplierDebtRow, SupplierPaymentMethod, SupplierPaymentRow,
    TaxMode,
};
use crate::error::AppResult;
use crate::master::{MasterPageQuery, like_pattern};

fn page_bounds(limit: i64, offset: i64) -> (i64, i64) {
    (limit.clamp(1, 500), offset.max(0))
}

/// Teks pencarian → query FTS5: setiap kata dicari sebagai awalan, semua kata wajib ada.
fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

// ─── Supplier ────────────────────────────────────────────────────────────────

/// Kode otomatis berikutnya (SUP0001). Kode yang pernah dipakai, termasuk milik supplier yang sudah
/// dihapus, dilewati.
pub fn next_supplier_code(conn: &Connection) -> AppResult<String> {
    let mut n: i64 = conn.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM suppliers", [], |r| r.get(0))?;
    loop {
        let code = format!("SUP{n:04}");
        let used: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM suppliers WHERE code = ?1 COLLATE NOCASE)",
            [&code],
            |r| r.get(0),
        )?;
        if !used {
            return Ok(code);
        }
        n += 1;
    }
}

const SUPPLIER_COLUMNS: &str = "id, code, name, address, phone, npwp, payment_term_days, is_active, created_at,
     (SELECT u.username FROM users u WHERE u.id = suppliers.created_by)";

fn supplier_row(r: &rusqlite::Row) -> rusqlite::Result<Supplier> {
    Ok(Supplier {
        id: r.get(0)?,
        code: r.get(1)?,
        name: r.get(2)?,
        address: r.get(3)?,
        phone: r.get(4)?,
        npwp: r.get(5)?,
        payment_term_days: r.get(6)?,
        is_active: r.get(7)?,
        created_at: r.get(8)?,
        created_by: r.get(9)?,
    })
}

const SUPPLIER_FILTER: &str = "deleted_at IS NULL
      AND (:like IS NULL OR name LIKE :like ESCAPE '\\' OR code LIKE :like ESCAPE '\\'
           OR phone LIKE :like ESCAPE '\\' OR npwp LIKE :like ESCAPE '\\')";

pub fn list_suppliers(conn: &Connection) -> AppResult<Vec<Supplier>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SUPPLIER_COLUMNS} FROM suppliers WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE, id"
    ))?;
    let rows = stmt.query_map([], supplier_row)?.collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn page_suppliers(conn: &Connection, q: &MasterPageQuery) -> AppResult<(Vec<Supplier>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let total = conn.query_row(
        &format!("SELECT count(*) FROM suppliers WHERE {SUPPLIER_FILTER}"),
        named_params! { ":like": like },
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SUPPLIER_COLUMNS} FROM suppliers WHERE {SUPPLIER_FILTER}
         ORDER BY name COLLATE NOCASE, id LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(named_params! { ":like": like, ":limit": limit, ":offset": offset }, supplier_row)?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn get_supplier(conn: &Connection, id: i64) -> AppResult<Option<Supplier>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SUPPLIER_COLUMNS} FROM suppliers WHERE id = ?1 AND deleted_at IS NULL"),
            [id],
            supplier_row,
        )
        .optional()?)
}

pub struct SupplierFields<'a> {
    pub name: &'a str,
    pub address: Option<&'a str>,
    pub phone: Option<&'a str>,
    pub npwp: Option<&'a str>,
    pub payment_term_days: i64,
    pub is_active: bool,
}

pub fn insert_supplier(conn: &Connection, code: &str, f: &SupplierFields<'_>, created_by: i64) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO suppliers (code, name, address, phone, npwp, payment_term_days, is_active, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![code, f.name, f.address, f.phone, f.npwp, f.payment_term_days, f.is_active, created_by],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_supplier(conn: &Connection, id: i64, f: &SupplierFields<'_>) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE suppliers SET name = ?2, address = ?3, phone = ?4, npwp = ?5, payment_term_days = ?6,
                              is_active = ?7, updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, f.name, f.address, f.phone, f.npwp, f.payment_term_days, f.is_active],
    )?)
}

/// Supplier lain (belum dihapus) dengan nama yang sama (tanpa beda huruf besar/kecil).
pub fn supplier_name_taken(conn: &Connection, name: &str, except_id: Option<i64>) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT code FROM suppliers WHERE name = ?1 COLLATE NOCASE AND id IS NOT ?2 AND deleted_at IS NULL",
            params![name, except_id],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn set_supplier_active(conn: &Connection, id: i64, active: bool) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE suppliers SET is_active = ?2, updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, active],
    )?)
}

pub fn soft_delete_supplier(conn: &Connection, id: i64, user_id: i64) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE suppliers SET deleted_at = datetime('now', 'localtime'), deleted_by = ?2, is_active = 0,
                              updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, user_id],
    )?)
}

/// Jumlah faktur (semua status) dari supplier ini.
pub fn supplier_usage(conn: &Connection, id: i64) -> AppResult<i64> {
    Ok(conn.query_row("SELECT count(*) FROM purchases WHERE supplier_id = ?1", [id], |r| r.get(0))?)
}

// ─── Faktur ──────────────────────────────────────────────────────────────────

fn purchase_columns() -> String {
    format!(
        "pu.id, pu.number, pu.supplier_id, sp.name, pu.invoice_number, pu.invoice_date, pu.received_date,
         pu.due_date, pu.payment_type, pu.status,
         (SELECT count(*) FROM purchase_items i WHERE i.purchase_id = pu.id),
         pu.grand_total, pu.created_at, cu.username, pu.posted_at,
         CASE WHEN pu.status = 'POSTED' AND pu.payment_type = 'CREDIT' THEN {OUTSTANDING} END"
    )
}

const PURCHASE_FROM: &str = "
    FROM purchases pu
    JOIN suppliers sp ON sp.id = pu.supplier_id
    LEFT JOIN users cu ON cu.id = pu.created_by";

fn purchase_row(r: &rusqlite::Row) -> rusqlite::Result<PurchaseRow> {
    Ok(PurchaseRow {
        id: r.get(0)?,
        number: r.get(1)?,
        supplier_id: r.get(2)?,
        supplier_name: r.get(3)?,
        invoice_number: r.get(4)?,
        invoice_date: r.get(5)?,
        received_date: r.get(6)?,
        due_date: r.get(7)?,
        payment_type: r.get(8)?,
        status: r.get(9)?,
        item_count: r.get(10)?,
        grand_total: r.get(11)?,
        created_at: r.get(12)?,
        created_by: r.get(13)?,
        posted_at: r.get(14)?,
        outstanding: r.get(15)?,
    })
}

pub fn page_purchases(conn: &Connection, q: &PurchaseQuery) -> AppResult<(Vec<PurchaseRow>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let status = q.status.map(PurchaseStatus::as_str);
    let filter = "WHERE (:status IS NULL OR pu.status = :status)
                    AND (:supplier_id IS NULL OR pu.supplier_id = :supplier_id)
                    AND (:date_from IS NULL OR pu.received_date >= :date_from)
                    AND (:date_to IS NULL OR pu.received_date <= :date_to)
                    AND (:like IS NULL OR pu.number LIKE :like ESCAPE '\\'
                         OR pu.invoice_number LIKE :like ESCAPE '\\' OR sp.name LIKE :like ESCAPE '\\')";
    let total: i64 = conn.query_row(
        &format!("SELECT count(*) {PURCHASE_FROM} {filter}"),
        named_params! {
            ":status": status, ":supplier_id": q.supplier_id, ":date_from": q.date_from,
            ":date_to": q.date_to, ":like": like,
        },
        |r| r.get(0),
    )?;
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let mut stmt = conn.prepare(&format!(
        "SELECT {} {PURCHASE_FROM} {filter}
         ORDER BY pu.status = 'DRAFT' DESC, pu.received_date DESC, pu.id DESC
         LIMIT :limit OFFSET :offset",
        purchase_columns()
    ))?;
    let rows = stmt
        .query_map(
            named_params! {
                ":status": status, ":supplier_id": q.supplier_id, ":date_from": q.date_from,
                ":date_to": q.date_to, ":like": like, ":limit": limit, ":offset": offset,
            },
            purchase_row,
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

/// Kolom header faktur di luar `PurchaseRow`.
pub struct Header {
    pub row: PurchaseRow,
    pub tax_mode: TaxMode,
    pub tax_rate_bp: i64,
    pub note: Option<String>,
    pub subtotal: i64,
    pub extra_discount: i64,
    pub discount_total: i64,
    pub tax_total: i64,
    pub tax_in_cost: Option<bool>,
    pub posted_by: Option<String>,
    pub voided_at: Option<String>,
    pub voided_by: Option<String>,
    pub void_reason: Option<String>,
}

pub fn get_header(conn: &Connection, id: i64) -> AppResult<Option<Header>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {}, pu.tax_mode, pu.tax_rate_bp, pu.note, pu.subtotal, pu.extra_discount, pu.discount_total,
                        pu.tax_total, pu.tax_in_cost,
                        (SELECT username FROM users WHERE id = pu.posted_by), pu.voided_at,
                        (SELECT username FROM users WHERE id = pu.voided_by), pu.void_reason
                 {PURCHASE_FROM} WHERE pu.id = ?1",
                purchase_columns()
            ),
            [id],
            |r| {
                Ok(Header {
                    row: purchase_row(r)?,
                    tax_mode: r.get(16)?,
                    tax_rate_bp: r.get(17)?,
                    note: r.get(18)?,
                    subtotal: r.get(19)?,
                    extra_discount: r.get(20)?,
                    discount_total: r.get(21)?,
                    tax_total: r.get(22)?,
                    tax_in_cost: r.get(23)?,
                    posted_by: r.get(24)?,
                    voided_at: r.get(25)?,
                    voided_by: r.get(26)?,
                    void_reason: r.get(27)?,
                })
            },
        )
        .optional()?)
}

pub fn items(conn: &Connection, purchase_id: i64) -> AppResult<Vec<PurchaseItemDetail>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.line_no, i.product_id, p.code, p.name, p.drug_class, i.product_unit_id, un.name, bu.name,
                i.conversion, i.qty, i.bonus_qty, i.unit_price, i.discount1_bp, i.discount2_bp, i.line_total,
                i.batch_number, i.expiry_date, (i.qty + i.bonus_qty) * i.conversion, i.unit_cost_x100,
                p.last_cost_x100, i.batch_id
         FROM purchase_items i
         JOIN products p ON p.id = i.product_id
         JOIN product_units pu ON pu.id = i.product_unit_id
         JOIN units un ON un.id = pu.unit_id
         JOIN units bu ON bu.id = p.base_unit_id
         WHERE i.purchase_id = ?1
         ORDER BY i.line_no, i.id",
    )?;
    let rows = stmt
        .query_map([purchase_id], |r| {
            Ok(PurchaseItemDetail {
                id: r.get(0)?,
                line_no: r.get(1)?,
                product_id: r.get(2)?,
                product_code: r.get(3)?,
                product_name: r.get(4)?,
                drug_class: r.get(5)?,
                product_unit_id: r.get(6)?,
                unit_name: r.get(7)?,
                base_unit_name: r.get(8)?,
                conversion: r.get(9)?,
                qty: r.get(10)?,
                bonus_qty: r.get(11)?,
                unit_price: r.get(12)?,
                discount1_bp: r.get(13)?,
                discount2_bp: r.get(14)?,
                line_total: r.get(15)?,
                batch_number: r.get(16)?,
                expiry_date: r.get(17)?,
                qty_base: r.get(18)?,
                unit_cost_x100: r.get(19)?,
                last_cost_x100: r.get(20)?,
                batch_id: r.get(21)?,
                units: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Satuan beli yang dipilih di baris faktur.
pub struct UnitInfo {
    pub product_id: i64,
    pub product_name: String,
    pub unit_name: String,
    pub conversion: i64,
    /// Obat & satuan aktif dan belum dihapus.
    pub selectable: bool,
    pub last_cost_x100: Option<i64>,
}

pub fn unit_info(conn: &Connection, product_unit_id: i64) -> AppResult<Option<UnitInfo>> {
    Ok(conn
        .query_row(
            "SELECT p.id, p.name, un.name, pu.conversion,
                    pu.is_active = 1 AND p.is_active = 1 AND p.deleted_at IS NULL, p.last_cost_x100
             FROM product_units pu
             JOIN products p ON p.id = pu.product_id
             JOIN units un ON un.id = pu.unit_id
             WHERE pu.id = ?1",
            [product_unit_id],
            |r| {
                Ok(UnitInfo {
                    product_id: r.get(0)?,
                    product_name: r.get(1)?,
                    unit_name: r.get(2)?,
                    conversion: r.get(3)?,
                    selectable: r.get(4)?,
                    last_cost_x100: r.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Faktur lain (belum batal) dari supplier yang sama dengan nomor faktur yang sama.
pub fn invoice_taken(conn: &Connection, supplier_id: i64, invoice: &str, except_id: Option<i64>) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT number FROM purchases
             WHERE supplier_id = ?1 AND invoice_number = ?2 COLLATE NOCASE AND status <> 'VOID' AND id IS NOT ?3",
            params![supplier_id, invoice, except_id],
            |r| r.get(0),
        )
        .optional()?)
}

pub struct HeaderFields<'a> {
    pub supplier_id: i64,
    pub invoice_number: &'a str,
    /// Tanggal `YYYY-MM-DD` yang sudah dinormalisasi.
    pub invoice_date: String,
    pub received_date: String,
    pub due_date: Option<String>,
    pub payment_type: PurchasePaymentType,
    pub tax_mode: TaxMode,
    pub tax_rate_bp: i64,
    pub extra_discount: i64,
    pub note: Option<&'a str>,
}

pub struct Amounts {
    pub subtotal: i64,
    pub discount_total: i64,
    pub tax_total: i64,
    pub grand_total: i64,
}

pub fn insert_purchase(conn: &Connection, number: &str, f: &HeaderFields<'_>, a: &Amounts, user_id: i64) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO purchases (number, supplier_id, invoice_number, invoice_date, received_date, due_date,
                                payment_type, tax_mode, tax_rate_bp, extra_discount, note,
                                subtotal, discount_total, tax_total, grand_total, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            number, f.supplier_id, f.invoice_number, f.invoice_date, f.received_date, f.due_date,
            f.payment_type, f.tax_mode, f.tax_rate_bp, f.extra_discount, f.note,
            a.subtotal, a.discount_total, a.tax_total, a.grand_total, user_id
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Ubah header faktur DRAFT. Mengembalikan jumlah baris yang berubah (0 = bukan draft lagi).
pub fn update_purchase(conn: &Connection, id: i64, f: &HeaderFields<'_>, a: &Amounts) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE purchases
         SET supplier_id = ?2, invoice_number = ?3, invoice_date = ?4, received_date = ?5, due_date = ?6,
             payment_type = ?7, tax_mode = ?8, tax_rate_bp = ?9, extra_discount = ?10, note = ?11,
             subtotal = ?12, discount_total = ?13, tax_total = ?14, grand_total = ?15,
             updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND status = 'DRAFT'",
        params![
            id, f.supplier_id, f.invoice_number, f.invoice_date, f.received_date, f.due_date,
            f.payment_type, f.tax_mode, f.tax_rate_bp, f.extra_discount, f.note,
            a.subtotal, a.discount_total, a.tax_total, a.grand_total
        ],
    )?)
}

pub fn delete_items(conn: &Connection, purchase_id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM purchase_items WHERE purchase_id = ?1", [purchase_id])?;
    Ok(())
}

pub struct ItemFields<'a> {
    pub line_no: i64,
    pub product_id: i64,
    pub product_unit_id: i64,
    pub qty: i64,
    pub bonus_qty: i64,
    pub conversion: i64,
    pub unit_price: i64,
    pub discount1_bp: i64,
    pub discount2_bp: i64,
    pub line_total: i64,
    pub batch_number: &'a str,
    pub expiry_date: &'a str,
}

pub fn insert_item(conn: &Connection, purchase_id: i64, f: &ItemFields<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO purchase_items (purchase_id, line_no, product_id, product_unit_id, qty, bonus_qty, conversion,
                                     unit_price, discount1_bp, discount2_bp, line_total, batch_number, expiry_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            purchase_id, f.line_no, f.product_id, f.product_unit_id, f.qty, f.bonus_qty, f.conversion,
            f.unit_price, f.discount1_bp, f.discount2_bp, f.line_total, f.batch_number, f.expiry_date
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// HPP hasil hitung dan batch yang dibuat, diisi saat posting (faktur masih DRAFT).
pub fn set_item_posted(conn: &Connection, id: i64, unit_cost_x100: i64, batch_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE purchase_items SET unit_cost_x100 = ?2, batch_id = ?3, updated_at = datetime('now', 'localtime')
         WHERE id = ?1",
        params![id, unit_cost_x100, batch_id],
    )?;
    Ok(())
}

pub fn mark_posted(conn: &Connection, id: i64, a: &Amounts, tax_in_cost: bool, user_id: i64) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE purchases
         SET status = 'POSTED', subtotal = ?2, discount_total = ?3, tax_total = ?4, grand_total = ?5,
             tax_in_cost = ?6, posted_by = ?7, posted_at = datetime('now', 'localtime'),
             updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND status = 'DRAFT'",
        params![id, a.subtotal, a.discount_total, a.tax_total, a.grand_total, tax_in_cost, user_id],
    )?)
}

pub fn mark_void(conn: &Connection, id: i64, from: PurchaseStatus, user_id: i64, reason: &str) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE purchases
         SET status = 'VOID', voided_by = ?3, void_reason = ?4, voided_at = datetime('now', 'localtime'),
             updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND status = ?2",
        params![id, from, user_id, reason],
    )?)
}

/// Baris kartu stok batch ini yang bukan berasal dari faktur ini (terjual, disesuaikan, ...).
pub fn batch_other_movements(conn: &Connection, batch_id: i64, purchase_id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM stock_movements
         WHERE batch_id = ?1 AND NOT (ref_type = 'purchase' AND ref_id = ?2)",
        [batch_id, purchase_id],
        |r| r.get(0),
    )?)
}

pub fn set_last_cost(conn: &Connection, product_id: i64, cost_x100: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE products SET last_cost_x100 = ?2, updated_at = datetime('now', 'localtime') WHERE id = ?1",
        params![product_id, cost_x100],
    )?;
    Ok(())
}

/// Harga jual per satuan aktif `(nama satuan, harga)` untuk log audit.
pub fn unit_prices(conn: &Connection, product_id: i64) -> AppResult<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT un.name, pu.sell_price FROM product_units pu JOIN units un ON un.id = pu.unit_id
         WHERE pu.product_id = ?1 AND pu.is_active = 1 ORDER BY pu.conversion, pu.id",
    )?;
    let rows = stmt.query_map([product_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    Ok(rows)
}

// ─── Pencarian obat ──────────────────────────────────────────────────────────

/// Obat aktif untuk baris faktur: barcode persis (langsung satuannya), kode persis, atau nama/generik.
pub fn search_products(conn: &Connection, text: &str, limit: i64) -> AppResult<Vec<PurchaseProduct>> {
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
                   AND (:text = '' OR upper(p.code) = upper(:text)
                        OR (:fts IS NOT NULL AND p.id IN (SELECT rowid FROM products_fts WHERE products_fts MATCH :fts)))
                 ORDER BY upper(p.code) = upper(:text) DESC, p.name COLLATE NOCASE
                 LIMIT :limit",
            )?;
            stmt.query_map(named_params! { ":text": text, ":fts": fts, ":limit": limit }, |r| r.get(0))?
                .collect::<Result<_, _>>()?
        }
    };

    let mut head = conn.prepare(
        "SELECT p.id, p.code, p.name, p.generic_name, p.drug_class, bu.name, p.last_cost_x100
         FROM products p JOIN units bu ON bu.id = p.base_unit_id WHERE p.id = ?1",
    )?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let mut p = head.query_row([id], |r| {
            Ok(PurchaseProduct {
                product_id: r.get(0)?,
                code: r.get(1)?,
                name: r.get(2)?,
                generic_name: r.get(3)?,
                drug_class: r.get(4)?,
                base_unit_name: r.get(5)?,
                units: Vec::new(),
                matched_unit_id: barcode.map(|(_, unit)| unit),
                last_cost_x100: r.get(6)?,
            })
        })?;
        p.units = purchase_units(conn, id)?;
        out.push(p);
    }
    Ok(out)
}

/// Satuan aktif obat, isi terbesar dulu (box, strip, tablet).
pub fn purchase_units(conn: &Connection, product_id: i64) -> AppResult<Vec<PurchaseUnit>> {
    let mut stmt = conn.prepare_cached(
        "SELECT pu.id, un.name, pu.conversion
         FROM product_units pu JOIN units un ON un.id = pu.unit_id
         WHERE pu.product_id = ?1 AND pu.is_active = 1
         ORDER BY pu.conversion DESC, pu.id",
    )?;
    let rows = stmt
        .query_map([product_id], |r| {
            Ok(PurchaseUnit { product_unit_id: r.get(0)?, unit_name: r.get(1)?, conversion: r.get(2)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ─── Pembayaran hutang ───────────────────────────────────────────────────────

pub fn payments(conn: &Connection, purchase_id: i64) -> AppResult<Vec<SupplierPaymentRow>> {
    let mut stmt = conn.prepare(
        "SELECT sp.id, sp.number, sp.purchase_id, sp.payment_date, sp.amount, sp.method, sp.reference, sp.note,
                sp.created_at, cu.username, sp.voided_at, vu.username, sp.void_reason
         FROM supplier_payments sp
         LEFT JOIN users cu ON cu.id = sp.created_by
         LEFT JOIN users vu ON vu.id = sp.voided_by
         WHERE sp.purchase_id = ?1
         ORDER BY sp.payment_date, sp.id",
    )?;
    let rows = stmt
        .query_map([purchase_id], |r| {
            Ok(SupplierPaymentRow {
                id: r.get(0)?,
                number: r.get(1)?,
                purchase_id: r.get(2)?,
                payment_date: r.get(3)?,
                amount: r.get(4)?,
                method: r.get(5)?,
                reference: r.get(6)?,
                note: r.get(7)?,
                created_at: r.get(8)?,
                created_by: r.get(9)?,
                voided_at: r.get(10)?,
                voided_by: r.get(11)?,
                void_reason: r.get(12)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Pembayaran yang belum dibatalkan.
pub fn active_payment_count(conn: &Connection, purchase_id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM supplier_payments WHERE purchase_id = ?1 AND voided_at IS NULL",
        [purchase_id],
        |r| r.get(0),
    )?)
}

pub struct NewPayment<'a> {
    pub number: &'a str,
    pub supplier_id: i64,
    pub purchase_id: i64,
    pub payment_date: &'a str,
    pub amount: i64,
    pub method: SupplierPaymentMethod,
    pub reference: Option<&'a str>,
    pub note: Option<&'a str>,
    pub user_id: i64,
}

pub fn insert_payment(conn: &Connection, p: &NewPayment<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO supplier_payments (number, supplier_id, purchase_id, payment_date, amount, method, reference,
                                        note, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            p.number, p.supplier_id, p.purchase_id, p.payment_date, p.amount, p.method, p.reference, p.note,
            p.user_id
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// `(purchase_id, number, amount, sudah dibatalkan)`.
pub fn get_payment(conn: &Connection, id: i64) -> AppResult<Option<(i64, String, i64, bool)>> {
    Ok(conn
        .query_row(
            "SELECT purchase_id, number, amount, voided_at IS NOT NULL FROM supplier_payments WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?)
}

pub fn void_payment(conn: &Connection, id: i64, user_id: i64, reason: &str) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE supplier_payments SET voided_at = datetime('now', 'localtime'), voided_by = ?2, void_reason = ?3
         WHERE id = ?1 AND voided_at IS NULL",
        params![id, user_id, reason],
    )?)
}

// ─── Hutang ──────────────────────────────────────────────────────────────────

/// Faktur kredit yang sudah diposting beserta pembayaran dan sisa hutangnya.
fn debts_base() -> String {
    format!(
        "SELECT pu.id, pu.number, pu.supplier_id, sp.name AS supplier_name, pu.invoice_number, pu.invoice_date,
                pu.due_date, pu.grand_total,
                COALESCE((SELECT SUM(amount) FROM supplier_payments x
                          WHERE x.purchase_id = pu.id AND x.voided_at IS NULL), 0) AS paid,
                COALESCE((SELECT SUM(total) FROM supplier_returns x
                          WHERE x.purchase_id = pu.id AND x.settlement = 'DEBT_CUT'), 0) AS returned,
                {OUTSTANDING} AS outstanding
         FROM purchases pu JOIN suppliers sp ON sp.id = pu.supplier_id
         WHERE pu.payment_type = 'CREDIT' AND pu.status = 'POSTED'"
    )
}

pub fn page_debts(conn: &Connection, q: &DebtQuery, today: &str, soon: &str) -> AppResult<(Vec<SupplierDebtRow>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let filter = match q.filter {
        DebtFilter::Open => "d.outstanding > 0",
        DebtFilter::Overdue => "d.outstanding > 0 AND d.due_date < :today",
        DebtFilter::DueSoon => "d.outstanding > 0 AND d.due_date >= :today AND d.due_date <= :soon",
        DebtFilter::Paid => "d.outstanding <= 0",
        DebtFilter::All => "1",
    };
    // `:today` dan `:soon` selalu disebut agar rusqlite mengenali parameternya di semua filter.
    let from = format!(
        "FROM ({}) d
         WHERE {filter} AND :today IS NOT NULL AND :soon IS NOT NULL
           AND (:supplier_id IS NULL OR d.supplier_id = :supplier_id)
           AND (:like IS NULL OR d.number LIKE :like ESCAPE '\\' OR d.invoice_number LIKE :like ESCAPE '\\'
                OR d.supplier_name LIKE :like ESCAPE '\\')",
        debts_base()
    );
    let total: i64 = conn.query_row(
        &format!("SELECT count(*) {from}"),
        named_params! { ":today": today, ":soon": soon, ":supplier_id": q.supplier_id, ":like": like },
        |r| r.get(0),
    )?;
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let mut stmt = conn.prepare(&format!(
        "SELECT d.id, d.number, d.supplier_id, d.supplier_name, d.invoice_number, d.invoice_date, d.due_date,
                d.grand_total, d.paid, d.returned, d.outstanding,
                CAST(julianday(d.due_date) - julianday(:today) AS INTEGER)
         {from}
         ORDER BY d.outstanding <= 0, d.due_date, d.id
         LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(
            named_params! {
                ":today": today, ":soon": soon, ":supplier_id": q.supplier_id, ":like": like,
                ":limit": limit, ":offset": offset,
            },
            |r| {
                Ok(SupplierDebtRow {
                    purchase_id: r.get(0)?,
                    number: r.get(1)?,
                    supplier_id: r.get(2)?,
                    supplier_name: r.get(3)?,
                    invoice_number: r.get(4)?,
                    invoice_date: r.get(5)?,
                    due_date: r.get(6)?,
                    grand_total: r.get(7)?,
                    paid: r.get(8)?,
                    returned: r.get(9)?,
                    outstanding: r.get(10)?,
                    days_left: r.get(11)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn debt_summary(conn: &Connection, today: &str, soon: &str) -> AppResult<DebtSummary> {
    Ok(conn.query_row(
        &format!(
            "SELECT count(*), COALESCE(SUM(outstanding), 0),
                    COALESCE(SUM(due_date < :today), 0),
                    COALESCE(SUM(CASE WHEN due_date < :today THEN outstanding END), 0),
                    COALESCE(SUM(due_date >= :today AND due_date <= :soon), 0),
                    COALESCE(SUM(CASE WHEN due_date >= :today AND due_date <= :soon THEN outstanding END), 0)
             FROM ({}) WHERE outstanding > 0",
            debts_base()
        ),
        named_params! { ":today": today, ":soon": soon },
        |r| {
            Ok(DebtSummary {
                open_count: r.get(0)?,
                open_amount: r.get(1)?,
                overdue_count: r.get(2)?,
                overdue_amount: r.get(3)?,
                due_soon_count: r.get(4)?,
                due_soon_amount: r.get(5)?,
            })
        },
    )?)
}
