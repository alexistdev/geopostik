use std::collections::HashSet;

use rusqlite::{Connection, TransactionBehavior};
use serde_json::json;

use super::model::{
    Category, CategoryInput, DrugClass, PriceMode, ProductDetail, ProductInput, ProductListQuery,
    ProductListResult, ProductPricesInput, ProductSaveResult, ProductUnitDetail, ProductUnitInput, Unit,
};
use super::pricing::{auto_price, unit_cost};
use super::repo::{self, ProductFields};
use crate::audit;
use crate::auth::{Permission, SessionUser};
use crate::error::{AppError, AppResult};
use crate::settings::{self, PriceSettings};

const MAX_MARGIN_BP: i64 = 100_000; // 1000%

/// Apa saja yang boleh dilihat/diubah user terkait harga.
#[derive(Debug, Clone, Copy)]
pub struct PriceAccess {
    pub view_cost: bool,
    pub manage_price: bool,
}

impl PriceAccess {
    pub fn of(user: &SessionUser) -> Self {
        Self {
            view_cost: user.permissions.contains(&Permission::ViewCost),
            manage_price: user.permissions.contains(&Permission::PriceManage),
        }
    }

    fn view_margin(self) -> bool {
        self.view_cost || self.manage_price
    }
}

// ─── Kategori & satuan ───────────────────────────────────────────────────────

pub fn list_categories(conn: &Connection, access: PriceAccess) -> AppResult<Vec<Category>> {
    let mut cats = repo::list_categories(conn)?;
    if !access.view_margin() {
        cats.iter_mut().for_each(|c| c.margin_bp = None);
    }
    Ok(cats)
}

pub fn save_category(conn: &mut Connection, user: &SessionUser, input: &CategoryInput) -> AppResult<Category> {
    let access = PriceAccess::of(user);
    let name = required(&input.name, "Nama kategori")?;
    validate_margin(input.margin_bp)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if repo::category_name_taken(&tx, name, input.id)? {
        return Err(AppError::Conflict(format!("Kategori \"{name}\" sudah ada")));
    }

    let (id, margin_changed) = match input.id {
        Some(id) => {
            let old = repo::category_margin(&tx, id)?
                .ok_or_else(|| AppError::NotFound("Kategori tidak ditemukan".into()))?;
            // User tanpa hak harga tidak melihat margin, jadi margin lama dipertahankan.
            let margin = if access.manage_price { input.margin_bp } else { old };
            repo::update_category(&tx, id, name, margin, input.is_active)?;
            (id, margin != old)
        }
        None => {
            if input.margin_bp.is_some() && !access.manage_price {
                return Err(AppError::Forbidden);
            }
            (repo::insert_category(&tx, name, input.margin_bp, input.is_active)?, false)
        }
    };

    if margin_changed {
        let price = settings::price(&tx)?;
        for product_id in repo::products_in_category(&tx, id)? {
            recalc_auto_prices(&tx, product_id, price)?;
        }
        audit::log(
            &tx,
            audit::Entry {
                user_id: Some(user.id),
                action: "CATEGORY_MARGIN_CHANGE",
                entity: Some("categories"),
                entity_id: Some(id),
                detail: Some(json!({ "marginBp": input.margin_bp })),
                ..Default::default()
            },
        )?;
    }
    tx.commit()?;

    list_categories(conn, access)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Internal("kategori hilang setelah disimpan".into()))
}

pub fn list_units(conn: &Connection) -> AppResult<Vec<Unit>> {
    repo::list_units(conn)
}

pub fn create_unit(conn: &Connection, name: &str) -> AppResult<Unit> {
    let name = required(name, "Nama satuan")?;
    if repo::unit_name_taken(conn, name)? {
        return Err(AppError::Conflict(format!("Satuan \"{name}\" sudah ada")));
    }
    let id = repo::insert_unit(conn, name)?;
    Ok(Unit { id, name: name.to_owned() })
}

// ─── Obat ────────────────────────────────────────────────────────────────────

pub fn list_products(conn: &Connection, query: &ProductListQuery) -> AppResult<ProductListResult> {
    let (rows, total) = repo::list_products(conn, query)?;
    Ok(ProductListResult { rows, total })
}

