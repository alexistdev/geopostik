use chrono::NaiveDate;
use rusqlite::Connection;

use super::model::*;
use super::service;
use crate::auth::{AccessSettings, Role, SessionUser, effective_permissions};
use crate::db;
use crate::error::AppError;
use crate::master::MasterPageQuery;
use crate::settings;

const PCT_TABLET: i64 = 1; // product_units.id
const PCT_STRIP: i64 = 2;
const PCT_BOX: i64 = 3;
const AMX_BOX: i64 = 4;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 26).unwrap()
}

/// Paracetamol (tablet / strip isi 10 / box isi 100, harga AUTO, HPP acuan Rp100/tablet) dan
/// Amoxicillin (box isi 100, harga MANUAL Rp5.000).
fn setup() -> Connection {
    let conn = db::open_in_memory().unwrap();
    conn.execute_batch(
        "INSERT INTO users (id, username, full_name, password_hash) VALUES (1, 'owner', 'Pemilik', 'x'),
                                                                           (2, 'ttk', 'TTK', 'x');
         INSERT INTO products (id, code, name, drug_class, base_unit_id, last_cost_x100) VALUES
             (1, 'OBT0001', 'Paracetamol 500 mg', 'FREE', 1, 10000),
             (2, 'OBT0002', 'Amoxicillin 500 mg', 'HARD', 1, NULL);
         INSERT INTO product_units (id, product_id, unit_id, conversion, sell_price, price_mode, is_default_sale) VALUES
             (1, 1, 1, 1, 200, 'AUTO', 0), (2, 1, 4, 10, 1200, 'AUTO', 1), (3, 1, 6, 100, 12000, 'AUTO', 0),
             (4, 2, 6, 100, 5000, 'MANUAL', 1);
         INSERT INTO product_barcodes (product_unit_id, barcode) VALUES (3, '8990000000001');",
    )
    .unwrap();
    conn
}

fn user(id: i64, roles: &[Role]) -> SessionUser {
    SessionUser {
        id,
        username: "u".into(),
        full_name: "U".into(),
        roles: roles.to_vec(),
        permissions: effective_permissions(roles, AccessSettings::default()),
    }
}

fn owner() -> SessionUser {
    user(1, &[Role::Owner])
}

fn ttk() -> SessionUser {
    user(2, &[Role::Technician])
}

fn supplier(conn: &Connection, name: &str) -> Supplier {
    service::save_supplier(
        conn,
        &owner(),
        &SupplierInput {
            id: None,
            name: name.into(),
            address: None,
            phone: None,
            npwp: None,
            payment_term_days: 30,
            is_active: true,
        },
    )
    .unwrap()
}

fn item(unit: i64, qty: i64, price: i64, batch: &str) -> PurchaseItemInput {
    PurchaseItemInput {
        product_unit_id: unit,
        qty,
        bonus_qty: 0,
        unit_price: price,
        discount1_bp: 0,
        discount2_bp: 0,
        batch_number: batch.into(),
        expiry_date: "2028-12-31".into(),
    }
}

fn invoice(supplier_id: i64, number: &str, items: Vec<PurchaseItemInput>) -> PurchaseInput {
    PurchaseInput {
        id: None,
        supplier_id,
        invoice_number: number.into(),
        invoice_date: "2026-09-25".into(),
        received_date: "2026-09-26".into(),
        due_date: Some("2026-10-25".into()),
        payment_type: PurchasePaymentType::Credit,
        tax_mode: TaxMode::Excluded,
        tax_rate_bp: 1_100,
        extra_discount: 0,
        note: None,
        items,
    }
}

fn save(conn: &mut Connection, u: &SessionUser, input: &PurchaseInput) -> Result<PurchaseSaveResult, AppError> {
    service::save_purchase(conn, u, input, today())
}

fn post(conn: &mut Connection, id: i64, update_prices: bool) -> PurchaseSaveResult {
    service::post_purchase(conn, &owner(), &PurchasePostInput { id, update_prices }).unwrap()
}

fn stock(conn: &Connection, product_id: i64) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(qty_on_hand_base), 0) FROM batches WHERE product_id = ?1",
        [product_id],
        |r| r.get(0),
    )
    .unwrap()
}

fn unit_price(conn: &Connection, product_unit_id: i64) -> i64 {
    conn.query_row("SELECT sell_price FROM product_units WHERE id = ?1", [product_unit_id], |r| r.get(0))
        .unwrap()
}

