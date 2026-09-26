use std::collections::{BTreeMap, HashSet};

use chrono::{Days, NaiveDate};
use rusqlite::{Connection, TransactionBehavior};
use serde_json::{Map, Value, json};

use super::calc::{self, Line};
use super::model::{
    DebtPage, DebtQuery, PurchaseDefaults, PurchaseDetail, PurchaseInput, PurchaseItemDetail, PurchasePage,
    PurchasePaymentType, PurchasePostInput, PurchaseProduct, PurchaseQuery, PurchaseSaveResult, PurchaseStatus,
    Supplier, SupplierInput, SupplierPage, SupplierPaymentInput, TaxMode,
};
use super::repo::{self, Amounts, HeaderFields, ItemFields, SupplierFields, UnitInfo};
use crate::audit::{self, Change, Entry};
use crate::auth::{Permission, SessionUser};
use crate::db::{self, sequence};
use crate::error::{AppError, AppResult};
use crate::inventory::{self, NEAR_EXPIRY_DAYS, NewBatch, NewMovement};
use crate::master::{MasterPageQuery, recalc_auto_prices};
use crate::settings;

pub const ENTITY: &str = "purchases";
pub const ENTITY_SUPPLIER: &str = "suppliers";
pub const ENTITY_PAYMENT: &str = "supplier_payments";

/// Aksi log audit khusus pembelian.
pub const POST: &str = "POST";
pub const VOID: &str = "VOID";

const MAX_ITEMS: usize = 300;
const MAX_QTY: i64 = 1_000_000;
/// Hutang yang jatuh tempo sampai sekian hari ke depan dianggap "segera jatuh tempo".
const DUE_SOON_DAYS: u64 = 7;

fn can(user: &SessionUser, p: Permission) -> bool {
    user.permissions.contains(&p)
}

fn day(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn defaults(conn: &Connection, today: NaiveDate) -> AppResult<PurchaseDefaults> {
    let tax = settings::tax(conn)?;
    Ok(PurchaseDefaults {
        tax_rate_bp: tax.ppn_rate_bp,
        is_pkp: tax.is_pkp,
        near_expiry_days: NEAR_EXPIRY_DAYS as i64,
        today: day(today),
    })
}

// ─── Supplier ────────────────────────────────────────────────────────────────

pub fn list_suppliers(conn: &Connection) -> AppResult<Vec<Supplier>> {
    repo::list_suppliers(conn)
}

pub fn page_suppliers(conn: &Connection, query: &MasterPageQuery) -> AppResult<SupplierPage> {
    let (rows, total) = repo::page_suppliers(conn, query)?;
    Ok(SupplierPage { rows, total })
}

pub fn save_supplier(conn: &Connection, user: &SessionUser, input: &SupplierInput) -> AppResult<Supplier> {
    if !(0..=365).contains(&input.payment_term_days) {
        return Err(AppError::Validation("Tempo pembayaran harus 0–365 hari".into()));
    }
    let fields = SupplierFields {
        name: required(&input.name, "Nama supplier")?,
        address: optional(&input.address),
        phone: optional(&input.phone),
        npwp: optional(&input.npwp),
        payment_term_days: input.payment_term_days,
        is_active: input.is_active,
    };
    let tx = conn.unchecked_transaction()?;
    if let Some(code) = repo::supplier_name_taken(&tx, fields.name, input.id)? {
        return Err(AppError::Conflict(format!("Supplier {} sudah ada ({code})", fields.name)));
    }
    let before = match input.id {
        Some(id) => Some(supplier_snapshot(&tx, id)?.ok_or_else(|| AppError::NotFound("Supplier tidak ditemukan".into()))?),
        None => None,
    };
    let id = match input.id {
        Some(id) => {
            repo::update_supplier(&tx, id, &fields)?;
            id
        }
        None => {
            let code = repo::next_supplier_code(&tx)?;
            repo::insert_supplier(&tx, &code, &fields, user.id)?
        }
    };
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if before.is_some() { audit::UPDATE } else { audit::CREATE },
            entity: ENTITY_SUPPLIER,
            entity_id: id,
            before,
            after: supplier_snapshot(&tx, id)?,
            reason: None,
        },
    )?;
    tx.commit()?;
    repo::get_supplier(conn, id)?.ok_or_else(|| AppError::Internal("supplier hilang setelah disimpan".into()))
}

