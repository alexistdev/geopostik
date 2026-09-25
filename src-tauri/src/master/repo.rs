use rusqlite::{Connection, OptionalExtension, params};

use super::model::{
    Category, DrugClass, MasterPageQuery, NamedItem, PriceMode, PriceTierDetail, ProductListQuery, ProductListRow, Unit,
};
use crate::error::AppResult;

// ─── Kategori & satuan ───────────────────────────────────────────────────────

/// Kolom tanggal dibuat dan username pembuat data master di tabel `table`.
fn created_columns(table: &str) -> String {
    format!("created_at, (SELECT u.username FROM users u WHERE u.id = {table}.created_by)")
}

fn category_row(r: &rusqlite::Row) -> rusqlite::Result<Category> {
    Ok(Category {
        id: r.get(0)?,
        code: r.get(1)?,
        name: r.get(2)?,
        margin_bp: r.get(3)?,
        is_active: r.get(4)?,
        created_at: r.get(5)?,
        created_by: r.get(6)?,
    })
}

pub fn list_categories(conn: &Connection) -> AppResult<Vec<Category>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT id, code, name, margin_bp, is_active, {} FROM categories
         WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE",
        created_columns("categories")
    ))?;
    let rows = stmt.query_map([], category_row)?.collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Filter pencarian halaman Master Data: nama atau kode mengandung teks (tanpa beda huruf besar/kecil).
const MASTER_PAGE_FILTER: &str = "deleted_at IS NULL
      AND (:like IS NULL OR name LIKE :like ESCAPE '\\' OR code LIKE :like ESCAPE '\\')";

/// Pola LIKE `%teks%` dengan karakter khusus LIKE di-escape; `None` bila teks kosong.
pub fn like_pattern(q: Option<&str>) -> Option<String> {
    let q = q.map(str::trim).filter(|q| !q.is_empty())?;
    let escaped = q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    Some(format!("%{escaped}%"))
}

fn page_bounds(q: &MasterPageQuery) -> (i64, i64) {
    (q.limit.clamp(1, 500), q.offset.max(0))
}

pub fn page_categories(conn: &Connection, q: &MasterPageQuery) -> AppResult<(Vec<Category>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let (limit, offset) = page_bounds(q);
    let total: i64 = conn.query_row(
        &format!("SELECT count(*) FROM categories WHERE {MASTER_PAGE_FILTER}"),
        rusqlite::named_params! { ":like": like },
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT id, code, name, margin_bp, is_active, {} FROM categories WHERE {MASTER_PAGE_FILTER}
         ORDER BY name COLLATE NOCASE, id LIMIT :limit OFFSET :offset",
        created_columns("categories")
    ))?;
    let rows = stmt
        .query_map(rusqlite::named_params! { ":like": like, ":limit": limit, ":offset": offset }, category_row)?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn category_margin(conn: &Connection, id: i64) -> AppResult<Option<Option<i64>>> {
    Ok(conn
        .query_row(
            "SELECT margin_bp FROM categories WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn insert_category(
    conn: &Connection,
    code: &str,
    name: &str,
    margin_bp: Option<i64>,
    is_active: bool,
    created_by: i64,
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO categories (code, name, margin_bp, is_active, created_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![code, name, margin_bp, is_active, created_by],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_category(conn: &Connection, id: i64, name: &str, margin_bp: Option<i64>, is_active: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE categories SET name = ?2, margin_bp = ?3, is_active = ?4,
                               updated_at = datetime('now', 'localtime')
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, name, margin_bp, is_active],
    )?;
    Ok(())
}

pub fn category_name_taken(conn: &Connection, name: &str, except_id: Option<i64>) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM categories
                        WHERE name = ?1 COLLATE NOCASE AND id IS NOT ?2 AND deleted_at IS NULL)",
        params![name, except_id],
        |r| r.get(0),
    )?)
}

pub fn products_in_category(conn: &Connection, category_id: i64) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT id FROM products WHERE category_id = ?1")?;
    let ids = stmt.query_map([category_id], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(ids)
}

