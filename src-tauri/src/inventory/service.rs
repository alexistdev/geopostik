use chrono::{Days, NaiveDate};
use rusqlite::{Connection, TransactionBehavior};
use serde_json::json;

use super::model::{
    OpnameCreateInput, OpnameDetail, OpnameFillInput, OpnameItemInput, OpnamePage, OpnamePageQuery, OpnameSaveResult,
    OpnameStatus, OpnameType, ProductBatches, StockCardPage, StockCardQuery, StockListQuery, StockPage,
};
use super::repo::{self, ItemFields, NewBatch, NewMovement};
use crate::audit::{self, Entry};
use crate::auth::{Permission, SessionUser};
use crate::db::sequence;
use crate::error::{AppError, AppResult};
use crate::settings;

/// Batch dengan ED sampai sekian hari ke depan dianggap "hampir ED" (FLOW.md: ≤ 3 bulan).
pub const NEAR_EXPIRY_DAYS: u64 = 90;

/// Pengaturan: stok awal sudah dikunci, opname stok awal baru ditolak.
pub const STOCK_OPENING_LOCKED: &str = "stock.opening_locked";

pub const ENTITY_OPNAME: &str = "stock_opnames";
pub const ENTITY_BATCH: &str = "batches";

pub const SUBMIT: &str = "SUBMIT";
pub const REOPEN: &str = "REOPEN";
pub const APPROVE: &str = "APPROVE";
pub const CANCEL: &str = "CANCEL";
pub const LOCK: &str = "LOCK";
pub const UNLOCK: &str = "UNLOCK";
pub const OPENING_LOCK: &str = "OPENING_LOCK";

fn view_cost(user: &SessionUser) -> bool {
    user.permissions.contains(&Permission::ViewCost)
}

// ─── Stok per obat & batch ───────────────────────────────────────────────────

pub fn list_stock(conn: &Connection, user: &SessionUser, query: &StockListQuery, today: NaiveDate) -> AppResult<StockPage> {
    let near = today.checked_add_days(Days::new(NEAR_EXPIRY_DAYS)).unwrap_or(today);
    let today_s = today.format("%Y-%m-%d").to_string();
    let (mut rows, total) = repo::list_stock(conn, query, &today_s, &near.format("%Y-%m-%d").to_string())?;
    if !view_cost(user) {
        rows.iter_mut().for_each(|r| r.stock_value = None);
    }
    Ok(StockPage { rows, total, today: today_s })
}

pub fn product_batches(conn: &Connection, user: &SessionUser, product_id: i64, include_empty: bool) -> AppResult<ProductBatches> {
    let p = repo::product_head(conn, product_id)?.ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
    let mut batches = repo::product_batches(conn, product_id, include_empty)?;
    if !view_cost(user) {
        batches.iter_mut().for_each(|b| b.unit_cost_x100 = None);
    }
    Ok(ProductBatches {
        product_id: p.id,
        code: p.code,
        name: p.name,
        base_unit_name: p.base_unit_name,
        batches,
    })
}

/// Kunci batch (recall, rusak, menunggu pemusnahan) agar tidak ikut FEFO, atau buka kuncinya.
pub fn set_batch_locked(conn: &Connection, user: &SessionUser, id: i64, locked: bool, reason: Option<&str>) -> AppResult<()> {
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    if locked && reason.is_none() {
        return Err(AppError::Validation("Alasan kunci batch wajib diisi".into()));
    }
    let tx = conn.unchecked_transaction()?;
    let (product_id, batch) = repo::get_batch(&tx, id)?.ok_or_else(|| AppError::NotFound("Batch tidak ditemukan".into()))?;
    if batch.is_locked == locked {
        return Ok(());
    }
    let product = repo::product_head(&tx, product_id)?.ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
    repo::set_batch_locked(&tx, id, locked, reason)?;
    audit::log(
        &tx,
        Entry {
            user_id: Some(user.id),
            action: if locked { LOCK } else { UNLOCK },
            entity: Some(ENTITY_BATCH),
            entity_id: Some(id),
            detail: Some(json!({
                "code": product.code,
                "name": product.name,
                "batchNumber": batch.batch_number,
                "expiryDate": batch.expiry_date,
            })),
            reason,
            ..Default::default()
        },
    )?;
    tx.commit()?;
    Ok(())
}

