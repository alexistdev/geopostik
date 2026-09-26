use chrono::{Datelike, Days, NaiveDate};
use rusqlite::Connection;

use super::model::{
    ExpiryReport, ExpiryReportQuery, InventoryReport, ProductReport, PurchaseReport, ReportDay, ReportProductRow,
    ReportRange, ReportShift, SalesReport, SipnapQuery, SipnapReport,
};
use super::repo;
use crate::auth::{Permission, SessionUser};
use crate::error::{AppError, AppResult};

/// Rentang laporan terpanjang (hari), agar tabel harian tetap wajar.
const MAX_RANGE_DAYS: i64 = 366;
/// Baris terbanyak di daftar obat terlaris & slow moving.
const PRODUCT_LIMIT: i64 = 100;

fn can(user: &SessionUser, p: Permission) -> bool {
    user.permissions.contains(&p)
}

fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn parse_date(s: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("Tanggal tidak valid: {s}")))
}

/// Rentang inklusif yang sudah divalidasi, plus batas atas eksklusif untuk SQL.
struct Range {
    from: NaiveDate,
    to: NaiveDate,
    to_excl: String,
}

fn range(r: &ReportRange) -> AppResult<Range> {
    let (from, to) = (parse_date(&r.from)?, parse_date(&r.to)?);
    if to < from {
        return Err(AppError::Validation("Tanggal akhir tidak boleh sebelum tanggal awal".into()));
    }
    if (to - from).num_days() >= MAX_RANGE_DAYS {
        return Err(AppError::Validation("Rentang laporan paling lama 1 tahun".into()));
    }
    let next = to.checked_add_days(Days::new(1)).ok_or_else(|| AppError::Validation("Tanggal tidak valid".into()))?;
    Ok(Range { from, to, to_excl: ymd(next) })
}

// ─── Penjualan ───────────────────────────────────────────────────────────────

/// Laporan penjualan. Tanpa `REPORT_SALES` (hanya `REPORT_SALES_OWN_SHIFT`) semua angka dibatasi
/// pada nota user sendiri, dan angka kas hanya untuk shift miliknya.
pub fn sales(conn: &Connection, user: &SessionUser, query: &ReportRange) -> AppResult<SalesReport> {
    let r = range(query)?;
    let (from, to) = (ymd(r.from), r.to_excl.as_str());
    let all = can(user, Permission::ReportSales);
    let cashier = (!all).then_some(user.id);
    let view_cost = can(user, Permission::ViewCost);

    let mut summary = repo::sales_summary(conn, &from, to, cashier)?;
    if view_cost {
        let cost = repo::sales_cost(conn, &from, to, cashier)?;
        summary.cost = Some(cost);
        summary.gross_profit = Some(summary.net - summary.tax - cost);
    }

    // Lengkapi setiap tanggal dalam rentang agar tabel & grafik tidak bolong.
    let found = repo::sales_daily(conn, &from, to, cashier)?;
    let costs = if view_cost { repo::cost_daily(conn, &from, to, cashier)? } else { Vec::new() };
    let daily = r
        .from
        .iter_days()
        .take_while(|d| *d <= r.to)
        .map(|d| {
            let date = ymd(d);
            let row = found.iter().find(|f| f.date == date);
            let cost = costs.iter().find(|(d, _)| *d == date).map_or(0, |(_, c)| *c);
            ReportDay {
                count: row.map_or(0, |f| f.count),
                amount: row.map_or(0, |f| f.amount),
                gross_profit: view_cost.then(|| row.map_or(0, |f| f.net_of_tax) - cost),
                date,
            }
        })
        .collect();

    let shifts = repo::shifts(conn, &from, to, cashier)?
        .into_iter()
        .map(|s| {
            let show = all || s.user_id == user.id;
            // Shift terbuka belum punya angka tutup; hitung tunai seharusnya seperti di Dashboard.
            let expected = match s.expected_cash {
                Some(v) => Some(v),
                None if show => Some(s.opening_cash + repo::shift_cash_in(conn, s.id)?),
                None => None,
            };
            Ok(ReportShift {
                id: s.id,
                opened_by: s.opened_by,
                opened_at: s.opened_at,
                closed_at: s.closed_at,
                status: s.status,
                count: s.count,
                amount: s.amount,
                opening_cash: show.then_some(s.opening_cash),
                expected_cash: if show { expected } else { None },
                counted_cash: if show { s.counted_cash } else { None },
                difference: if show { s.difference } else { None },
            })
        })
        .collect::<AppResult<Vec<_>>>()?;

    Ok(SalesReport {
        from,
        to: ymd(r.to),
        own_only: !all,
        summary,
        daily,
        by_method: repo::by_method(conn, &ymd(r.from), to, cashier)?,
        by_cashier: repo::by_cashier(conn, &ymd(r.from), to, cashier)?,
        shifts,
    })
}