// ─── Kode master (kategori, rak, pabrik) ─────────────────────────────────────

/// Tabel master yang punya kolom `code`. Nama tabel berasal dari konstanta, bukan input user.
#[derive(Debug, Clone, Copy)]
pub enum CodedTable {
    Categories,
    Racks,
    Manufacturers,
}

impl CodedTable {
    fn table(self) -> &'static str {
        match self {
            CodedTable::Categories => "categories",
            CodedTable::Racks => "racks",
            CodedTable::Manufacturers => "manufacturers",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            CodedTable::Categories => "KTG",
            CodedTable::Racks => "RAK",
            CodedTable::Manufacturers => "PBR",
        }
    }

    /// Kolom di `products` yang menunjuk ke tabel ini.
    fn product_column(self) -> &'static str {
        match self {
            CodedTable::Categories => "category_id",
            CodedTable::Racks => "rack_id",
            CodedTable::Manufacturers => "manufacturer_id",
        }
    }
}

pub fn set_master_active(conn: &Connection, t: CodedTable, id: i64, active: bool) -> AppResult<usize> {
    Ok(conn.execute(
        &format!(
            "UPDATE {} SET is_active = ?2, updated_at = datetime('now', 'localtime')
             WHERE id = ?1 AND deleted_at IS NULL",
            t.table()
        ),
        params![id, active],
    )?)
}

/// Jumlah obat (aktif maupun nonaktif) yang memakai data master ini.
pub fn master_usage(conn: &Connection, t: CodedTable, id: i64) -> AppResult<i64> {
    Ok(conn.query_row(
        &format!("SELECT count(*) FROM products WHERE {} = ?1", t.product_column()),
        [id],
        |r| r.get(0),
    )?)
}

/// Soft delete: data ditandai terhapus dan disembunyikan, tidak pernah dihapus permanen.
pub fn soft_delete_master(conn: &Connection, t: CodedTable, id: i64, user_id: i64) -> AppResult<usize> {
    Ok(conn.execute(
        &format!(
            "UPDATE {} SET deleted_at = datetime('now', 'localtime'), deleted_by = ?2, is_active = 0,
                          updated_at = datetime('now', 'localtime')
             WHERE id = ?1 AND deleted_at IS NULL",
            t.table()
        ),
        params![id, user_id],
    )?)
}

pub fn code_of(conn: &Connection, t: CodedTable, id: i64) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            &format!("SELECT code FROM {} WHERE id = ?1 AND deleted_at IS NULL", t.table()),
            [id],
            |r| r.get(0),
        )
        .optional()?)
}

/// Kode otomatis berikutnya, misal RAK0001. Kode yang pernah dipakai (termasuk milik data yang
/// sudah dihapus) dilewati, agar label lama yang masih tertempel tidak menunjuk ke data lain.
/// Kode master selalu dibuat di sini, tidak pernah diisi manusia.
pub fn next_code(conn: &Connection, t: CodedTable) -> AppResult<String> {
    let mut n: i64 = conn.query_row(
        &format!("SELECT COALESCE(MAX(id), 0) + 1 FROM {}", t.table()),
        [],
        |r| r.get(0),
    )?;
    loop {
        let code = format!("{}{n:04}", t.prefix());
        let ever_used: bool = conn.query_row(
            &format!("SELECT EXISTS (SELECT 1 FROM {} WHERE code = ?1 COLLATE NOCASE)", t.table()),
            [&code],
            |r| r.get(0),
        )?;
        if !ever_used {
            return Ok(code);
        }
        n += 1;
    }
}

// ─── Rak & pabrik ────────────────────────────────────────────────────────────

/// Tabel master yang hanya berisi nama. Nama tabel berasal dari konstanta, bukan input user.
#[derive(Debug, Clone, Copy)]
pub enum NamedTable {
    Racks,
    Manufacturers,
}

