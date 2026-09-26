use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use chrono::NaiveDate;
use rusqlite::Connection;

use super::model::*;
use super::service::{self, Clock};
use crate::auth::{AccessSettings, Role, SessionUser, effective_permissions, password};
use crate::db;
use crate::error::AppError;

fn clock() -> Clock {
    Clock { today: NaiveDate::from_ymd_opt(2026, 9, 26).unwrap(), now: "2026-09-26 10:00:00".into() }
}

/// Kasir (1), apoteker PIN 1234 (2), pemilik PIN 5678 (3).
/// P1 Paracetamol (bebas): tablet Rp500, strip isi 10 Rp4.500, tier strip ≥ 5 → Rp4.000.
///    Batch: A ED 2026-12 = 30, B ED 2026-10 = 25, C sudah ED = 100, D terkunci = 100 → bisa dijual 55.
/// P2 Amoxicillin (keras) tablet Rp1.000, stok 50. P3 Morfin (narkotika) stok 10.
fn seed(conn: &Connection) {
    let pin_apt = password::hash("1234").unwrap();
    let pin_own = password::hash("5678").unwrap();
    conn.execute_batch(&format!(
        "INSERT INTO users (id, username, full_name, password_hash, pin_hash) VALUES
             (1, 'kasir', 'Kasir', 'x', NULL),
             (2, 'apt', 'Apoteker', 'x', '{pin_apt}'),
             (3, 'owner', 'Pemilik', 'x', '{pin_own}');
         INSERT INTO user_roles (user_id, role) VALUES (1, 'CASHIER'), (2, 'PHARMACIST'), (3, 'OWNER');
         INSERT INTO products (id, code, name, drug_class, base_unit_id) VALUES
             (9001, 'T-P1', 'Paracetamol', 'FREE', 1),
             (9002, 'T-P2', 'Amoxicillin', 'HARD', 1),
             (9003, 'T-P3', 'Morfin', 'NARCOTIC', 1);
         INSERT INTO product_units (id, product_id, unit_id, conversion, sell_price, is_default_sale) VALUES
             (9011, 9001, 1, 1, 500, 1),
             (9012, 9001, 4, 10, 4500, 0),
             (9021, 9002, 1, 1, 1000, 1),
             (9031, 9003, 1, 1, 2000, 1);
         INSERT INTO price_tiers (product_unit_id, min_qty, price) VALUES (9012, 5, 4000);
         INSERT INTO batches (id, product_id, batch_number, expiry_date, unit_cost_x100, source_type, is_locked) VALUES
             (901, 9001, 'A', '2026-12-31', 20000, 'OPENING', 0),
             (902, 9001, 'B', '2026-10-31', 21000, 'OPENING', 0),
             (903, 9001, 'C', '2026-09-01', 20000, 'OPENING', 0),
             (904, 9001, 'D', '2026-10-01', 20000, 'OPENING', 1),
             (905, 9002, 'X', '2027-06-30', 50000, 'OPENING', 0),
             (906, 9003, 'N', '2027-06-30', 90000, 'OPENING', 0);
         INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id)
             VALUES (901, 9001, 'OPENING', 30, 'stock_opname', 1, 3),
                    (902, 9001, 'OPENING', 25, 'stock_opname', 1, 3),
                    (903, 9001, 'OPENING', 100, 'stock_opname', 1, 3),
                    (904, 9001, 'OPENING', 100, 'stock_opname', 1, 3),
                    (905, 9002, 'OPENING', 50, 'stock_opname', 1, 3),
                    (906, 9003, 'OPENING', 10, 'stock_opname', 1, 3);"
    ))
    .unwrap();
}

fn user(id: i64, roles: &[Role]) -> SessionUser {
    SessionUser {
        id,
        username: format!("u{id}"),
        full_name: format!("U{id}"),
        roles: roles.to_vec(),
        permissions: effective_permissions(roles, AccessSettings::default()),
    }
}

fn cashier() -> SessionUser {
    user(1, &[Role::Cashier])
}

fn pharmacist() -> SessionUser {
    user(2, &[Role::Pharmacist])
}

fn setup() -> Connection {
    let mut conn = db::open_in_memory().unwrap();
    seed(&conn);
    open_shift(&mut conn);
    conn
}

fn open_shift(conn: &mut Connection) -> ShiftSummary {
    service::open_shift(conn, &cashier(), &ShiftOpenInput { opening_cash: 100_000, note: None }, &clock()).unwrap()
}

