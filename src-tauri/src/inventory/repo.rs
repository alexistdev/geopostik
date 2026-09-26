use rusqlite::{Connection, OptionalExtension, named_params, params};

use super::model::{
    BatchRow, OpnameItem, OpnamePageQuery, OpnameRow, OpnameStatus, OpnameType, StockCardQuery, StockCardRow,
    StockFilter, StockListQuery, StockRow,
};
use crate::error::AppResult;

// ─── Pencarian obat ──────────────────────────────────────────────────────────

/// Teks pencarian → query FTS5: setiap kata dicari sebagai awalan, semua kata wajib ada
/// (sama seperti daftar Obat).
fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

fn like_pattern(q: Option<&str>) -> Option<String> {
    let q = q.map(str::trim).filter(|q| !q.is_empty())?;
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    Some(format!("%{escaped}%"))
}

fn page_bounds(limit: i64, offset: i64) -> (i64, i64) {
    (limit.clamp(1, 500), offset.max(0))
}

// ─── Stok per obat ───────────────────────────────────────────────────────────

/// Ringkasan batch ber-stok per obat. `:today` = tanggal hari ini (`YYYY-MM-DD`).
const STOCK_SUMMARY: &str = "
    SELECT product_id,
           SUM(qty_on_hand_base) AS total,
           SUM(CASE WHEN is_locked = 0 AND expiry_date > :today THEN qty_on_hand_base ELSE 0 END) AS sellable,
           SUM(CASE WHEN expiry_date <= :today THEN qty_on_hand_base ELSE 0 END) AS expired,
           MIN(expiry_date) AS nearest,
           COUNT(*) AS batch_count,
           SUM(qty_on_hand_base * unit_cost_x100) AS value_x100
    FROM batches
    WHERE qty_on_hand_base > 0
    GROUP BY product_id";