impl NamedTable {
    fn table(self) -> &'static str {
        match self {
            NamedTable::Racks => "racks",
            NamedTable::Manufacturers => "manufacturers",
        }
    }

    pub fn coded(self) -> CodedTable {
        match self {
            NamedTable::Racks => CodedTable::Racks,
            NamedTable::Manufacturers => CodedTable::Manufacturers,
        }
    }
}

fn named_row(r: &rusqlite::Row) -> rusqlite::Result<NamedItem> {
    Ok(NamedItem {
        id: r.get(0)?,
        code: r.get(1)?,
        name: r.get(2)?,
        is_active: r.get(3)?,
        created_at: r.get(4)?,
        created_by: r.get(5)?,
    })
}

pub fn list_named(conn: &Connection, t: NamedTable) -> AppResult<Vec<NamedItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT id, code, name, is_active, {} FROM {} WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE",
        created_columns(t.table()),
        t.table()
    ))?;
    let rows = stmt.query_map([], named_row)?.collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn get_named(conn: &Connection, t: NamedTable, id: i64) -> AppResult<Option<NamedItem>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT id, code, name, is_active, {} FROM {} WHERE id = ?1 AND deleted_at IS NULL",
                created_columns(t.table()),
                t.table()
            ),
            [id],
            named_row,
        )
        .optional()?)
}

pub fn page_named(conn: &Connection, t: NamedTable, q: &MasterPageQuery) -> AppResult<(Vec<NamedItem>, i64)> {
    let like = like_pattern(q.q.as_deref());
    let (limit, offset) = page_bounds(q);
    let total: i64 = conn.query_row(
        &format!("SELECT count(*) FROM {} WHERE {MASTER_PAGE_FILTER}", t.table()),
        rusqlite::named_params! { ":like": like },
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT id, code, name, is_active, {} FROM {} WHERE {MASTER_PAGE_FILTER}
         ORDER BY name COLLATE NOCASE, id LIMIT :limit OFFSET :offset",
        created_columns(t.table()),
        t.table()
    ))?;
    let rows = stmt
        .query_map(rusqlite::named_params! { ":like": like, ":limit": limit, ":offset": offset }, named_row)?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

pub fn named_exists(conn: &Connection, t: NamedTable, id: i64) -> AppResult<bool> {
    Ok(conn.query_row(
        &format!("SELECT EXISTS (SELECT 1 FROM {} WHERE id = ?1 AND deleted_at IS NULL)", t.table()),
        [id],
        |r| r.get(0),
    )?)
}

pub fn named_name_taken(conn: &Connection, t: NamedTable, name: &str, except_id: Option<i64>) -> AppResult<bool> {
    Ok(conn.query_row(
        &format!(
            "SELECT EXISTS (SELECT 1 FROM {}
                            WHERE name = ?1 COLLATE NOCASE AND id IS NOT ?2 AND deleted_at IS NULL)",
            t.table()
        ),
        params![name, except_id],
        |r| r.get(0),
    )?)
}

pub fn insert_named(conn: &Connection, t: NamedTable, code: &str, name: &str, is_active: bool, created_by: i64) -> AppResult<i64> {
    conn.execute(
        &format!("INSERT INTO {} (code, name, is_active, created_by) VALUES (?1, ?2, ?3, ?4)", t.table()),
        params![code, name, is_active, created_by],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_named(conn: &Connection, t: NamedTable, id: i64, name: &str, is_active: bool) -> AppResult<usize> {
    Ok(conn.execute(
        &format!(
            "UPDATE {} SET name = ?2, is_active = ?3, updated_at = datetime('now', 'localtime')
             WHERE id = ?1 AND deleted_at IS NULL",
            t.table()
        ),
        params![id, name, is_active],
    )?)
}

pub fn list_units(conn: &Connection) -> AppResult<Vec<Unit>> {
    let mut stmt = conn.prepare("SELECT id, name FROM units ORDER BY name")?;
    let rows = stmt
        .query_map([], |r| Ok(Unit { id: r.get(0)?, name: r.get(1)? }))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn unit_exists(conn: &Connection, id: i64) -> AppResult<bool> {
    Ok(conn.query_row("SELECT EXISTS (SELECT 1 FROM units WHERE id = ?1)", [id], |r| r.get(0))?)
}

pub fn unit_name_taken(conn: &Connection, name: &str) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM units WHERE name = ?1 COLLATE NOCASE)",
        [name],
        |r| r.get(0),
    )?)
}

