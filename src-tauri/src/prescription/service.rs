use std::collections::BTreeMap;

use chrono::{Local, NaiveDate};
use rusqlite::{Connection, TransactionBehavior};
use serde_json::{Map, Value, json};

use super::model::{
    Doctor, DoctorInput, DoctorPage, ItemKind, Patient, PatientInput, PatientPage, PrescriptionDetail,
    PrescriptionInput, PrescriptionItemDetail, PrescriptionItemInput, PrescriptionPage, PrescriptionQuery,
    PrescriptionStatus, ScreeningInput,
};
use super::pricing::price_for_qty;
use super::repo::{self, DoctorFields, HeaderFields, ItemFields, PatientFields, PersonTable};
use crate::audit::{self, Change};
use crate::auth::SessionUser;
use crate::db::sequence;
use crate::error::{AppError, AppResult};
use crate::master::{DrugClass, MasterPageQuery};

/// Aksi log audit khusus resep.
const SCREEN: &str = "SCREEN";
const CANCEL: &str = "CANCEL";

const ENTITY: &str = "prescriptions";
const MAX_QTY: i64 = 100_000;
const MAX_ITEMS: usize = 100;

// ─── Dokter ──────────────────────────────────────────────────────────────────

pub fn list_doctors(conn: &Connection) -> AppResult<Vec<Doctor>> {
    repo::list_doctors(conn)
}

pub fn page_doctors(conn: &Connection, query: &MasterPageQuery) -> AppResult<DoctorPage> {
    let (rows, total) = repo::page_doctors(conn, query)?;
    Ok(DoctorPage { rows, total })
}

pub fn save_doctor(conn: &Connection, user: &SessionUser, input: &DoctorInput) -> AppResult<Doctor> {
    let fields = DoctorFields {
        name: required(&input.name, "Nama dokter")?,
        sip_number: optional(&input.sip_number),
        specialty: optional(&input.specialty),
        address: optional(&input.address),
        phone: optional(&input.phone),
        is_active: input.is_active,
    };
    let tx = conn.unchecked_transaction()?;
    if let Some(sip) = fields.sip_number
        && let Some(other) = repo::doctor_sip_taken(&tx, sip, input.id)?
    {
        return Err(AppError::Conflict(format!("Nomor SIP {sip} sudah dipakai dokter {other}")));
    }
    let before = match input.id {
        Some(id) => Some(doctor_snapshot(&tx, id)?.ok_or_else(|| AppError::NotFound("Dokter tidak ditemukan".into()))?),
        None => None,
    };
    let id = match input.id {
        Some(id) => {
            repo::update_doctor(&tx, id, &fields)?;
            id
        }
        None => {
            let code = repo::next_code(&tx, PersonTable::Doctors)?;
            repo::insert_doctor(&tx, &code, &fields, user.id)?
        }
    };
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if before.is_some() { audit::UPDATE } else { audit::CREATE },
            entity: PersonTable::Doctors.table(),
            entity_id: id,
            before,
            after: doctor_snapshot(&tx, id)?,
            reason: None,
        },
    )?;
    tx.commit()?;
    repo::get_doctor(conn, id)?.ok_or_else(|| AppError::Internal("dokter hilang setelah disimpan".into()))
}

// ─── Pasien ──────────────────────────────────────────────────────────────────

pub fn page_patients(conn: &Connection, query: &MasterPageQuery, active_only: bool) -> AppResult<PatientPage> {
    let (rows, total) = repo::page_patients(conn, query, active_only)?;
    Ok(PatientPage { rows, total })
}

