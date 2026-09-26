use chrono::NaiveDate;
use rusqlite::Connection;

use super::service;
use crate::auth::{AccessSettings, Role, SessionUser, effective_permissions};
use crate::db;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 26).unwrap()
}

/// User 1 kasir (shift terbuka miliknya), user 2 kasir lain.
/// P1 Paracetamol min 50: stok 30 (ED 2026-11-30) + 10 sudah ED → menipis.
/// P2 Amoxicillin min 0: stok 20 (ED 2028). P3 Vitamin min 5: stok kosong.
/// Penjualan hari ini: 2 nota selesai + 1 batal, kemarin 1 nota.
fn setup() -> Connection {
    let conn = db::open_in_memory().unwrap();
    conn.execute_batch(
        "INSERT INTO users (id, username, full_name, password_hash) VALUES (1, 'kasir', 'Kasir Satu', 'x'),
                                                                        (2, 'lain', 'Kasir Dua', 'x');
         INSERT INTO products (id, code, name, drug_class, base_unit_id, min_stock_base)
             VALUES (1, 'P1', 'Paracetamol', 'FREE', 1, 50),
                    (2, 'P2', 'Amoxicillin', 'HARD', 1, 0),
                    (3, 'P3', 'Vitamin C', 'FREE', 1, 5);
         INSERT INTO product_units (id, product_id, unit_id, conversion, sell_price)
             VALUES (1, 1, 1, 1, 500), (3, 2, 1, 1, 1000);
         INSERT INTO batches (id, product_id, batch_number, expiry_date, unit_cost_x100, qty_on_hand_base, source_type)
             VALUES (1, 1, 'A1', '2026-11-30', 15000, 30, 'OPENING'),
                    (2, 1, 'A0', '2026-09-01', 15000, 10, 'OPENING'),
                    (3, 2, 'X1', '2028-05-01', 50000, 20, 'OPENING');

         -- Shift kemarin: nota harus masuk saat shift masih terbuka, baru ditutup.
         INSERT INTO shifts (id, user_id, opened_at, opening_cash, status)
             VALUES (2, 2, '2026-09-25 07:00:00', 50000, 'OPEN');
         INSERT INTO sales (id, number, shift_id, cashier_id, sale_type, sold_at, subtotal, grand_total, status)
             VALUES (4, 'PJ-0', 2, 2, 'OTC', '2026-09-25 15:00:00', 4000, 4000, 'COMPLETED');
         UPDATE shifts SET status = 'CLOSED', closed_at = '2026-09-25 21:00:00' WHERE id = 2;

         INSERT INTO shifts (id, user_id, opened_at, opening_cash, status)
             VALUES (1, 1, '2026-09-26 07:00:00', 100000, 'OPEN');
         INSERT INTO sales (id, number, shift_id, cashier_id, sale_type, sold_at, subtotal, grand_total, status,
                            voided_at, voided_by, void_reason)
             VALUES (1, 'PJ-1', 1, 1, 'OTC', '2026-09-26 08:00:00', 10000, 10000, 'COMPLETED', NULL, NULL, NULL),
                    (2, 'PJ-2', 1, 1, 'OTC', '2026-09-26 09:00:00', 5000, 5000, 'COMPLETED', NULL, NULL, NULL),
                    (3, 'PJ-3', 1, 1, 'OTC', '2026-09-26 09:30:00', 7000, 7000, 'VOID',
                     '2026-09-26 09:40:00', 1, 'salah input');
         INSERT INTO sale_items (id, sale_id, line_no, item_kind, product_id, product_unit_id, description,
                                 qty, conversion, qty_base, unit_price, line_total)
             VALUES (1, 1, 1, 'PRODUCT', 1, 1, 'Paracetamol', 20, 1, 20, 500, 10000),
                    (2, 2, 1, 'PRODUCT', 2, 3, 'Amoxicillin', 5, 1, 5, 1000, 5000);
         INSERT INTO sale_item_batches (sale_item_id, batch_id, qty_base, unit_cost_x100)
             VALUES (1, 1, 20, 15000), (2, 3, 5, 50000);
         INSERT INTO sale_payments (sale_id, method, amount, tendered, change_amount)
             VALUES (1, 'CASH', 10000, 20000, 10000);
         INSERT INTO sale_payments (sale_id, method, amount) VALUES (2, 'QRIS', 5000);

         INSERT INTO suppliers (id, name) VALUES (1, 'PBF Sehat');
         INSERT INTO purchases (id, number, supplier_id, invoice_number, invoice_date, received_date, due_date,
                                payment_type, tax_mode, grand_total, status, created_by)
             VALUES (1, 'PB-1', 1, 'F-1', '2026-08-01', '2026-08-01', '2026-09-20', 'CREDIT', 'NONE', 500000, 'POSTED', 1),
                    (2, 'PB-2', 1, 'F-2', '2026-09-01', '2026-09-01', '2026-09-30', 'CREDIT', 'NONE', 300000, 'POSTED', 1),
                    (3, 'PB-3', 1, 'F-3', '2026-09-01', '2026-09-01', '2026-09-10', 'CREDIT', 'NONE', 100000, 'POSTED', 1),
                    (4, 'PB-4', 1, 'F-4', '2026-09-01', '2026-09-01', NULL, 'CASH', 'NONE', 900000, 'POSTED', 1);
         INSERT INTO supplier_payments (supplier_id, purchase_id, payment_date, amount, method, created_by)
             VALUES (1, 1, '2026-09-01', 200000, 'TRANSFER', 1),
                    (1, 3, '2026-09-05', 100000, 'CASH', 1);",
    )
    .unwrap();
    conn
}