pub fn insert_unit(conn: &Connection, name: &str) -> AppResult<i64> {
    conn.execute("INSERT INTO units (name) VALUES (?1)", [name])?;
    Ok(conn.last_insert_rowid())
}

// ─── Daftar obat ─────────────────────────────────────────────────────────────

/// Ubah teks pencarian menjadi query FTS5: setiap kata dicari sebagai awalan, semua kata wajib ada.
pub fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// `{text}` diganti klausa pencarian (lihat `list_products`).
const PRODUCT_FILTER: &str = "
    FROM products p
    JOIN units bu ON bu.id = p.base_unit_id
    LEFT JOIN categories c ON c.id = p.category_id
    LEFT JOIN product_units pu ON pu.product_id = p.id AND pu.is_default_sale = 1 AND pu.is_active = 1
    LEFT JOIN units su ON su.id = pu.unit_id
    LEFT JOIN racks r ON r.id = p.rack_id
    WHERE {text}
      AND (:category_id IS NULL OR p.category_id = :category_id)
      AND (:drug_class IS NULL OR p.drug_class = :drug_class)
      AND (:include_inactive = 1 OR p.is_active = 1)";

pub fn list_products(conn: &Connection, q: &ProductListQuery) -> AppResult<(Vec<ProductListRow>, i64)> {
    let text = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let fts = text.and_then(fts_query);
    let barcode_clause = "p.id IN (SELECT xpu.product_id FROM product_barcodes b
                                   JOIN product_units xpu ON xpu.id = b.product_unit_id
                                   WHERE b.barcode = :barcode AND b.deleted_at IS NULL)";
    // Teks tanpa huruf/angka tidak dicari lewat FTS, tapi tetap dicek sebagai barcode.
    let text_clause = match (text, &fts) {
        (None, _) => "(:barcode IS NULL AND :fts IS NULL)".to_owned(),
        (Some(_), Some(_)) => format!(
            "(p.id IN (SELECT rowid FROM products_fts WHERE products_fts MATCH :fts) OR {barcode_clause})"
        ),
        (Some(_), None) => format!("(:fts IS NULL AND {barcode_clause})"),
    };
    let filter = PRODUCT_FILTER.replace("{text}", &text_clause);
    let drug_class = q.drug_class.map(DrugClass::as_str);
    let limit = q.limit.clamp(1, 500);
    let offset = q.offset.max(0);

    let total: i64 = conn.query_row(
        &format!("SELECT count(*) {filter}"),
        rusqlite::named_params! {
            ":fts": fts, ":barcode": text,
            ":category_id": q.category_id, ":drug_class": drug_class,
            ":include_inactive": q.include_inactive,
        },
        |r| r.get(0),
    )?;

    let sql = format!(
        "SELECT p.id, p.code, p.name, p.generic_name, c.name, p.drug_class, p.is_owa, bu.name,
                su.name, pu.sell_price,
                COALESCE((SELECT SUM(qty_on_hand_base) FROM batches b WHERE b.product_id = p.id), 0),
                p.min_stock_base, r.name, p.is_active
         {filter}
         ORDER BY p.name COLLATE NOCASE, p.id
         LIMIT :limit OFFSET :offset"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::named_params! {
                ":fts": fts, ":barcode": text,
                ":category_id": q.category_id, ":drug_class": drug_class,
                ":include_inactive": q.include_inactive,
                ":limit": limit, ":offset": offset,
            },
            |r| {
                Ok(ProductListRow {
                    id: r.get(0)?,
                    code: r.get(1)?,
                    name: r.get(2)?,
                    generic_name: r.get(3)?,
                    category_name: r.get(4)?,
                    drug_class: r.get(5)?,
                    is_owa: r.get(6)?,
                    base_unit_name: r.get(7)?,
                    sale_unit_name: r.get(8)?,
                    sale_price: r.get(9)?,
                    stock_base: r.get(10)?,
                    min_stock_base: r.get(11)?,
                    rack_name: r.get(12)?,
                    is_active: r.get(13)?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok((rows, total))
}

// ─── Obat ────────────────────────────────────────────────────────────────────

pub struct ProductRow {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub manufacturer_id: Option<i64>,
    pub category_id: Option<i64>,
    pub drug_class: DrugClass,
    pub is_owa: bool,
    pub base_unit_id: i64,
    pub min_stock_base: i64,
    pub rack_id: Option<i64>,
    pub margin_bp: Option<i64>,
    pub last_cost_x100: Option<i64>,
    pub is_active: bool,
}

pub fn find_product(conn: &Connection, id: i64) -> AppResult<Option<ProductRow>> {
    Ok(conn
        .query_row(
            "SELECT id, code, name, generic_name, manufacturer_id, category_id, drug_class, is_owa,
                    base_unit_id, min_stock_base, rack_id, margin_bp, last_cost_x100, is_active
             FROM products WHERE id = ?1",
            [id],
            |r| {
                Ok(ProductRow {
                    id: r.get(0)?,
                    code: r.get(1)?,
                    name: r.get(2)?,
                    generic_name: r.get(3)?,
                    manufacturer_id: r.get(4)?,
                    category_id: r.get(5)?,
                    drug_class: r.get(6)?,
                    is_owa: r.get(7)?,
                    base_unit_id: r.get(8)?,
                    min_stock_base: r.get(9)?,
                    rack_id: r.get(10)?,
                    margin_bp: r.get(11)?,
                    last_cost_x100: r.get(12)?,
                    is_active: r.get(13)?,
                })
            },
        )
        .optional()?)
}

pub fn product_has_stock(conn: &Connection, product_id: i64) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM batches WHERE product_id = ?1)",
        [product_id],
        |r| r.get(0),
    )?)
}