#[test]
fn draft_does_not_touch_stock_and_posting_creates_batches() {
    let mut conn = setup();
    let sp = supplier(&conn, "PBF Sehat");
    // 2 box @ Rp20.000 − 10% + PPN 11% di luar harga; bonus 1 box.
    let mut line = item(PCT_BOX, 2, 20_000, "A1");
    line.discount1_bp = 1_000;
    line.bonus_qty = 1;
    let draft = save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![line, item(AMX_BOX, 1, 30_000, "X9")])).unwrap();
    let p = draft.purchase;
    assert_eq!(p.status, PurchaseStatus::Draft);
    assert!(p.number.starts_with("PB-2609-"));
    // Barang: 36.000 + 30.000 = 66.000; PPN 7.260; total 73.260.
    assert_eq!((p.subtotal, p.discount_total, p.tax_total, p.grand_total), (70_000, 4_000, 7_260, 73_260));
    assert_eq!(stock(&conn, 1), 0, "draft belum menambah stok");

    let posted = post(&mut conn, p.id, true).purchase;
    assert_eq!(posted.status, PurchaseStatus::Posted);
    assert!(posted.tax_in_cost, "default non-PKP: PPN masuk HPP");
    assert_eq!(stock(&conn, 1), 300, "2 box + bonus 1 box isi 100");
    assert_eq!(stock(&conn, 2), 100);

    // HPP Paracetamol = 36.000 × 1,11 / 300 tablet = Rp133,20.
    let pct = &posted.items[0];
    assert_eq!(pct.unit_cost_x100, Some(13_320));
    let batch_id = pct.batch_id.unwrap();
    let (cost, source): (i64, String) = conn
        .query_row("SELECT unit_cost_x100, source_type FROM batches WHERE id = ?1", [batch_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((cost, source.as_str()), (13_320, "PURCHASE"));
    let movement: (String, i64, String) = conn
        .query_row(
            "SELECT movement_type, qty_change_base, ref_type FROM stock_movements WHERE batch_id = ?1",
            [batch_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(movement, ("PURCHASE".into(), 300, "purchase".into()));

    // HPP acuan diperbarui, harga AUTO dihitung ulang (margin default 20%, bulat ke atas Rp100).
    let last: i64 = conn.query_row("SELECT last_cost_x100 FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(last, 13_320);
    assert_eq!(unit_price(&conn, PCT_TABLET), 200); // 133,2 × 1,2 = 159,84 → 200
    assert_eq!(unit_price(&conn, PCT_STRIP), 1_600); // 1.598,4 → 1.600
    // Harga MANUAL tetap.
    assert_eq!(unit_price(&conn, AMX_BOX), 5_000);
}

#[test]
fn posting_without_price_update_keeps_reference_cost() {
    let mut conn = setup();
    let sp = supplier(&conn, "PBF Sehat");
    let p = save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![item(PCT_BOX, 1, 50_000, "A1")])).unwrap().purchase;
    post(&mut conn, p.id, false);
    let last: i64 = conn.query_row("SELECT last_cost_x100 FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(last, 10_000);
    assert_eq!(unit_price(&conn, PCT_STRIP), 1_200);
}

#[test]
fn pkp_cost_excludes_tax() {
    let mut conn = setup();
    settings::set(&conn, settings::TAX_IS_PKP, &true).unwrap();
    let sp = supplier(&conn, "PBF Sehat");
    let p = save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![item(PCT_BOX, 1, 20_000, "A1")])).unwrap().purchase;
    let posted = post(&mut conn, p.id, false).purchase;
    assert!(!posted.tax_in_cost);
    assert_eq!(posted.grand_total, 22_200);
    assert_eq!(posted.items[0].unit_cost_x100, Some(20_000));
}

#[test]
fn validation_and_warnings() {
    let mut conn = setup();
    let sp = supplier(&conn, "PBF Sehat");

    let mut expired = item(PCT_BOX, 1, 20_000, "A1");
    expired.expiry_date = "2026-09-26".into();
    let err = save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![expired])).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)), "{err}");

    let mut near = item(PCT_BOX, 1, 20_000, "A1");
    near.expiry_date = "2026-11-30".into();
    let r = save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![near])).unwrap();
    assert!(r.warnings.iter().any(|w| w.contains("ED 2026-11-30")), "{:?}", r.warnings);
    // Harga naik: Rp222/tablet vs HPP acuan Rp100.
    assert!(r.warnings.iter().any(|w| w.contains("naik")), "{:?}", r.warnings);

    let mut credit_without_due = invoice(sp.id, "F-002", vec![item(PCT_BOX, 1, 1, "A1")]);
    credit_without_due.due_date = None;
    assert!(matches!(save(&mut conn, &owner(), &credit_without_due), Err(AppError::Validation(_))));

    let duplicate_batch = invoice(sp.id, "F-003", vec![item(PCT_BOX, 1, 1, "A1"), item(PCT_BOX, 2, 1, "a1")]);
    assert!(matches!(save(&mut conn, &owner(), &duplicate_batch), Err(AppError::Validation(_))));

    let mut too_much_discount = invoice(sp.id, "F-004", vec![item(PCT_BOX, 1, 1_000, "A1")]);
    too_much_discount.extra_discount = 1_001;
    assert!(matches!(save(&mut conn, &owner(), &too_much_discount), Err(AppError::Validation(_))));
}