fn user(id: i64, roles: &[Role], access: AccessSettings) -> SessionUser {
    SessionUser {
        id,
        username: "u".into(),
        full_name: "U".into(),
        roles: roles.to_vec(),
        permissions: effective_permissions(roles, access),
    }
}

fn as_role(role: Role) -> SessionUser {
    user(1, &[role], AccessSettings::default())
}

#[test]
fn cashier_sees_only_own_shift_and_prescriptions_to_pay() {
    let conn = setup();
    let d = service::get(&conn, &as_role(Role::Cashier), today()).unwrap();

    assert!(d.sales.is_none() && d.stock.is_none() && d.opname.is_none() && d.debts.is_none());

    let shift = d.shift.unwrap();
    assert_eq!((shift.my_sales_today.count, shift.my_sales_today.amount), (2, 15_000));
    let open = shift.open.unwrap();
    assert!(open.is_mine);
    assert_eq!(open.sales.unwrap().amount, 15_000);
    // Modal 100.000 + tunai 10.000 (nota QRIS dan nota batal tidak masuk laci).
    assert_eq!(open.expected_cash, Some(110_000));

    let rx = d.prescriptions.unwrap();
    assert_eq!(rx.queue_status, "SCREENED");
}

#[test]
fn other_cashiers_shift_numbers_are_hidden() {
    let conn = setup();
    let d = service::get(&conn, &user(2, &[Role::Cashier], AccessSettings::default()), today()).unwrap();
    let shift = d.shift.unwrap();
    assert_eq!(shift.my_sales_today.count, 0);
    let open = shift.open.unwrap();
    assert!(!open.is_mine);
    assert_eq!(open.opened_by, "Kasir Satu");
    assert!(open.opening_cash.is_none() && open.sales.is_none() && open.expected_cash.is_none());
}

#[test]
fn technician_sees_stock_without_cost() {
    let conn = setup();
    let d = service::get(&conn, &as_role(Role::Technician), today()).unwrap();
    assert!(d.sales.is_none() && d.debts.is_none());

    let stock = d.stock.unwrap();
    assert!(stock.inventory_value.is_none());
    assert_eq!((stock.low_count, stock.empty_count), (2, 1));
    assert_eq!((stock.near_expiry_count, stock.expired_count), (1, 1));
    // Sudah ED dulu, lalu yang hampir ED; batch ED 2028 tidak ikut.
    let batches: Vec<_> = stock.expiring.iter().map(|e| (e.batch_number.as_str(), e.days_left)).collect();
    assert_eq!(batches, [("A0", -25), ("A1", 65)]);
    // Vitamin C kosong paling kritis, lalu Paracetamol 40/50.
    let low: Vec<_> = stock.low_stock.iter().map(|l| (l.code.as_str(), l.stock_base)).collect();
    assert_eq!(low, [("P3", 0), ("P1", 40)]);

    assert_eq!(d.prescriptions.unwrap().queue_status, "DRAFT");
    assert!(!d.opname.unwrap().can_approve);
}

#[test]
fn owner_sees_sales_profit_value_and_debts() {
    let conn = setup();
    let d = service::get(&conn, &as_role(Role::Owner), today()).unwrap();

    let sales = d.sales.unwrap();
    assert_eq!((sales.today.count, sales.today.amount), (2, 15_000));
    assert_eq!(sales.yesterday.amount, 4_000);
    assert_eq!(sales.month.amount, 19_000);
    assert_eq!((sales.void_today.count, sales.void_today.amount), (1, 7_000));
    assert_eq!(sales.daily.len(), 7);
    assert_eq!(sales.daily[6].amount, 15_000);
    assert_eq!(sales.daily[5].amount, 4_000);
    assert_eq!(sales.daily[0].amount, 0);
    let methods: Vec<_> = sales.by_method_today.iter().map(|m| (m.method.as_str(), m.amount)).collect();
    assert_eq!(methods, [("CASH", 10_000), ("QRIS", 5_000)]);
    assert_eq!(sales.top_products[0].name, "Paracetamol");
    // 15.000 − (20 × 150 + 5 × 500) = 9.500
    assert_eq!(sales.gross_profit_today, Some(9_500));

    // (30 + 10) × 150 + 20 × 500
    assert_eq!(d.stock.unwrap().inventory_value, Some(16_000));
    assert!(d.opname.unwrap().can_approve);

    let debts = d.debts.unwrap();
    assert_eq!((debts.outstanding.count, debts.outstanding.amount), (2, 600_000));
    assert_eq!((debts.overdue.count, debts.overdue.amount), (1, 300_000));
    assert_eq!((debts.due_soon.count, debts.due_soon.amount), (1, 300_000));
    let rows: Vec<_> = debts.upcoming.iter().map(|r| (r.number.as_str(), r.days_left)).collect();
    assert_eq!(rows, [("PB-1", -6), ("PB-2", 4)]);
}

#[test]
fn pharmacist_profit_follows_cost_setting() {
    let conn = setup();
    let off = AccessSettings { pharmacist_can_view_cost: false, pharmacist_can_edit_price: false };
    let d = service::get(&conn, &user(1, &[Role::Pharmacist], off), today()).unwrap();
    let sales = d.sales.unwrap();
    assert_eq!(sales.today.amount, 15_000);
    assert!(sales.gross_profit_today.is_none() && sales.gross_profit_month.is_none());
    assert!(d.stock.unwrap().inventory_value.is_none());
    assert!(d.debts.is_none());
    assert!(d.opname.unwrap().can_approve);
}