fn item(unit: i64, qty: i64) -> SaleItemInput {
    SaleItemInput { product_unit_id: unit, qty, discount_amount: 0 }
}

fn cash(amount: i64, tendered: i64) -> PaymentInput {
    PaymentInput { method: PaymentMethod::Cash, amount, tendered: Some(tendered), reference: None }
}

fn sale(client_ref: &str, items: Vec<SaleItemInput>, total: i64) -> SaleInput {
    SaleInput {
        client_ref: client_ref.into(),
        prescription_id: None,
        items,
        payments: if total > 0 { vec![cash(total, total)] } else { vec![] },
        expected_total: total,
        hard_drug_pin: None,
        discount_pin: None,
    }
}

fn batch_qty(conn: &Connection, id: i64) -> i64 {
    conn.query_row("SELECT qty_on_hand_base FROM batches WHERE id = ?1", [id], |r| r.get(0)).unwrap()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// Ringkasan stok setiap batch harus sama dengan jumlah kartu stoknya.
fn assert_ledger_consistent(conn: &Connection) {
    let bad = count(
        conn,
        "SELECT count(*) FROM batches b
         WHERE b.qty_on_hand_base <> COALESCE((SELECT SUM(qty_change_base) FROM stock_movements m WHERE m.batch_id = b.id), 0)",
    );
    assert_eq!(bad, 0, "batch tidak cocok dengan kartu stok");
}

// ─── FEFO & akurasi stok ─────────────────────────────────────────────────────

#[test]
fn fefo_splits_batches_and_skips_expired_and_locked() {
    let mut conn = setup();
    // 3 strip = 30 tablet: batch B (ED Okt, 25) habis dulu, sisa 5 dari batch A (ED Des).
    let d = service::create_sale(&mut conn, &cashier(), &sale("ref-fefo-0001", vec![item(9012, 3)], 13_500), &clock()).unwrap();
    assert_eq!(d.number, "PJ-260926-0001");
    assert_eq!(d.grand_total, 13_500);
    let used: Vec<_> = d.items[0].batches.iter().map(|b| (b.batch_number.as_str(), b.qty_base)).collect();
    assert_eq!(used, [("B", 25), ("A", 5)]);

    assert_eq!((batch_qty(&conn, 902), batch_qty(&conn, 901)), (0, 25));
    // Batch sudah ED dan terkunci tidak tersentuh.
    assert_eq!((batch_qty(&conn, 903), batch_qty(&conn, 904)), (100, 100));
    assert_eq!(count(&conn, "SELECT count(*) FROM stock_movements WHERE movement_type = 'SALE'"), 2);
    assert_ledger_consistent(&conn);
}

#[test]
fn several_lines_of_same_product_share_the_same_stock() {
    let mut conn = setup();
    // 2 strip (20) + 30 tablet = 50 dari 55 yang bisa dijual.
    let d = service::create_sale(&mut conn, &cashier(), &sale("ref-lines-001", vec![item(9012, 2), item(9011, 30)], 24_000), &clock())
        .unwrap();
    assert_eq!(d.items.len(), 2);
    assert_eq!(batch_qty(&conn, 902) + batch_qty(&conn, 901), 5);
    assert_ledger_consistent(&conn);

    // Sisa 5; minta 6 → ditolak, tidak ada yang berubah.
    let err = service::create_sale(&mut conn, &cashier(), &sale("ref-lines-002", vec![item(9011, 6)], 3_000), &clock()).unwrap_err();
    assert!(matches!(err, AppError::Conflict(ref m) if m.contains("tidak cukup")), "{err:?}");
    assert_eq!(batch_qty(&conn, 902) + batch_qty(&conn, 901), 5);
}

#[test]
fn failed_sale_leaves_nothing_behind() {
    let mut conn = setup();
    // Baris kedua kekurangan stok → baris pertama yang sudah dialokasikan ikut dibatalkan.
    let err = service::create_sale(&mut conn, &pharmacist(), &sale("ref-fail-0001", vec![item(9021, 10), item(9011, 56)], 38_000), &clock())
        .unwrap_err();
    assert!(matches!(err, AppError::Conflict(_)));
    assert_eq!(count(&conn, "SELECT count(*) FROM sales"), 0);
    assert_eq!(count(&conn, "SELECT count(*) FROM sale_items"), 0);
    assert_eq!(count(&conn, "SELECT count(*) FROM stock_movements WHERE movement_type = 'SALE'"), 0);
    assert_eq!(batch_qty(&conn, 905), 50);
    // Nomor nota tidak terbuang.
    let d = service::create_sale(&mut conn, &cashier(), &sale("ref-fail-0002", vec![item(9011, 1)], 500), &clock()).unwrap();
    assert_eq!(d.number, "PJ-260926-0001");
}

#[test]
fn tier_price_follows_quantity() {
    let mut conn = setup();
    let d = service::create_sale(&mut conn, &cashier(), &sale("ref-tier-0001", vec![item(9012, 5)], 20_000), &clock()).unwrap();
    assert_eq!((d.items[0].unit_price, d.items[0].tier_min_qty), (4_000, Some(5)));
}

// ─── Idempotensi & konkurensi ────────────────────────────────────────────────

#[test]
fn resubmitting_the_same_checkout_does_not_sell_twice() {
    let mut conn = setup();
    let input = sale("ref-same-0001", vec![item(9011, 10)], 5_000);
    let first = service::create_sale(&mut conn, &cashier(), &input, &clock()).unwrap();
    let again = service::create_sale(&mut conn, &cashier(), &input, &clock()).unwrap();
    assert_eq!(first.id, again.id);
    assert!(!first.replayed && again.replayed);
    assert_eq!(count(&conn, "SELECT count(*) FROM sales"), 1);
    assert_eq!(batch_qty(&conn, 902), 15);
}

/// Seperti di aplikasi: satu koneksi di balik mutex, banyak command bersamaan.
#[test]
fn concurrent_checkouts_never_oversell() {
    let conn = Arc::new(Mutex::new(setup()));
    let start = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let (conn, start) = (conn.clone(), start.clone());
            thread::spawn(move || {
                start.wait();
                let input = sale(&format!("ref-race-{i:04}"), vec![item(9011, 10)], 5_000);
                service::create_sale(&mut conn.lock().unwrap(), &cashier(), &input, &clock())
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let ok = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(ok, 5, "55 tablet hanya cukup untuk 5 × 10");
    assert!(results.iter().filter_map(|r| r.as_ref().err()).all(|e| matches!(e, AppError::Conflict(_))));

    let conn = conn.lock().unwrap();
    assert_eq!(batch_qty(&conn, 901) + batch_qty(&conn, 902), 5);
    assert_eq!(count(&conn, "SELECT SUM(qty_base) FROM sale_item_batches"), 50);
    assert_ledger_consistent(&conn);
}

#[test]
fn concurrent_duplicates_create_one_sale() {
    let conn = Arc::new(Mutex::new(setup()));
    let start = Arc::new(Barrier::new(6));
    let ids: Vec<i64> = (0..6)
        .map(|_| {
            let (conn, start) = (conn.clone(), start.clone());
            thread::spawn(move || {
                start.wait();
                let input = sale("ref-dup-00001", vec![item(9011, 3)], 1_500);
                service::create_sale(&mut conn.lock().unwrap(), &cashier(), &input, &clock()).unwrap().id
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    assert!(ids.windows(2).all(|w| w[0] == w[1]));
    let conn = conn.lock().unwrap();
    assert_eq!(count(&conn, "SELECT count(*) FROM sales"), 1);
    assert_eq!(batch_qty(&conn, 902), 22);
}

/// Koneksi terpisah ke file yang sama (seperti proses lain / backup): kunci BEGIN IMMEDIATE
/// SQLite yang menjaga, tanpa mutex bersama. Tidak boleh oversell dan tidak boleh "database is locked".
#[test]
fn separate_connections_are_serialized_by_immediate_transactions() {
    let dir = std::env::temp_dir().join(format!("geopostik-race-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("race.db");
    let _ = std::fs::remove_file(&path);
    {
        let mut conn = db::open(&path).unwrap();
        seed(&conn);
        open_shift(&mut conn);
    }
    let start = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let (path, start) = (path.clone(), start.clone());
            thread::spawn(move || {
                let mut conn = db::open(&path).unwrap();
                start.wait();
                let input = sale(&format!("ref-file-{i:04}"), vec![item(9011, 10)], 5_000);
                service::create_sale(&mut conn, &cashier(), &input, &clock())
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for r in &results {
        if let Err(e) = r {
            assert!(matches!(e, AppError::Conflict(m) if m.contains("tidak cukup")), "{e:?}");
        }
    }
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 5);
    let conn = db::open(&path).unwrap();
    assert_eq!(batch_qty(&conn, 901) + batch_qty(&conn, 902), 5);
    assert_ledger_consistent(&conn);
    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

// ─── Aturan penjualan ────────────────────────────────────────────────────────

#[test]
fn stale_total_is_rejected() {
    let mut conn = setup();
    conn.execute("UPDATE product_units SET sell_price = 600 WHERE id = 9011", []).unwrap();
    let err = service::create_sale(&mut conn, &cashier(), &sale("ref-stale-001", vec![item(9011, 2)], 1_000), &clock()).unwrap_err();
    assert!(matches!(err, AppError::Conflict(ref m) if m.contains("Rp1.200")), "{err:?}");
    assert_eq!(count(&conn, "SELECT count(*) FROM sales"), 0);
}

#[test]
fn hard_drug_needs_pharmacist_pin_and_narcotic_needs_prescription() {
    let mut conn = setup();
    let mut input = sale("ref-hard-0001", vec![item(9021, 2)], 2_000);
    assert!(matches!(service::create_sale(&mut conn, &cashier(), &input, &clock()), Err(AppError::Validation(_))));

    // PIN pemilik tidak berhak (pemilik tidak memegang kewenangan obat keras).
    input.hard_drug_pin = Some("5678".into());
    assert!(matches!(service::create_sale(&mut conn, &cashier(), &input, &clock()), Err(AppError::Validation(_))));
    assert_eq!(count(&conn, "SELECT count(*) FROM audit_logs WHERE action = 'PIN_REJECTED'"), 1);

    input.hard_drug_pin = Some("1234".into());
    let d = service::create_sale(&mut conn, &cashier(), &input, &clock()).unwrap();
    let (by, action): (i64, String) = conn
        .query_row("SELECT authorized_by, action FROM audit_logs WHERE entity_id = ?1 AND entity = 'sales'", [d.id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((by, action.as_str()), (2, "HARD_DRUG_SALE"));

    // Apoteker yang berjualan sendiri tidak perlu PIN.
    service::create_sale(&mut conn, &pharmacist(), &sale("ref-hard-0002", vec![item(9021, 1)], 1_000), &clock()).unwrap();

    let err = service::create_sale(&mut conn, &pharmacist(), &sale("ref-narc-0001", vec![item(9031, 1)], 2_000), &clock()).unwrap_err();
    assert!(matches!(err, AppError::Validation(ref m) if m.contains("resep")));
}

#[test]
fn discount_above_limit_needs_authorization() {
    let mut conn = setup();
    // Batas default 10%: diskon Rp500 dari Rp5.000 boleh, Rp600 butuh PIN.
    let mut ok = sale("ref-disc-0001", vec![SaleItemInput { product_unit_id: 9011, qty: 10, discount_amount: 500 }], 4_500);
    service::create_sale(&mut conn, &cashier(), &ok, &clock()).unwrap();

    ok.client_ref = "ref-disc-0002".into();
    ok.items[0].discount_amount = 600;
    ok.expected_total = 4_400;
    ok.payments = vec![cash(4_400, 5_000)];
    assert!(matches!(service::create_sale(&mut conn, &cashier(), &ok, &clock()), Err(AppError::Validation(_))));
    ok.discount_pin = Some("5678".into());
    let d = service::create_sale(&mut conn, &cashier(), &ok, &clock()).unwrap();
    assert_eq!((d.discount_total, d.change_amount), (600, 600));
}

#[test]
fn split_payment_rules() {
    let mut conn = setup();
    let mut input = sale("ref-split-001", vec![item(9011, 20)], 10_000);
    input.payments = vec![
        PaymentInput { method: PaymentMethod::Qris, amount: 7_000, tendered: None, reference: Some("QR1".into()) },
        cash(3_000, 5_000),
    ];
    let d = service::create_sale(&mut conn, &cashier(), &input, &clock()).unwrap();
    assert_eq!(d.change_amount, 2_000);

    input.client_ref = "ref-split-002".into();
    input.payments = vec![cash(9_000, 10_000)];
    assert!(matches!(service::create_sale(&mut conn, &cashier(), &input, &clock()), Err(AppError::Validation(_))));
    input.payments = vec![cash(5_000, 5_000), cash(5_000, 5_000)];
    assert!(matches!(service::create_sale(&mut conn, &cashier(), &input, &clock()), Err(AppError::Validation(_))));
    input.payments = vec![cash(10_000, 9_000)];
    assert!(matches!(service::create_sale(&mut conn, &cashier(), &input, &clock()), Err(AppError::Validation(_))));
}

#[test]
fn total_rounding_goes_down() {
    let mut conn = setup();
    crate::settings::set(&conn, service::TOTAL_ROUNDING, &100i64).unwrap();
    let d = service::create_sale(
        &mut conn,
        &cashier(),
        &sale("ref-round-001", vec![SaleItemInput { product_unit_id: 9011, qty: 3, discount_amount: 50 }], 1_400),
        &clock(),
    )
    .unwrap();
    assert_eq!((d.subtotal, d.discount_total, d.rounding, d.grand_total), (1_500, 50, -50, 1_400));
}

// ─── Void & shift ────────────────────────────────────────────────────────────

#[test]
fn void_returns_stock_to_original_batches_once() {
    let mut conn = setup();
    let d = service::create_sale(&mut conn, &cashier(), &sale("ref-void-0001", vec![item(9012, 3)], 13_500), &clock()).unwrap();
    let input = SaleVoidInput { sale_id: d.id, reason: "salah input".into(), pin: None };
    // Kasir tanpa PIN ditolak.
    assert!(matches!(service::void_sale(&mut conn, &cashier(), &input, &clock()), Err(AppError::Validation(_))));

    let with_pin = SaleVoidInput { pin: Some("5678".into()), ..input };
    let v = service::void_sale(&mut conn, &cashier(), &with_pin, &clock()).unwrap();
    assert_eq!(v.status, "VOID");
    assert_eq!((batch_qty(&conn, 902), batch_qty(&conn, 901)), (25, 30));
    assert!(matches!(service::void_sale(&mut conn, &cashier(), &with_pin, &clock()), Err(AppError::Conflict(_))));
    assert_eq!((batch_qty(&conn, 902), batch_qty(&conn, 901)), (25, 30));
    assert_ledger_consistent(&conn);
    // Nota tidak bisa diubah/dihapus langsung.
    assert!(conn.execute("UPDATE sales SET status = 'COMPLETED' WHERE id = ?1", [d.id]).is_err());
    assert!(conn.execute("DELETE FROM sales", []).is_err());
}

#[test]
fn closed_shift_blocks_sales_and_voids() {
    let mut conn = setup();
    let d = service::create_sale(&mut conn, &cashier(), &sale("ref-shift-001", vec![item(9011, 10)], 5_000), &clock()).unwrap();
    let state = service::pos_state(&conn, &cashier()).unwrap();
    let shift = state.shift.unwrap();
    assert_eq!(shift.figures.as_ref().unwrap().expected_cash, 105_000);

    // Kasir lain tanpa hak laporan tidak bisa menutup shift orang lain.
    let other = user(4, &[Role::Cashier]);
    let close = ShiftCloseInput { shift_id: shift.id, counted_cash: 104_000, note: None };
    assert!(service::close_shift(&mut conn, &other, &close, &clock()).is_err());

    let closed = service::close_shift(&mut conn, &cashier(), &close, &clock()).unwrap();
    let f = closed.figures.unwrap();
    assert_eq!((f.expected_cash, f.counted_cash, f.difference), (105_000, Some(104_000), Some(-1_000)));
    assert!(service::close_shift(&mut conn, &cashier(), &close, &clock()).is_err());

    let err = service::create_sale(&mut conn, &cashier(), &sale("ref-shift-002", vec![item(9011, 1)], 500), &clock()).unwrap_err();
    assert!(matches!(err, AppError::Conflict(_)));
    let void = SaleVoidInput { sale_id: d.id, reason: "x".into(), pin: Some("5678".into()) };
    assert!(matches!(service::void_sale(&mut conn, &cashier(), &void, &clock()), Err(AppError::Conflict(_))));
    assert!(conn.execute("UPDATE shifts SET status = 'OPEN'", []).is_err());

    // Hanya satu shift terbuka.
    open_shift(&mut conn);
    assert!(service::open_shift(&mut conn, &pharmacist(), &ShiftOpenInput { opening_cash: 0, note: None }, &clock()).is_err());
}

// ─── Resep ───────────────────────────────────────────────────────────────────

/// Resep SCREENED: 5 tablet amoxicillin + racikan (4 tablet paracetamol + 2 tablet amoxicillin) + jasa racik.
fn screened_prescription(conn: &Connection) -> i64 {
    conn.execute_batch(
        "INSERT INTO doctors (id, code, name) VALUES (1, 'DOK0001', 'dr. A');
         INSERT INTO prescriptions (id, number, prescription_number, prescription_date, doctor_id, patient_name, status,
                                    screened_by, screened_at)
             VALUES (1, 'RSP-2609-0001', 'R/1', '2026-09-26', 1, 'Budi', 'SCREENED', 2, '2026-09-26 09:00:00');
         INSERT INTO prescription_items (id, prescription_id, line_no, item_kind, parent_item_id, product_id, product_unit_id,
                                         description, compound_form, qty, conversion, qty_base, unit_price, line_total)
             VALUES (1, 1, 1, 'PRODUCT', NULL, 9002, 9021, 'Amoxicillin', NULL, 5, 1, 5, 1000, 5000),
                    (2, 1, 2, 'COMPOUND', NULL, NULL, NULL, 'Puyer batuk', 'POWDER', 10, 1, 10, 0, 4000),
                    (3, 1, 3, 'PRODUCT', 2, 9001, 9011, 'Paracetamol', NULL, 4, 1, 4, 500, 2000),
                    (4, 1, 4, 'PRODUCT', 2, 9002, 9021, 'Amoxicillin', NULL, 2, 1, 2, 1000, 2000),
                    (5, 1, 5, 'SERVICE', NULL, NULL, NULL, 'Jasa racik', NULL, 1, 1, 1, 3000, 3000);",
    )
    .unwrap();
    1
}

#[test]
fn prescription_payment_uses_screened_items_once() {
    let mut conn = setup();
    let rx = screened_prescription(&conn);
    let q = service::prescription_quote(&conn, rx, &clock()).unwrap();
    // 5.000 + racikan (2.000 + 2.000) + 3.000
    assert_eq!(q.grand_total, 12_000);
    assert!(q.problems.is_empty());
    assert_eq!(q.lines[1].components.len(), 2);

    let mut input = sale("ref-rx-00001", vec![], 12_000);
    input.prescription_id = Some(rx);
    // Obat keras resep tidak butuh PIN lagi (sudah diskrining apoteker).
    let d = service::create_sale(&mut conn, &cashier(), &input, &clock()).unwrap();
    assert_eq!((d.sale_type.as_str(), d.grand_total), ("PRESCRIPTION", 12_000));
    assert_eq!(batch_qty(&conn, 905), 43);
    assert_eq!(batch_qty(&conn, 902), 21);
    let status: String = conn.query_row("SELECT status FROM prescriptions WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(status, "PAID");

    input.client_ref = "ref-rx-00002".into();
    assert!(matches!(service::create_sale(&mut conn, &cashier(), &input, &clock()), Err(AppError::Conflict(_))));

    // Void mengembalikan stok dan resep bisa dibayar lagi.
    service::void_sale(&mut conn, &pharmacist(), &SaleVoidInput { sale_id: d.id, reason: "batal".into(), pin: None }, &clock())
        .unwrap();
    assert_eq!(batch_qty(&conn, 905), 50);
    let status: String = conn.query_row("SELECT status FROM prescriptions WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(status, "SCREENED");
    service::create_sale(&mut conn, &cashier(), &input, &clock()).unwrap();
    assert_ledger_consistent(&conn);
}

#[test]
fn cashier_sees_only_own_sales() {
    let mut conn = setup();
    service::create_sale(&mut conn, &cashier(), &sale("ref-own-00001", vec![item(9011, 1)], 500), &clock()).unwrap();
    let other = service::create_sale(&mut conn, &pharmacist(), &sale("ref-own-00002", vec![item(9011, 1)], 500), &clock()).unwrap();
    let q = SaleQuery { shift_id: None, q: None, offset: 0, limit: 50 };
    assert_eq!(service::page_sales(&conn, &cashier(), &q).unwrap().total, 1);
    assert_eq!(service::page_sales(&conn, &pharmacist(), &q).unwrap().total, 2);
    assert!(matches!(service::get_sale(&conn, &cashier(), other.id), Err(AppError::Forbidden)));
}

#[test]
fn search_finds_by_name_code_and_barcode() {
    let conn = setup();
    conn.execute("INSERT INTO product_barcodes (product_unit_id, barcode) VALUES (9012, '899000111')", []).unwrap();
    let by_barcode = service::search(&conn, "899000111", &clock()).unwrap();
    assert_eq!((by_barcode.len(), by_barcode[0].matched_unit_id), (1, Some(9012)));
    let by_name = service::search(&conn, "parace", &clock()).unwrap();
    assert!(by_name.iter().any(|p| p.product_id == 9001 && p.sellable_base == 55));
    assert_eq!(service::search(&conn, "t-p2", &clock()).unwrap()[0].product_id, 9002);
}