pub fn set_supplier_active(conn: &Connection, user: &SessionUser, id: i64, active: bool) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    let before = supplier_snapshot(&tx, id)?;
    if repo::set_supplier_active(&tx, id, active)? == 0 {
        return Err(AppError::NotFound("Supplier tidak ditemukan".into()));
    }
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if active { audit::ACTIVATE } else { audit::DEACTIVATE },
            entity: ENTITY_SUPPLIER,
            entity_id: id,
            before,
            after: supplier_snapshot(&tx, id)?,
            reason: None,
        },
    )?;
    tx.commit()?;
    Ok(())
}

/// Hapus (soft delete). Ditolak bila supplier sudah punya faktur; nonaktifkan saja.
pub fn delete_supplier(conn: &mut Connection, user: &SessionUser, id: i64) -> AppResult<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let before = supplier_snapshot(&tx, id)?.ok_or_else(|| AppError::NotFound("Supplier tidak ditemukan".into()))?;
    let used = repo::supplier_usage(&tx, id)?;
    if used > 0 {
        return Err(AppError::Conflict(format!(
            "Supplier {} punya {used} faktur sehingga tidak bisa dihapus. Nonaktifkan saja.",
            before["name"].as_str().unwrap_or_default()
        )));
    }
    repo::soft_delete_supplier(&tx, id, user.id)?;
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: audit::DELETE,
            entity: ENTITY_SUPPLIER,
            entity_id: id,
            before: Some(before),
            after: None,
            reason: None,
        },
    )?;
    tx.commit()?;
    Ok(())
}

// ─── Faktur: baca ────────────────────────────────────────────────────────────

pub fn page_purchases(conn: &Connection, user: &SessionUser, query: &PurchaseQuery) -> AppResult<PurchasePage> {
    for d in [&query.date_from, &query.date_to].into_iter().flatten() {
        parse_date(d, "Tanggal")?;
    }
    let (mut rows, total) = repo::page_purchases(conn, query)?;
    if !can(user, Permission::SupplierDebtManage) {
        rows.iter_mut().for_each(|r| r.outstanding = None);
    }
    Ok(PurchasePage { rows, total })
}

fn line_of(i: &PurchaseItemDetail) -> Line {
    Line {
        qty: i.qty,
        bonus_qty: i.bonus_qty,
        conversion: i.conversion,
        unit_price: i.unit_price,
        discount1_bp: i.discount1_bp,
        discount2_bp: i.discount2_bp,
    }
}

pub fn get_purchase(conn: &Connection, user: &SessionUser, id: i64) -> AppResult<PurchaseDetail> {
    let h = repo::get_header(conn, id)?.ok_or_else(|| AppError::NotFound("Faktur tidak ditemukan".into()))?;
    let mut items = repo::items(conn, id)?;
    let is_draft = h.row.status == PurchaseStatus::Draft;
    // Draft: HPP perkiraan dengan pengaturan pajak saat ini. Setelah posting: snapshot.
    let tax_in_cost = match h.tax_in_cost {
        Some(v) => v,
        None => !settings::tax(conn)?.is_pkp,
    };
    if is_draft {
        let lines: Vec<Line> = items.iter().map(line_of).collect();
        let t = calc::compute(&lines, h.extra_discount, h.tax_mode, h.tax_rate_bp, tax_in_cost);
        for (i, cost) in items.iter_mut().zip(t.unit_costs_x100) {
            i.unit_cost_x100 = Some(cost);
            i.units = repo::purchase_units(conn, i.product_id)?;
        }
    }
    if !can(user, Permission::ViewCost) {
        items.iter_mut().for_each(|i| {
            i.unit_cost_x100 = None;
            i.last_cost_x100 = None;
        });
    } else if !is_draft {
        // HPP acuan obat sudah bisa berubah oleh faktur ini; pembanding hanya berguna sebelum posting.
        items.iter_mut().for_each(|i| i.last_cost_x100 = None);
    }
    let debt = can(user, Permission::SupplierDebtManage);
    let payments = if debt && h.row.payment_type == PurchasePaymentType::Credit {
        Some(repo::payments(conn, id)?)
    } else {
        None
    };
    let r = h.row;
    Ok(PurchaseDetail {
        id: r.id,
        number: r.number,
        supplier_id: r.supplier_id,
        supplier_name: r.supplier_name,
        invoice_number: r.invoice_number,
        invoice_date: r.invoice_date,
        received_date: r.received_date,
        due_date: r.due_date,
        payment_type: r.payment_type,
        tax_mode: h.tax_mode,
        tax_rate_bp: h.tax_rate_bp,
        status: r.status,
        note: h.note,
        subtotal: h.subtotal,
        extra_discount: h.extra_discount,
        discount_total: h.discount_total,
        tax_total: h.tax_total,
        grand_total: r.grand_total,
        tax_in_cost,
        items,
        created_at: r.created_at,
        created_by: r.created_by,
        posted_at: r.posted_at,
        posted_by: h.posted_by,
        voided_at: h.voided_at,
        voided_by: h.voided_by,
        void_reason: h.void_reason,
        outstanding: if debt { r.outstanding } else { None },
        payments,
    })
}

