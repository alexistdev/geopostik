use chrono::{Local, NaiveDate};
use tauri::State;

use super::model::{
    DebtPage, DebtQuery, PurchaseDefaults, PurchaseDetail, PurchaseInput, PurchasePage, PurchasePostInput,
    PurchaseProduct, PurchaseQuery, PurchaseSaveResult, Supplier, SupplierInput, SupplierPage, SupplierPaymentInput,
};
use super::service;
use crate::auth::Permission;
use crate::error::AppResult;
use crate::master::MasterPageQuery;
use crate::state::AppState;

fn today() -> NaiveDate {
    Local::now().date_naive()
}

// Supplier dikelola oleh yang berhak menerima barang (pemilik, apoteker, TTK), dari menu
// Master Data maupun langsung dari form faktur.

#[tauri::command]
pub async fn supplier_list(state: State<'_, AppState>) -> AppResult<Vec<Supplier>> {
    state.current_user()?;
    service::list_suppliers(&*state.db()?)
}

#[tauri::command]
pub async fn supplier_page(state: State<'_, AppState>, query: MasterPageQuery) -> AppResult<SupplierPage> {
    state.current_user()?;
    service::page_suppliers(&*state.db()?, &query)
}

#[tauri::command]
pub async fn supplier_save(state: State<'_, AppState>, input: SupplierInput) -> AppResult<Supplier> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::save_supplier(&*state.db()?, &user, &input)
}

#[tauri::command]
pub async fn supplier_set_active(state: State<'_, AppState>, id: i64, active: bool) -> AppResult<()> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::set_supplier_active(&*state.db()?, &user, id, active)
}

#[tauri::command]
pub async fn supplier_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::delete_supplier(&mut *state.db()?, &user, id)
}

// Faktur: HPP hasil hitung dan HPP acuan hanya dikirim untuk VIEW_COST; sisa hutang dan
// pembayaran hanya untuk SUPPLIER_DEBT_MANAGE.

#[tauri::command]
pub async fn purchase_defaults(state: State<'_, AppState>) -> AppResult<PurchaseDefaults> {
    state.require(Permission::PurchaseReceive)?;
    service::defaults(&*state.db()?, today())
}

#[tauri::command]
pub async fn purchase_page(state: State<'_, AppState>, query: PurchaseQuery) -> AppResult<PurchasePage> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::page_purchases(&*state.db()?, &user, &query)
}

#[tauri::command]
pub async fn purchase_get(state: State<'_, AppState>, id: i64) -> AppResult<PurchaseDetail> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::get_purchase(&*state.db()?, &user, id)
}

#[tauri::command]
pub async fn purchase_product_search(state: State<'_, AppState>, q: String) -> AppResult<Vec<PurchaseProduct>> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::search_products(&*state.db()?, &user, &q)
}

#[tauri::command]
pub async fn purchase_save(state: State<'_, AppState>, input: PurchaseInput) -> AppResult<PurchaseSaveResult> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::save_purchase(&mut *state.db()?, &user, &input, today())
}

#[tauri::command]
pub async fn purchase_post(state: State<'_, AppState>, input: PurchasePostInput) -> AppResult<PurchaseSaveResult> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::post_purchase(&mut *state.db()?, &user, &input)
}

/// Draft: hak `PURCHASE_RECEIVE`. Faktur yang sudah diposting: juga `TRANSACTION_VOID` (dicek di service).
#[tauri::command]
pub async fn purchase_void(state: State<'_, AppState>, id: i64, reason: Option<String>) -> AppResult<PurchaseDetail> {
    let user = state.require(Permission::PurchaseReceive)?;
    service::void_purchase(&mut *state.db()?, &user, id, reason.as_deref())
}

// Hutang supplier: hanya pemilik (SUPPLIER_DEBT_MANAGE).

#[tauri::command]
pub async fn debt_page(state: State<'_, AppState>, query: DebtQuery) -> AppResult<DebtPage> {
    state.require(Permission::SupplierDebtManage)?;
    service::page_debts(&*state.db()?, &query, today())
}

#[tauri::command]
pub async fn supplier_payment_create(state: State<'_, AppState>, input: SupplierPaymentInput) -> AppResult<PurchaseDetail> {
    let user = state.require(Permission::SupplierDebtManage)?;
    service::create_payment(&mut *state.db()?, &user, &input, today())
}

#[tauri::command]
pub async fn supplier_payment_void(state: State<'_, AppState>, id: i64, reason: String) -> AppResult<PurchaseDetail> {
    let user = state.require(Permission::SupplierDebtManage)?;
    service::void_payment(&mut *state.db()?, &user, id, &reason)
}