pub fn save_patient(conn: &Connection, user: &SessionUser, input: &PatientInput) -> AppResult<Patient> {
    let birth_date = optional(&input.birth_date);
    if let Some(d) = birth_date {
        let date = parse_date(d, "Tanggal lahir")?;
        if date > today() {
            return Err(AppError::Validation("Tanggal lahir tidak boleh setelah hari ini".into()));
        }
    }
    let fields = PatientFields {
        name: required(&input.name, "Nama pasien")?,
        gender: input.gender,
        birth_date,
        address: optional(&input.address),
        phone: optional(&input.phone),
        is_active: input.is_active,
    };
    let tx = conn.unchecked_transaction()?;
    let before = match input.id {
        Some(id) => Some(patient_snapshot(&tx, id)?.ok_or_else(|| AppError::NotFound("Pasien tidak ditemukan".into()))?),
        None => None,
    };
    let id = match input.id {
        Some(id) => {
            repo::update_patient(&tx, id, &fields)?;
            id
        }
        None => {
            let code = repo::next_code(&tx, PersonTable::Patients)?;
            repo::insert_patient(&tx, &code, &fields, user.id)?
        }
    };
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if before.is_some() { audit::UPDATE } else { audit::CREATE },
            entity: PersonTable::Patients.table(),
            entity_id: id,
            before,
            after: patient_snapshot(&tx, id)?,
            reason: None,
        },
    )?;
    tx.commit()?;
    repo::get_patient(conn, id)?.ok_or_else(|| AppError::Internal("pasien hilang setelah disimpan".into()))
}

// ─── Aksi bersama dokter & pasien ────────────────────────────────────────────

fn person_label(t: PersonTable) -> &'static str {
    match t {
        PersonTable::Doctors => "Dokter",
        PersonTable::Patients => "Pasien",
    }
}

fn person_snapshot(conn: &Connection, t: PersonTable, id: i64) -> AppResult<Option<Value>> {
    match t {
        PersonTable::Doctors => doctor_snapshot(conn, id),
        PersonTable::Patients => patient_snapshot(conn, id),
    }
}

pub fn set_person_active(conn: &Connection, user: &SessionUser, t: PersonTable, id: i64, active: bool) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    let before = person_snapshot(&tx, t, id)?;
    if repo::set_active(&tx, t, id, active)? == 0 {
        return Err(AppError::NotFound(format!("{} tidak ditemukan", person_label(t))));
    }
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if active { audit::ACTIVATE } else { audit::DEACTIVATE },
            entity: t.table(),
            entity_id: id,
            before,
            after: person_snapshot(&tx, t, id)?,
            reason: None,
        },
    )?;
    tx.commit()?;
    Ok(())
}

/// Hapus (soft delete). Ditolak bila sudah tercatat di resep, agar riwayat resep tetap utuh;
/// untuk data yang sudah dipakai, nonaktifkan saja.
pub fn delete_person(conn: &mut Connection, user: &SessionUser, t: PersonTable, id: i64) -> AppResult<()> {
    let label = person_label(t);
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let before = person_snapshot(&tx, t, id)?.ok_or_else(|| AppError::NotFound(format!("{label} tidak ditemukan")))?;
    let used = repo::usage(&tx, t, id)?;
    if used > 0 {
        return Err(AppError::Conflict(format!(
            "{label} {} tercatat di {used} resep sehingga tidak bisa dihapus. Nonaktifkan saja.",
            before["name"].as_str().unwrap_or_default()
        )));
    }
    repo::soft_delete(&tx, t, id, user.id)?;
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: audit::DELETE,
            entity: t.table(),
            entity_id: id,
            before: Some(before),
            after: None,
            reason: None,
        },
    )?;
    tx.commit()?;
    Ok(())
}

// ─── Resep ───────────────────────────────────────────────────────────────────

pub fn page_prescriptions(conn: &Connection, query: &PrescriptionQuery) -> AppResult<PrescriptionPage> {
    let (rows, total) = repo::page_prescriptions(conn, query)?;
    Ok(PrescriptionPage {
        rows,
        total,
        draft_count: repo::count_by_status(conn, PrescriptionStatus::Draft)?,
        screened_count: repo::count_by_status(conn, PrescriptionStatus::Screened)?,
    })
}

