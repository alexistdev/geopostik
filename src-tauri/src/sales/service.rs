use std::collections::BTreeMap;

use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use serde_json::json;

use super::model::{
    PaymentDetail, PaymentInput, PaymentMethod, PosPrescription, PosProduct, PosState, PrescriptionQuote,
    QuoteLine, SaleDetail, SaleInput, SaleItemDetail, SalePage, SaleQuery, SaleVoidInput, ShiftCloseInput,
    ShiftFigures, ShiftOpenInput, ShiftSummary,
};
use super::repo::{self, NewItem, NewSale, ShiftRow};
use crate::audit;
use crate::auth::{Authorizer, Permission, SessionUser, authorize_pin};
use crate::db::{self, sequence};
use crate::error::{AppError, AppResult};
use crate::master::DrugClass;
use crate::prescription::pricing::price_for_qty;
use crate::settings;

/// Diskon per baris maksimal tanpa otorisasi (basis point dari harga baris).
pub const MAX_DISCOUNT_BP: &str = "sales.max_discount_bp";
/// Total nota dibulatkan ke bawah ke kelipatan ini (rupiah); 0 = tanpa pembulatan.
pub const TOTAL_ROUNDING: &str = "sales.total_rounding";

const MAX_QTY: i64 = 100_000;
const MAX_LINES: usize = 200;
const MAX_MONEY: i64 = 10_000_000_000;
const ENTITY: &str = "sales";

/// Waktu transaksi. Diambil sekali per command agar semua baris satu transaksi memakai waktu yang
/// sama, dan bisa diisi tetap saat test.
#[derive(Debug, Clone)]
pub struct Clock {
    pub today: NaiveDate,
    pub now: String,
}

impl Clock {
    pub fn now() -> Self {
        let now = Local::now();
        Self { today: now.date_naive(), now: now.format("%Y-%m-%d %H:%M:%S").to_string() }
    }

    fn today_s(&self) -> String {
        self.today.format("%Y-%m-%d").to_string()
    }
}

fn can(user: &SessionUser, p: Permission) -> bool {
    user.permissions.contains(&p)
}

fn rupiah(v: i64) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    format!("{}Rp{out}", if v < 0 { "-" } else { "" })
}

struct Rules {
    max_discount_bp: i64,
    total_rounding: i64,
}

fn rules(conn: &Connection) -> AppResult<Rules> {
    Ok(Rules {
        max_discount_bp: settings::get(conn, MAX_DISCOUNT_BP)?.unwrap_or(1_000),
        total_rounding: settings::get(conn, TOTAL_ROUNDING)?.unwrap_or(0),
    })
}

/// Pembulatan ke bawah (menguntungkan pembeli): nilai ≤ 0 yang ditambahkan ke total.
fn rounding_for(total: i64, unit: i64) -> i64 {
    if unit > 1 { -(total % unit) } else { 0 }
}

// ─── Status kasir & shift ────────────────────────────────────────────────────

pub fn pos_state(conn: &Connection, user: &SessionUser) -> AppResult<PosState> {
    let r = rules(conn)?;
    let shift = repo::open_shift(conn)?.map(|s| summary(conn, user, s)).transpose()?;
    Ok(PosState {
        shift,
        max_discount_bp: r.max_discount_bp,
        total_rounding: r.total_rounding,
        prescriptions_ready: repo::count_ready_prescriptions(conn)?,
    })
}

fn summary(conn: &Connection, user: &SessionUser, s: ShiftRow) -> AppResult<ShiftSummary> {
    let is_mine = s.user_id == user.id;
    let figures = if is_mine || can(user, Permission::ReportSales) {
        let t = repo::shift_totals(conn, s.id)?;
        Some(ShiftFigures {
            opening_cash: s.opening_cash,
            sale_count: t.sale_count,
            sales_total: t.sales_total,
            void_count: t.void_count,
            cash_in: t.cash_in,
            expected_cash: s.opening_cash + t.cash_in,
            by_method: t.by_method,
            counted_cash: s.counted_cash,
            difference: s.difference,
        })
    } else {
        None
    };
    Ok(ShiftSummary {
        id: s.id,
        opened_by_id: s.user_id,
        opened_by: s.opened_by,
        opened_at: s.opened_at,
        is_mine,
        status: s.status,
        closed_at: s.closed_at,
        figures,
    })
}

fn money(v: i64, label: &str) -> AppResult<i64> {
    if !(0..=MAX_MONEY).contains(&v) {
        return Err(AppError::Validation(format!("{label} tidak valid")));
    }
    Ok(v)
}