pub fn search_products(conn: &Connection, user: &SessionUser, q: &str) -> AppResult<Vec<PurchaseProduct>> {
    let mut rows = repo::search_products(conn, q.trim(), 20)?;
    if !can(user, Permission::ViewCost) {
        rows.iter_mut().for_each(|p| p.last_cost_x100 = None);
    }
    Ok(rows)
}

// ─── Faktur: simpan draft ────────────────────────────────────────────────────

struct PreparedItem<'a> {
    unit: UnitInfo,
    line: Line,
    batch_number: &'a str,
    expiry_date: String,
}

struct Prepared<'a> {
    fields: HeaderFields<'a>,
    items: Vec<PreparedItem<'a>>,
    warnings: Vec<String>,
}

fn validate_bp(value: i64, label: &str) -> AppResult<()> {
    if !(0..=10_000).contains(&value) {
        return Err(AppError::Validation(format!("{label} harus 0–100%")));
    }
    Ok(())
}

/// Validasi input faktur. Peringatan (ED dekat, harga beli naik) tidak menggagalkan simpan.
fn prepare<'a>(conn: &Connection, user: &SessionUser, input: &'a PurchaseInput, today: NaiveDate) -> AppResult<Prepared<'a>> {
    let supplier = repo::get_supplier(conn, input.supplier_id)?.ok_or_else(|| AppError::Validation("Pilih supplier".into()))?;
    if !supplier.is_active {
        return Err(AppError::Validation(format!("Supplier {} nonaktif", supplier.name)));
    }
    let invoice_number = required(&input.invoice_number, "Nomor faktur")?;
    let invoice_date = parse_date(input.invoice_date.trim(), "Tanggal faktur")?;
    let received_date = parse_date(input.received_date.trim(), "Tanggal terima")?;
    if invoice_date > today || received_date > today {
        return Err(AppError::Validation("Tanggal faktur dan tanggal terima tidak boleh setelah hari ini".into()));
    }
    let due_date = match input.payment_type {
        PurchasePaymentType::Cash => None,
        PurchasePaymentType::Credit => {
            let due = optional(&input.due_date).ok_or_else(|| AppError::Validation("Jatuh tempo wajib diisi untuk faktur kredit".into()))?;
            let d = parse_date(due, "Jatuh tempo")?;
            if d < invoice_date {
                return Err(AppError::Validation("Jatuh tempo tidak boleh sebelum tanggal faktur".into()));
            }
            Some(d)
        }
    };
    let tax_rate_bp = if input.tax_mode == TaxMode::None { 0 } else { input.tax_rate_bp };
    validate_bp(tax_rate_bp, "Tarif PPN")?;
    if input.extra_discount < 0 {
        return Err(AppError::Validation("Diskon faktur tidak boleh negatif".into()));
    }
    if input.items.len() > MAX_ITEMS {
        return Err(AppError::Validation(format!("Maksimal {MAX_ITEMS} baris per faktur")));
    }

    let near = received_date.checked_add_days(Days::new(NEAR_EXPIRY_DAYS)).unwrap_or(received_date);
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    let mut items = Vec::with_capacity(input.items.len());
    for (n, it) in input.items.iter().enumerate() {
        let no = n + 1;
        let unit = repo::unit_info(conn, it.product_unit_id)?
            .ok_or_else(|| AppError::Validation(format!("Baris {no}: satuan obat tidak ditemukan")))?;
        let name = &unit.product_name;
        if !unit.selectable {
            return Err(AppError::Validation(format!("Baris {no}: {name} ({}) nonaktif", unit.unit_name)));
        }
        if !(1..=MAX_QTY).contains(&it.qty) {
            return Err(AppError::Validation(format!("Baris {no}: jumlah {name} harus 1–{MAX_QTY}")));
        }
        if !(0..=MAX_QTY).contains(&it.bonus_qty) {
            return Err(AppError::Validation(format!("Baris {no}: bonus {name} tidak valid")));
        }
        if it.unit_price < 0 {
            return Err(AppError::Validation(format!("Baris {no}: harga {name} tidak boleh negatif")));
        }
        validate_bp(it.discount1_bp, &format!("Baris {no}: diskon 1"))?;
        validate_bp(it.discount2_bp, &format!("Baris {no}: diskon 2"))?;
        let batch_number = it.batch_number.trim();
        if batch_number.is_empty() {
            return Err(AppError::Validation(format!("Baris {no}: nomor batch {name} wajib diisi")));
        }
        if batch_number.chars().count() > 50 {
            return Err(AppError::Validation(format!("Baris {no}: nomor batch terlalu panjang")));
        }
        let ed = parse_date(it.expiry_date.trim(), &format!("Baris {no}: tanggal ED"))?;
        if ed <= received_date {
            return Err(AppError::Validation(format!(
                "Baris {no}: {name} batch {batch_number} sudah lewat ED ({}); barang kedaluwarsa tidak boleh diterima",
                day(ed)
            )));
        }
        if ed <= near {
            warnings.push(format!("{name} batch {batch_number}: ED {} kurang dari {NEAR_EXPIRY_DAYS} hari", day(ed)));
        }
        if !seen.insert((it.product_unit_id, batch_number.to_uppercase())) {
            return Err(AppError::Validation(format!(
                "Baris {no}: {name} batch {batch_number} diisi dua kali; gabungkan jumlahnya di satu baris"
            )));
        }
        items.push(PreparedItem {
            line: Line {
                qty: it.qty,
                bonus_qty: it.bonus_qty,
                conversion: unit.conversion,
                unit_price: it.unit_price,
                discount1_bp: it.discount1_bp,
                discount2_bp: it.discount2_bp,
            },
            unit,
            batch_number,
            expiry_date: day(ed),
        });
    }

    // Harga beli naik dibanding HPP acuan obat.
    let tax_in_cost = !settings::tax(conn)?.is_pkp;
    let lines: Vec<Line> = items.iter().map(|i| i.line).collect();
    let totals = calc::compute(&lines, input.extra_discount, input.tax_mode, tax_rate_bp, tax_in_cost);
    if input.extra_discount > totals.lines_total {
        return Err(AppError::Validation("Diskon faktur melebihi nilai barang".into()));
    }
    let view_cost = can(user, Permission::ViewCost);
    let mut warned = HashSet::new();
    for (i, &cost) in items.iter().zip(&totals.unit_costs_x100) {
        let Some(last) = i.unit.last_cost_x100.filter(|&l| l > 0) else { continue };
        if cost <= last || !warned.insert(i.unit.product_id) {
            continue;
        }
        let name = &i.unit.product_name;
        warnings.push(if view_cost {
            let pct = format!("{:.1}", (cost - last) as f64 * 100.0 / last as f64).replace('.', ",");
            format!(
                "Harga beli {name} naik {pct}% dari pembelian terakhir (HPP Rp{} → Rp{} per satuan dasar)",
                format_x100(last),
                format_x100(cost)
            )
        } else {
            format!("Harga beli {name} lebih tinggi dari pembelian terakhir")
        });
    }

    Ok(Prepared {
        fields: HeaderFields {
            supplier_id: supplier.id,
            invoice_number,
            invoice_date: day(invoice_date),
            received_date: day(received_date),
            due_date: due_date.map(day),
            payment_type: input.payment_type,
            tax_mode: input.tax_mode,
            tax_rate_bp,
            extra_discount: input.extra_discount,
            note: optional(&input.note),
        },
        items,
        warnings,
    })
}