pub fn get_product(conn: &Connection, id: i64, access: PriceAccess) -> AppResult<ProductDetail> {
    let p = repo::find_product(conn, id)?.ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
    let price = settings::price(conn)?;
    let effective_margin = effective_margin(conn, p.margin_bp, p.category_id, price)?;

    let mut units = Vec::new();
    for u in repo::product_units(conn, id)? {
        let mut tiers = repo::active_tiers(conn, u.id)?;
        if !access.view_margin() {
            tiers.iter_mut().for_each(|t| t.margin_bp = None);
        }
        units.push(ProductUnitDetail {
            id: u.id,
            unit_id: u.unit_id,
            unit_name: u.unit_name,
            conversion: u.conversion,
            sell_price: u.sell_price,
            price_mode: u.price_mode,
            is_default_sale: u.is_default_sale,
            is_active: u.is_active,
            barcodes: repo::barcodes_of(conn, u.id)?,
            tiers,
        });
    }

    Ok(ProductDetail {
        id: p.id,
        code: p.code,
        name: p.name,
        generic_name: p.generic_name,
        manufacturer: p.manufacturer,
        category_id: p.category_id,
        drug_class: p.drug_class,
        is_owa: p.is_owa,
        base_unit_id: p.base_unit_id,
        min_stock_base: p.min_stock_base,
        rack_location: p.rack_location,
        is_active: p.is_active,
        has_stock: repo::product_has_stock(conn, id)?,
        margin_bp: p.margin_bp.filter(|_| access.view_margin()),
        effective_margin_bp: access.view_margin().then_some(effective_margin),
        last_cost_x100: p.last_cost_x100.filter(|_| access.view_cost),
        units,
    })
}

/// Simpan data obat, satuan, dan barcode (hak PRODUCT_MANAGE). Harga tidak diubah di sini,
/// kecuali harga otomatis dihitung ulang bila konversi berubah.
pub fn save_product(conn: &mut Connection, user: &SessionUser, input: &ProductInput) -> AppResult<ProductSaveResult> {
    let name = required(&input.name, "Nama obat")?;
    let code_input = input.code.as_deref().map(str::trim).filter(|c| !c.is_empty());
    if input.is_owa && input.drug_class != DrugClass::Hard {
        return Err(AppError::Validation("Tanda OWA hanya untuk obat keras".into()));
    }
    if input.min_stock_base < 0 {
        return Err(AppError::Validation("Stok minimal tidak boleh negatif".into()));
    }
    validate_units(input)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    if let Some(category_id) = input.category_id
        && repo::category_margin(&tx, category_id)?.is_none()
    {
        return Err(AppError::Validation("Kategori tidak ditemukan".into()));
    }
    for u in &input.units {
        if !repo::unit_exists(&tx, u.unit_id)? {
            return Err(AppError::Validation("Satuan tidak ditemukan".into()));
        }
    }
    if let Some(code) = code_input
        && repo::product_code_taken(&tx, code, input.id)?
    {
        return Err(AppError::Conflict(format!("Kode \"{code}\" sudah dipakai obat lain")));
    }
    for barcode in input.units.iter().flat_map(|u| &u.barcodes) {
        if let Some(owner) = repo::barcode_owner(&tx, barcode.trim(), input.id)? {
            return Err(AppError::Conflict(format!("Barcode {} sudah dipakai oleh {owner}", barcode.trim())));
        }
    }

    let code = match code_input {
        Some(c) => c.to_owned(),
        None => match input.id {
            Some(id) => repo::find_product(&tx, id)?
                .map(|p| p.code)
                .ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?,
            None => repo::next_product_code(&tx)?,
        },
    };
    let fields = ProductFields {
        code: &code,
        name,
        generic_name: optional(&input.generic_name),
        manufacturer: optional(&input.manufacturer),
        category_id: input.category_id,
        drug_class: input.drug_class,
        is_owa: input.is_owa,
        base_unit_id: input.base_unit_id,
        min_stock_base: input.min_stock_base,
        rack_location: optional(&input.rack_location),
    };

    let (product_id, action) = match input.id {
        None => (repo::insert_product(&tx, &fields)?, "PRODUCT_CREATE"),
        Some(id) => {
            let old = repo::find_product(&tx, id)?.ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
            if repo::product_has_stock(&tx, id)? && old.base_unit_id != input.base_unit_id {
                return Err(AppError::Validation(
                    "Satuan dasar tidak bisa diubah karena obat ini sudah punya stok".into(),
                ));
            }
            repo::update_product(&tx, id, &fields)?;
            (id, "PRODUCT_UPDATE")
        }
    };

    save_units(&tx, product_id, input)?;
    let price = settings::price(&tx)?;
    let mut warnings = recalc_auto_prices(&tx, product_id, price)?;
    if repo::product_units(&tx, product_id)?
        .iter()
        .any(|u| u.is_active && u.sell_price == 0)
    {
        warnings.push("Ada satuan yang harganya masih 0 dan belum bisa dijual".into());
    }

    audit::log(
        &tx,
        audit::Entry {
            user_id: Some(user.id),
            action,
            entity: Some("products"),
            entity_id: Some(product_id),
            detail: Some(json!({ "code": code, "name": name })),
            ..Default::default()
        },
    )?;
    tx.commit()?;

    Ok(ProductSaveResult {
        product: get_product(conn, product_id, PriceAccess::of(user))?,
        warnings,
    })
}

