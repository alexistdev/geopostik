use std::collections::HashSet;

use rusqlite::{Connection, TransactionBehavior};
use serde_json::{Map, Value, json};

use super::model::{
    Category, CategoryInput, CategoryPage, DrugClass, MasterKind, MasterPageQuery, NamedItem, NamedItemInput,
    NamedItemPage, PriceMode, ProductDetail, ProductInput, ProductListQuery,
    ProductListResult, BatchResult, ProductPricesInput, ProductSaveResult, ProductUnitDetail, ProductUnitInput, Unit,
};
use super::pricing::{auto_price, unit_cost};
use super::repo::{self, CodedTable, NamedTable, ProductFields};
use crate::audit::{self, Change};
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

pub fn page_categories(conn: &Connection, access: PriceAccess, query: &MasterPageQuery) -> AppResult<CategoryPage> {
    let (mut rows, total) = repo::page_categories(conn, query)?;
    let default_margin_bp = if access.view_margin() {
        Some(settings::price(conn)?.default_margin_bp)
    } else {
        rows.iter_mut().for_each(|c| c.margin_bp = None);
        None
    };
    Ok(CategoryPage { rows, total, default_margin_bp })
}

pub fn save_category(conn: &mut Connection, user: &SessionUser, input: &CategoryInput) -> AppResult<Category> {
    let access = PriceAccess::of(user);
    let name = required(&input.name, "Nama kategori")?;
    validate_margin(input.margin_bp)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if repo::category_name_taken(&tx, name, input.id)? {
        return Err(AppError::Conflict(format!("Kategori \"{name}\" sudah ada")));
    }

    let before = match input.id {
        Some(id) => Some(category_snapshot(&tx, id)?.ok_or_else(|| AppError::NotFound("Kategori tidak ditemukan".into()))?),
        None => None,
    };
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
            let code = repo::next_code(&tx, CodedTable::Categories)?;
            (repo::insert_category(&tx, &code, name, input.margin_bp, input.is_active, user.id)?, false)
        }
    };

    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if before.is_some() { audit::UPDATE } else { audit::CREATE },
            entity: "categories",
            entity_id: id,
            before,
            after: category_snapshot(&tx, id)?,
            reason: None,
        },
    )?;
    if margin_changed {
        // Harga otomatis obat di kategori ini ikut berubah: dicatat per obat.
        let price = settings::price(&tx)?;
        let reason = format!("Margin kategori {name} diubah");
        for product_id in repo::products_in_category(&tx, id)? {
            let before = product_snapshot(&tx, product_id)?;
            recalc_auto_prices(&tx, product_id, price)?;
            audit::log_change(
                &tx,
                Change {
                    user_id: user.id,
                    action: audit::PRICE_RECALC,
                    entity: "products",
                    entity_id: product_id,
                    before: Some(before),
                    after: Some(product_snapshot(&tx, product_id)?),
                    reason: Some(&reason),
                },
            )?;
        }
    }
    tx.commit()?;

    list_categories(conn, access)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Internal("kategori hilang setelah disimpan".into()))
}

pub fn list_named(conn: &Connection, table: NamedTable) -> AppResult<Vec<NamedItem>> {
    repo::list_named(conn, table)
}

pub fn page_named(conn: &Connection, table: NamedTable, query: &MasterPageQuery) -> AppResult<NamedItemPage> {
    let (rows, total) = repo::page_named(conn, table, query)?;
    Ok(NamedItemPage { rows, total })
}