/// Rupiah × 100 → format Indonesia: 112500 → "1.125", 15050 → "150,5".
fn format_x100(v: i64) -> String {
    let whole = group_thousands(v / 100);
    match v % 100 {
        0 => whole,
        c if c % 10 == 0 => format!("{whole},{}", c / 10),
        c => format!("{whole},{c:02}"),
    }
}

fn amounts(t: &calc::Totals) -> Amounts {
    Amounts { subtotal: t.subtotal, discount_total: t.discount_total, tax_total: t.tax_total, grand_total: t.grand_total }
}

/// Simpan faktur sebagai DRAFT (baru atau ubah). Stok belum berubah.
pub fn save_purchase(conn: &mut Connection, user: &SessionUser, input: &PurchaseInput, today: NaiveDate) -> AppResult<PurchaseSaveResult> {
    let (id, warnings) = db::write_tx(conn, |tx| {
        let p = prepare(tx, user, input, today)?;
        if let Some(number) = repo::invoice_taken(tx, p.fields.supplier_id, p.fields.invoice_number, input.id)? {
            return Err(AppError::Conflict(format!(
                "Faktur {} dari supplier ini sudah diinput ({number})",
                p.fields.invoice_number
            )));
        }
        let tax_in_cost = !settings::tax(tx)?.is_pkp;
        let lines: Vec<Line> = p.items.iter().map(|i| i.line).collect();
        let t = calc::compute(&lines, p.fields.extra_discount, p.fields.tax_mode, p.fields.tax_rate_bp, tax_in_cost);
        let a = amounts(&t);

        let (id, before) = match input.id {
            Some(id) => {
                let before = purchase_snapshot(tx, id)?;
                if repo::update_purchase(tx, id, &p.fields, &a)? == 0 {
                    let h = repo::get_header(tx, id)?.ok_or_else(|| AppError::NotFound("Faktur tidak ditemukan".into()))?;
                    return Err(AppError::Conflict(format!(
                        "Faktur {} sudah {}, tidak bisa diubah",
                        h.row.number,
                        status_label(h.row.status)
                    )));
                }
                repo::delete_items(tx, id)?;
                (id, Some(before))
            }
            None => {
                let number = sequence::next_number(tx, "PB", &today.format("%y%m").to_string())?;
                (repo::insert_purchase(tx, &number, &p.fields, &a, user.id)?, None)
            }
        };
        for (n, i) in p.items.iter().enumerate() {
            repo::insert_item(
                tx,
                id,
                &ItemFields {
                    line_no: n as i64 + 1,
                    product_id: i.unit.product_id,
                    product_unit_id: input.items[n].product_unit_id,
                    qty: i.line.qty,
                    bonus_qty: i.line.bonus_qty,
                    conversion: i.line.conversion,
                    unit_price: i.line.unit_price,
                    discount1_bp: i.line.discount1_bp,
                    discount2_bp: i.line.discount2_bp,
                    line_total: i.line.total(),
                    batch_number: i.batch_number,
                    expiry_date: &i.expiry_date,
                },
            )?;
        }
        audit::log_change(
            tx,
            Change {
                user_id: user.id,
                action: if before.is_some() { audit::UPDATE } else { audit::CREATE },
                entity: ENTITY,
                entity_id: id,
                before,
                after: Some(purchase_snapshot(tx, id)?),
                reason: None,
            },
        )?;
        Ok((id, p.warnings))
    })?;
    Ok(PurchaseSaveResult { purchase: get_purchase(conn, user, id)?, warnings })
}