#[test]
fn invoice_number_is_unique_per_supplier_until_voided() {
    let mut conn = setup();
    let sp = supplier(&conn, "PBF Sehat");
    let other = supplier(&conn, "PBF Lain");
    let first = save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![item(PCT_BOX, 1, 1_000, "A1")])).unwrap().purchase;

    let dup = save(&mut conn, &owner(), &invoice(sp.id, "f-001", vec![item(PCT_BOX, 1, 1_000, "A1")]));
    assert!(matches!(dup, Err(AppError::Conflict(_))));
    // Supplier lain boleh punya nomor faktur yang sama.
    save(&mut conn, &owner(), &invoice(other.id, "F-001", vec![item(PCT_BOX, 1, 1_000, "A1")])).unwrap();

    // Draft yang dibatalkan tidak lagi memblokir nomornya.
    service::void_purchase(&mut conn, &ttk(), first.id, None).unwrap();
    save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![item(PCT_BOX, 1, 1_000, "A1")])).unwrap();
}

#[test]
fn posted_invoice_cannot_be_edited() {
    let mut conn = setup();
    let sp = supplier(&conn, "PBF Sehat");
    let mut input = invoice(sp.id, "F-001", vec![item(PCT_BOX, 1, 1_000, "A1")]);
    let p = save(&mut conn, &owner(), &input).unwrap().purchase;
    post(&mut conn, p.id, false);

    input.id = Some(p.id);
    assert!(matches!(save(&mut conn, &owner(), &input), Err(AppError::Conflict(_))));
    let again = service::post_purchase(&mut conn, &owner(), &PurchasePostInput { id: p.id, update_prices: false });
    assert!(matches!(again, Err(AppError::Conflict(_))));

    // Trigger database juga menolak perubahan langsung.
    assert!(conn.execute("UPDATE purchases SET grand_total = 1 WHERE id = ?1", [p.id]).is_err());
    assert!(conn.execute("UPDATE purchase_items SET qty = 9 WHERE purchase_id = ?1", [p.id]).is_err());
    assert!(conn.execute("DELETE FROM purchases WHERE id = ?1", [p.id]).is_err());
}