/// Simpan rak atau pabrik.
pub fn save_named(conn: &Connection, user: &SessionUser, table: NamedTable, input: &NamedItemInput) -> AppResult<NamedItem> {
    let label = match table {
        NamedTable::Racks => "rak",
        NamedTable::Manufacturers => "pabrik",
    };
    let name = required(&input.name, &format!("Nama {label}"))?;
    let tx = conn.unchecked_transaction()?;
    if repo::named_name_taken(&tx, table, name, input.id)? {
        return Err(AppError::Conflict(format!("Nama {label} \"{name}\" sudah ada")));
    }
    let before = match input.id {
        Some(id) => Some(named_snapshot(&tx, table, id)?.ok_or_else(|| AppError::NotFound(format!("Data {label} tidak ditemukan")))?),
        None => None,
    };
    let id = match input.id {
        Some(id) => {
            repo::update_named(&tx, table, id, name, input.is_active)?;
            id
        }
        None => {
            let code = repo::next_code(&tx, table.coded())?;
            repo::insert_named(&tx, table, &code, name, input.is_active, user.id)?
        }
    };
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if before.is_some() { audit::UPDATE } else { audit::CREATE },
            entity: table.entity(),
            entity_id: id,
            before,
            after: named_snapshot(&tx, table, id)?,
            reason: None,
        },
    )?;
    tx.commit()?;
    repo::get_named(conn, table, id)?.ok_or_else(|| AppError::Internal(format!("data {label} hilang setelah disimpan")))
}

// ─── Aksi bersama Master Data ────────────────────────────────────────────────

fn coded_table(kind: MasterKind) -> (CodedTable, &'static str) {
    match kind {
        MasterKind::Category => (CodedTable::Categories, "Kategori"),
        MasterKind::Rack => (CodedTable::Racks, "Rak"),
        MasterKind::Manufacturer => (CodedTable::Manufacturers, "Pabrik"),
    }
}

pub fn set_master_active(conn: &Connection, user: &SessionUser, kind: MasterKind, id: i64, active: bool) -> AppResult<()> {
    let (table, label) = coded_table(kind);
    let tx = conn.unchecked_transaction()?;
    let before = master_snapshot(&tx, kind, id)?;
    if repo::set_master_active(&tx, table, id, active)? == 0 {
        return Err(AppError::NotFound(format!("{label} tidak ditemukan")));
    }
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if active { audit::ACTIVATE } else { audit::DEACTIVATE },
            entity: kind_entity(kind),
            entity_id: id,
            before,
            after: master_snapshot(&tx, kind, id)?,
            reason: None,
        },
    )?;
    tx.commit()?;
    Ok(())
}

/// Hapus (soft delete): data ditandai terhapus dan disembunyikan dari daftar dan pilihan, tidak
/// pernah dihapus permanen. Ditolak bila masih dipakai obat, agar obat tidak menunjuk ke data yang
/// tersembunyi; untuk data yang masih dipakai, nonaktifkan saja.
pub fn delete_master(conn: &mut Connection, user: &SessionUser, kind: MasterKind, id: i64) -> AppResult<()> {
    let (table, label) = coded_table(kind);
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let code = repo::code_of(&tx, table, id)?.ok_or_else(|| AppError::NotFound(format!("{label} tidak ditemukan")))?;
    let used = repo::master_usage(&tx, table, id)?;
    if used > 0 {
        return Err(AppError::Conflict(format!(
            "{label} {code} dipakai oleh {used} obat sehingga tidak bisa dihapus. Nonaktifkan saja."
        )));
    }
    let before = master_snapshot(&tx, kind, id)?;
    repo::soft_delete_master(&tx, table, id, user.id)?;
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: audit::DELETE,
            entity: kind_entity(kind),
            entity_id: id,
            before,
            after: None,
            reason: None,
        },
    )?;
    tx.commit()?;
    Ok(())
}

fn kind_entity(kind: MasterKind) -> &'static str {
    match kind {
        MasterKind::Category => "categories",
        MasterKind::Rack => "racks",
        MasterKind::Manufacturer => "manufacturers",
    }
}

pub fn list_units(conn: &Connection) -> AppResult<Vec<Unit>> {
    repo::list_units(conn)
}