// ─── Kartu stok ──────────────────────────────────────────────────────────────

pub fn stock_card(conn: &Connection, query: &StockCardQuery) -> AppResult<StockCardPage> {
    for d in [&query.date_from, &query.date_to].into_iter().flatten() {
        parse_date(d, "Tanggal")?;
    }
    repo::product_head(conn, query.product_id)?.ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
    let totals = repo::stock_card_totals(conn, query)?;
    let rows = repo::stock_card(conn, query)?;
    Ok(StockCardPage {
        rows,
        total: totals.total,
        opening_balance: totals.opening,
        total_in: totals.total_in,
        total_out: totals.total_out,
        closing_balance: totals.opening + totals.total_in - totals.total_out,
    })
}

// ─── Stok opname ─────────────────────────────────────────────────────────────

pub fn opening_locked(conn: &Connection) -> AppResult<bool> {
    Ok(settings::get(conn, STOCK_OPENING_LOCKED)?.unwrap_or(false))
}

pub fn page_opnames(conn: &Connection, query: &OpnamePageQuery) -> AppResult<OpnamePage> {
    let (rows, total) = repo::page_opnames(conn, query)?;
    Ok(OpnamePage { rows, total, opening_locked: opening_locked(conn)? })
}

pub fn get_opname(conn: &Connection, user: &SessionUser, id: i64) -> AppResult<OpnameDetail> {
    let header = repo::get_opname(conn, id)?.ok_or_else(|| AppError::NotFound("Opname tidak ditemukan".into()))?;
    let mut items = repo::opname_items(conn, id)?;
    let difference_value = if view_cost(user) {
        let x100: i64 = items
            .iter()
            .filter_map(|i| Some((i.physical_qty_base? - i.system_qty_base) * i.unit_cost_x100?))
            .sum();
        Some(round_x100(x100))
    } else {
        items.iter_mut().for_each(|i| i.unit_cost_x100 = None);
        None
    };
    Ok(OpnameDetail { header, items, difference_value })
}

/// Rupiah × 100 → rupiah, dibulatkan setengah menjauhi nol.
fn round_x100(v: i64) -> i64 {
    if v >= 0 { (v + 50) / 100 } else { -((-v + 50) / 100) }
}

pub fn create_opname(conn: &mut Connection, user: &SessionUser, input: &OpnameCreateInput, today: NaiveDate) -> AppResult<OpnameDetail> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if input.opname_type == OpnameType::Opening && opening_locked(&tx)? {
        return Err(AppError::Conflict(
            "Stok awal sudah dikunci. Gunakan opname berkala untuk menyesuaikan stok.".into(),
        ));
    }
    let scope = optional(&input.scope_note);
    let number = sequence::next_number(&tx, "OP", &today.format("%y%m").to_string())?;
    let id = repo::insert_opname(&tx, &number, input.opname_type, scope, user.id)?;
    audit::log(
        &tx,
        Entry {
            user_id: Some(user.id),
            action: audit::CREATE,
            entity: Some(ENTITY_OPNAME),
            entity_id: Some(id),
            detail: Some(json!({ "code": number, "name": scope, "opnameType": input.opname_type })),
            ..Default::default()
        },
    )?;
    tx.commit()?;
    get_opname(conn, user, id)
}

fn require_status(conn: &Connection, id: i64, allowed: &[OpnameStatus], what: &str) -> AppResult<super::model::OpnameRow> {
    let header = repo::get_opname(conn, id)?.ok_or_else(|| AppError::NotFound("Opname tidak ditemukan".into()))?;
    if !allowed.contains(&header.status) {
        return Err(AppError::Conflict(format!(
            "Opname {} berstatus {}, tidak bisa {what}",
            header.number,
            status_label(header.status)
        )));
    }
    Ok(header)
}

fn status_label(s: OpnameStatus) -> &'static str {
    match s {
        OpnameStatus::Draft => "Draft",
        OpnameStatus::Submitted => "Diajukan",
        OpnameStatus::Approved => "Disetujui",
        OpnameStatus::Cancelled => "Dibatalkan",
    }
}

