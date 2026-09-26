use chrono::{Datelike, Days, NaiveDate};
use rusqlite::Connection;

use super::model::{
    DailyTotal, Dashboard, DebtPanel, OpenShift, OpnamePanel, RxPanel, SalesPanel, ShiftPanel, StockPanel,
};
use super::repo;
use crate::auth::{Permission, SessionUser};
use crate::error::AppResult;
use crate::settings;

/// Batch dengan ED sampai sekian hari ke depan dianggap "hampir ED" (FLOW.md: ≤ 3 bulan),
/// sama dengan menu Stok.
const NEAR_EXPIRY_DAYS: u64 = 90;
/// Hutang yang jatuh tempo dalam sekian hari ke depan ditandai "segera".
const DEBT_SOON_DAYS: u64 = 7;
/// Jumlah baris daftar di tiap panel.
const LIST_LIMIT: i64 = 8;

fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn plus_days(d: NaiveDate, n: u64) -> NaiveDate {
    d.checked_add_days(Days::new(n)).unwrap_or(d)
}

fn minus_days(d: NaiveDate, n: u64) -> NaiveDate {
    d.checked_sub_days(Days::new(n)).unwrap_or(d)
}

/// Menyusun dashboard sesuai hak `user`. Panel yang tidak berhak tidak dihitung sama sekali.
pub fn get(conn: &Connection, user: &SessionUser, today: NaiveDate) -> AppResult<Dashboard> {
    let can = |p: Permission| user.permissions.contains(&p);

    let shift = if can(Permission::SaleCreate) { Some(shift_panel(conn, user, today)?) } else { None };
    let sales = if can(Permission::ReportSales) { Some(sales_panel(conn, can(Permission::ViewCost), today)?) } else { None };
    let stock = if can(Permission::StockCountInput) { Some(stock_panel(conn, can(Permission::ViewCost), today)?) } else { None };
    let prescriptions = if can(Permission::PrescriptionInput) || can(Permission::PrescriptionValidate) {
        Some(rx_panel(conn, "DRAFT")?)
    } else if can(Permission::SaleCreate) {
        Some(rx_panel(conn, "SCREENED")?)
    } else {
        None
    };
    let opname = if can(Permission::StockCountInput) {
        Some(opname_panel(conn, can(Permission::StockCountApprove))?)
    } else {
        None
    };
    let debts = if can(Permission::SupplierDebtManage) { Some(debt_panel(conn, today)?) } else { None };

    Ok(Dashboard { today: ymd(today), shift, sales, stock, prescriptions, opname, debts })
}

fn shift_panel(conn: &Connection, user: &SessionUser, today: NaiveDate) -> AppResult<ShiftPanel> {
    let tomorrow = ymd(plus_days(today, 1));
    let my_sales_today = repo::cashier_sales(conn, user.id, &ymd(today), &tomorrow)?;

    let open = match repo::open_shift(conn)? {
        None => None,
        Some(s) => {
            let is_mine = s.user_id == user.id;
            // Angka shift kasir lain hanya untuk yang berhak melihat laporan penjualan.
            let show = is_mine || user.permissions.contains(&Permission::ReportSales);
            let (sales, expected_cash) = if show {
                (Some(repo::shift_sales(conn, s.id)?), Some(s.opening_cash + repo::shift_cash_in(conn, s.id)?))
            } else {
                (None, None)
            };
            Some(OpenShift {
                id: s.id,
                opened_by: s.opened_by,
                opened_at: s.opened_at,
                is_mine,
                opening_cash: show.then_some(s.opening_cash),
                sales,
                expected_cash,
            })
        }
    };
    Ok(ShiftPanel { open, my_sales_today })
}

fn sales_panel(conn: &Connection, view_cost: bool, today: NaiveDate) -> AppResult<SalesPanel> {
    let (d0, d1) = (ymd(today), ymd(plus_days(today, 1)));
    let yesterday = ymd(minus_days(today, 1));
    let month_start = ymd(today.with_day(1).unwrap_or(today));
    let week_start = minus_days(today, 6);

    // Lengkapi 7 hari terakhir agar grafik tetap punya 7 batang walau ada hari tanpa penjualan.
    let found = repo::daily_between(conn, &ymd(week_start), &d1)?;
    let daily = (0..7)
        .map(|i| {
            let date = ymd(plus_days(week_start, i));
            found
                .iter()
                .find(|f| f.date == date)
                .map(|f| DailyTotal { date: f.date.clone(), count: f.count, amount: f.amount })
                .unwrap_or(DailyTotal { date, count: 0, amount: 0 })
        })
        .collect();

    Ok(SalesPanel {
        today: repo::sales_between(conn, &d0, &d1)?,
        yesterday: repo::sales_between(conn, &yesterday, &d0)?,
        month: repo::sales_between(conn, &month_start, &d1)?,
        void_today: repo::voids_between(conn, &d0, &d1)?,
        prescription_today: repo::prescription_sales_between(conn, &d0, &d1)?,
        by_method_today: repo::by_method_between(conn, &d0, &d1)?,
        daily,
        top_products: repo::top_products_between(conn, &month_start, &d1, 5)?,
        gross_profit_today: if view_cost { Some(repo::gross_profit_between(conn, &d0, &d1)?) } else { None },
        gross_profit_month: if view_cost { Some(repo::gross_profit_between(conn, &month_start, &d1)?) } else { None },
    })
}

fn stock_panel(conn: &Connection, view_cost: bool, today: NaiveDate) -> AppResult<StockPanel> {
    let (t, near) = (ymd(today), ymd(plus_days(today, NEAR_EXPIRY_DAYS)));
    let counts = repo::stock_counts(conn, &t, &near)?;
    Ok(StockPanel {
        low_count: counts.low,
        empty_count: counts.empty,
        near_expiry_count: counts.near_expiry,
        expired_count: counts.expired,
        expiring: repo::expiring(conn, &t, &near, LIST_LIMIT)?,
        low_stock: repo::low_stock(conn, LIST_LIMIT)?,
        inventory_value: if view_cost { Some(repo::inventory_value(conn)?) } else { None },
    })
}

fn rx_panel(conn: &Connection, queue_status: &str) -> AppResult<RxPanel> {
    Ok(RxPanel {
        awaiting_screening: repo::prescription_count(conn, "DRAFT")?,
        ready_to_pay: repo::prescription_count(conn, "SCREENED")?,
        queue_status: queue_status.to_owned(),
        queue: repo::prescription_queue(conn, queue_status, LIST_LIMIT)?,
    })
}

fn opname_panel(conn: &Connection, can_approve: bool) -> AppResult<OpnamePanel> {
    Ok(OpnamePanel {
        drafts: repo::opname_count(conn, "DRAFT")?,
        awaiting_approval: repo::opname_count(conn, "SUBMITTED")?,
        // Kunci yang sama dengan `inventory::service::STOCK_OPENING_LOCKED`.
        opening_locked: settings::get(conn, "stock.opening_locked")?.unwrap_or(false),
        can_approve,
        pending: repo::opname_pending(conn, LIST_LIMIT)?,
    })
}

fn debt_panel(conn: &Connection, today: NaiveDate) -> AppResult<DebtPanel> {
    let t = ymd(today);
    let totals = repo::debt_totals(conn, &t, &ymd(plus_days(today, DEBT_SOON_DAYS)))?;
    Ok(DebtPanel {
        outstanding: totals.outstanding,
        overdue: totals.overdue,
        due_soon: totals.due_soon,
        upcoming: repo::debts_upcoming(conn, &t, LIST_LIMIT)?,
    })
}