pub fn product_code_taken(conn: &Connection, code: &str, except_id: Option<i64>) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM products WHERE code = ?1 COLLATE NOCASE AND id IS NOT ?2)",
        params![code, except_id],
        |r| r.get(0),
    )?)
}

/// Kode otomatis berikutnya: OBT00001, OBT00002, ... (dilewati bila sudah dipakai manual).
pub fn next_product_code(conn: &Connection) -> AppResult<String> {
    let mut n: i64 = conn.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM products", [], |r| r.get(0))?;
    loop {
        let code = format!("OBT{n:05}");
        if !product_code_taken(conn, &code, None)? {
            return Ok(code);
        }
        n += 1;
    }
}

pub struct ProductFields<'a> {
    pub code: &'a str,
    pub name: &'a str,
    pub generic_name: Option<&'a str>,
    pub manufacturer_id: Option<i64>,
    pub category_id: Option<i64>,
    pub drug_class: DrugClass,
    pub is_owa: bool,
    pub base_unit_id: i64,
    pub min_stock_base: i64,
    pub rack_id: Option<i64>,
}

pub fn insert_product(conn: &Connection, f: &ProductFields<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO products (code, name, generic_name, manufacturer_id, category_id, drug_class, is_owa,
                               base_unit_id, min_stock_base, rack_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            f.code, f.name, f.generic_name, f.manufacturer_id, f.category_id, f.drug_class, f.is_owa,
            f.base_unit_id, f.min_stock_base, f.rack_id
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_product(conn: &Connection, id: i64, f: &ProductFields<'_>) -> AppResult<()> {
    conn.execute(
        "UPDATE products SET code = ?2, name = ?3, generic_name = ?4, manufacturer_id = ?5, category_id = ?6,
                             drug_class = ?7, is_owa = ?8, base_unit_id = ?9, min_stock_base = ?10,
                             rack_id = ?11, updated_at = datetime('now', 'localtime')
         WHERE id = ?1",
        params![
            id, f.code, f.name, f.generic_name, f.manufacturer_id, f.category_id, f.drug_class, f.is_owa,
            f.base_unit_id, f.min_stock_base, f.rack_id
        ],
    )?;
    Ok(())
}

pub fn set_product_active(conn: &Connection, id: i64, active: bool) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE products SET is_active = ?2, updated_at = datetime('now', 'localtime') WHERE id = ?1",
        params![id, active],
    )?)
}