fn validate_units(input: &ProductInput) -> AppResult<()> {
    if input.units.is_empty() {
        return Err(AppError::Validation("Minimal satu satuan".into()));
    }
    let base: Vec<_> = input.units.iter().filter(|u| u.unit_id == input.base_unit_id).collect();
    if base.len() != 1 || base[0].conversion != 1 {
        return Err(AppError::Validation("Satuan dasar harus ada dengan isi 1".into()));
    }
    let mut unit_ids = HashSet::new();
    let mut conversions = HashSet::new();
    let mut barcodes = HashSet::new();
    for u in &input.units {
        if u.conversion < 1 {
            return Err(AppError::Validation("Isi satuan minimal 1".into()));
        }
        if !unit_ids.insert(u.unit_id) {
            return Err(AppError::Validation("Satuan yang sama tidak boleh dipakai dua kali".into()));
        }
        if !conversions.insert(u.conversion) {
            return Err(AppError::Validation("Dua satuan tidak boleh punya isi yang sama".into()));
        }
        for b in &u.barcodes {
            let b = b.trim();
            if b.is_empty() {
                return Err(AppError::Validation("Barcode tidak boleh kosong".into()));
            }
            if !barcodes.insert(b.to_owned()) {
                return Err(AppError::Validation(format!("Barcode {b} diisi lebih dari sekali")));
            }
        }
    }
    if input.units.iter().filter(|u| u.is_default_sale).count() != 1 {
        return Err(AppError::Validation("Pilih tepat satu satuan jual utama".into()));
    }
    Ok(())
}

fn save_units(conn: &Connection, product_id: i64, input: &ProductInput) -> AppResult<()> {
    let has_stock = repo::product_has_stock(conn, product_id)?;
    let existing = repo::product_units(conn, product_id)?;
    let mut kept = HashSet::new();

    for u in &input.units {
        let old = match_existing(&existing, u);
        if let Some(old) = old {
            if old.unit_id != u.unit_id {
                return Err(AppError::Validation(format!(
                    "Satuan {} tidak bisa diganti; hapus lalu tambahkan satuan baru",
                    old.unit_name
                )));
            }
            if has_stock && old.conversion != u.conversion {
                return Err(AppError::Validation(format!(
                    "Isi satuan {} tidak bisa diubah karena obat ini sudah punya stok",
                    old.unit_name
                )));
            }
            kept.insert(old.id);
        } else if u.id.is_some() {
            return Err(AppError::Validation("Satuan tidak ditemukan pada obat ini".into()));
        }
    }
    for e in existing.iter().filter(|e| !kept.contains(&e.id)) {
        repo::deactivate_product_unit(conn, e.id)?;
    }

    repo::delete_product_barcodes(conn, product_id)?;
    for u in &input.units {
        let pu_id = match match_existing(&existing, u) {
            Some(old) => {
                repo::update_product_unit(conn, old.id, u.conversion, u.is_default_sale)?;
                old.id
            }
            None => repo::insert_product_unit(conn, product_id, u.unit_id, u.conversion, u.is_default_sale)?,
        };
        for b in &u.barcodes {
            repo::insert_barcode(conn, pu_id, b.trim())?;
        }
    }
    Ok(())
}