pub fn list_stock(conn: &Connection, q: &StockListQuery, today: &str, near_expiry: &str) -> AppResult<(Vec<StockRow>, i64)> {
    let text = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let fts = text.and_then(fts_query);
    let barcode_clause = "p.id IN (SELECT xpu.product_id FROM product_barcodes b
                                   JOIN product_units xpu ON xpu.id = b.product_unit_id
                                   WHERE b.barcode = :barcode AND b.deleted_at IS NULL)";
    let text_clause = match (text, &fts) {
        (None, _) => "(:barcode IS NULL AND :fts IS NULL)".to_owned(),
        (Some(_), Some(_)) => format!(
            "(p.id IN (SELECT rowid FROM products_fts WHERE products_fts MATCH :fts) OR {barcode_clause})"
        ),
        (Some(_), None) => format!("(:fts IS NULL AND {barcode_clause})"),
    };
    let filter_clause = match q.filter {
        StockFilter::All => "1",
        StockFilter::Low => "p.min_stock_base > 0 AND COALESCE(s.total, 0) < p.min_stock_base",
        StockFilter::Empty => "COALESCE(s.total, 0) = 0",
        StockFilter::NearExpiry => "EXISTS (SELECT 1 FROM batches nb WHERE nb.product_id = p.id AND nb.qty_on_hand_base > 0
                                            AND nb.expiry_date > :today AND nb.expiry_date <= :near)",
        StockFilter::Expired => "COALESCE(s.expired, 0) > 0",
    };
    let from = format!(
        "FROM products p
         JOIN units bu ON bu.id = p.base_unit_id
         LEFT JOIN racks r ON r.id = p.rack_id
         LEFT JOIN ({STOCK_SUMMARY}) s ON s.product_id = p.id
         WHERE p.deleted_at IS NULL AND p.is_active = 1
           AND {text_clause}
           AND (:category_id IS NULL OR p.category_id = :category_id)
           AND (:rack_id IS NULL OR p.rack_id = :rack_id)
           AND {filter_clause}"
    );
    let (limit, offset) = page_bounds(q.limit, q.offset);

    // Parameter yang tidak dipakai klausa tertentu tetap harus ada di SQL (rusqlite menolak
    // parameter bernama yang tidak dikenal), jadi `:near` selalu disebut.
    let total: i64 = conn.query_row(
        &format!("SELECT count(*), :near {from}"),
        named_params! {
            ":fts": fts, ":barcode": text, ":today": today, ":near": near_expiry,
            ":category_id": q.category_id, ":rack_id": q.rack_id,
        },
        |r| r.get(0),
    )?;

    let sql = format!(
        "SELECT p.id, p.code, p.name, p.generic_name, bu.name, r.name, p.min_stock_base,
                COALESCE(s.total, 0), COALESCE(s.sellable, 0), COALESCE(s.expired, 0), s.nearest,
                COALESCE(s.batch_count, 0), COALESCE(s.value_x100, 0), :near
         {from}
         ORDER BY p.name COLLATE NOCASE, p.id
         LIMIT :limit OFFSET :offset"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            named_params! {
                ":fts": fts, ":barcode": text, ":today": today, ":near": near_expiry,
                ":category_id": q.category_id, ":rack_id": q.rack_id,
                ":limit": limit, ":offset": offset,
            },
            |r| {
                let value_x100: i64 = r.get(12)?;
                Ok(StockRow {
                    product_id: r.get(0)?,
                    code: r.get(1)?,
                    name: r.get(2)?,
                    generic_name: r.get(3)?,
                    base_unit_name: r.get(4)?,
                    rack_name: r.get(5)?,
                    min_stock_base: r.get(6)?,
                    stock_base: r.get(7)?,
                    sellable_base: r.get(8)?,
                    expired_base: r.get(9)?,
                    nearest_expiry: r.get(10)?,
                    batch_count: r.get(11)?,
                    stock_value: Some((value_x100 + 50) / 100),
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub struct ProductHead {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub base_unit_name: String,
    pub is_deleted: bool,
}

pub fn product_head(conn: &Connection, id: i64) -> AppResult<Option<ProductHead>> {
    Ok(conn
        .query_row(
            "SELECT p.id, p.code, p.name, u.name, p.deleted_at IS NOT NULL
             FROM products p JOIN units u ON u.id = p.base_unit_id WHERE p.id = ?1",
            [id],
            |r| {
                Ok(ProductHead {
                    id: r.get(0)?,
                    code: r.get(1)?,
                    name: r.get(2)?,
                    base_unit_name: r.get(3)?,
                    is_deleted: r.get(4)?,
                })
            },
        )
        .optional()?)
}

fn batch_row(r: &rusqlite::Row) -> rusqlite::Result<BatchRow> {
    Ok(BatchRow {
        id: r.get(0)?,
        batch_number: r.get(1)?,
        expiry_date: r.get(2)?,
        qty_on_hand_base: r.get(3)?,
        unit_cost_x100: r.get(4)?,
        is_locked: r.get(5)?,
        lock_reason: r.get(6)?,
        source_type: r.get(7)?,
        created_at: r.get(8)?,
    })
}

const BATCH_COLUMNS: &str =
    "id, batch_number, expiry_date, qty_on_hand_base, unit_cost_x100, is_locked, lock_reason, source_type, created_at";

/// Batch obat, urut FEFO. `include_empty` = termasuk batch yang stoknya sudah habis.
pub fn product_batches(conn: &Connection, product_id: i64, include_empty: bool) -> AppResult<Vec<BatchRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {BATCH_COLUMNS} FROM batches
         WHERE product_id = ?1 AND (?2 = 1 OR qty_on_hand_base > 0)
         ORDER BY expiry_date, id"
    ))?;
    let rows = stmt.query_map(params![product_id, include_empty], batch_row)?.collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn get_batch(conn: &Connection, id: i64) -> AppResult<Option<(i64, BatchRow)>> {
    Ok(conn
        .query_row(&format!("SELECT {BATCH_COLUMNS}, product_id FROM batches WHERE id = ?1"), [id], |r| {
            Ok((r.get(9)?, batch_row(r)?))
        })
        .optional()?)
}

pub fn set_batch_locked(conn: &Connection, id: i64, locked: bool, reason: Option<&str>) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE batches SET is_locked = ?2, lock_reason = ?3, updated_at = datetime('now', 'localtime') WHERE id = ?1",
        params![id, locked, if locked { reason } else { None }],
    )?)
}

pub struct NewBatch<'a> {
    pub product_id: i64,
    pub batch_number: &'a str,
    pub expiry_date: &'a str,
    pub unit_cost_x100: i64,
    pub source_type: &'a str,
    pub source_id: i64,
}