pub fn get_prescription(conn: &Connection, id: i64) -> AppResult<PrescriptionDetail> {
    let h = repo::get_header(conn, id)?.ok_or_else(|| AppError::NotFound("Resep tidak ditemukan".into()))?;
    let rows = repo::items(conn, id)?;

    // Total kebutuhan per obat (semua baris + komponen racikan) untuk dibandingkan dengan stok.
    let mut needed: BTreeMap<i64, (String, i64)> = BTreeMap::new();
    let mut stock: BTreeMap<i64, i64> = BTreeMap::new();
    for r in &rows {
        if let Some(pid) = r.product_id {
            needed.entry(pid).or_insert((r.description.clone(), 0)).1 += r.qty_base;
            if !stock.contains_key(&pid) {
                stock.insert(pid, repo::sellable_stock(conn, pid)?);
            }
        }
    }

    let detail = |r: &repo::ItemRow| PrescriptionItemDetail {
        id: r.id,
        line_no: r.line_no,
        kind: r.kind,
        product_id: r.product_id,
        product_unit_id: r.product_unit_id,
        product_code: r.product_code.clone(),
        unit_name: r.unit_name.clone(),
        drug_class: r.drug_class,
        description: r.description.clone(),
        compound_form: r.compound_form,
        qty: r.qty,
        conversion: r.conversion,
        qty_base: r.qty_base,
        unit_price: r.unit_price,
        tier_min_qty: r.tier_min_qty,
        line_total: r.line_total,
        usage_instruction: r.usage_instruction.clone(),
        stock_base: r.product_id.and_then(|p| stock.get(&p).copied()),
        components: Vec::new(),
    };
    let mut items: Vec<PrescriptionItemDetail> = Vec::new();
    for r in rows.iter().filter(|r| r.parent_item_id.is_none()) {
        let mut item = detail(r);
        item.components = rows.iter().filter(|c| c.parent_item_id == Some(r.id)).map(detail).collect();
        items.push(item);
    }
    let total = items.iter().map(|i| i.line_total).sum();

    let open = matches!(h.status, PrescriptionStatus::Draft | PrescriptionStatus::Screened);
    let warnings = if open {
        needed
            .iter()
            .filter_map(|(pid, (name, qty))| {
                let have = stock.get(pid).copied().unwrap_or(0);
                (*qty > have).then(|| format!("Stok {name} kurang: dibutuhkan {qty}, tersedia {have} (satuan terkecil)"))
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(PrescriptionDetail {
        id: h.id,
        number: h.number,
        prescription_number: h.prescription_number,
        prescription_date: h.prescription_date,
        doctor_id: h.doctor_id,
        doctor_name: h.doctor_name,
        doctor_sip_number: h.doctor_sip_number,
        customer_id: h.customer_id,
        patient_name: h.patient_name,
        patient_age: h.patient_age,
        patient_address: h.patient_address,
        note: h.note,
        status: h.status,
        screened_by: h.screened_by,
        screened_at: h.screened_at,
        screening_note: h.screening_note,
        created_at: h.created_at,
        created_by: h.created_by,
        cancelled_at: h.cancelled_at,
        cancelled_by: h.cancelled_by,
        cancel_reason: h.cancel_reason,
        items,
        total,
        warnings,
    })
}

/// Baris siap simpan: baris utama beserta komponennya (untuk racikan).
struct PreparedLine {
    fields: OwnedItem,
    components: Vec<OwnedItem>,
}

struct OwnedItem {
    kind: ItemKind,
    product_id: Option<i64>,
    product_unit_id: Option<i64>,
    description: String,
    compound_form: Option<super::model::CompoundForm>,
    qty: i64,
    conversion: i64,
    unit_price: i64,
    price_tier_id: Option<i64>,
    line_total: i64,
    usage_instruction: Option<String>,
    drug_class: Option<DrugClass>,
}

fn validate_qty(qty: i64, what: &str) -> AppResult<()> {
    if !(1..=MAX_QTY).contains(&qty) {
        return Err(AppError::Validation(format!("Jumlah {what} harus antara 1 dan {MAX_QTY}")));
    }
    Ok(())
}

/// Baris obat (baris utama atau komponen racikan) dengan harga dihitung sistem.
fn product_line(conn: &Connection, product_unit_id: i64, qty: i64, usage: Option<String>) -> AppResult<OwnedItem> {
    let unit = repo::sale_unit(conn, product_unit_id)?.ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
    if !unit.usable {
        return Err(AppError::Validation(format!(
            "{} ({}) sudah nonaktif atau dihapus, pilih obat/satuan lain",
            unit.product_name, unit.unit_name
        )));
    }
    validate_qty(qty, &unit.product_name)?;
    let price = price_for_qty(conn, product_unit_id, qty)?;
    Ok(OwnedItem {
        kind: ItemKind::Product,
        product_id: Some(unit.product_id),
        product_unit_id: Some(product_unit_id),
        description: unit.product_name,
        compound_form: None,
        qty,
        conversion: unit.conversion,
        unit_price: price.unit_price,
        price_tier_id: price.tier_id,
        line_total: price.unit_price * qty,
        usage_instruction: usage,
        drug_class: Some(unit.drug_class),
    })
}

fn prepare_line(conn: &Connection, no: usize, item: &PrescriptionItemInput) -> AppResult<PreparedLine> {
    let usage = optional(&item.usage_instruction).map(str::to_owned);
    match item.kind {
        ItemKind::Product => {
            let unit_id = item
                .product_unit_id
                .ok_or_else(|| AppError::Validation(format!("Baris {no}: pilih obat dan satuannya")))?;
            Ok(PreparedLine { fields: product_line(conn, unit_id, item.qty, usage)?, components: Vec::new() })
        }
        ItemKind::Compound => {
            let name = required(item.description.as_deref().unwrap_or(""), &format!("Baris {no}: nama racikan"))?;
            let form = item
                .compound_form
                .ok_or_else(|| AppError::Validation(format!("Racikan {name}: pilih bentuk sediaan")))?;
            validate_qty(item.qty, &format!("racikan {name}"))?;
            if item.components.is_empty() {
                return Err(AppError::Validation(format!("Racikan {name} belum berisi obat")));
            }
            let components = item
                .components
                .iter()
                .map(|c| product_line(conn, c.product_unit_id, c.qty, None))
                .collect::<AppResult<Vec<_>>>()?;
            let total = components.iter().map(|c| c.line_total).sum();
            Ok(PreparedLine {
                fields: OwnedItem {
                    kind: ItemKind::Compound,
                    product_id: None,
                    product_unit_id: None,
                    description: name.to_owned(),
                    compound_form: Some(form),
                    qty: item.qty,
                    conversion: 1,
                    unit_price: 0,
                    price_tier_id: None,
                    line_total: total,
                    usage_instruction: usage,
                    drug_class: None,
                },
                components,
            })
        }
        ItemKind::Service => {
            let name = required(item.description.as_deref().unwrap_or(""), &format!("Baris {no}: nama jasa"))?;
            validate_qty(item.qty, name)?;
            let price = item.unit_price.unwrap_or(-1);
            if !(0..=100_000_000).contains(&price) {
                return Err(AppError::Validation(format!("Tarif {name} tidak valid")));
            }
            Ok(PreparedLine {
                fields: OwnedItem {
                    kind: ItemKind::Service,
                    product_id: None,
                    product_unit_id: None,
                    description: name.to_owned(),
                    compound_form: None,
                    qty: item.qty,
                    conversion: 1,
                    unit_price: price,
                    price_tier_id: None,
                    line_total: price * item.qty,
                    usage_instruction: None,
                    drug_class: None,
                },
                components: Vec::new(),
            })
        }
    }
}

fn is_controlled(class: Option<DrugClass>) -> bool {
    matches!(class, Some(DrugClass::Psychotropic | DrugClass::Narcotic))
}

pub fn save_prescription(conn: &mut Connection, user: &SessionUser, input: &PrescriptionInput) -> AppResult<PrescriptionDetail> {
    let prescription_number = required(&input.prescription_number, "Nomor resep")?;
    let date = parse_date(input.prescription_date.trim(), "Tanggal resep")?;
    if date > today() {
        return Err(AppError::Validation("Tanggal resep tidak boleh setelah hari ini".into()));
    }
    let date = date.format("%Y-%m-%d").to_string();
    let patient_name = required(&input.patient_name, "Nama pasien")?;
    if input.items.is_empty() {
        return Err(AppError::Validation("Resep belum berisi obat".into()));
    }
    if input.items.len() > MAX_ITEMS {
        return Err(AppError::Validation(format!("Resep maksimal {MAX_ITEMS} baris")));
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let before = match input.id {
        Some(id) => {
            match repo::status_of(&tx, id)? {
                None => return Err(AppError::NotFound("Resep tidak ditemukan".into())),
                Some(PrescriptionStatus::Draft | PrescriptionStatus::Screened) => {}
                Some(_) => return Err(AppError::Conflict("Resep sudah dibayar atau dibatalkan, tidak bisa diubah".into())),
            }
            Some(prescription_snapshot(&tx, id)?)
        }
        None => None,
    };
    let old_doctor = match input.id {
        Some(id) => repo::get_header(&tx, id)?.map(|h| h.doctor_id),
        None => None,
    };
    match repo::doctor_selectable(&tx, input.doctor_id)? {
        None => return Err(AppError::Validation("Pilih dokter penulis resep".into())),
        // Dokter yang sudah nonaktif masih boleh dipakai resep lama yang sedang diubah.
        Some(false) if old_doctor != Some(input.doctor_id) => {
            return Err(AppError::Validation("Dokter sudah nonaktif, pilih dokter lain".into()));
        }
        Some(_) => {}
    }
    if let Some(cid) = input.customer_id
        && !repo::patient_exists(&tx, cid)?
    {
        return Err(AppError::Validation("Pasien tidak ditemukan".into()));
    }

    let lines = input
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| prepare_line(&tx, i + 1, item))
        .collect::<AppResult<Vec<_>>>()?;

    let patient_address = optional(&input.patient_address);
    let controlled = lines
        .iter()
        .flat_map(|l| std::iter::once(&l.fields).chain(&l.components))
        .any(|i| is_controlled(i.drug_class));
    if controlled && patient_address.is_none() {
        return Err(AppError::Validation(
            "Resep berisi narkotika/psikotropika: alamat pasien wajib diisi untuk laporan SIPNAP".into(),
        ));
    }

    let header = HeaderFields {
        prescription_number,
        prescription_date: &date,
        doctor_id: input.doctor_id,
        customer_id: input.customer_id,
        patient_name,
        patient_age: optional(&input.patient_age),
        patient_address,
        note: optional(&input.note),
    };
    let id = match input.id {
        Some(id) => {
            repo::update_header(&tx, id, &header)?;
            repo::delete_items(&tx, id)?;
            id
        }
        None => {
            let number = sequence::next_number(&tx, "RSP", &Local::now().format("%y%m").to_string())?;
            repo::insert_header(&tx, &number, &header, user.id)?
        }
    };

    for (i, line) in lines.iter().enumerate() {
        let line_no = i as i64 + 1;
        let parent = insert_owned(&tx, id, line_no, None, &line.fields)?;
        for c in &line.components {
            insert_owned(&tx, id, line_no, Some(parent), c)?;
        }
    }

    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: if before.is_some() { audit::UPDATE } else { audit::CREATE },
            entity: ENTITY,
            entity_id: id,
            before,
            after: Some(prescription_snapshot(&tx, id)?),
            reason: None,
        },
    )?;
    tx.commit()?;
    get_prescription(conn, id)
}

fn insert_owned(conn: &Connection, prescription_id: i64, line_no: i64, parent: Option<i64>, i: &OwnedItem) -> AppResult<i64> {
    repo::insert_item(
        conn,
        prescription_id,
        &ItemFields {
            line_no,
            kind: i.kind,
            parent_item_id: parent,
            product_id: i.product_id,
            product_unit_id: i.product_unit_id,
            description: &i.description,
            compound_form: i.compound_form,
            qty: i.qty,
            conversion: i.conversion,
            unit_price: i.unit_price,
            price_tier_id: i.price_tier_id,
            line_total: i.line_total,
            usage_instruction: i.usage_instruction.as_deref(),
        },
    )
}

/// Skrining/validasi apoteker: resep siap dibayar di kasir.
pub fn screen_prescription(conn: &mut Connection, user: &SessionUser, input: &ScreeningInput) -> AppResult<PrescriptionDetail> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match repo::status_of(&tx, input.id)? {
        None => return Err(AppError::NotFound("Resep tidak ditemukan".into())),
        Some(PrescriptionStatus::Draft) => {}
        Some(PrescriptionStatus::Screened) => return Err(AppError::Conflict("Resep sudah divalidasi".into())),
        Some(_) => return Err(AppError::Conflict("Resep sudah dibayar atau dibatalkan".into())),
    }
    let before = prescription_snapshot(&tx, input.id)?;
    repo::mark_screened(&tx, input.id, user.id, optional(&input.note))?;
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: SCREEN,
            entity: ENTITY,
            entity_id: input.id,
            before: Some(before),
            after: Some(prescription_snapshot(&tx, input.id)?),
            reason: optional(&input.note),
        },
    )?;
    tx.commit()?;
    get_prescription(conn, input.id)
}

