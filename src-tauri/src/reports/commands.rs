use chrono::Local;
use tauri::State;

use super::model::{
    ExpiryReport, ExpiryReportQuery, InventoryReport, ProductReport, PurchaseReport, ReportRange, SalesReport,
    SipnapQuery, SipnapReport,
};
use super::service;
use crate::auth::Permission;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Penjualan per periode. `REPORT_SALES` = semua kasir; `REPORT_SALES_OWN_SHIFT` = nota sendiri.
#[tauri::command]
pub async fn report_sales(state: State<'_, AppState>, query: ReportRange) -> AppResult<SalesReport> {
    let user = state.current_user()?;
    let p = &user.permissions;
    if !p.contains(&Permission::ReportSales) && !p.contains(&Permission::ReportSalesOwnShift) {
        return Err(AppError::Forbidden);
    }
    service::sales(&*state.db()?, &user, &query)
}

#[tauri::command]
pub async fn report_products(state: State<'_, AppState>, query: ReportRange) -> AppResult<ProductReport> {
    let user = state.require(Permission::ReportSales)?;
    service::products(&*state.db()?, &user, &query)
}

#[tauri::command]
pub async fn report_inventory(state: State<'_, AppState>) -> AppResult<InventoryReport> {
    state.require(Permission::ViewCost)?;
    service::inventory(&*state.db()?, Local::now().date_naive())
}

#[tauri::command]
pub async fn report_expiry(state: State<'_, AppState>, query: ExpiryReportQuery) -> AppResult<ExpiryReport> {
    let user = state.require(Permission::StockCountInput)?;
    service::expiry(&*state.db()?, &user, &query, Local::now().date_naive())
}

#[tauri::command]
pub async fn report_purchases(state: State<'_, AppState>, query: ReportRange) -> AppResult<PurchaseReport> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::purchases(&*state.db()?, &user, &query)
}

#[tauri::command]
pub async fn report_sipnap(state: State<'_, AppState>, query: SipnapQuery) -> AppResult<SipnapReport> {
    state.require(Permission::ReportSipnap)?;
    service::sipnap(&*state.db()?, &query)
}