fn note(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

pub fn open_shift(conn: &mut Connection, user: &SessionUser, input: &ShiftOpenInput, clock: &Clock) -> AppResult<ShiftSummary> {
    let cash = money(input.opening_cash, "Uang modal awal")?;
    let id = db::write_tx(conn, |tx| {
        if let Some(open) = repo::open_shift(tx)? {
            return Err(AppError::Conflict(if open.user_id == user.id {
                "Shift Anda sudah terbuka".into()
            } else {
                format!("Shift masih dibuka oleh {}. Tutup shift tersebut terlebih dahulu.", open.opened_by)
            }));
        }
        let id = repo::insert_shift(tx, user.id, cash, note(&input.note), &clock.now)?;
        audit::log(
            tx,
            audit::Entry {
                user_id: Some(user.id),
                action: "SHIFT_OPEN",
                entity: Some("shifts"),
                entity_id: Some(id),
                detail: Some(json!({ "openingCash": cash })),
                ..Default::default()
            },
        )?;
        Ok(id)
    })?;
    let row = repo::get_shift(conn, id)?.ok_or_else(|| AppError::Internal("shift hilang".into()))?;
    summary(conn, user, row)
}

pub fn close_shift(conn: &mut Connection, user: &SessionUser, input: &ShiftCloseInput, clock: &Clock) -> AppResult<ShiftSummary> {
    let counted = money(input.counted_cash, "Uang fisik di laci")?;
    db::write_tx(conn, |tx| {
        let open = repo::open_shift(tx)?.ok_or_else(|| AppError::Conflict("Tidak ada shift yang terbuka".into()))?;
        if open.id != input.shift_id {
            return Err(AppError::Conflict("Shift sudah berganti. Muat ulang layar kasir.".into()));
        }
        if open.user_id != user.id && !can(user, Permission::ReportSales) {
            return Err(AppError::Validation(format!(
                "Shift ini hanya bisa ditutup oleh {} atau Pemilik",
                open.opened_by
            )));
        }
        // Dihitung di dalam transaksi tulis: tidak ada nota yang bisa masuk di antara hitung dan tutup.
        let t = repo::shift_totals(tx, open.id)?;
        let expected = open.opening_cash + t.cash_in;
        if repo::close_shift(tx, open.id, expected, counted, note(&input.note), &clock.now)? != 1 {
            return Err(AppError::Conflict("Shift sudah ditutup".into()));
        }
        audit::log(
            tx,
            audit::Entry {
                user_id: Some(user.id),
                action: "SHIFT_CLOSE",
                entity: Some("shifts"),
                entity_id: Some(open.id),
                detail: Some(json!({
                    "openingCash": open.opening_cash,
                    "cashIn": t.cash_in,
                    "expectedCash": expected,
                    "countedCash": counted,
                    "difference": counted - expected,
                    "saleCount": t.sale_count,
                    "salesTotal": t.sales_total,
                })),
                reason: note(&input.note),
                ..Default::default()
            },
        )?;
        Ok(())
    })?;
    let row = repo::get_shift(conn, input.shift_id)?.ok_or_else(|| AppError::Internal("shift hilang".into()))?;
    summary(conn, user, row)
}

// ─── Pencarian ───────────────────────────────────────────────────────────────

pub fn search(conn: &Connection, q: &str, clock: &Clock) -> AppResult<Vec<PosProduct>> {
    let q = q.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    repo::search_products(conn, q, &clock.today_s(), 15)
}

// ─── Susun baris nota ────────────────────────────────────────────────────────

/// Satu baris nota sebelum disimpan. Komponen racikan menunjuk ke indeks baris induknya.
struct Line {
    kind: &'static str,
    parent: Option<usize>,
    product_id: Option<i64>,
    product_unit_id: Option<i64>,
    description: String,
    unit_name: Option<String>,
    qty: i64,
    conversion: i64,
    unit_price: i64,
    tier_id: Option<i64>,
    discount: i64,
    /// Nilai yang masuk subtotal (0 untuk komponen racikan; nilainya ada di baris induk).
    line_total: i64,
    usage: Option<String>,
    drug_class: Option<DrugClass>,
    /// Diskon melebihi batas → butuh otorisasi.
    discount_override: bool,
}

impl Line {
    fn qty_base(&self) -> i64 {
        self.qty * self.conversion
    }
}

struct Totals {
    subtotal: i64,
    discount_total: i64,
    rounding: i64,
    grand_total: i64,
}

fn totals(lines: &[Line], rounding_unit: i64) -> Totals {
    let top = lines.iter().filter(|l| l.parent.is_none());
    let net: i64 = top.clone().map(|l| l.line_total).sum();
    let discount_total: i64 = top.map(|l| l.discount).sum();
    let rounding = rounding_for(net, rounding_unit);
    Totals { subtotal: net + discount_total, discount_total, rounding, grand_total: net + rounding }
}

fn check_qty(qty: i64, what: &str) -> AppResult<()> {
    if !(1..=MAX_QTY).contains(&qty) {
        return Err(AppError::Validation(format!("Jumlah {what} harus antara 1 dan {MAX_QTY}")));
    }
    Ok(())
}

/// Baris penjualan bebas. Harga selalu dihitung ulang di sini (harga dari layar tidak dipercaya).
fn otc_lines(conn: &Connection, input: &SaleInput, max_discount_bp: i64) -> AppResult<Vec<Line>> {
    if input.items.is_empty() {
        return Err(AppError::Validation("Keranjang masih kosong".into()));
    }
    if input.items.len() > MAX_LINES {
        return Err(AppError::Validation(format!("Satu nota maksimal {MAX_LINES} baris")));
    }
    let mut lines = Vec::with_capacity(input.items.len());
    for it in &input.items {
        let unit = repo::sale_unit(conn, it.product_unit_id)?
            .ok_or_else(|| AppError::NotFound("Obat tidak ditemukan".into()))?;
        let label = format!("{} ({})", unit.product_name, unit.unit_name);
        if !unit.usable {
            return Err(AppError::Validation(format!("{label} sudah nonaktif, hapus dari keranjang")));
        }
        if matches!(unit.drug_class, DrugClass::Psychotropic | DrugClass::Narcotic) {
            return Err(AppError::Validation(format!(
                "{} golongan psikotropika/narkotika hanya bisa dijual lewat resep",
                unit.product_name
            )));
        }
        check_qty(it.qty, &label)?;
        let price = price_for_qty(conn, it.product_unit_id, it.qty)?;
        let gross = price.unit_price * it.qty;
        if it.discount_amount < 0 || it.discount_amount > gross {
            return Err(AppError::Validation(format!("Diskon {label} tidak boleh melebihi harga baris")));
        }
        lines.push(Line {
            kind: "PRODUCT",
            parent: None,
            product_id: Some(unit.product_id),
            product_unit_id: Some(it.product_unit_id),
            description: unit.product_name,
            unit_name: Some(unit.unit_name),
            qty: it.qty,
            conversion: unit.conversion,
            unit_price: price.unit_price,
            tier_id: price.tier_id,
            discount: it.discount_amount,
            line_total: gross - it.discount_amount,
            usage: None,
            drug_class: Some(unit.drug_class),
            discount_override: it.discount_amount * 10_000 > gross * max_discount_bp,
        });
    }
    Ok(lines)
}

/// Baris resep dengan harga terbaru, beserta masalah yang membuat resep belum bisa dibayar.
fn rx_lines(conn: &Connection, prescription_id: i64, today: &str) -> AppResult<(Vec<Line>, Vec<String>)> {
    let items = repo::rx_items(conn, prescription_id)?;
    let mut lines: Vec<Line> = Vec::with_capacity(items.len());
    let mut index: BTreeMap<i64, usize> = BTreeMap::new();
    let mut problems = Vec::new();

    for it in &items {
        let parent = match it.parent_item_id {
            Some(pid) => Some(*index.get(&pid).ok_or_else(|| AppError::Internal("komponen tanpa induk".into()))?),
            None => None,
        };
        let line = match it.kind.as_str() {
            "PRODUCT" => {
                let unit_id = it.product_unit_id.ok_or_else(|| AppError::Internal("item resep tanpa satuan".into()))?;
                let unit = repo::sale_unit(conn, unit_id)?.ok_or_else(|| AppError::NotFound("Obat resep tidak ditemukan".into()))?;
                if !unit.usable {
                    problems.push(format!("{} ({}) sudah nonaktif; ubah resep terlebih dahulu", unit.product_name, unit.unit_name));
                }
                let price = price_for_qty(conn, unit_id, it.qty)?;
                let value = price.unit_price * it.qty;
                Line {
                    kind: "PRODUCT",
                    parent,
                    product_id: Some(unit.product_id),
                    product_unit_id: Some(unit_id),
                    description: unit.product_name,
                    unit_name: Some(unit.unit_name),
                    qty: it.qty,
                    conversion: unit.conversion,
                    unit_price: price.unit_price,
                    tier_id: price.tier_id,
                    discount: 0,
                    line_total: if parent.is_some() { 0 } else { value },
                    usage: it.usage_instruction.clone(),
                    drug_class: Some(unit.drug_class),
                    discount_override: false,
                }
            }
            "COMPOUND" | "SERVICE" => Line {
                kind: if it.kind == "COMPOUND" { "COMPOUND" } else { "SERVICE" },
                parent: None,
                product_id: None,
                product_unit_id: None,
                description: it.description.clone(),
                unit_name: None,
                qty: it.qty,
                conversion: 1,
                unit_price: if it.kind == "SERVICE" { it.unit_price } else { 0 },
                tier_id: None,
                discount: 0,
                line_total: if it.kind == "SERVICE" { it.unit_price * it.qty } else { 0 },
                usage: it.usage_instruction.clone(),
                drug_class: None,
                discount_override: false,
            },
            other => return Err(AppError::Internal(format!("jenis item resep tidak dikenal: {other}"))),
        };
        // Nilai racikan = jumlah nilai komponennya.
        if let Some(p) = parent {
            lines[p].line_total += line.unit_price * line.qty;
        }
        index.insert(it.id, lines.len());
        lines.push(line);
    }
    if lines.is_empty() {
        problems.push("Resep tidak berisi obat".into());
    }

    // Kebutuhan total per obat (baris biasa + komponen racikan) dibanding stok yang bisa dijual.
    let mut need: BTreeMap<i64, (String, i64)> = BTreeMap::new();
    for l in lines.iter().filter(|l| l.kind == "PRODUCT") {
        if let Some(pid) = l.product_id {
            need.entry(pid).or_insert((l.description.clone(), 0)).1 += l.qty_base();
        }
    }
    for (pid, (name, qty)) in need {
        let have: i64 = repo::fefo_batches(conn, pid, today)?.iter().map(|b| b.qty_on_hand).sum();
        if qty > have {
            problems.push(format!("Stok {name} kurang: dibutuhkan {qty}, tersedia {have} (satuan terkecil)"));
        }
    }
    Ok((lines, problems))
}

fn quote_line(l: &Line, lines: &[Line], idx: usize) -> QuoteLine {
    QuoteLine {
        kind: l.kind.to_owned(),
        description: l.description.clone(),
        unit_name: l.unit_name.clone(),
        qty: l.qty,
        unit_price: l.unit_price,
        line_total: if l.parent.is_some() { l.unit_price * l.qty } else { l.line_total },
        components: lines
            .iter()
            .enumerate()
            .filter(|(_, c)| c.parent == Some(idx))
            .map(|(i, c)| quote_line(c, lines, i))
            .collect(),
    }
}

// ─── Resep di kasir ──────────────────────────────────────────────────────────

pub fn ready_prescriptions(conn: &Connection) -> AppResult<Vec<PosPrescription>> {
    repo::ready_prescriptions(conn)
}

pub fn prescription_quote(conn: &Connection, id: i64, clock: &Clock) -> AppResult<PrescriptionQuote> {
    let h = repo::rx_header(conn, id)?.ok_or_else(|| AppError::NotFound("Resep tidak ditemukan".into()))?;
    if h.status != "SCREENED" {
        return Err(AppError::Conflict(format!("Resep {} tidak menunggu pembayaran", h.number)));
    }
    let (lines, problems) = rx_lines(conn, id, &clock.today_s())?;
    let t = totals(&lines, rules(conn)?.total_rounding);
    Ok(PrescriptionQuote {
        id: h.id,
        number: h.number,
        prescription_number: h.prescription_number,
        patient_name: h.patient_name,
        doctor_name: h.doctor_name,
        lines: lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.parent.is_none())
            .map(|(i, l)| quote_line(l, &lines, i))
            .collect(),
        subtotal: t.subtotal,
        rounding: t.rounding,
        grand_total: t.grand_total,
        problems,
    })
}