// ─── Faktur: posting ─────────────────────────────────────────────────────────

/// Posting faktur: HPP per baris dihitung, satu batch per baris dibuat, kartu stok `PURCHASE`
/// dicatat, lalu (opsional) HPP acuan obat diperbarui dan harga `AUTO` dihitung ulang. Semua dalam
/// satu transaksi.
pub fn post_purchase(conn: &mut Connection, user: &SessionUser, input: &PurchasePostInput) -> AppResult<PurchaseSaveResult> {
    let warnings = db::write_tx(conn, |tx| {
        let h = repo::get_header(tx, input.id)?.ok_or_else(|| AppError::NotFound("Faktur tidak ditemukan".into()))?;
        let number = h.row.number.clone();
        if h.row.status != PurchaseStatus::Draft {
            return Err(AppError::Conflict(format!("Faktur {number} sudah {}", status_label(h.row.status))));
        }
        let items = repo::items(tx, input.id)?;
        if items.is_empty() {
            return Err(AppError::Validation("Faktur belum berisi obat".into()));
        }
        let tax_in_cost = !settings::tax(tx)?.is_pkp;
        let lines: Vec<Line> = items.iter().map(line_of).collect();
        let t = calc::compute(&lines, h.extra_discount, h.tax_mode, h.tax_rate_bp, tax_in_cost);
        if h.extra_discount > t.lines_total {
            return Err(AppError::Validation("Diskon faktur melebihi nilai barang".into()));
        }

        let note = format!("Faktur {} · {}", h.row.invoice_number, h.row.supplier_name);
        // HPP baru per obat (tertinggi bila satu obat ada di beberapa baris; baris bernilai 0 dilewati).
        let mut new_costs: BTreeMap<i64, i64> = BTreeMap::new();
        for (i, &cost) in items.iter().zip(&t.unit_costs_x100) {
            let batch_id = inventory::insert_batch(
                tx,
                &NewBatch {
                    product_id: i.product_id,
                    batch_number: &i.batch_number,
                    expiry_date: &i.expiry_date,
                    unit_cost_x100: cost,
                    source_type: "PURCHASE",
                    source_id: i.id,
                },
            )?;
            inventory::insert_movement(
                tx,
                &NewMovement {
                    batch_id,
                    product_id: i.product_id,
                    movement_type: "PURCHASE",
                    qty_change_base: i.qty_base,
                    ref_type: "purchase",
                    ref_id: input.id,
                    ref_line_id: Some(i.id),
                    user_id: user.id,
                    note: Some(&note),
                },
            )?;
            repo::set_item_posted(tx, i.id, cost, batch_id)?;
            if cost > 0 {
                let e = new_costs.entry(i.product_id).or_default();
                *e = (*e).max(cost);
            }
        }
        if repo::mark_posted(tx, input.id, &amounts(&t), tax_in_cost, user.id)? != 1 {
            return Err(AppError::Conflict(format!("Faktur {number} sudah diposting")));
        }
        audit::log(
            tx,
            Entry {
                user_id: Some(user.id),
                action: POST,
                entity: Some(ENTITY),
                entity_id: Some(input.id),
                detail: Some(json!({
                    "code": number,
                    "name": h.row.supplier_name,
                    "invoiceNumber": h.row.invoice_number,
                    "grandTotal": t.grand_total,
                    "itemCount": items.len(),
                    "updatePrices": input.update_prices,
                })),
                ..Default::default()
            },
        )?;

        let mut warnings = Vec::new();
        if input.update_prices {
            let price = settings::price(tx)?;
            for (&product_id, &cost) in &new_costs {
                let before = price_snapshot(tx, product_id)?;
                repo::set_last_cost(tx, product_id, cost)?;
                let name = before["name"].as_str().unwrap_or_default().to_owned();
                warnings.extend(recalc_auto_prices(tx, product_id, price)?.into_iter().map(|w| format!("{name}: {w}")));
                audit::log_change(
                    tx,
                    Change {
                        user_id: user.id,
                        action: audit::PRICE_RECALC,
                        entity: "products",
                        entity_id: product_id,
                        before: Some(before),
                        after: Some(price_snapshot(tx, product_id)?),
                        reason: Some(&note),
                    },
                )?;
            }
        }
        Ok(warnings)
    })?;
    Ok(PurchaseSaveResult { purchase: get_purchase(conn, user, input.id)?, warnings })
}