/// Tambah atau ubah satu baris opname (hanya saat DRAFT).
pub fn save_item(conn: &mut Connection, user: &SessionUser, input: &OpnameItemInput, today: NaiveDate) -> AppResult<OpnameSaveResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let header = require_status(&tx, input.opname_id, &[OpnameStatus::Draft], "diubah")?;
    let existing = match input.id {
        Some(id) => Some(
            repo::get_item(&tx, input.opname_id, id)?.ok_or_else(|| AppError::NotFound("Baris opname tidak ditemukan".into()))?,
        ),
        None => None,
    };
    let product = repo::product_head(&tx, input.product_id)?
        .filter(|p| !p.is_deleted)
        .ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
    if input.physical_qty_base.is_some_and(|q| q < 0) {
        return Err(AppError::Validation("Stok fisik tidak boleh negatif".into()));
    }
    if input.unit_cost_x100.is_some_and(|c| c < 0) {
        return Err(AppError::Validation("HPP tidak boleh negatif".into()));
    }
    let note = optional(&input.note);
    let mut warnings = Vec::new();
    let expiry_text: String;

    let fields = match input.batch_id {
        Some(batch_id) => {
            if header.opname_type == OpnameType::Opening {
                return Err(AppError::Validation(
                    "Stok awal hanya berisi batch baru; batch yang sudah ada dihitung lewat opname berkala".into(),
                ));
            }
            let (batch_product, batch) =
                repo::get_batch(&tx, batch_id)?.ok_or_else(|| AppError::NotFound("Batch tidak ditemukan".into()))?;
            if batch_product != product.id {
                return Err(AppError::Validation("Batch bukan milik obat ini".into()));
            }
            if repo::batch_in_opname(&tx, input.opname_id, batch_id, input.id)? {
                return Err(AppError::Conflict(format!(
                    "Batch {} sudah ada di opname ini",
                    batch.batch_number
                )));
            }
            // Snapshot stok sistem diambil saat batch pertama kali masuk opname.
            let system = match &existing {
                Some(e) if e.batch_id == Some(batch_id) => e.system_qty_base,
                _ => batch.qty_on_hand_base,
            };
            ItemFields {
                product_id: product.id,
                batch_id: Some(batch_id),
                batch_number: None,
                expiry_date: None,
                unit_cost_x100: None,
                system_qty_base: system,
                physical_qty_base: input.physical_qty_base,
                note,
            }
        }
        None => {
            let batch_number = input
                .batch_number
                .as_deref()
                .map(str::trim)
                .filter(|b| !b.is_empty())
                .ok_or_else(|| AppError::Validation("Nomor batch wajib diisi".into()))?;
            let expiry = input
                .expiry_date
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .ok_or_else(|| AppError::Validation("Tanggal ED wajib diisi".into()))?;
            let ed = parse_date(expiry, "Tanggal ED")?;
            expiry_text = ed.format("%Y-%m-%d").to_string();
            let expiry = expiry_text.as_str();
            if ed <= today {
                warnings.push(format!(
                    "{} batch {batch_number}: ED {expiry} sudah lewat, batch tidak bisa dijual",
                    product.name
                ));
            }
            // User tanpa VIEW_COST tidak melihat HPP lama; kosong berarti pertahankan.
            let cost = input
                .unit_cost_x100
                .or_else(|| existing.as_ref().filter(|e| e.batch_id.is_none()).and_then(|e| e.unit_cost_x100))
                .ok_or_else(|| AppError::Validation("HPP per satuan dasar wajib diisi untuk batch baru".into()))?;
            ItemFields {
                product_id: product.id,
                batch_id: None,
                batch_number: Some(batch_number),
                expiry_date: Some(expiry),
                unit_cost_x100: Some(cost),
                system_qty_base: 0,
                physical_qty_base: input.physical_qty_base,
                note,
            }
        }
    };

    match &existing {
        Some(e) => repo::update_item(&tx, e.id, &fields)?,
        None => {
            repo::insert_item(&tx, input.opname_id, &fields)?;
        }
    }
    tx.commit()?;
    Ok(OpnameSaveResult { opname: get_opname(conn, user, input.opname_id)?, warnings })
}