// ─── Obat terlaris & slow moving ─────────────────────────────────────────────

pub fn products(conn: &Connection, user: &SessionUser, query: &ReportRange) -> AppResult<ProductReport> {
    let r = range(query)?;
    let from = ymd(r.from);
    let view_cost = can(user, Permission::ViewCost);
    let top = repo::product_sales(conn, &from, &r.to_excl, PRODUCT_LIMIT)?
        .into_iter()
        .map(|p| ReportProductRow {
            cost: view_cost.then_some(p.cost),
            gross_profit: view_cost.then_some(p.amount - p.cost),
            product_id: p.product_id,
            code: p.code,
            name: p.name,
            qty_base: p.qty_base,
            base_unit_name: p.base_unit_name,
            sale_count: p.sale_count,
            amount: p.amount,
        })
        .collect();
    let slow = repo::slow_moving(conn, &from, &r.to_excl, PRODUCT_LIMIT)?
        .into_iter()
        .map(|mut s| {
            if !view_cost {
                s.value = None;
            }
            s
        })
        .collect();
    Ok(ProductReport { from, to: ymd(r.to), top, slow })
}

// ─── Nilai persediaan ────────────────────────────────────────────────────────

pub fn inventory(conn: &Connection, today: NaiveDate) -> AppResult<InventoryReport> {
    let totals = repo::inventory_totals(conn, &ymd(today))?;
    let rows = repo::inventory_rows(conn)?;
    Ok(InventoryReport {
        total_value: totals.value,
        product_count: rows.len() as i64,
        batch_count: totals.batch_count,
        expired_value: totals.expired_value,
        by_category: repo::inventory_by_category(conn)?,
        rows,
    })
}

// ─── ED ──────────────────────────────────────────────────────────────────────

pub fn expiry(conn: &Connection, user: &SessionUser, query: &ExpiryReportQuery, today: NaiveDate) -> AppResult<ExpiryReport> {
    if !(1..=730).contains(&query.days) {
        return Err(AppError::Validation("Batas hari ED harus 1–730".into()));
    }
    let until = today.checked_add_days(Days::new(query.days as u64)).unwrap_or(today);
    let view_cost = can(user, Permission::ViewCost);
    let mut rows = repo::expiry_rows(conn, &ymd(today), &ymd(until))?;

    let (mut expired_count, mut near_count, mut expired_value, mut near_value) = (0, 0, 0, 0);
    for row in &mut rows {
        let v = row.value.unwrap_or(0);
        if row.days_left <= 0 {
            expired_count += 1;
            expired_value += v;
        } else {
            near_count += 1;
            near_value += v;
        }
        if !view_cost {
            row.value = None;
        }
    }
    Ok(ExpiryReport {
        today: ymd(today),
        expired_count,
        near_count,
        expired_value: view_cost.then_some(expired_value),
        near_value: view_cost.then_some(near_value),
        rows,
    })
}

// ─── Pembelian ───────────────────────────────────────────────────────────────

pub fn purchases(conn: &Connection, user: &SessionUser, query: &ReportRange) -> AppResult<PurchaseReport> {
    let r = range(query)?;
    let from = ymd(r.from);
    let shows_debt = can(user, Permission::SupplierDebtManage);
    let t = repo::purchase_totals(conn, &from, &r.to_excl)?;
    Ok(PurchaseReport {
        by_supplier: repo::purchase_by_supplier(conn, &from, &r.to_excl)?,
        invoices: repo::purchase_invoices(conn, &from, &r.to_excl, shows_debt)?,
        from,
        to: ymd(r.to),
        count: t.count,
        subtotal: t.subtotal,
        discount: t.discount,
        tax: t.tax,
        total: t.total,
        cash_total: t.cash_total,
        credit_total: t.credit_total,
        shows_debt,
    })
}

// ─── SIPNAP ──────────────────────────────────────────────────────────────────

pub fn sipnap(conn: &Connection, query: &SipnapQuery) -> AppResult<SipnapReport> {
    let invalid = || AppError::Validation("Bulan laporan tidak valid".into());
    let start = NaiveDate::from_ymd_opt(query.year, query.month, 1).ok_or_else(invalid)?;
    let next = if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
    }
    .ok_or_else(invalid)?;
    Ok(SipnapReport {
        year: query.year,
        month: query.month,
        rows: repo::sipnap(conn, &ymd(start), &ymd(next))?,
    })
}