// ─── Faktur: batal ───────────────────────────────────────────────────────────

/// Batalkan faktur. Draft: cukup ditandai batal (hak `PURCHASE_RECEIVE`). Faktur yang sudah diposting
/// (hak `TRANSACTION_VOID`, wajib alasan): stok setiap batch ditarik kembali lewat `PURCHASE_VOID`,
/// hanya bila batch belum dipakai transaksi lain dan belum ada pembayaran hutang.
pub fn void_purchase(conn: &mut Connection, user: &SessionUser, id: i64, reason: Option<&str>) -> AppResult<PurchaseDetail> {
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    db::write_tx(conn, |tx| {
        let h = repo::get_header(tx, id)?.ok_or_else(|| AppError::NotFound("Faktur tidak ditemukan".into()))?;
        let number = &h.row.number;
        match h.row.status {
            PurchaseStatus::Void => return Err(AppError::Conflict(format!("Faktur {number} sudah dibatalkan"))),
            PurchaseStatus::Draft => {}
            PurchaseStatus::Posted => {
                if !can(user, Permission::TransactionVoid) {
                    return Err(AppError::Forbidden);
                }
                if reason.is_none() {
                    return Err(AppError::Validation("Alasan batal wajib diisi".into()));
                }
                let paid = repo::active_payment_count(tx, id)?;
                if paid > 0 {
                    return Err(AppError::Conflict(format!(
                        "Faktur {number} sudah punya {paid} pembayaran hutang. Batalkan pembayarannya terlebih dahulu."
                    )));
                }
                let items = repo::items(tx, id)?;
                for i in &items {
                    let Some(batch_id) = i.batch_id else { continue };
                    if repo::batch_other_movements(tx, batch_id, id)? > 0 {
                        return Err(AppError::Conflict(format!(
                            "Stok {} batch {} sudah dipakai (terjual, disesuaikan, atau diretur) sehingga faktur \
                             tidak bisa dibatalkan. Gunakan retur ke supplier.",
                            i.product_name, i.batch_number
                        )));
                    }
                    inventory::insert_movement(
                        tx,
                        &NewMovement {
                            batch_id,
                            product_id: i.product_id,
                            movement_type: "PURCHASE_VOID",
                            qty_change_base: -i.qty_base,
                            ref_type: "purchase",
                            ref_id: id,
                            ref_line_id: Some(i.id),
                            user_id: user.id,
                            note: reason,
                        },
                    )?;
                }
            }
        }
        if repo::mark_void(tx, id, h.row.status, user.id, reason.unwrap_or_default())? != 1 {
            return Err(AppError::Conflict(format!("Faktur {number} sudah berubah, muat ulang")));
        }
        audit::log(
            tx,
            Entry {
                user_id: Some(user.id),
                action: VOID,
                entity: Some(ENTITY),
                entity_id: Some(id),
                detail: Some(json!({
                    "code": number,
                    "name": h.row.supplier_name,
                    "invoiceNumber": h.row.invoice_number,
                    "previousStatus": h.row.status,
                    "grandTotal": h.row.grand_total,
                })),
                reason,
                ..Default::default()
            },
        )
    })?;
    get_purchase(conn, user, id)
}