pub fn cancel_prescription(conn: &mut Connection, user: &SessionUser, id: i64, reason: &str) -> AppResult<PrescriptionDetail> {
    let reason = required(reason, "Alasan batal")?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match repo::status_of(&tx, id)? {
        None => return Err(AppError::NotFound("Resep tidak ditemukan".into())),
        Some(PrescriptionStatus::Draft | PrescriptionStatus::Screened) => {}
        Some(PrescriptionStatus::Paid) => return Err(AppError::Conflict("Resep sudah dibayar; batalkan lewat void penjualan".into())),
        Some(PrescriptionStatus::Cancelled) => return Err(AppError::Conflict("Resep sudah dibatalkan".into())),
    }
    let before = prescription_snapshot(&tx, id)?;
    repo::mark_cancelled(&tx, id, user.id, reason)?;
    audit::log_change(
        &tx,
        Change {
            user_id: user.id,
            action: CANCEL,
            entity: ENTITY,
            entity_id: id,
            before: Some(before),
            after: Some(prescription_snapshot(&tx, id)?),
            reason: Some(reason),
        },
    )?;
    tx.commit()?;
    get_prescription(conn, id)
}

// ─── Snapshot audit ──────────────────────────────────────────────────────────

fn doctor_snapshot(conn: &Connection, id: i64) -> AppResult<Option<Value>> {
    Ok(repo::get_doctor(conn, id)?.map(|d| {
        json!({
            "code": d.code, "name": d.name, "sipNumber": d.sip_number, "specialty": d.specialty,
            "address": d.address, "phone": d.phone, "isActive": d.is_active,
        })
    }))
}