pub fn delete_item(conn: &mut Connection, user: &SessionUser, opname_id: i64, item_id: i64) -> AppResult<OpnameDetail> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    require_status(&tx, opname_id, &[OpnameStatus::Draft], "diubah")?;
    if repo::delete_item(&tx, opname_id, item_id)? == 0 {
        return Err(AppError::NotFound("Baris opname tidak ditemukan".into()));
    }
    tx.commit()?;
    get_opname(conn, user, opname_id)
}

/// Opname berkala: tambahkan semua batch ber-stok dalam cakupan (obat / rak / kategori) dengan
/// snapshot stok sistem saat ini. Batch yang sudah ada di opname dilewati.
pub fn fill_opname(conn: &mut Connection, user: &SessionUser, input: &OpnameFillInput) -> AppResult<OpnameSaveResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let header = require_status(&tx, input.opname_id, &[OpnameStatus::Draft], "diubah")?;
    if header.opname_type == OpnameType::Opening {
        return Err(AppError::Validation("Stok awal diisi per batch baru, bukan dari batch yang ada".into()));
    }
    let candidates = repo::fill_candidates(&tx, input.opname_id, input.product_id, input.rack_id, input.category_id)?;
    for &(batch_id, product_id, qty) in &candidates {
        repo::insert_item(
            &tx,
            input.opname_id,
            &ItemFields {
                product_id,
                batch_id: Some(batch_id),
                batch_number: None,
                expiry_date: None,
                unit_cost_x100: None,
                system_qty_base: qty,
                physical_qty_base: None,
                note: None,
            },
        )?;
    }
    tx.commit()?;
    let warnings = if candidates.is_empty() {
        vec!["Tidak ada batch ber-stok baru dalam cakupan ini".into()]
    } else {
        Vec::new()
    };
    Ok(OpnameSaveResult { opname: get_opname(conn, user, input.opname_id)?, warnings })
}

fn log_status(conn: &Connection, user: &SessionUser, header: &super::model::OpnameRow, action: &str, reason: Option<&str>) -> AppResult<()> {
    audit::log(
        conn,
        Entry {
            user_id: Some(user.id),
            action,
            entity: Some(ENTITY_OPNAME),
            entity_id: Some(header.id),
            detail: Some(json!({ "code": header.number, "name": header.scope_note, "opnameType": header.opname_type })),
            reason,
            ..Default::default()
        },
    )
}

/// Ajukan opname untuk disetujui. Semua baris wajib sudah dihitung.
pub fn submit_opname(conn: &mut Connection, user: &SessionUser, id: i64) -> AppResult<OpnameDetail> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let header = require_status(&tx, id, &[OpnameStatus::Draft], "diajukan")?;
    if header.item_count == 0 {
        return Err(AppError::Validation("Opname belum berisi obat".into()));
    }
    let uncounted = header.item_count - header.counted_count;
    if uncounted > 0 {
        return Err(AppError::Validation(format!("{uncounted} baris belum diisi stok fisik")));
    }
    repo::set_opname_status(&tx, id, OpnameStatus::Submitted)?;
    log_status(&tx, user, &header, SUBMIT, None)?;
    tx.commit()?;
    get_opname(conn, user, id)
}

/// Kembalikan opname yang sudah diajukan ke draft untuk diperbaiki.
pub fn reopen_opname(conn: &mut Connection, user: &SessionUser, id: i64) -> AppResult<OpnameDetail> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let header = require_status(&tx, id, &[OpnameStatus::Submitted], "dikembalikan ke draft")?;
    repo::set_opname_status(&tx, id, OpnameStatus::Draft)?;
    log_status(&tx, user, &header, REOPEN, None)?;
    tx.commit()?;
    get_opname(conn, user, id)
}

pub fn cancel_opname(conn: &mut Connection, user: &SessionUser, id: i64, reason: Option<&str>) -> AppResult<OpnameDetail> {
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let header = require_status(&tx, id, &[OpnameStatus::Draft, OpnameStatus::Submitted], "dibatalkan")?;
    repo::set_opname_status(&tx, id, OpnameStatus::Cancelled)?;
    log_status(&tx, user, &header, CANCEL, reason)?;
    tx.commit()?;
    get_opname(conn, user, id)
}