pub fn create_unit(conn: &Connection, user: &SessionUser, name: &str) -> AppResult<Unit> {
    let name = required(name, "Nama satuan")?;
    let tx = conn.unchecked_transaction()?;
    if repo::unit_name_taken(&tx, name)? {
        return Err(AppError::Conflict(format!("Satuan \"{name}\" sudah ada")));
    }
    let id = repo::insert_unit(&tx, name)?;
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: audit::CREATE,
            entity: "units",
            entity_id: id,
            before: None,
            after: Some(json!({ "name": name })),
            reason: None,
        },
    )?;
    tx.commit()?;
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
        manufacturer_id: p.manufacturer_id,
        category_id: p.category_id,
        drug_class: p.drug_class,
        is_owa: p.is_owa,
        base_unit_id: p.base_unit_id,
        min_stock_base: p.min_stock_base,
        rack_id: p.rack_id,
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
    if let Some(id) = input.manufacturer_id
        && !repo::named_exists(&tx, NamedTable::Manufacturers, id)?
    {
        return Err(AppError::Validation("Pabrik tidak ditemukan".into()));
    }
    if let Some(id) = input.rack_id
        && !repo::named_exists(&tx, NamedTable::Racks, id)?
    {
        return Err(AppError::Validation("Rak tidak ditemukan".into()));
    }
    for u in &input.units {
        if !repo::unit_exists(&tx, u.unit_id)? {
            return Err(AppError::Validation("Satuan tidak ditemukan".into()));
        }
    }
    for barcode in input.units.iter().flat_map(|u| &u.barcodes) {
        if let Some(owner) = repo::barcode_owner(&tx, barcode.trim(), input.id)? {
            return Err(AppError::Conflict(format!("Barcode {} sudah dipakai oleh {owner}", barcode.trim())));
        }
    }

    // Kode obat selalu dibuat sistem dan tidak berubah, seperti kode master data.
    let code = match input.id {
        Some(id) => repo::find_product(&tx, id)?
            .map(|p| p.code)
            .ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?,
        None => repo::next_product_code(&tx)?,
    };
    let fields = ProductFields {
        code: &code,
        name,
        generic_name: optional(&input.generic_name),
        manufacturer_id: input.manufacturer_id,
        category_id: input.category_id,
        drug_class: input.drug_class,
        is_owa: input.is_owa,
        base_unit_id: input.base_unit_id,
        min_stock_base: input.min_stock_base,
        rack_id: input.rack_id,
    };

    let (product_id, before) = match input.id {
        None => (repo::insert_product(&tx, &fields)?, None),
        Some(id) => {
            let old = repo::find_product(&tx, id)?.ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
            if repo::product_has_stock(&tx, id)? && old.base_unit_id != input.base_unit_id {
                return Err(AppError::Validation(
                    "Satuan dasar tidak bisa diubah karena obat ini sudah punya stok".into(),
                ));
            }
            let before = product_snapshot(&tx, id)?;
            repo::update_product(&tx, id, &fields)?;
            (id, Some(before))
        }
    };

    save_units(&tx, product_id, user.id, input)?;
    let price = settings::price(&tx)?;
    let mut warnings = recalc_auto_prices(&tx, product_id, price)?;
    if repo::product_units(&tx, product_id)?
        .iter()
        .any(|u| u.is_active && u.sell_price == 0)
    {
        warnings.push("Ada satuan yang harganya masih 0 dan belum bisa dijual".into());
    }

    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if before.is_some() { audit::UPDATE } else { audit::CREATE },
            entity: "products",
            entity_id: product_id,
            before,
            after: Some(product_snapshot(&tx, product_id)?),
            reason: None,
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