fn patient_snapshot(conn: &Connection, id: i64) -> AppResult<Option<Value>> {
    Ok(repo::get_patient(conn, id)?.map(|p| {
        json!({
            "code": p.code, "name": p.name, "gender": p.gender, "birthDate": p.birth_date,
            "address": p.address, "phone": p.phone, "isActive": p.is_active,
        })
    }))
}

/// Isi resep untuk log audit. Nomor internal menjadi kode, nama pasien menjadi nama.
fn prescription_snapshot(conn: &Connection, id: i64) -> AppResult<Value> {
    let p = get_prescription(conn, id)?;
    let describe = |i: &PrescriptionItemDetail| match &i.unit_name {
        Some(unit) => format!("{} {} {}", i.qty, unit, i.description),
        None => format!("{} × {}", i.qty, i.description),
    };
    let items: Map<String, Value> = p
        .items
        .iter()
        .enumerate()
        .map(|(n, i)| {
            let mut v = json!({ "qty": i.qty, "lineTotal": i.line_total, "usageInstruction": i.usage_instruction });
            if i.kind == ItemKind::Product {
                v["unitPrice"] = json!(i.unit_price);
            }
            if i.kind == ItemKind::Service {
                v["price"] = json!(i.unit_price);
            }
            if !i.components.is_empty() {
                v["components"] = json!(i.components.iter().map(describe).collect::<Vec<_>>().join(", "));
            }
            let unit = i.unit_name.as_deref().map(|u| format!(" ({u})")).unwrap_or_default();
            (format!("{}. {}{unit}", n + 1, i.description), v)
        })
        .collect();
    Ok(json!({
        "code": p.number,
        "name": p.patient_name,
        "prescriptionNumber": p.prescription_number,
        "prescriptionDate": p.prescription_date,
        "doctor": p.doctor_name,
        "patientAge": p.patient_age,
        "patientAddress": p.patient_address,
        "note": p.note,
        "status": p.status,
        "screeningNote": p.screening_note,
        "total": p.total,
        "items": items,
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

fn parse_date(value: &str, label: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("{label} tidak valid (format YYYY-MM-DD)")))
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}