/// Setujui opname: batch baru dibuat, lalu satu baris kartu stok per selisih (`OPENING` untuk stok
/// awal, `ADJUSTMENT` untuk opname berkala). Semua dalam satu transaksi; stok minus menggagalkan
/// semuanya.
pub fn approve_opname(conn: &mut Connection, user: &SessionUser, id: i64) -> AppResult<OpnameDetail> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let header = require_status(&tx, id, &[OpnameStatus::Submitted], "disetujui")?;
    let (movement_type, source_type) = match header.opname_type {
        OpnameType::Opening => ("OPENING", "OPENING"),
        OpnameType::Periodic => ("ADJUSTMENT", "ADJUSTMENT"),
    };
    let note = format!("Opname {}", header.number);
    for item in repo::raw_items(&tx, id)? {
        let physical = item
            .physical_qty_base
            .ok_or_else(|| AppError::Validation("Ada baris yang belum diisi stok fisik".into()))?;
        let batch_id = match item.batch_id {
            Some(b) => b,
            None if physical == 0 => continue, // batch baru tanpa stok tidak perlu dibuat
            None => {
                let batch_id = repo::insert_batch(
                    &tx,
                    &NewBatch {
                        product_id: item.product_id,
                        batch_number: item.batch_number.as_deref().unwrap_or_default(),
                        expiry_date: item.expiry_date.as_deref().unwrap_or_default(),
                        unit_cost_x100: item.unit_cost_x100.unwrap_or_default(),
                        source_type,
                        source_id: item.id,
                    },
                )?;
                repo::link_item_batch(&tx, item.id, batch_id)?;
                batch_id
            }
        };
        let diff = physical - item.system_qty_base;
        if diff == 0 {
            continue;
        }
        let result = repo::insert_movement(
            &tx,
            &NewMovement {
                batch_id,
                product_id: item.product_id,
                movement_type,
                qty_change_base: diff,
                ref_type: "stock_opname",
                ref_id: id,
                ref_line_id: Some(item.id),
                user_id: user.id,
                note: item.note.as_deref().or(Some(&note)),
            },
        );
        if let Err(AppError::Database(rusqlite::Error::SqliteFailure(e, _))) = &result
            && e.code == rusqlite::ErrorCode::ConstraintViolation
        {
            let batch = repo::get_batch(&tx, batch_id)?.map(|(_, b)| b.batch_number).unwrap_or_default();
            return Err(AppError::Conflict(format!(
                "Stok batch {batch} sudah berubah sejak opname dimulai sehingga penyesuaian membuat stok minus. \
                 Kembalikan ke draft dan hitung ulang baris ini."
            )));
        }
        result?;
    }
    repo::approve_opname(&tx, id, user.id)?;
    log_status(&tx, user, &header, APPROVE, None)?;
    tx.commit()?;
    get_opname(conn, user, id)
}

/// Kunci stok awal: setelah ini stok hanya bisa berubah lewat transaksi biasa dan opname berkala.
pub fn lock_opening(conn: &mut Connection, user: &SessionUser) -> AppResult<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if opening_locked(&tx)? {
        return Ok(());
    }
    let open: i64 = tx.query_row(
        "SELECT count(*) FROM stock_opnames WHERE opname_type = 'OPENING' AND status IN ('DRAFT', 'SUBMITTED')",
        [],
        |r| r.get(0),
    )?;
    if open > 0 {
        return Err(AppError::Conflict(format!(
            "Masih ada {open} opname stok awal yang belum disetujui atau dibatalkan"
        )));
    }
    settings::set(&tx, STOCK_OPENING_LOCKED, &true)?;
    audit::log(
        &tx,
        Entry {
            user_id: Some(user.id),
            action: OPENING_LOCK,
            entity: Some(ENTITY_OPNAME),
            ..Default::default()
        },
    )?;
    tx.commit()?;
    Ok(())
}

// ─── Validasi umum ───────────────────────────────────────────────────────────

fn parse_date(value: &str, label: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("{label} harus berformat YYYY-MM-DD")))
}

fn optional(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}