// ─── Pembayaran ──────────────────────────────────────────────────────────────

/// Validasi pembayaran split (docs/SCHEMA.md `sale_payments`).
fn settle(payments: &[PaymentInput], grand_total: i64) -> AppResult<Vec<PaymentDetail>> {
    if grand_total == 0 {
        if payments.iter().any(|p| p.amount != 0) {
            return Err(AppError::Validation("Total nota Rp0, tidak perlu pembayaran".into()));
        }
        return Ok(Vec::new());
    }
    if payments.iter().filter(|p| p.method == PaymentMethod::Cash).count() > 1 {
        return Err(AppError::Validation("Pembayaran tunai hanya boleh satu baris".into()));
    }
    let mut out = Vec::with_capacity(payments.len());
    let mut sum = 0i64;
    for p in payments {
        money(p.amount, "Jumlah pembayaran")?;
        if p.amount == 0 {
            return Err(AppError::Validation("Jumlah pembayaran harus lebih dari 0".into()));
        }
        sum += p.amount;
        let reference = p.reference.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
        if reference.as_ref().is_some_and(|r| r.chars().count() > 64) {
            return Err(AppError::Validation("Kode referensi maksimal 64 karakter".into()));
        }
        let (tendered, change) = if p.method == PaymentMethod::Cash {
            let tendered = money(p.tendered.unwrap_or(p.amount), "Uang diterima")?;
            if tendered < p.amount {
                return Err(AppError::Validation(format!(
                    "Uang tunai diterima {} kurang dari bagian tunai {}",
                    rupiah(tendered),
                    rupiah(p.amount)
                )));
            }
            (Some(tendered), Some(tendered - p.amount))
        } else {
            if p.tendered.is_some() {
                return Err(AppError::Validation("Uang diterima & kembalian hanya untuk tunai".into()));
            }
            (None, None)
        };
        out.push(PaymentDetail { method: p.method, amount: p.amount, tendered, change_amount: change, reference });
    }
    if sum != grand_total {
        return Err(AppError::Validation(format!(
            "Jumlah pembayaran {} tidak sama dengan total {}",
            rupiah(sum),
            rupiah(grand_total)
        )));
    }
    Ok(out)
}