/// Baris satuan lama yang cocok: lewat id, atau lewat jenis satuan yang sama (mengaktifkan kembali
/// satuan yang pernah dihapus).
fn match_existing<'a>(existing: &'a [repo::ProductUnitRow], u: &ProductUnitInput) -> Option<&'a repo::ProductUnitRow> {
    match u.id {
        Some(id) => existing.iter().find(|e| e.id == id),
        None => existing.iter().find(|e| e.unit_id == u.unit_id),
    }
}

pub fn set_product_active(conn: &Connection, user: &SessionUser, id: i64, active: bool) -> AppResult<()> {
    if repo::set_product_active(conn, id, active)? == 0 {
        return Err(AppError::NotFound("Obat tidak ditemukan".into()));
    }
    audit::log(
        conn,
        audit::Entry {
            user_id: Some(user.id),
            action: if active { "PRODUCT_ACTIVATE" } else { "PRODUCT_DEACTIVATE" },
            entity: Some("products"),
            entity_id: Some(id),
            ..Default::default()
        },
    )
}

// ─── Harga ───────────────────────────────────────────────────────────────────

/// Simpan margin, HPP acuan, harga per satuan, dan tier (hak PRICE_MANAGE).
pub fn save_prices(conn: &mut Connection, user: &SessionUser, input: &ProductPricesInput) -> AppResult<ProductSaveResult> {
    let access = PriceAccess::of(user);
    validate_margin(input.margin_bp)?;
    if input.last_cost_x100.is_some_and(|c| c < 0) {
        return Err(AppError::Validation("HPP tidak boleh negatif".into()));
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let product = repo::find_product(&tx, input.product_id)?
        .ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
    let units = repo::product_units(&tx, input.product_id)?;

    // User tanpa VIEW_COST tidak melihat HPP, jadi HPP lama dipertahankan.
    let last_cost = if access.view_cost { input.last_cost_x100 } else { product.last_cost_x100 };
    repo::update_product_pricing(&tx, product.id, input.margin_bp, last_cost)?;

    let price = settings::price(&tx)?;
    let margin = effective_margin(&tx, input.margin_bp, product.category_id, price)?;

    for up in &input.units {
        let unit = units
            .iter()
            .find(|u| u.id == up.product_unit_id && u.is_active)
            .ok_or_else(|| AppError::Validation("Satuan tidak ditemukan pada obat ini".into()))?;
        if up.price_mode == PriceMode::Manual && up.sell_price < 0 {
            return Err(AppError::Validation("Harga tidak boleh negatif".into()));
        }
        let sell_price = match (up.price_mode, last_cost) {
            (PriceMode::Manual, _) => up.sell_price,
            (PriceMode::Auto, Some(cost)) => auto_price(cost, unit.conversion, margin, price.rounding),
            (PriceMode::Auto, None) => unit.sell_price,
        };
        repo::set_unit_price(&tx, unit.id, up.price_mode, sell_price)?;

        // Tier: urut naik, harga harus turun seiring naiknya jumlah minimal.
        let mut tiers: Vec<_> = up.tiers.iter().collect();
        tiers.sort_by_key(|t| t.min_qty);
        let mut previous_price = sell_price;
        let mut seen = HashSet::new();
        repo::deactivate_tiers(&tx, unit.id)?;
        for t in tiers {
            if t.min_qty <= 1 {
                return Err(AppError::Validation(format!("Jumlah minimal tier {} harus lebih dari 1", unit.unit_name)));
            }
            if !seen.insert(t.min_qty) {
                return Err(AppError::Validation(format!("Tier {} dengan jumlah minimal {} diisi dua kali", unit.unit_name, t.min_qty)));
            }
            let tier_price = match t.price_mode {
                PriceMode::Manual => t.price,
                PriceMode::Auto => {
                    let tier_margin = t.margin_bp.ok_or_else(|| {
                        AppError::Validation(format!("Margin tier {} wajib diisi untuk harga otomatis", unit.unit_name))
                    })?;
                    validate_margin(Some(tier_margin))?;
                    last_cost.map_or(t.price, |cost| auto_price(cost, unit.conversion, tier_margin, price.rounding))
                }
            };
            if tier_price < 0 {
                return Err(AppError::Validation("Harga tier tidak boleh negatif".into()));
            }
            if tier_price >= previous_price {
                return Err(AppError::Validation(format!(
                    "Harga tier {} ≥ {} harus lebih murah dari harga sebelumnya",
                    unit.unit_name, t.min_qty
                )));
            }
            previous_price = tier_price;
            repo::upsert_tier(&tx, unit.id, t.min_qty, t.price_mode, t.margin_bp, tier_price)?;
        }
    }

    audit::log(
        &tx,
        audit::Entry {
            user_id: Some(user.id),
            action: "PRICE_CHANGE",
            entity: Some("products"),
            entity_id: Some(product.id),
            detail: Some(json!({
                "before": units.iter().map(|u| json!({ "unit": u.unit_name, "price": u.sell_price })).collect::<Vec<_>>(),
                "marginBp": input.margin_bp,
            })),
            ..Default::default()
        },
    )?;
    let warnings = cost_warnings(&tx, product.id)?;
    tx.commit()?;

    Ok(ProductSaveResult {
        product: get_product(conn, input.product_id, access)?,
        warnings,
    })
}

/// Hitung ulang semua harga `AUTO` (satuan & tier) dari HPP acuan dan margin.
/// Dipanggil saat konversi/margin berubah, dan nanti saat penerimaan barang.
pub fn recalc_auto_prices(conn: &Connection, product_id: i64, price: PriceSettings) -> AppResult<Vec<String>> {
    let Some(p) = repo::find_product(conn, product_id)? else {
        return Ok(Vec::new());
    };
    let Some(cost) = p.last_cost_x100 else {
        return Ok(Vec::new());
    };
    let margin = effective_margin(conn, p.margin_bp, p.category_id, price)?;
    for u in repo::product_units(conn, product_id)?.iter().filter(|u| u.is_active) {
        if u.price_mode == PriceMode::Auto {
            repo::set_unit_price(conn, u.id, PriceMode::Auto, auto_price(cost, u.conversion, margin, price.rounding))?;
        }
        for t in repo::active_tiers(conn, u.id)? {
            if let (PriceMode::Auto, Some(tier_margin)) = (t.price_mode, t.margin_bp) {
                repo::set_tier_price(conn, t.id, auto_price(cost, u.conversion, tier_margin, price.rounding))?;
            }
        }
    }
    cost_warnings(conn, product_id)
}

/// Peringatan harga jual / tier di bawah HPP.
fn cost_warnings(conn: &Connection, product_id: i64) -> AppResult<Vec<String>> {
    let Some(cost) = repo::find_product(conn, product_id)?.and_then(|p| p.last_cost_x100) else {
        return Ok(Vec::new());
    };
    let mut warnings = Vec::new();
    for u in repo::product_units(conn, product_id)?.iter().filter(|u| u.is_active) {
        let hpp = unit_cost(cost, u.conversion);
        if u.sell_price < hpp {
            warnings.push(format!("Harga per {} di bawah HPP", u.unit_name));
        }
        for t in repo::active_tiers(conn, u.id)? {
            if t.price < hpp {
                warnings.push(format!("Harga tier {} ≥ {} di bawah HPP", u.unit_name, t.min_qty));
            }
        }
    }
    Ok(warnings)
}

fn effective_margin(conn: &Connection, own: Option<i64>, category_id: Option<i64>, price: PriceSettings) -> AppResult<i64> {
    if let Some(m) = own {
        return Ok(m);
    }
    let category = match category_id {
        Some(id) => repo::category_margin(conn, id)?.flatten(),
        None => None,
    };
    Ok(category.unwrap_or(price.default_margin_bp))
}

// ─── Validasi umum ───────────────────────────────────────────────────────────

fn required<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    let v = value.trim();
    if v.is_empty() {
        return Err(AppError::Validation(format!("{label} wajib diisi")));
    }
    Ok(v)
}

fn optional(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

fn validate_margin(margin_bp: Option<i64>) -> AppResult<()> {
    if margin_bp.is_some_and(|m| !(0..=MAX_MARGIN_BP).contains(&m)) {
        return Err(AppError::Validation("Margin harus antara 0% dan 1000%".into()));
    }
    Ok(())
}
