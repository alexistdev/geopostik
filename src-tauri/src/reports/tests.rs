use chrono::NaiveDate;
use rusqlite::Connection;

use super::model::{ExpiryReportQuery, ReportRange, SipnapQuery};
use super::service;
use crate::auth::{AccessSettings, Role, SessionUser, effective_permissions};
use crate::db;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 26).unwrap()
}

fn range(from: &str, to: &str) -> ReportRange {
    ReportRange { from: from.into(), to: to.into() }
}

/// User 1 & 2 kasir. P1 Paracetamol (bebas), P2 Amoxicillin (keras), P3 Vitamin C (tanpa penjualan),
/// P4 Codein (narkotika). Shift 2 kemarin milik user 2, shift 1 hari ini milik user 1 (masih buka);
/// user 2 juga berjualan di shift 1.
fn setup() -> Connection {
    let conn = db::open_in_memory().unwrap();
    conn.execute_batch(
        "INSERT INTO users (id, username, full_name, password_hash) VALUES (1, 'kasir', 'Kasir Satu', 'x'),
                                                                        (2, 'lain', 'Kasir Dua', 'x');
         INSERT INTO categories (id, code, name) VALUES (1, 'KAT001', 'Analgesik');
         INSERT INTO products (id, code, name, drug_class, base_unit_id, category_id)
             VALUES (1, 'P1', 'Paracetamol', 'FREE', 1, 1),
                    (2, 'P2', 'Amoxicillin', 'HARD', 1, NULL),
                    (3, 'P3', 'Vitamin C', 'FREE', 1, NULL),
                    (4, 'P4', 'Codein', 'NARCOTIC', 1, 1);
         INSERT INTO product_units (id, product_id, unit_id, conversion, sell_price)
             VALUES (1, 1, 1, 1, 500), (3, 2, 1, 1, 1000);
         INSERT INTO batches (id, product_id, batch_number, expiry_date, unit_cost_x100, qty_on_hand_base, source_type)
             VALUES (1, 1, 'A1', '2026-11-30', 15000, 30, 'OPENING'),
                    (2, 1, 'A0', '2026-09-01', 15000, 10, 'OPENING'),
                    (3, 2, 'X1', '2028-05-01', 50000, 20, 'OPENING'),
                    (4, 3, 'V1', '2027-01-31', 20000, 5, 'OPENING');

         INSERT INTO shifts (id, user_id, opened_at, opening_cash, status)
             VALUES (2, 2, '2026-09-25 07:00:00', 50000, 'OPEN');
         INSERT INTO sales (id, number, shift_id, cashier_id, sale_type, sold_at, subtotal, grand_total, status)
             VALUES (4, 'PJ-0', 2, 2, 'OTC', '2026-09-25 15:00:00', 4000, 4000, 'COMPLETED');
         INSERT INTO sale_payments (sale_id, method, amount) VALUES (4, 'QRIS', 4000);
         UPDATE shifts SET status = 'CLOSED', closed_at = '2026-09-25 21:00:00',
                           expected_cash = 50000, counted_cash = 49000, difference = -1000 WHERE id = 2;

         INSERT INTO shifts (id, user_id, opened_at, opening_cash, status)
             VALUES (1, 1, '2026-09-26 07:00:00', 100000, 'OPEN');
         INSERT INTO sales (id, number, shift_id, cashier_id, sale_type, sold_at, subtotal, discount_total,
                            grand_total, status, voided_at, voided_by, void_reason)
             VALUES (1, 'PJ-1', 1, 1, 'OTC', '2026-09-26 08:00:00', 10500, 500, 10000, 'COMPLETED', NULL, NULL, NULL),
                    (2, 'PJ-2', 1, 2, 'OTC', '2026-09-26 09:00:00', 5000, 0, 5000, 'COMPLETED', NULL, NULL, NULL),
                    (3, 'PJ-3', 1, 1, 'OTC', '2026-09-26 09:30:00', 7000, 0, 7000, 'VOID',
                     '2026-09-26 09:40:00', 1, 'salah input');
         INSERT INTO sale_items (id, sale_id, line_no, item_kind, product_id, product_unit_id, description,
                                 qty, conversion, qty_base, unit_price, line_total)
             VALUES (1, 1, 1, 'PRODUCT', 1, 1, 'Paracetamol', 20, 1, 20, 500, 10000),
                    (2, 2, 1, 'PRODUCT', 2, 3, 'Amoxicillin', 5, 1, 5, 1000, 5000),
                    (3, 4, 1, 'PRODUCT', 1, 1, 'Paracetamol', 8, 1, 8, 500, 4000);
         INSERT INTO sale_item_batches (sale_item_id, batch_id, qty_base, unit_cost_x100)
             VALUES (1, 1, 20, 15000), (2, 3, 5, 50000), (3, 1, 8, 15000);
         INSERT INTO sale_payments (sale_id, method, amount, tendered, change_amount)
             VALUES (1, 'CASH', 10000, 20000, 10000);
         INSERT INTO sale_payments (sale_id, method, amount) VALUES (2, 'QRIS', 5000);

         INSERT INTO suppliers (id, code, name) VALUES (1, 'SUP0001', 'PBF Sehat'), (2, 'SUP0002', 'PBF Maju');
         INSERT INTO purchases (id, number, supplier_id, invoice_number, invoice_date, received_date, due_date,
                                payment_type, tax_mode, subtotal, tax_total, grand_total, status, created_by)
             VALUES (1, 'PB-1', 1, 'F-1', '2026-09-01', '2026-09-01', '2026-10-01', 'CREDIT', 'NONE', 500000, 0, 500000, 'POSTED', 1),
                    (2, 'PB-2', 2, 'F-2', '2026-09-10', '2026-09-10', NULL, 'CASH', 'NONE', 200000, 0, 200000, 'POSTED', 1),
                    (3, 'PB-3', 1, 'F-3', '2026-08-10', '2026-08-10', NULL, 'CASH', 'NONE', 90000, 0, 90000, 'POSTED', 1),
                    (4, 'PB-4', 1, 'F-4', '2026-09-12', '2026-09-12', NULL, 'CASH', 'NONE', 70000, 0, 70000, 'DRAFT', 1);
         INSERT INTO supplier_payments (supplier_id, purchase_id, payment_date, amount, method, created_by)
             VALUES (1, 1, '2026-09-05', 200000, 'TRANSFER', 1);

         -- Kartu stok Codein: 100 masuk Agustus, September terjual 30 (5 dibatalkan), musnah 10, opname −2.
         INSERT INTO batches (id, product_id, batch_number, expiry_date, unit_cost_x100, source_type)
             VALUES (5, 4, 'C1', '2028-01-01', 100000, 'PURCHASE');
         INSERT INTO stock_movements (batch_id, product_id, movement_type, qty_change_base, ref_type, ref_id, user_id, created_at)
             VALUES (5, 4, 'PURCHASE', 100, 'PURCHASE', 3, 1, '2026-08-10 10:00:00'),
                    (5, 4, 'SALE', -30, 'SALE', 9, 1, '2026-09-03 10:00:00'),
                    (5, 4, 'SALE_VOID', 5, 'SALE', 9, 1, '2026-09-03 10:05:00'),
                    (5, 4, 'DESTRUCTION', -10, 'DESTRUCTION', 1, 1, '2026-09-20 10:00:00'),
                    (5, 4, 'ADJUSTMENT', -2, 'OPNAME', 1, 1, '2026-09-25 10:00:00'),
                    (5, 4, 'PURCHASE', 50, 'PURCHASE', 5, 1, '2026-10-02 10:00:00');",
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
fn owner_sales_report_covers_all_cashiers_with_profit() {
    let conn = setup();
    let r = service::sales(&conn, &as_role(Role::Owner), &range("2026-09-25", "2026-09-26")).unwrap();
    assert!(!r.own_only);

    let s = &r.summary;
    assert_eq!((s.count, s.net, s.subtotal, s.discount), (3, 19_000, 19_500, 500));
    assert_eq!((s.void_count, s.void_amount), (1, 7_000));
    // HPP: 20 × 150 + 5 × 500 + 8 × 150 = 6.700 → laba 12.300
    assert_eq!((s.cost, s.gross_profit), (Some(6_700), Some(12_300)));

    let days: Vec<_> = r.daily.iter().map(|d| (d.date.as_str(), d.amount, d.gross_profit)).collect();
    assert_eq!(days, [("2026-09-25", 4_000, Some(2_800)), ("2026-09-26", 15_000, Some(9_500))]);

    let methods: Vec<_> = r.by_method.iter().map(|m| (m.method.as_str(), m.count, m.amount)).collect();
    assert_eq!(methods, [("CASH", 1, 10_000), ("QRIS", 2, 9_000)]);
    let cashiers: Vec<_> = r.by_cashier.iter().map(|c| (c.name.as_str(), c.amount)).collect();
    assert_eq!(cashiers, [("Kasir Satu", 10_000), ("Kasir Dua", 9_000)]);

    // Shift terbaru dulu; shift terbuka dihitung tunai seharusnya: 100.000 + 10.000.
    let shifts: Vec<_> = r.shifts.iter().map(|x| (x.id, x.amount, x.expected_cash, x.difference)).collect();
    assert_eq!(shifts, [(1, 15_000, Some(110_000), None), (2, 4_000, Some(50_000), Some(-1_000))]);
}

#[test]
fn cashier_sales_report_is_limited_to_own_sales() {
    let conn = setup();
    let r = service::sales(&conn, &user(2, &[Role::Cashier], AccessSettings::default()), &range("2026-09-25", "2026-09-26"))
        .unwrap();
    assert!(r.own_only);
    assert_eq!((r.summary.count, r.summary.net), (2, 9_000));
    assert_eq!(r.summary.void_count, 0);
    assert!(r.summary.cost.is_none() && r.summary.gross_profit.is_none());
    assert!(r.daily.iter().all(|d| d.gross_profit.is_none()));
    assert_eq!(r.by_cashier.len(), 1);

    // Shift 1 milik kasir lain: hanya nota sendiri yang terhitung, angka kas disembunyikan.
    let s1 = r.shifts.iter().find(|s| s.id == 1).unwrap();
    assert_eq!((s1.count, s1.amount), (1, 5_000));
    assert!(s1.opening_cash.is_none() && s1.expected_cash.is_none());
    let s2 = r.shifts.iter().find(|s| s.id == 2).unwrap();
    assert_eq!((s2.opening_cash, s2.counted_cash), (Some(50_000), Some(49_000)));
}

#[test]
fn sales_range_is_validated() {
    let conn = setup();
    let owner = as_role(Role::Owner);
    assert!(service::sales(&conn, &owner, &range("2026-09-26", "2026-09-25")).is_err());
    assert!(service::sales(&conn, &owner, &range("2026-09-xx", "2026-09-25")).is_err());
    assert!(service::sales(&conn, &owner, &range("2025-01-01", "2026-09-25")).is_err());
    let r = service::sales(&conn, &owner, &range("2026-09-01", "2026-09-30")).unwrap();
    assert_eq!(r.daily.len(), 30);
}

#[test]
fn product_report_ranks_sales_and_finds_slow_movers() {
    let conn = setup();
    let r = service::products(&conn, &as_role(Role::Owner), &range("2026-09-25", "2026-09-26")).unwrap();
    let top: Vec<_> = r.top.iter().map(|p| (p.code.as_str(), p.qty_base, p.amount, p.gross_profit)).collect();
    assert_eq!(top, [("P1", 28, 14_000, Some(9_800)), ("P2", 5, 5_000, Some(2_500))]);
    // Vitamin C masih ada stok, tidak terjual. Codein ada stok juga.
    let slow: Vec<_> = r.slow.iter().map(|p| (p.code.as_str(), p.value)).collect();
    assert_eq!(slow, [("P4", Some(113_000)), ("P3", Some(1_000))]);

    let tech = service::products(&conn, &as_role(Role::Pharmacist), &range("2026-09-26", "2026-09-26")).unwrap();
    assert_eq!(tech.top.len(), 2);
    let off = AccessSettings { pharmacist_can_view_cost: false, pharmacist_can_edit_price: false };
    let hidden = service::products(&conn, &user(1, &[Role::Pharmacist], off), &range("2026-09-26", "2026-09-26")).unwrap();
    assert!(hidden.top.iter().all(|p| p.cost.is_none() && p.gross_profit.is_none()));
    assert!(hidden.slow.iter().all(|p| p.value.is_none()));
}

#[test]
fn inventory_report_values_stock_per_product_and_category() {
    let conn = setup();
    let r = service::inventory(&conn, today()).unwrap();
    // P1 40 × 150, P2 20 × 500, P3 5 × 200, P4 113 × 1.000
    assert_eq!(r.total_value, 6_000 + 10_000 + 1_000 + 113_000);
    assert_eq!((r.product_count, r.batch_count), (4, 5));
    // Batch A0 (10 × 150) sudah ED.
    assert_eq!(r.expired_value, 1_500);
    assert_eq!(r.rows[0].code, "P4");
    let cats: Vec<_> = r.by_category.iter().map(|c| (c.category.as_deref(), c.value)).collect();
    assert_eq!(cats, [(Some("Analgesik"), 119_000), (None, 11_000)]);
}

#[test]
fn expiry_report_hides_value_without_cost() {
    let conn = setup();
    let r = service::expiry(&conn, &as_role(Role::Owner), &ExpiryReportQuery { days: 90 }, today()).unwrap();
    let rows: Vec<_> = r.rows.iter().map(|x| (x.batch_number.as_str(), x.days_left)).collect();
    assert_eq!(rows, [("A0", -25), ("A1", 65)]);
    assert_eq!((r.expired_count, r.near_count), (1, 1));
    assert_eq!((r.expired_value, r.near_value), (Some(1_500), Some(4_500)));

    let r = service::expiry(&conn, &as_role(Role::Technician), &ExpiryReportQuery { days: 180 }, today()).unwrap();
    assert_eq!(r.rows.len(), 3);
    assert!(r.expired_value.is_none() && r.rows.iter().all(|x| x.value.is_none()));

    assert!(service::expiry(&conn, &as_role(Role::Owner), &ExpiryReportQuery { days: 0 }, today()).is_err());
}

#[test]
fn purchase_report_counts_posted_invoices_in_range() {
    let conn = setup();
    let r = service::purchases(&conn, &as_role(Role::Owner), &range("2026-09-01", "2026-09-30")).unwrap();
    assert_eq!((r.count, r.total, r.cash_total, r.credit_total), (2, 700_000, 200_000, 500_000));
    let sup: Vec<_> = r.by_supplier.iter().map(|s| (s.name.as_str(), s.total)).collect();
    assert_eq!(sup, [("PBF Sehat", 500_000), ("PBF Maju", 200_000)]);
    let inv: Vec<_> = r.invoices.iter().map(|i| (i.number.as_str(), i.outstanding)).collect();
    assert_eq!(inv, [("PB-1", Some(300_000)), ("PB-2", None)]);

    // TTK boleh melihat pembelian tetapi tidak sisa hutang.
    let t = service::purchases(&conn, &as_role(Role::Technician), &range("2026-09-01", "2026-09-30")).unwrap();
    assert!(!t.shows_debt);
    assert!(t.invoices.iter().all(|i| i.outstanding.is_none()));
}

#[test]
fn sipnap_reports_monthly_narcotic_movements() {
    let conn = setup();
    let r = service::sipnap(&conn, &SipnapQuery { year: 2026, month: 9 }).unwrap();
    assert_eq!(r.rows.len(), 1);
    let c = &r.rows[0];
    assert_eq!(c.code, "P4");
    assert_eq!((c.opening, c.received, c.sold, c.destroyed, c.adjusted), (100, 0, 25, 10, -2));
    assert_eq!(c.closing, 63);

    let aug = service::sipnap(&conn, &SipnapQuery { year: 2026, month: 8 }).unwrap();
    assert_eq!((aug.rows[0].opening, aug.rows[0].received, aug.rows[0].closing), (0, 100, 100));

    assert!(service::sipnap(&conn, &SipnapQuery { year: 2026, month: 13 }).is_err());
}