// ─── Simpan penjualan ────────────────────────────────────────────────────────

fn valid_client_ref(r: &str) -> bool {
    (8..=64).contains(&r.len()) && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Otorisasi untuk `permission`: user sendiri bila berhak, atau pemilik PIN yang diberikan.
/// Dijalankan **sebelum** transaksi tulis karena verifikasi argon2 lambat dan tidak boleh
/// menahan kunci tulis database.
fn resolve_auth(conn: &Connection, user: &SessionUser, permission: Permission, pin: Option<&str>) -> AppResult<Option<Authorizer>> {
    if can(user, permission) {
        return Ok(Some(Authorizer { id: user.id }));
    }
    let Some(pin) = pin.map(str::trim).filter(|p| !p.is_empty()) else { return Ok(None) };
    match authorize_pin(conn, pin, permission) {
        Ok(a) => Ok(Some(a)),
        Err(e) => {
            audit::log(
                conn,
                audit::Entry {
                    user_id: Some(user.id),
                    action: "PIN_REJECTED",
                    detail: Some(json!({ "permission": format!("{permission:?}") })),
                    ..Default::default()
                },
            )?;
            Err(e)
        }
    }
}

fn replay(conn: &Connection, user: &SessionUser, client_ref: &str) -> AppResult<Option<SaleDetail>> {
    match repo::sale_by_client_ref(conn, client_ref)? {
        None => Ok(None),
        Some((id, cashier)) if cashier == user.id => {
            let mut d = get_sale(conn, user, id)?;
            d.replayed = true;
            Ok(Some(d))
        }
        Some(_) => Err(AppError::Conflict("Kode transaksi sudah dipakai kasir lain. Muat ulang layar kasir.".into())),
    }
}

/// Simpan penjualan (bebas atau resep) dalam **satu** transaksi `BEGIN IMMEDIATE`:
/// cek shift → hitung harga → cek total → nomor nota → baris → FEFO per batch + kartu stok →
/// pembayaran → audit → (resep jadi PAID). Gagal di langkah mana pun = tidak ada yang tersimpan.
pub fn create_sale(conn: &mut Connection, user: &SessionUser, input: &SaleInput, clock: &Clock) -> AppResult<SaleDetail> {
    let client_ref = input.client_ref.trim();
    if !valid_client_ref(client_ref) {
        return Err(AppError::Validation("Kode transaksi tidak valid".into()));
    }
    // Kiriman ulang (klik ganda, retry setelah timeout) → kembalikan nota yang sudah tersimpan.
    if let Some(done) = replay(conn, user, client_ref)? {
        return Ok(done);
    }
    let hard_auth = resolve_auth(conn, user, Permission::SellHardDrug, input.hard_drug_pin.as_deref())?;
    let discount_auth = resolve_auth(conn, user, Permission::DiscountOverride, input.discount_pin.as_deref())?;

    let today = clock.today_s();
    let outcome = db::write_tx(conn, |tx| {
        // Dicek ulang di dalam kunci tulis: permintaan kembar yang antre di belakang kunci
        // melihat nota yang baru saja disimpan permintaan pertama.
        if let Some((id, _)) = repo::sale_by_client_ref(tx, client_ref)? {
            return Ok(Err(id));
        }
        let shift = repo::open_shift(tx)?
            .ok_or_else(|| AppError::Conflict("Belum ada shift terbuka. Buka shift terlebih dahulu.".into()))?;
        let r = rules(tx)?;

        let (lines, rx) = match input.prescription_id {
            None => (otc_lines(tx, input, r.max_discount_bp)?, None),
            Some(rx_id) => {
                let h = repo::rx_header(tx, rx_id)?.ok_or_else(|| AppError::NotFound("Resep tidak ditemukan".into()))?;
                if h.status != "SCREENED" {
                    return Err(AppError::Conflict(format!(
                        "Resep {} tidak menunggu pembayaran (mungkin sudah dibayar)",
                        h.number
                    )));
                }
                let (lines, problems) = rx_lines(tx, rx_id, &today)?;
                if !problems.is_empty() {
                    return Err(AppError::Validation(problems.join("; ")));
                }
                (lines, Some(h))
            }
        };

        // Obat keras (termasuk OWA) di penjualan bebas butuh apoteker; resep sudah diskrining.
        let hard: Vec<&str> = lines
            .iter()
            .filter(|l| rx.is_none() && l.drug_class == Some(DrugClass::Hard))
            .map(|l| l.description.as_str())
            .collect();
        if !hard.is_empty() && hard_auth.is_none() {
            return Err(AppError::Validation(format!(
                "Obat keras ({}) butuh otorisasi PIN apoteker",
                hard.join(", ")
            )));
        }
        let over: Vec<&str> = lines.iter().filter(|l| l.discount_override).map(|l| l.description.as_str()).collect();
        if !over.is_empty() && discount_auth.is_none() {
            return Err(AppError::Validation(format!(
                "Diskon di atas batas ({}) butuh otorisasi PIN Pemilik/Apoteker",
                over.join(", ")
            )));
        }

        let t = totals(&lines, r.total_rounding);
        if t.grand_total != input.expected_total {
            return Err(AppError::Conflict(format!(
                "Total berubah menjadi {} (harga atau tier baru saja diperbarui). Periksa kembali sebelum membayar.",
                rupiah(t.grand_total)
            )));
        }
        let payments = settle(&input.payments, t.grand_total)?;

        let number = sequence::next_number(tx, "PJ", &clock.today.format("%y%m%d").to_string())?;
        let sale_id = repo::insert_sale(
            tx,
            &NewSale {
                number: &number,
                client_ref,
                shift_id: shift.id,
                cashier_id: user.id,
                customer_id: rx.as_ref().and_then(|h| h.customer_id),
                prescription_id: rx.as_ref().map(|h| h.id),
                sale_type: if rx.is_some() { "PRESCRIPTION" } else { "OTC" },
                sold_at: &clock.now,
                subtotal: t.subtotal,
                discount_total: t.discount_total,
                rounding: t.rounding,
                grand_total: t.grand_total,
            },
        )?;

        let mut ids: Vec<i64> = Vec::with_capacity(lines.len());
        let mut touched: Vec<i64> = Vec::new();
        for (n, l) in lines.iter().enumerate() {
            let authorized_by = if l.discount_override {
                discount_auth.as_ref().map(|a| a.id)
            } else if rx.is_none() && l.drug_class == Some(DrugClass::Hard) {
                hard_auth.as_ref().map(|a| a.id)
            } else {
                None
            };
            let item_id = repo::insert_item(
                tx,
                &NewItem {
                    sale_id,
                    line_no: n as i64 + 1,
                    kind: l.kind,
                    parent_item_id: l.parent.map(|p| ids[p]),
                    product_id: l.product_id,
                    product_unit_id: l.product_unit_id,
                    description: &l.description,
                    qty: l.qty,
                    conversion: l.conversion,
                    unit_price: l.unit_price,
                    price_tier_id: l.tier_id,
                    discount_amount: l.discount,
                    line_total: l.line_total,
                    usage_instruction: l.usage.as_deref(),
                    authorized_by,
                },
            )?;
            ids.push(item_id);
            if let (Some(product_id), "PRODUCT") = (l.product_id, l.kind) {
                allocate_fefo(tx, product_id, &l.description, l.qty_base(), sale_id, item_id, user.id, &today, &clock.now, &mut touched)?;
            }
        }

        // Pengaman terakhir: ringkasan stok setiap batch yang tersentuh harus sama dengan kartu stoknya.
        touched.sort_unstable();
        touched.dedup();
        let bad = repo::inconsistent_batches(tx, &touched)?;
        if !bad.is_empty() {
            return Err(AppError::Internal(format!("stok batch {bad:?} tidak cocok dengan kartu stok")));
        }

        for p in &payments {
            repo::insert_payment(tx, sale_id, p)?;
        }

        if let Some(h) = &rx
            && repo::mark_prescription_paid(tx, h.id)? != 1
        {
            return Err(AppError::Conflict(format!("Resep {} sudah dibayar", h.number)));
        }
        if !hard.is_empty()
            && let Some(a) = &hard_auth
        {
            audit::log(
                tx,
                audit::Entry {
                    user_id: Some(user.id),
                    authorized_by: Some(a.id),
                    action: "HARD_DRUG_SALE",
                    entity: Some(ENTITY),
                    entity_id: Some(sale_id),
                    detail: Some(json!({ "number": number, "items": hard })),
                    ..Default::default()
                },
            )?;
        }
        if !over.is_empty()
            && let Some(a) = &discount_auth
        {
            audit::log(
                tx,
                audit::Entry {
                    user_id: Some(user.id),
                    authorized_by: Some(a.id),
                    action: "DISCOUNT_OVERRIDE",
                    entity: Some(ENTITY),
                    entity_id: Some(sale_id),
                    detail: Some(json!({ "number": number, "items": over, "discountTotal": t.discount_total })),
                    ..Default::default()
                },
            )?;
        }
        Ok(Ok(sale_id))
    })?;

    match outcome {
        Ok(id) => get_sale(conn, user, id),
        // Permintaan kembar yang kalah antre: kembalikan nota milik permintaan pertama.
        Err(_) => replay(conn, user, client_ref)?.ok_or_else(|| AppError::Internal("nota hilang".into())),
    }
}

/// Ambil stok dari batch dengan ED terdekat (FEFO), pecah ke beberapa batch bila perlu.
/// Setiap potongan = satu `sale_item_batches` + satu kartu stok `SALE` (trigger mengurangi batch).
#[allow(clippy::too_many_arguments)]
fn allocate_fefo(
    tx: &Connection,
    product_id: i64,
    name: &str,
    need: i64,
    sale_id: i64,
    item_id: i64,
    user_id: i64,
    today: &str,
    now: &str,
    touched: &mut Vec<i64>,
) -> AppResult<()> {
    let mut left = need;
    for b in repo::fefo_batches(tx, product_id, today)? {
        if left == 0 {
            break;
        }
        let take = left.min(b.qty_on_hand);
        repo::insert_item_batch(tx, item_id, b.id, take, b.unit_cost_x100)?;
        repo::insert_movement(tx, b.id, product_id, "SALE", -take, sale_id, item_id, user_id, now)?;
        touched.push(b.id);
        left -= take;
    }
    if left > 0 {
        return Err(AppError::Conflict(format!(
            "Stok {name} tidak cukup: dibutuhkan {need}, tersedia {} (satuan terkecil). Stok mungkin baru saja terjual.",
            need - left
        )));
    }
    Ok(())
}

// ─── Baca nota ───────────────────────────────────────────────────────────────

pub fn get_sale(conn: &Connection, user: &SessionUser, id: i64) -> AppResult<SaleDetail> {
    let h = repo::sale_header(conn, id)?.ok_or_else(|| AppError::NotFound("Nota tidak ditemukan".into()))?;
    if h.cashier_id != user.id && !can(user, Permission::ReportSales) {
        return Err(AppError::Forbidden);
    }
    let items = repo::sale_items(conn, id)?
        .into_iter()
        .map(|r| {
            Ok(SaleItemDetail {
                batches: repo::item_batches(conn, r.id)?,
                id: r.id,
                line_no: r.line_no,
                kind: r.kind,
                parent_item_id: r.parent_item_id,
                description: r.description,
                unit_name: r.unit_name,
                qty: r.qty,
                unit_price: r.unit_price,
                tier_min_qty: r.tier_min_qty,
                discount_amount: r.discount_amount,
                line_total: r.line_total,
                usage_instruction: r.usage_instruction,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    let payments = repo::payments(conn, id)?;
    let change_amount = payments.iter().filter_map(|p| p.change_amount).sum();
    Ok(SaleDetail {
        id: h.id,
        number: h.number,
        shift_id: h.shift_id,
        cashier_name: h.cashier_name,
        sold_at: h.sold_at,
        sale_type: h.sale_type,
        prescription_id: h.prescription_id,
        prescription_number: h.prescription_number,
        patient_name: h.patient_name,
        subtotal: h.subtotal,
        discount_total: h.discount_total,
        rounding: h.rounding,
        grand_total: h.grand_total,
        status: h.status,
        voided_at: h.voided_at,
        voided_by: h.voided_by,
        void_reason: h.void_reason,
        items,
        payments,
        change_amount,
        replayed: false,
    })
}

pub fn page_sales(conn: &Connection, user: &SessionUser, query: &SaleQuery) -> AppResult<SalePage> {
    let shift_id = match query.shift_id {
        Some(id) => id,
        None => match repo::open_shift(conn)? {
            Some(s) => s.id,
            None => return Ok(SalePage { rows: Vec::new(), total: 0 }),
        },
    };
    let cashier = (!can(user, Permission::ReportSales)).then_some(user.id);
    let (rows, total) = repo::page_sales(conn, shift_id, cashier, query)?;
    Ok(SalePage { rows, total })
}

// ─── Void ────────────────────────────────────────────────────────────────────

/// Batalkan nota: tandai VOID, kembalikan stok ke batch asal (kartu stok `SALE_VOID`), satu transaksi.
pub fn void_sale(conn: &mut Connection, user: &SessionUser, input: &SaleVoidInput, clock: &Clock) -> AppResult<SaleDetail> {
    let reason = input.reason.trim();
    if reason.is_empty() {
        return Err(AppError::Validation("Alasan pembatalan wajib diisi".into()));
    }
    if reason.chars().count() > 200 {
        return Err(AppError::Validation("Alasan pembatalan maksimal 200 karakter".into()));
    }
    let auth = resolve_auth(conn, user, Permission::TransactionVoid, input.pin.as_deref())?
        .ok_or_else(|| AppError::Validation("Pembatalan nota butuh otorisasi PIN Pemilik/Apoteker".into()))?;

    db::write_tx(conn, |tx| {
        let h = repo::sale_header(tx, input.sale_id)?.ok_or_else(|| AppError::NotFound("Nota tidak ditemukan".into()))?;
        if h.status == "VOID" {
            return Err(AppError::Conflict(format!("Nota {} sudah dibatalkan", h.number)));
        }
        if repo::open_shift(tx)?.map(|s| s.id) != Some(h.shift_id) {
            return Err(AppError::Conflict(format!(
                "Nota {} berasal dari shift yang sudah ditutup; gunakan retur penjualan",
                h.number
            )));
        }
        if repo::return_count(tx, h.id)? > 0 {
            return Err(AppError::Conflict(format!("Nota {} sudah punya retur, tidak bisa dibatalkan", h.number)));
        }
        if repo::mark_void(tx, h.id, user.id, reason, &clock.now)? != 1 {
            return Err(AppError::Conflict(format!("Nota {} sudah dibatalkan", h.number)));
        }
        let mut touched = Vec::new();
        for b in repo::used_batches(tx, h.id)? {
            repo::insert_movement(tx, b.batch_id, b.product_id, "SALE_VOID", b.qty_base, h.id, b.sale_item_id, user.id, &clock.now)?;
            touched.push(b.batch_id);
        }
        touched.sort_unstable();
        touched.dedup();
        let bad = repo::inconsistent_batches(tx, &touched)?;
        if !bad.is_empty() {
            return Err(AppError::Internal(format!("stok batch {bad:?} tidak cocok dengan kartu stok")));
        }
        if let Some(rx) = h.prescription_id {
            repo::reopen_prescription(tx, rx)?;
        }
        audit::log(
            tx,
            audit::Entry {
                user_id: Some(user.id),
                authorized_by: Some(auth.id),
                action: "SALE_VOID",
                entity: Some(ENTITY),
                entity_id: Some(h.id),
                detail: Some(json!({ "number": h.number, "grandTotal": h.grand_total })),
                reason: Some(reason),
            },
        )?;
        Ok(())
    })?;
    get_sale(conn, user, input.sale_id)
}