// ─── Hutang supplier ─────────────────────────────────────────────────────────

pub fn page_debts(conn: &Connection, query: &DebtQuery, today: NaiveDate) -> AppResult<DebtPage> {
    let soon = day(today.checked_add_days(Days::new(DUE_SOON_DAYS)).unwrap_or(today));
    let today_s = day(today);
    let (rows, total) = repo::page_debts(conn, query, &today_s, &soon)?;
    Ok(DebtPage { rows, total, summary: repo::debt_summary(conn, &today_s, &soon)?, today: today_s })
}

/// Catat pembayaran hutang (faktur kredit yang sudah diposting). Tidak boleh melebihi sisa hutang.
pub fn create_payment(conn: &mut Connection, user: &SessionUser, input: &SupplierPaymentInput, today: NaiveDate) -> AppResult<PurchaseDetail> {
    let date = parse_date(input.payment_date.trim(), "Tanggal bayar")?;
    if date > today {
        return Err(AppError::Validation("Tanggal bayar tidak boleh setelah hari ini".into()));
    }
    if input.amount <= 0 {
        return Err(AppError::Validation("Jumlah bayar harus lebih dari 0".into()));
    }
    db::write_tx(conn, |tx| {
        let h = repo::get_header(tx, input.purchase_id)?.ok_or_else(|| AppError::NotFound("Faktur tidak ditemukan".into()))?;
        if h.row.status != PurchaseStatus::Posted || h.row.payment_type != PurchasePaymentType::Credit {
            return Err(AppError::Validation("Pembayaran hanya untuk faktur kredit yang sudah diposting".into()));
        }
        let outstanding = h.row.outstanding.unwrap_or(0);
        if outstanding <= 0 {
            return Err(AppError::Conflict(format!("Faktur {} sudah lunas", h.row.number)));
        }
        if input.amount > outstanding {
            return Err(AppError::Validation(format!(
                "Jumlah bayar melebihi sisa hutang (Rp{})",
                group_thousands(outstanding)
            )));
        }
        let number = sequence::next_number(tx, "BH", &today.format("%y%m").to_string())?;
        let payment_date = day(date);
        let id = repo::insert_payment(
            tx,
            &repo::NewPayment {
                number: &number,
                supplier_id: h.row.supplier_id,
                purchase_id: input.purchase_id,
                payment_date: &payment_date,
                amount: input.amount,
                method: input.method,
                reference: optional(&input.reference),
                note: optional(&input.note),
                user_id: user.id,
            },
        )?;
        audit::log(
            tx,
            Entry {
                user_id: Some(user.id),
                action: audit::CREATE,
                entity: Some(ENTITY_PAYMENT),
                entity_id: Some(id),
                detail: Some(json!({
                    "code": number,
                    "name": h.row.supplier_name,
                    "purchase": h.row.number,
                    "invoiceNumber": h.row.invoice_number,
                    "paymentDate": payment_date,
                    "amount": input.amount,
                    "method": input.method,
                    "reference": optional(&input.reference),
                })),
                ..Default::default()
            },
        )
    })?;
    get_purchase(conn, user, input.purchase_id)
}