fn save_units(conn: &Connection, product_id: i64, user_id: i64, input: &ProductInput) -> AppResult<()> {
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

    // Barcode yang diinginkan per satuan (id satuan obat, barcode).
    let mut wanted = HashSet::new();
    for u in &input.units {
        let pu_id = match match_existing(&existing, u) {
            Some(old) => {
                repo::update_product_unit(conn, old.id, u.conversion, u.is_default_sale)?;
                old.id
            }
            None => repo::insert_product_unit(conn, product_id, u.unit_id, u.conversion, u.is_default_sale)?,
        };
        for b in &u.barcodes {
            wanted.insert((pu_id, b.trim().to_owned()));
        }
    }

    // Barcode tidak pernah dihapus permanen: yang dilepas di-soft-delete dulu, lalu yang baru
    // ditambahkan (urutan ini juga membolehkan barcode dipindah ke satuan lain di obat yang sama).
    let mut kept_barcodes = HashSet::new();
    for b in repo::active_product_barcodes(conn, product_id)? {
        let key = (b.product_unit_id, b.barcode);
        if wanted.contains(&key) {
            kept_barcodes.insert(key);
        } else {
            repo::soft_delete_barcode(conn, b.id, user_id)?;
        }
    }
    for (pu_id, barcode) in wanted.difference(&kept_barcodes) {
        repo::insert_barcode(conn, *pu_id, barcode)?;
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
    let tx = conn.unchecked_transaction()?;
    set_product_active_in(&tx, user, id, active)?;
    tx.commit()?;
    Ok(())
}

fn set_product_active_in(tx: &Connection, user: &SessionUser, id: i64, active: bool) -> AppResult<()> {
    let before = product_snapshot(tx, id)?;
    repo::set_product_active(tx, id, active)?;
    audit::log_change(
        tx,
        Change {
            user_id: user.id,
            action: if active { audit::ACTIVATE } else { audit::DEACTIVATE },
            entity: "products",
            entity_id: id,
            before: Some(before),
            after: Some(product_snapshot(tx, id)?),
            reason: None,
        },
    )?;
    Ok(())
}

/// Aktifkan/nonaktifkan banyak obat sekaligus dalam satu transaksi. Obat yang statusnya sudah
/// sesuai dilewati tanpa dicatat di log.
pub fn set_products_active(conn: &mut Connection, user: &SessionUser, ids: &[i64], active: bool) -> AppResult<BatchResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut result = BatchResult::default();
    for &id in unique(ids).iter() {
        match repo::find_product(&tx, id)? {
            None => result.skipped.push(format!("Obat #{id} tidak ditemukan")),
            Some(p) if p.is_active == active => {}
            Some(_) => {
                set_product_active_in(&tx, user, id, active)?;
                result.done += 1;
            }
        }
    }
    tx.commit()?;
    Ok(result)
}

/// Hapus (soft delete) banyak obat sekaligus dalam satu transaksi. Obat yang masih punya stok
/// dilewati dan disebutkan di `skipped`; sisanya tetap dihapus.
pub fn delete_products(conn: &mut Connection, user: &SessionUser, ids: &[i64]) -> AppResult<BatchResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut result = BatchResult::default();
    for &id in unique(ids).iter() {
        match delete_product_in(&tx, user, id) {
            Ok(()) => result.done += 1,
            Err(AppError::Conflict(m) | AppError::NotFound(m)) => result.skipped.push(m),
            Err(e) => return Err(e),
        }
    }
    tx.commit()?;
    Ok(result)
}

fn unique(ids: &[i64]) -> Vec<i64> {
    let mut seen = HashSet::new();
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

/// Hapus obat (soft delete): obat ditandai terhapus dan disembunyikan dari daftar dan pencarian,
/// tidak pernah dihapus permanen sehingga riwayat transaksinya tetap utuh. Barcode-nya ikut
/// dilepas agar bisa dipakai obat lain. Ditolak bila masih ada stok; nonaktifkan saja.
pub fn delete_product(conn: &mut Connection, user: &SessionUser, id: i64) -> AppResult<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    delete_product_in(&tx, user, id)?;
    tx.commit()?;
    Ok(())
}