pub fn update_product_pricing(conn: &Connection, id: i64, margin_bp: Option<i64>, last_cost_x100: Option<i64>) -> AppResult<()> {
    conn.execute(
        "UPDATE products SET margin_bp = ?2, last_cost_x100 = ?3, updated_at = datetime('now', 'localtime')
         WHERE id = ?1",
        params![id, margin_bp, last_cost_x100],
    )?;
    Ok(())
}

// ─── Satuan obat ─────────────────────────────────────────────────────────────

pub struct ProductUnitRow {
    pub id: i64,
    pub unit_id: i64,
    pub unit_name: String,
    pub conversion: i64,
    pub sell_price: i64,
    pub price_mode: PriceMode,
    pub is_default_sale: bool,
    pub is_active: bool,
}

pub fn product_units(conn: &Connection, product_id: i64) -> AppResult<Vec<ProductUnitRow>> {
    let mut stmt = conn.prepare(
        "SELECT pu.id, pu.unit_id, u.name, pu.conversion, pu.sell_price, pu.price_mode,
                pu.is_default_sale, pu.is_active
         FROM product_units pu JOIN units u ON u.id = pu.unit_id
         WHERE pu.product_id = ?1
         ORDER BY pu.is_active DESC, pu.conversion, pu.id",
    )?;
    let rows = stmt
        .query_map([product_id], |r| {
            Ok(ProductUnitRow {
                id: r.get(0)?,
                unit_id: r.get(1)?,
                unit_name: r.get(2)?,
                conversion: r.get(3)?,
                sell_price: r.get(4)?,
                price_mode: r.get(5)?,
                is_default_sale: r.get(6)?,
                is_active: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

pub fn insert_product_unit(conn: &Connection, product_id: i64, unit_id: i64, conversion: i64, is_default_sale: bool) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO product_units (product_id, unit_id, conversion, sell_price, price_mode, is_default_sale)
         VALUES (?1, ?2, ?3, 0, 'AUTO', ?4)",
        params![product_id, unit_id, conversion, is_default_sale],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_product_unit(conn: &Connection, id: i64, conversion: i64, is_default_sale: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE product_units SET conversion = ?2, is_default_sale = ?3, is_active = 1,
                                  updated_at = datetime('now', 'localtime')
         WHERE id = ?1",
        params![id, conversion, is_default_sale],
    )?;
    Ok(())
}

pub fn deactivate_product_unit(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE product_units SET is_active = 0, is_default_sale = 0,
                                  updated_at = datetime('now', 'localtime')
         WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn set_unit_price(conn: &Connection, id: i64, mode: PriceMode, price: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE product_units SET price_mode = ?2, sell_price = ?3, updated_at = datetime('now', 'localtime')
         WHERE id = ?1",
        params![id, mode, price],
    )?;
    Ok(())
}

// ─── Barcode ─────────────────────────────────────────────────────────────────

pub fn barcodes_of(conn: &Connection, product_unit_id: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT barcode FROM product_barcodes WHERE product_unit_id = ?1 AND deleted_at IS NULL ORDER BY id",
    )?;
    let rows = stmt.query_map([product_unit_id], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Nama obat lain yang sudah memakai barcode ini.
pub fn barcode_owner(conn: &Connection, barcode: &str, except_product_id: Option<i64>) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT p.name FROM product_barcodes b
             JOIN product_units pu ON pu.id = b.product_unit_id
             JOIN products p ON p.id = pu.product_id
             WHERE b.barcode = ?1 AND b.deleted_at IS NULL AND p.id IS NOT ?2",
            params![barcode, except_product_id],
            |r| r.get(0),
        )
        .optional()?)
}

pub struct BarcodeRow {
    pub id: i64,
    pub product_unit_id: i64,
    pub barcode: String,
}

/// Barcode aktif (belum dihapus) milik semua satuan obat ini.
pub fn active_product_barcodes(conn: &Connection, product_id: i64) -> AppResult<Vec<BarcodeRow>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.product_unit_id, b.barcode FROM product_barcodes b
         JOIN product_units pu ON pu.id = b.product_unit_id
         WHERE pu.product_id = ?1 AND b.deleted_at IS NULL",
    )?;
    let rows = stmt
        .query_map([product_id], |r| {
            Ok(BarcodeRow { id: r.get(0)?, product_unit_id: r.get(1)?, barcode: r.get(2)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Soft delete: barcode dilepas dari obat, riwayatnya tetap tersimpan.
pub fn soft_delete_barcode(conn: &Connection, id: i64, user_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE product_barcodes SET deleted_at = datetime('now', 'localtime'), deleted_by = ?2
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn insert_barcode(conn: &Connection, product_unit_id: i64, barcode: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO product_barcodes (product_unit_id, barcode) VALUES (?1, ?2)",
        params![product_unit_id, barcode],
    )?;
    Ok(())
}

// ─── Tier harga ──────────────────────────────────────────────────────────────

pub fn active_tiers(conn: &Connection, product_unit_id: i64) -> AppResult<Vec<PriceTierDetail>> {
    let mut stmt = conn.prepare(
        "SELECT id, min_qty, price_mode, margin_bp, price FROM price_tiers
         WHERE product_unit_id = ?1 AND is_active = 1 ORDER BY min_qty",
    )?;
    let rows = stmt
        .query_map([product_unit_id], |r| {
            Ok(PriceTierDetail {
                id: r.get(0)?,
                min_qty: r.get(1)?,
                price_mode: r.get(2)?,
                margin_bp: r.get(3)?,
                price: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Tier (aktif maupun tidak) milik satuan ini dengan `min_qty` tertentu.
pub fn find_tier_by_min_qty(conn: &Connection, product_unit_id: i64, min_qty: i64) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM price_tiers WHERE product_unit_id = ?1 AND min_qty = ?2",
            params![product_unit_id, min_qty],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn deactivate_tiers(conn: &Connection, product_unit_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE price_tiers SET is_active = 0, updated_at = datetime('now', 'localtime')
         WHERE product_unit_id = ?1 AND is_active = 1",
        [product_unit_id],
    )?;
    Ok(())
}

pub fn upsert_tier(
    conn: &Connection,
    product_unit_id: i64,
    min_qty: i64,
    mode: PriceMode,
    margin_bp: Option<i64>,
    price: i64,
) -> AppResult<()> {
    match find_tier_by_min_qty(conn, product_unit_id, min_qty)? {
        Some(id) => {
            conn.execute(
                "UPDATE price_tiers SET price_mode = ?2, margin_bp = ?3, price = ?4, is_active = 1,
                                        updated_at = datetime('now', 'localtime')
                 WHERE id = ?1",
                params![id, mode, margin_bp, price],
            )?;
        }
        None => {
            conn.execute(
                "INSERT INTO price_tiers (product_unit_id, min_qty, price_mode, margin_bp, price)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![product_unit_id, min_qty, mode, margin_bp, price],
            )?;
        }
    }
    Ok(())
}

pub fn set_tier_price(conn: &Connection, id: i64, price: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE price_tiers SET price = ?2, updated_at = datetime('now', 'localtime') WHERE id = ?1",
        params![id, price],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fts_query;

    #[test]
    fn builds_prefix_query() {
        assert_eq!(fts_query("para 500").as_deref(), Some("\"para\"* \"500\"*"));
        assert_eq!(fts_query("  amox-clav ").as_deref(), Some("\"amox\"* \"clav\"*"));
        assert_eq!(fts_query("\"*"), None);
    }
}