pub fn void_payment(conn: &mut Connection, user: &SessionUser, id: i64, reason: &str) -> AppResult<PurchaseDetail> {
    let reason = required(reason, "Alasan batal")?;
    let purchase_id = db::write_tx(conn, |tx| {
        let (purchase_id, number, amount, voided) =
            repo::get_payment(tx, id)?.ok_or_else(|| AppError::NotFound("Pembayaran tidak ditemukan".into()))?;
        if voided || repo::void_payment(tx, id, user.id, reason)? != 1 {
            return Err(AppError::Conflict(format!("Pembayaran {number} sudah dibatalkan")));
        }
        let h = repo::get_header(tx, purchase_id)?.ok_or_else(|| AppError::NotFound("Faktur tidak ditemukan".into()))?;
        audit::log(
            tx,
            Entry {
                user_id: Some(user.id),
                action: VOID,
                entity: Some(ENTITY_PAYMENT),
                entity_id: Some(id),
                detail: Some(json!({
                    "code": number,
                    "name": h.row.supplier_name,
                    "purchase": h.row.number,
                    "amount": amount,
                })),
                reason: Some(reason),
                ..Default::default()
            },
        )?;
        Ok(purchase_id)
    })?;
    get_purchase(conn, user, purchase_id)
}

// ─── Snapshot audit ──────────────────────────────────────────────────────────

fn supplier_snapshot(conn: &Connection, id: i64) -> AppResult<Option<Value>> {
    Ok(repo::get_supplier(conn, id)?.map(|s| {
        json!({
            "code": s.code, "name": s.name, "address": s.address, "phone": s.phone, "npwp": s.npwp,
            "paymentTermDays": s.payment_term_days, "isActive": s.is_active,
        })
    }))
}

/// Isi faktur untuk log audit. Nomor internal menjadi kode, nama supplier menjadi nama.
fn purchase_snapshot(conn: &Connection, id: i64) -> AppResult<Value> {
    let h = repo::get_header(conn, id)?.ok_or_else(|| AppError::NotFound("Faktur tidak ditemukan".into()))?;
    let items: Map<String, Value> = repo::items(conn, id)?
        .iter()
        .map(|i| {
            (
                format!("{}. {} ({})", i.line_no, i.product_name, i.unit_name),
                json!({
                    "qty": i.qty, "bonusQty": i.bonus_qty, "unitPrice": i.unit_price,
                    "discount1Bp": i.discount1_bp, "discount2Bp": i.discount2_bp, "lineTotal": i.line_total,
                    "batchNumber": i.batch_number, "expiryDate": i.expiry_date,
                }),
            )
        })
        .collect();
    let r = h.row;
    Ok(json!({
        "code": r.number,
        "name": r.supplier_name,
        "invoiceNumber": r.invoice_number,
        "invoiceDate": r.invoice_date,
        "receivedDate": r.received_date,
        "dueDate": r.due_date,
        "paymentType": r.payment_type,
        "taxMode": h.tax_mode,
        "taxRateBp": h.tax_rate_bp,
        "extraDiscount": h.extra_discount,
        "grandTotal": r.grand_total,
        "note": h.note,
        "items": items,
    }))
}

/// HPP acuan dan harga jual per satuan, untuk log hitung ulang harga saat posting.
fn price_snapshot(conn: &Connection, product_id: i64) -> AppResult<Value> {
    let (code, name, cost): (String, String, Option<i64>) = conn.query_row(
        "SELECT code, name, last_cost_x100 FROM products WHERE id = ?1",
        [product_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let units: Map<String, Value> = repo::unit_prices(conn, product_id)?
        .into_iter()
        .map(|(unit, price)| (unit, json!({ "sellPrice": price })))
        .collect();
    Ok(json!({ "code": code, "name": name, "lastCostX100": cost, "units": units }))
}

// ─── Validasi umum ───────────────────────────────────────────────────────────

fn status_label(s: PurchaseStatus) -> &'static str {
    match s {
        PurchaseStatus::Draft => "draft",
        PurchaseStatus::Posted => "diposting",
        PurchaseStatus::Void => "dibatalkan",
    }
}

fn group_thousands(v: i64) -> String {
    let s = v.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    if v < 0 { format!("-{out}") } else { out }
}

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

fn parse_date(value: &str, label: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("{label} tidak valid (format YYYY-MM-DD)")))
}