fn delete_product_in(tx: &Connection, user: &SessionUser, id: i64) -> AppResult<()> {
    let product = repo::find_product(tx, id)?.ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
    let stock = repo::product_stock_on_hand(tx, id)?;
    if stock > 0 {
        return Err(AppError::Conflict(format!(
            "Obat {} masih punya stok {stock} sehingga tidak bisa dihapus. Nonaktifkan saja.",
            product.name
        )));
    }
    let before = product_snapshot(tx, id)?;
    for b in repo::active_product_barcodes(tx, id)? {
        repo::soft_delete_barcode(tx, b.id, user.id)?;
    }
    repo::soft_delete_product(tx, id, user.id)?;
    audit::log_change(
        tx,
        Change {
            user_id: user.id,
            action: audit::DELETE,
            entity: "products",
            entity_id: id,
            before: Some(before),
            after: None,
            reason: None,
        },
    )?;
    Ok(())
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
    let before = product_snapshot(&tx, product.id)?;

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

    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: audit::PRICE_CHANGE,
            entity: "products",
            entity_id: product.id,
            before: Some(before),
            after: Some(product_snapshot(&tx, product.id)?),
            reason: None,
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

// ─── Snapshot audit ──────────────────────────────────────────────────────────
// Isi lengkap satu data untuk log audit (sebelum/sesudah). Relasi ditulis sebagai nama agar log
// tetap terbaca walau data relasinya kelak diubah. Kunci camelCase diterjemahkan di menu Log.

fn category_snapshot(conn: &Connection, id: i64) -> AppResult<Option<Value>> {
    Ok(repo::get_category(conn, id)?.map(|c| {
        json!({ "code": c.code, "name": c.name, "marginBp": c.margin_bp, "isActive": c.is_active })
    }))
}

fn named_snapshot(conn: &Connection, table: NamedTable, id: i64) -> AppResult<Option<Value>> {
    Ok(repo::get_named(conn, table, id)?.map(|n| json!({ "code": n.code, "name": n.name, "isActive": n.is_active })))
}

fn master_snapshot(conn: &Connection, kind: MasterKind, id: i64) -> AppResult<Option<Value>> {
    match kind {
        MasterKind::Category => category_snapshot(conn, id),
        MasterKind::Rack => named_snapshot(conn, NamedTable::Racks, id),
        MasterKind::Manufacturer => named_snapshot(conn, NamedTable::Manufacturers, id),
    }
}

/// Data obat lengkap termasuk HPP, margin, satuan aktif, barcode, harga, dan tier.
fn product_snapshot(conn: &Connection, id: i64) -> AppResult<Value> {
    let full = PriceAccess { view_cost: true, manage_price: true };
    let p = get_product(conn, id, full)?;
    let names = repo::product_ref_names(conn, id)?;
    let units: Map<String, Value> = p
        .units
        .iter()
        .filter(|u| u.is_active)
        .map(|u| {
            let mut barcodes = u.barcodes.clone();
            barcodes.sort();
            let tiers: Map<String, Value> = u
                .tiers
                .iter()
                .map(|t| {
                    let tier = json!({ "priceMode": t.price_mode, "marginBp": t.margin_bp, "price": t.price });
                    (format!("≥ {}", t.min_qty), tier)
                })
                .collect();
            let unit = json!({
                "conversion": u.conversion,
                "isDefaultSale": u.is_default_sale,
                "priceMode": u.price_mode,
                "sellPrice": u.sell_price,
                "barcodes": barcodes.join(", "),
                "tiers": tiers,
            });
            (u.unit_name.clone(), unit)
        })
        .collect();
    Ok(json!({
        "code": p.code,
        "name": p.name,
        "genericName": p.generic_name,
        "manufacturer": names.manufacturer,
        "category": names.category,
        "rack": names.rack,
        "drugClass": p.drug_class,
        "isOwa": p.is_owa,
        "baseUnit": names.base_unit,
        "minStockBase": p.min_stock_base,
        "isActive": p.is_active,
        "marginBp": p.margin_bp,
        "lastCostX100": p.last_cost_x100,
        "units": units,
    }))
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
