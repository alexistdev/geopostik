use chrono::NaiveDate;
use rusqlite::Connection;

use super::model::*;
use super::service;
use crate::auth::{AccessSettings, Role, SessionUser, effective_permissions};
use crate::db;
use crate::error::AppError;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 26).unwrap()
}

/// Satu user, dua obat (satuan dasar tablet): P1 Paracetamol di rak 1, P2 Amoxicillin.
fn setup() -> Connection {
    let conn = db::open_in_memory().unwrap();
    conn.execute_batch(
        "INSERT INTO users (id, username, full_name, password_hash) VALUES (1, 'u', 'U', 'x');
         INSERT INTO racks (id, code, name) VALUES (1, 'RAK0001', 'A1');
         INSERT INTO products (id, code, name, drug_class, base_unit_id, min_stock_base, rack_id)
             VALUES (1, 'P1', 'Paracetamol', 'FREE', 1, 50, 1),
                    (2, 'P2', 'Amoxicillin', 'HARD', 1, 0, NULL);",
    )
    .unwrap();
    conn
}

fn user(roles: &[Role]) -> SessionUser {
    SessionUser {
        id: 1,
        username: "u".into(),
        full_name: "U".into(),
        roles: roles.to_vec(),
        permissions: effective_permissions(roles, AccessSettings::default()),
    }
}

fn owner() -> SessionUser {
    user(&[Role::Owner])
}

fn new_batch(opname_id: i64, product_id: i64, batch: &str, ed: &str, cost: Option<i64>, qty: Option<i64>) -> OpnameItemInput {
    OpnameItemInput {
        opname_id,
        id: None,
        product_id,
        batch_id: None,
        batch_number: Some(batch.into()),
        expiry_date: Some(ed.into()),
        unit_cost_x100: cost,
        physical_qty_base: qty,
        note: None,
    }
}

fn create(conn: &mut Connection, t: OpnameType) -> OpnameDetail {
    service::create_opname(conn, &owner(), &OpnameCreateInput { opname_type: t, scope_note: None }, today()).unwrap()
}

/// Stok awal: P1 dua batch (100 @Rp150,50 ED 2027-01, 30 @Rp160 ED 2026-11), P2 satu batch 20.
fn opening(conn: &mut Connection) -> OpnameDetail {
    let o = create(conn, OpnameType::Opening);
    let id = o.header.id;
    for input in [
        new_batch(id, 1, "A100", "2027-01-31", Some(15_050), Some(100)),
        new_batch(id, 1, "A200", "2026-11-30", Some(16_000), Some(30)),
        new_batch(id, 2, "X1", "2028-05-01", Some(50_000), Some(20)),
    ] {
        service::save_item(conn, &owner(), &input, today()).unwrap();
    }
    service::submit_opname(conn, &owner(), id).unwrap();
    service::approve_opname(conn, &owner(), id).unwrap()
}