#[test]
fn voiding_a_posted_invoice_returns_stock_only_if_unused() {
    let mut conn = setup();
    let sp = supplier(&conn, "PBF Sehat");
    let p = save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![item(PCT_BOX, 1, 1_000, "A1")])).unwrap().purchase;
    let posted = post(&mut conn, p.id, false).purchase;

    // TTK tidak berhak membatalkan faktur yang sudah diposting; alasan wajib.
    assert!(matches!(service::void_purchase(&mut conn, &ttk(), p.id, Some("salah")), Err(AppError::Forbidden)));
    assert!(matches!(service::void_purchase(&mut conn, &owner(), p.id, None), Err(AppError::Validation(_))));

    // Stok sudah dipakai → ditolak.
    let batch_id = posted.items[0].batch_id.unwrap();
    conn.execute(
        "INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id)
         VALUES (?1, 1, 'SALE', -1, 'sale', 1, 1)",
        [batch_id],
    )
    .unwrap();
    assert!(matches!(
        service::void_purchase(&mut conn, &owner(), p.id, Some("salah input")),
        Err(AppError::Conflict(_))
    ));

    // Faktur lain yang belum dipakai bisa dibatalkan; stok kembali 0.
    let q = save(&mut conn, &owner(), &invoice(sp.id, "F-002", vec![item(AMX_BOX, 2, 1_000, "X1")])).unwrap().purchase;
    post(&mut conn, q.id, false);
    assert_eq!(stock(&conn, 2), 200);
    let voided = service::void_purchase(&mut conn, &owner(), q.id, Some("salah input")).unwrap();
    assert_eq!(voided.status, PurchaseStatus::Void);
    assert_eq!(voided.void_reason.as_deref(), Some("salah input"));
    assert_eq!(stock(&conn, 2), 0);
    let void_moves: i64 = conn
        .query_row(
            "SELECT count(*) FROM stock_movements WHERE movement_type = 'PURCHASE_VOID' AND ref_id = ?1",
            [q.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(void_moves, 1);
}

#[test]
fn supplier_payments_reduce_debt() {
    let mut conn = setup();
    let sp = supplier(&conn, "PBF Sehat");
    let p = save(&mut conn, &owner(), &invoice(sp.id, "F-001", vec![item(PCT_BOX, 1, 100_000, "A1")])).unwrap().purchase;
    let pay = |amount| SupplierPaymentInput {
        purchase_id: p.id,
        payment_date: "2026-09-26".into(),
        amount,
        method: SupplierPaymentMethod::Transfer,
        reference: Some("TRF-1".into()),
        note: None,
    };
    // Draft belum bisa dibayar.
    assert!(service::create_payment(&mut conn, &owner(), &pay(1_000), today()).is_err());
    post(&mut conn, p.id, false);

    let page = |conn: &Connection, filter| {
        service::page_debts(conn, &DebtQuery { q: None, supplier_id: None, filter, offset: 0, limit: 25 }, today()).unwrap()
    };
    let d = page(&conn, DebtFilter::Open);
    assert_eq!((d.total, d.summary.open_amount), (1, 111_000));

    assert!(matches!(
        service::create_payment(&mut conn, &owner(), &pay(111_001), today()),
        Err(AppError::Validation(_))
    ));
    let after = service::create_payment(&mut conn, &owner(), &pay(100_000), today()).unwrap();
    assert_eq!(after.outstanding, Some(11_000));
    let payment_id = after.payments.as_ref().unwrap()[0].id;

    // Faktur yang sudah dibayar tidak bisa dibatalkan sebelum pembayarannya dibatalkan.
    assert!(matches!(
        service::void_purchase(&mut conn, &owner(), p.id, Some("x")),
        Err(AppError::Conflict(_))
    ));

    let paid_off = service::create_payment(&mut conn, &owner(), &pay(11_000), today()).unwrap();
    assert_eq!(paid_off.outstanding, Some(0));
    assert_eq!(page(&conn, DebtFilter::Open).total, 0);
    assert_eq!(page(&conn, DebtFilter::Paid).total, 1);

    let reopened = service::void_payment(&mut conn, &owner(), payment_id, "salah nominal").unwrap();
    assert_eq!(reopened.outstanding, Some(100_000));
    assert!(service::void_payment(&mut conn, &owner(), payment_id, "lagi").is_err());
    assert!(conn.execute("DELETE FROM supplier_payments", []).is_err());
}

#[test]
fn cost_is_hidden_without_view_cost() {
    let mut conn = setup();
    let sp = supplier(&conn, "PBF Sehat");
    let p = save(&mut conn, &ttk(), &invoice(sp.id, "F-001", vec![item(PCT_BOX, 1, 100_000, "A1")])).unwrap();
    assert!(p.purchase.items.iter().all(|i| i.unit_cost_x100.is_none() && i.last_cost_x100.is_none()));
    // Peringatan harga naik tanpa angka HPP.
    assert!(p.warnings.iter().any(|w| w.contains("lebih tinggi") && !w.contains("Rp")), "{:?}", p.warnings);
    assert!(p.purchase.outstanding.is_none() && p.purchase.payments.is_none());

    let found = service::search_products(&conn, &ttk(), "8990000000001").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].matched_unit_id, Some(PCT_BOX));
    assert!(found[0].last_cost_x100.is_none());
    assert_eq!(service::search_products(&conn, &owner(), "parace").unwrap()[0].last_cost_x100, Some(10_000));

    let owner_view = service::get_purchase(&conn, &owner(), p.purchase.id).unwrap();
    assert_eq!(owner_view.items[0].unit_cost_x100, Some(111_000));
}

#[test]
fn supplier_master_rules() {
    let mut conn = setup();
    let a = supplier(&conn, "PBF Sehat");
    assert_eq!(a.code, "SUP0001");
    let dup = service::save_supplier(
        &conn,
        &owner(),
        &SupplierInput {
            id: None,
            name: "pbf sehat".into(),
            address: None,
            phone: None,
            npwp: None,
            payment_term_days: 0,
            is_active: true,
        },
    );
    assert!(matches!(dup, Err(AppError::Conflict(_))));

    save(&mut conn, &owner(), &invoice(a.id, "F-001", vec![item(PCT_BOX, 1, 1_000, "A1")])).unwrap();
    assert!(matches!(service::delete_supplier(&mut conn, &owner(), a.id), Err(AppError::Conflict(_))));

    let b = supplier(&conn, "PBF Baru");
    service::delete_supplier(&mut conn, &owner(), b.id).unwrap();
    let page = service::page_suppliers(&conn, &MasterPageQuery { q: None, offset: 0, limit: 25 }).unwrap();
    assert_eq!(page.total, 1);
    // Kode supplier terhapus tidak dipakai ulang.
    assert_eq!(supplier(&conn, "PBF Ketiga").code, "SUP0003");

    service::set_supplier_active(&conn, &owner(), a.id, false).unwrap();
    let inactive = save(&mut conn, &owner(), &invoice(a.id, "F-002", vec![item(PCT_BOX, 1, 1_000, "B1")]));
    assert!(matches!(inactive, Err(AppError::Validation(_))));
}