pub fn insert_batch(conn: &Connection, b: &NewBatch<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO batches (product_id, batch_number, expiry_date, unit_cost_x100, source_type, source_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![b.product_id, b.batch_number, b.expiry_date, b.unit_cost_x100, b.source_type, b.source_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub struct NewMovement<'a> {
    pub batch_id: i64,
    pub product_id: i64,
    pub movement_type: &'a str,
    pub qty_change_base: i64,
    pub ref_type: &'a str,
    pub ref_id: i64,
    pub ref_line_id: Option<i64>,
    pub user_id: i64,
    pub note: Option<&'a str>,
}

/// Tambah baris kartu stok; trigger `stock_movements_apply` memperbarui stok batch (dan menolak
/// stok minus).
pub fn insert_movement(conn: &Connection, m: &NewMovement<'_>) -> AppResult<()> {
    conn.execute(
        "INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id,
                                      ref_line_id, user_id, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            m.batch_id, m.product_id, m.movement_type, m.qty_change_base, m.ref_type, m.ref_id,
            m.ref_line_id, m.user_id, m.note
        ],
    )?;
    Ok(())
}

// ─── Kartu stok ──────────────────────────────────────────────────────────────

/// Saldo berjalan dihitung dengan window function atas seluruh riwayat obat (atau satu batch),
/// lalu baru disaring per tanggal agar saldo tetap benar.
const CARD_BASE: &str = "
    WITH card AS (
        SELECT m.*, SUM(m.qty_change_base) OVER (ORDER BY m.id) AS balance
        FROM stock_movements m
        WHERE m.product_id = :product_id AND (:batch_id IS NULL OR m.batch_id = :batch_id)
    )";

const CARD_RANGE: &str = "(:date_from IS NULL OR c.created_at >= :date_from)
    AND (:date_to IS NULL OR c.created_at < date(:date_to, '+1 day'))";

pub struct CardTotals {
    pub total: i64,
    pub opening: i64,
    pub total_in: i64,
    pub total_out: i64,
}

pub fn stock_card_totals(conn: &Connection, q: &StockCardQuery) -> AppResult<CardTotals> {
    let sql = format!(
        "{CARD_BASE}
         SELECT
            (SELECT count(*) FROM card c WHERE {CARD_RANGE}),
            COALESCE((SELECT SUM(qty_change_base) FROM card c
                      WHERE :date_from IS NOT NULL AND c.created_at < :date_from), 0),
            COALESCE((SELECT SUM(qty_change_base) FROM card c WHERE {CARD_RANGE} AND qty_change_base > 0), 0),
            COALESCE((SELECT -SUM(qty_change_base) FROM card c WHERE {CARD_RANGE} AND qty_change_base < 0), 0)"
    );
    Ok(conn.query_row(
        &sql,
        named_params! {
            ":product_id": q.product_id, ":batch_id": q.batch_id,
            ":date_from": q.date_from, ":date_to": q.date_to,
        },
        |r| {
            Ok(CardTotals {
                total: r.get(0)?,
                opening: r.get(1)?,
                total_in: r.get(2)?,
                total_out: r.get(3)?,
            })
        },
    )?)
}

pub fn stock_card(conn: &Connection, q: &StockCardQuery) -> AppResult<Vec<StockCardRow>> {
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let sql = format!(
        "{CARD_BASE}
         SELECT c.id, c.created_at, c.movement_type, c.batch_id, b.batch_number, b.expiry_date,
                c.qty_change_base, c.balance, c.ref_type, c.ref_id,
                CASE c.ref_type WHEN 'stock_opname' THEN (SELECT number FROM stock_opnames o WHERE o.id = c.ref_id) END,
                u.username, c.note
         FROM card c
         JOIN batches b ON b.id = c.batch_id
         LEFT JOIN users u ON u.id = c.user_id
         WHERE {CARD_RANGE}
         ORDER BY c.id
         LIMIT :limit OFFSET :offset"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            named_params! {
                ":product_id": q.product_id, ":batch_id": q.batch_id,
                ":date_from": q.date_from, ":date_to": q.date_to,
                ":limit": limit, ":offset": offset,
            },
            |r| {
                Ok(StockCardRow {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    movement_type: r.get(2)?,
                    batch_id: r.get(3)?,
                    batch_number: r.get(4)?,
                    expiry_date: r.get(5)?,
                    qty_change_base: r.get(6)?,
                    balance_base: r.get(7)?,
                    ref_type: r.get(8)?,
                    ref_id: r.get(9)?,
                    ref_number: r.get(10)?,
                    username: r.get(11)?,
                    note: r.get(12)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

// ─── Stok opname ─────────────────────────────────────────────────────────────

const OPNAME_COLUMNS: &str = "
    o.id, o.number, o.opname_type, o.scope_note, o.status,
    (SELECT count(*) FROM stock_opname_items i WHERE i.opname_id = o.id),
    (SELECT count(*) FROM stock_opname_items i WHERE i.opname_id = o.id AND i.physical_qty_base IS NOT NULL),
    o.created_at, cu.username, o.approved_at, au.username";

const OPNAME_FROM: &str = "
    FROM stock_opnames o
    LEFT JOIN users cu ON cu.id = o.created_by
    LEFT JOIN users au ON au.id = o.approved_by";

fn opname_row(r: &rusqlite::Row) -> rusqlite::Result<OpnameRow> {
    Ok(OpnameRow {
        id: r.get(0)?,
        number: r.get(1)?,
        opname_type: r.get(2)?,
        scope_note: r.get(3)?,
        status: r.get(4)?,
        item_count: r.get(5)?,
        counted_count: r.get(6)?,
        created_at: r.get(7)?,
        created_by: r.get(8)?,
        approved_at: r.get(9)?,
        approved_by: r.get(10)?,
    })
}

pub fn page_opnames(conn: &Connection, q: &OpnamePageQuery) -> AppResult<(Vec<OpnameRow>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let status = q.status.map(OpnameStatus::as_str);
    let filter = "WHERE (:status IS NULL OR o.status = :status)
                    AND (:like IS NULL OR o.number LIKE :like ESCAPE '\\' OR o.scope_note LIKE :like ESCAPE '\\')";
    let total: i64 = conn.query_row(
        &format!("SELECT count(*) {OPNAME_FROM} {filter}"),
        named_params! { ":status": status, ":like": like },
        |r| r.get(0),
    )?;
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let mut stmt = conn.prepare(&format!(
        "SELECT {OPNAME_COLUMNS} {OPNAME_FROM} {filter} ORDER BY o.id DESC LIMIT :limit OFFSET :offset"
    ))?;
    let rows = stmt
        .query_map(
            named_params! { ":status": status, ":like": like, ":limit": limit, ":offset": offset },
            opname_row,
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn get_opname(conn: &Connection, id: i64) -> AppResult<Option<OpnameRow>> {
    Ok(conn
        .query_row(&format!("SELECT {OPNAME_COLUMNS} {OPNAME_FROM} WHERE o.id = ?1"), [id], opname_row)
        .optional()?)
}

pub fn insert_opname(conn: &Connection, number: &str, t: OpnameType, scope_note: Option<&str>, user_id: i64) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO stock_opnames (number, opname_type, scope_note, created_by) VALUES (?1, ?2, ?3, ?4)",
        params![number, t, scope_note, user_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_opname_status(conn: &Connection, id: i64, status: OpnameStatus) -> AppResult<()> {
    conn.execute(
        "UPDATE stock_opnames SET status = ?2, updated_at = datetime('now', 'localtime') WHERE id = ?1",
        params![id, status],
    )?;
    Ok(())
}

pub fn approve_opname(conn: &Connection, id: i64, user_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE stock_opnames SET status = 'APPROVED', approved_by = ?2,
                approved_at = datetime('now', 'localtime'), updated_at = datetime('now', 'localtime')
         WHERE id = ?1",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn opname_items(conn: &Connection, opname_id: i64) -> AppResult<Vec<OpnameItem>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.product_id, p.code, p.name, u.name, i.batch_id,
                COALESCE(b.batch_number, i.batch_number), COALESCE(b.expiry_date, i.expiry_date),
                COALESCE(i.unit_cost_x100, b.unit_cost_x100),
                i.system_qty_base, i.physical_qty_base, i.note
         FROM stock_opname_items i
         JOIN products p ON p.id = i.product_id
         JOIN units u ON u.id = p.base_unit_id
         LEFT JOIN batches b ON b.id = i.batch_id
         WHERE i.opname_id = ?1
         ORDER BY p.name COLLATE NOCASE, COALESCE(b.expiry_date, i.expiry_date), i.id",
    )?;
    let rows = stmt
        .query_map([opname_id], |r| {
            let cost: Option<i64> = r.get(8)?;
            Ok(OpnameItem {
                id: r.get(0)?,
                product_id: r.get(1)?,
                product_code: r.get(2)?,
                product_name: r.get(3)?,
                base_unit_name: r.get(4)?,
                batch_id: r.get(5)?,
                batch_number: r.get(6)?,
                expiry_date: r.get(7)?,
                unit_cost_x100: cost,
                has_cost: cost.is_some(),
                system_qty_base: r.get(9)?,
                physical_qty_base: r.get(10)?,
                note: r.get(11)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub struct ItemRow {
    pub id: i64,
    pub product_id: i64,
    pub batch_id: Option<i64>,
    pub batch_number: Option<String>,
    pub expiry_date: Option<String>,
    pub unit_cost_x100: Option<i64>,
    pub system_qty_base: i64,
    pub physical_qty_base: Option<i64>,
    pub note: Option<String>,
}

fn item_row(r: &rusqlite::Row) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        id: r.get(0)?,
        product_id: r.get(1)?,
        batch_id: r.get(2)?,
        batch_number: r.get(3)?,
        expiry_date: r.get(4)?,
        unit_cost_x100: r.get(5)?,
        system_qty_base: r.get(6)?,
        physical_qty_base: r.get(7)?,
        note: r.get(8)?,
    })
}

const ITEM_COLUMNS: &str =
    "id, product_id, batch_id, batch_number, expiry_date, unit_cost_x100, system_qty_base, physical_qty_base, note";

pub fn raw_items(conn: &Connection, opname_id: i64) -> AppResult<Vec<ItemRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {ITEM_COLUMNS} FROM stock_opname_items WHERE opname_id = ?1 ORDER BY id"
    ))?;
    let rows = stmt.query_map([opname_id], item_row)?.collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn get_item(conn: &Connection, opname_id: i64, id: i64) -> AppResult<Option<ItemRow>> {
    Ok(conn
        .query_row(
            &format!("SELECT {ITEM_COLUMNS} FROM stock_opname_items WHERE opname_id = ?1 AND id = ?2"),
            [opname_id, id],
            item_row,
        )
        .optional()?)
}

pub fn batch_in_opname(conn: &Connection, opname_id: i64, batch_id: i64, except_id: Option<i64>) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM stock_opname_items
                        WHERE opname_id = ?1 AND batch_id = ?2 AND (?3 IS NULL OR id <> ?3))",
        params![opname_id, batch_id, except_id],
        |r| r.get(0),
    )?)
}

pub struct ItemFields<'a> {
    pub product_id: i64,
    pub batch_id: Option<i64>,
    pub batch_number: Option<&'a str>,
    pub expiry_date: Option<&'a str>,
    pub unit_cost_x100: Option<i64>,
    pub system_qty_base: i64,
    pub physical_qty_base: Option<i64>,
    pub note: Option<&'a str>,
}

pub fn insert_item(conn: &Connection, opname_id: i64, f: &ItemFields<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO stock_opname_items (opname_id, product_id, batch_id, batch_number, expiry_date, unit_cost_x100,
                                         system_qty_base, physical_qty_base, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            opname_id, f.product_id, f.batch_id, f.batch_number, f.expiry_date, f.unit_cost_x100,
            f.system_qty_base, f.physical_qty_base, f.note
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_item(conn: &Connection, id: i64, f: &ItemFields<'_>) -> AppResult<()> {
    conn.execute(
        "UPDATE stock_opname_items
         SET product_id = ?2, batch_id = ?3, batch_number = ?4, expiry_date = ?5, unit_cost_x100 = ?6,
             system_qty_base = ?7, physical_qty_base = ?8, note = ?9, updated_at = datetime('now', 'localtime')
         WHERE id = ?1",
        params![
            id, f.product_id, f.batch_id, f.batch_number, f.expiry_date, f.unit_cost_x100,
            f.system_qty_base, f.physical_qty_base, f.note
        ],
    )?;
    Ok(())
}

pub fn delete_item(conn: &Connection, opname_id: i64, id: i64) -> AppResult<usize> {
    Ok(conn.execute("DELETE FROM stock_opname_items WHERE opname_id = ?1 AND id = ?2", [opname_id, id])?)
}

/// Batch baru hasil approve dicatat balik ke baris opname.
pub fn link_item_batch(conn: &Connection, id: i64, batch_id: i64) -> AppResult<()> {
    conn.execute("UPDATE stock_opname_items SET batch_id = ?2 WHERE id = ?1", [id, batch_id])?;
    Ok(())
}

/// Batch ber-stok dari obat aktif dalam cakupan yang belum ada di opname.
pub fn fill_candidates(
    conn: &Connection,
    opname_id: i64,
    product_id: Option<i64>,
    rack_id: Option<i64>,
    category_id: Option<i64>,
) -> AppResult<Vec<(i64, i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.product_id, b.qty_on_hand_base
         FROM batches b
         JOIN products p ON p.id = b.product_id
         WHERE b.qty_on_hand_base > 0 AND p.deleted_at IS NULL
           AND (:product_id IS NULL OR p.id = :product_id)
           AND (:rack_id IS NULL OR p.rack_id = :rack_id)
           AND (:category_id IS NULL OR p.category_id = :category_id)
           AND b.id NOT IN (SELECT batch_id FROM stock_opname_items WHERE opname_id = :opname_id AND batch_id IS NOT NULL)
         ORDER BY p.name COLLATE NOCASE, b.expiry_date, b.id",
    )?;
    let rows = stmt
        .query_map(
            named_params! {
                ":product_id": product_id, ":rack_id": rack_id, ":category_id": category_id, ":opname_id": opname_id,
            },
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}