fn stock_of(conn: &Connection, product_id: i64) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(qty_on_hand_base), 0) FROM batches WHERE product_id = ?1",
        [product_id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn opening_opname_creates_batches_and_stock_card() {
    let mut conn = setup();
    let o = opening(&mut conn);
    assert_eq!(o.header.status, OpnameStatus::Approved);
    assert!(o.header.number.starts_with("OP-2609-"));
    assert!(o.items.iter().all(|i| i.batch_id.is_some()), "batch baru ditautkan ke baris opname");
    assert_eq!(stock_of(&conn, 1), 130);
    assert_eq!(stock_of(&conn, 2), 20);

    let card = service::stock_card(
        &conn,
        &StockCardQuery { product_id: 1, batch_id: None, date_from: None, date_to: None, offset: 0, limit: 50 },
    )
    .unwrap();
    assert_eq!(card.total, 2);
    assert!(card.rows.iter().all(|r| r.movement_type == MovementType::Opening));
    assert_eq!(card.rows.last().unwrap().balance_base, 130);
    assert_eq!(card.rows[0].ref_number.as_deref(), Some(o.header.number.as_str()));
    assert_eq!((card.total_in, card.total_out, card.closing_balance), (130, 0, 130));
}

#[test]
fn opening_rejects_existing_batches_and_requires_cost() {
    let mut conn = setup();
    opening(&mut conn);
    let o = create(&mut conn, OpnameType::Opening);
    let err = service::save_item(&mut conn, &owner(), &new_batch(o.header.id, 1, "B", "2027-01-01", None, Some(5)), today());
    assert!(matches!(err, Err(AppError::Validation(_))), "HPP wajib untuk batch baru");

    let batch_id: i64 = conn.query_row("SELECT id FROM batches WHERE batch_number = 'A100'", [], |r| r.get(0)).unwrap();
    let existing = OpnameItemInput { batch_id: Some(batch_id), ..new_batch(o.header.id, 1, "", "", None, Some(5)) };
    assert!(matches!(service::save_item(&mut conn, &owner(), &existing, today()), Err(AppError::Validation(_))));
}

#[test]
fn expired_batch_gives_warning_and_bad_date_is_rejected() {
    let mut conn = setup();
    let o = create(&mut conn, OpnameType::Opening);
    let r = service::save_item(&mut conn, &owner(), &new_batch(o.header.id, 1, "OLD", "2026-09-01", Some(100), Some(3)), today())
        .unwrap();
    assert_eq!(r.warnings.len(), 1);
    let bad = service::save_item(&mut conn, &owner(), &new_batch(o.header.id, 1, "B", "31-12-2027", Some(100), Some(3)), today());
    assert!(matches!(bad, Err(AppError::Validation(_))));
}

#[test]
fn submit_requires_all_items_counted() {
    let mut conn = setup();
    let o = create(&mut conn, OpnameType::Opening);
    assert!(matches!(service::submit_opname(&mut conn, &owner(), o.header.id), Err(AppError::Validation(_))));
    service::save_item(&mut conn, &owner(), &new_batch(o.header.id, 1, "A", "2027-01-01", Some(100), None), today()).unwrap();
    assert!(matches!(service::submit_opname(&mut conn, &owner(), o.header.id), Err(AppError::Validation(_))));
}

#[test]
fn periodic_opname_adjusts_differences() {
    let mut conn = setup();
    opening(&mut conn);
    let o = create(&mut conn, OpnameType::Periodic);
    let id = o.header.id;
    let filled = service::fill_opname(
        &mut conn,
        &owner(),
        &OpnameFillInput { opname_id: id, product_id: None, rack_id: Some(1), category_id: None },
    )
    .unwrap();
    assert_eq!(filled.opname.items.len(), 2, "hanya batch obat di rak 1");

    // Isi ulang tidak menggandakan baris.
    let again = service::fill_opname(
        &mut conn,
        &owner(),
        &OpnameFillInput { opname_id: id, product_id: Some(1), rack_id: None, category_id: None },
    )
    .unwrap();
    assert_eq!(again.opname.items.len(), 2);
    assert_eq!(again.warnings.len(), 1);

    // A100: sistem 100 → fisik 97; A200: sama; batch baru ditemukan 5.
    for item in &filled.opname.items {
        let physical = if item.batch_number == "A100" { 97 } else { item.system_qty_base };
        let input = OpnameItemInput {
            opname_id: id,
            id: Some(item.id),
            product_id: item.product_id,
            batch_id: item.batch_id,
            batch_number: None,
            expiry_date: None,
            unit_cost_x100: None,
            physical_qty_base: Some(physical),
            note: None,
        };
        service::save_item(&mut conn, &owner(), &input, today()).unwrap();
    }
    let r = service::save_item(&mut conn, &owner(), &new_batch(id, 1, "C9", "2027-06-30", Some(15_500), Some(5)), today()).unwrap();
    // Nilai selisih: -3 × 150,50 + 5 × 155 = -451,5 + 775 = 323,5 → 324.
    assert_eq!(r.opname.difference_value, Some(324));

    service::submit_opname(&mut conn, &owner(), id).unwrap();
    // Setelah diajukan baris tidak bisa diubah.
    let locked = service::save_item(&mut conn, &owner(), &new_batch(id, 1, "D", "2027-06-30", Some(1), Some(1)), today());
    assert!(matches!(locked, Err(AppError::Conflict(_))));

    service::approve_opname(&mut conn, &owner(), id).unwrap();
    assert_eq!(stock_of(&conn, 1), 130 - 3 + 5);
    let adjustments: i64 = conn
        .query_row("SELECT count(*) FROM stock_movements WHERE movement_type = 'ADJUSTMENT'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(adjustments, 2, "baris tanpa selisih tidak dicatat");
    let source: String = conn.query_row("SELECT source_type FROM batches WHERE batch_number = 'C9'", [], |r| r.get(0)).unwrap();
    assert_eq!(source, "ADJUSTMENT");
}

#[test]
fn approve_fails_when_adjustment_would_go_negative() {
    let mut conn = setup();
    opening(&mut conn);
    let o = create(&mut conn, OpnameType::Periodic);
    let id = o.header.id;
    let filled = service::fill_opname(
        &mut conn,
        &owner(),
        &OpnameFillInput { opname_id: id, product_id: Some(2), rack_id: None, category_id: None },
    )
    .unwrap();
    let item = &filled.opname.items[0];
    let input = OpnameItemInput {
        opname_id: id,
        id: Some(item.id),
        product_id: 2,
        batch_id: item.batch_id,
        batch_number: None,
        expiry_date: None,
        unit_cost_x100: None,
        physical_qty_base: Some(0),
        note: None,
    };
    service::save_item(&mut conn, &owner(), &input, today()).unwrap();
    service::submit_opname(&mut conn, &owner(), id).unwrap();

    // Stok batch berkurang (misal terjual) setelah snapshot: 20 → 5, penyesuaian -20 membuat minus.
    conn.execute(
        "INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id)
         VALUES (?1, 2, 'SALE', -15, 'sale', 1, 1)",
        [item.batch_id.unwrap()],
    )
    .unwrap();
    let err = service::approve_opname(&mut conn, &owner(), id);
    assert!(matches!(err, Err(AppError::Conflict(_))), "{err:?}");
    assert_eq!(stock_of(&conn, 2), 5, "seluruh approve di-rollback");
    let status = service::get_opname(&conn, &owner(), id).unwrap().header.status;
    assert_eq!(status, OpnameStatus::Submitted);

    // Kembalikan ke draft, perbaiki, lalu setujui.
    service::reopen_opname(&mut conn, &owner(), id).unwrap();
    service::delete_item(&mut conn, &owner(), id, item.id).unwrap();
    service::fill_opname(&mut conn, &owner(), &OpnameFillInput { opname_id: id, product_id: Some(2), rack_id: None, category_id: None })
        .unwrap();
    let item = service::get_opname(&conn, &owner(), id).unwrap().items.remove(0);
    assert_eq!(item.system_qty_base, 5, "snapshot baru");
    service::save_item(&mut conn, &owner(), &OpnameItemInput { id: Some(item.id), ..input }, today()).unwrap();
    service::submit_opname(&mut conn, &owner(), id).unwrap();
    service::approve_opname(&mut conn, &owner(), id).unwrap();
    assert_eq!(stock_of(&conn, 2), 0);
}

#[test]
fn cancelled_opname_changes_nothing_and_cannot_be_deleted() {
    let mut conn = setup();
    let o = create(&mut conn, OpnameType::Opening);
    service::save_item(&mut conn, &owner(), &new_batch(o.header.id, 1, "A", "2027-01-01", Some(100), Some(10)), today()).unwrap();
    let c = service::cancel_opname(&mut conn, &owner(), o.header.id, Some("salah input")).unwrap();
    assert_eq!(c.header.status, OpnameStatus::Cancelled);
    assert!(matches!(service::approve_opname(&mut conn, &owner(), o.header.id), Err(AppError::Conflict(_))));
    assert_eq!(stock_of(&conn, 1), 0);
    assert!(conn.execute("DELETE FROM stock_opnames", []).is_err());
}

#[test]
fn opening_lock_blocks_new_opening_opnames() {
    let mut conn = setup();
    let o = create(&mut conn, OpnameType::Opening);
    assert!(matches!(service::lock_opening(&mut conn, &owner()), Err(AppError::Conflict(_))), "masih ada draft");
    service::cancel_opname(&mut conn, &owner(), o.header.id, None).unwrap();
    service::lock_opening(&mut conn, &owner()).unwrap();
    let err = service::create_opname(
        &mut conn,
        &owner(),
        &OpnameCreateInput { opname_type: OpnameType::Opening, scope_note: None },
        today(),
    );
    assert!(matches!(err, Err(AppError::Conflict(_))));
    create(&mut conn, OpnameType::Periodic);
}

#[test]
fn stock_list_filters_and_hides_cost() {
    let mut conn = setup();
    opening(&mut conn);
    let q = |filter| StockListQuery { q: None, category_id: None, rack_id: None, filter, offset: 0, limit: 25 };

    let all = service::list_stock(&conn, &owner(), &q(StockFilter::All), today()).unwrap();
    assert_eq!(all.total, 2);
    let para = all.rows.iter().find(|r| r.product_id == 1).unwrap();
    assert_eq!((para.stock_base, para.sellable_base, para.batch_count), (130, 130, 2));
    assert_eq!(para.nearest_expiry.as_deref(), Some("2026-11-30"));
    // 100 × 150,50 + 30 × 160 = 15.050 + 4.800
    assert_eq!(para.stock_value, Some(19_850));

    // A200 ED 2026-11-30 ≤ 90 hari dari 2026-09-26.
    let near = service::list_stock(&conn, &owner(), &q(StockFilter::NearExpiry), today()).unwrap();
    assert_eq!(near.rows.iter().map(|r| r.product_id).collect::<Vec<_>>(), [1]);

    // Setelah A200 lewat ED: stok jual berkurang dan masuk filter expired.
    let later = NaiveDate::from_ymd_opt(2026, 12, 1).unwrap();
    let expired = service::list_stock(&conn, &owner(), &q(StockFilter::Expired), later).unwrap();
    assert_eq!(expired.rows.len(), 1);
    assert_eq!((expired.rows[0].sellable_base, expired.rows[0].expired_base), (100, 30));

    let cashier = service::list_stock(&conn, &user(&[Role::Cashier]), &q(StockFilter::All), today()).unwrap();
    assert!(cashier.rows.iter().all(|r| r.stock_value.is_none()));
    let batches = service::product_batches(&conn, &user(&[Role::Technician]), 1, false).unwrap();
    assert!(batches.batches.iter().all(|b| b.unit_cost_x100.is_none()));

    let search = service::list_stock(&conn, &owner(), &StockListQuery { q: Some("amox".into()), ..q(StockFilter::All) }, today()).unwrap();
    assert_eq!(search.rows.len(), 1);
}

#[test]
fn locked_batch_is_not_sellable() {
    let mut conn = setup();
    opening(&mut conn);
    let batch_id: i64 = conn.query_row("SELECT id FROM batches WHERE batch_number = 'A200'", [], |r| r.get(0)).unwrap();
    assert!(matches!(service::set_batch_locked(&conn, &owner(), batch_id, true, None), Err(AppError::Validation(_))));
    service::set_batch_locked(&conn, &owner(), batch_id, true, Some("Recall BPOM")).unwrap();

    let q = StockListQuery { q: None, category_id: None, rack_id: None, filter: StockFilter::All, offset: 0, limit: 25 };
    let rows = service::list_stock(&conn, &owner(), &q, today()).unwrap().rows;
    let para = rows.iter().find(|r| r.product_id == 1).unwrap();
    assert_eq!((para.stock_base, para.sellable_base), (130, 100));
    let b = service::product_batches(&conn, &owner(), 1, false).unwrap();
    assert_eq!(b.batches.iter().find(|b| b.id == batch_id).unwrap().lock_reason.as_deref(), Some("Recall BPOM"));

    let logged: i64 = conn
        .query_row("SELECT count(*) FROM audit_logs WHERE entity = 'batches' AND action = 'LOCK'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(logged, 1);
}

#[test]
fn stock_card_date_range_keeps_running_balance() {
    let mut conn = setup();
    opening(&mut conn);
    conn.execute_batch(
        "INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id, created_at)
             SELECT id, 1, 'SALE', -10, 'sale', 1, 1, '2099-01-05 10:00:00' FROM batches WHERE batch_number = 'A100';
         INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id, created_at)
             SELECT id, 1, 'SALE', -5, 'sale', 2, 1, '2099-01-06 10:00:00' FROM batches WHERE batch_number = 'A200';",
    )
    .unwrap();
    let card = service::stock_card(
        &conn,
        &StockCardQuery {
            product_id: 1,
            batch_id: None,
            date_from: Some("2099-01-05".into()),
            date_to: Some("2099-01-05".into()),
            offset: 0,
            limit: 50,
        },
    )
    .unwrap();
    assert_eq!(card.opening_balance, 130);
    assert_eq!(card.rows.len(), 1);
    assert_eq!(card.rows[0].balance_base, 120);
    assert_eq!((card.total_out, card.closing_balance), (10, 120));

    // Per batch: saldo hanya batch itu.
    let a200: i64 = conn.query_row("SELECT id FROM batches WHERE batch_number = 'A200'", [], |r| r.get(0)).unwrap();
    let card = service::stock_card(
        &conn,
        &StockCardQuery { product_id: 1, batch_id: Some(a200), date_from: None, date_to: None, offset: 0, limit: 50 },
    )
    .unwrap();
    assert_eq!(card.rows.iter().map(|r| r.balance_base).collect::<Vec<_>>(), [30, 25]);
}
