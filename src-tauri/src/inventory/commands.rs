use chrono::{Local, NaiveDate};
use tauri::State;

use super::model::{
    OpnameCreateInput, OpnameDetail, OpnameFillInput, OpnameItemInput, OpnamePage, OpnamePageQuery, OpnameSaveResult,
    ProductBatches, StockCardPage, StockCardQuery, StockListQuery, StockPage,
};
use super::service;
use crate::auth::Permission;
use crate::error::AppResult;
use crate::state::AppState;

fn today() -> NaiveDate {
    Local::now().date_naive()
}

// Melihat stok dan kartu stok cukup login (kasir juga perlu mengecek stok); HPP dan nilai
// persediaan hanya dikirim untuk user dengan VIEW_COST.

#[tauri::command]
pub async fn stock_list(state: State<'_, AppState>, query: StockListQuery) -> AppResult<StockPage> {
    let user = state.current_user()?;
    service::list_stock(&*state.db()?, &user, &query, today())
}

#[tauri::command]
pub async fn stock_batches(state: State<'_, AppState>, product_id: i64, include_empty: bool) -> AppResult<ProductBatches> {
    let user = state.current_user()?;
    service::product_batches(&*state.db()?, &user, product_id, include_empty)
}

#[tauri::command]
pub async fn batch_set_locked(state: State<'_, AppState>, id: i64, locked: bool, reason: Option<String>) -> AppResult<()> {
    let user = state.require(Permission::StockCountApprove)?;
    service::set_batch_locked(&*state.db()?, &user, id, locked, reason.as_deref())
}

#[tauri::command]
pub async fn stock_card(state: State<'_, AppState>, query: StockCardQuery) -> AppResult<StockCardPage> {
    state.current_user()?;
    service::stock_card(&*state.db()?, &query)
}

#[tauri::command]
pub async fn opname_page(state: State<'_, AppState>, query: OpnamePageQuery) -> AppResult<OpnamePage> {
    state.require(Permission::StockCountInput)?;
    service::page_opnames(&*state.db()?, &query)
}

#[tauri::command]
pub async fn opname_get(state: State<'_, AppState>, id: i64) -> AppResult<OpnameDetail> {
    let user = state.require(Permission::StockCountInput)?;
    service::get_opname(&*state.db()?, &user, id)
}

#[tauri::command]
pub async fn opname_create(state: State<'_, AppState>, input: OpnameCreateInput) -> AppResult<OpnameDetail> {
    let user = state.require(Permission::StockCountInput)?;
    service::create_opname(&mut *state.db()?, &user, &input, today())
}

#[tauri::command]
pub async fn opname_item_save(state: State<'_, AppState>, input: OpnameItemInput) -> AppResult<OpnameSaveResult> {
    let user = state.require(Permission::StockCountInput)?;
    service::save_item(&mut *state.db()?, &user, &input, today())
}

#[tauri::command]
pub async fn opname_item_delete(state: State<'_, AppState>, opname_id: i64, item_id: i64) -> AppResult<OpnameDetail> {
    let user = state.require(Permission::StockCountInput)?;
    service::delete_item(&mut *state.db()?, &user, opname_id, item_id)
}

#[tauri::command]
pub async fn opname_fill(state: State<'_, AppState>, input: OpnameFillInput) -> AppResult<OpnameSaveResult> {
    let user = state.require(Permission::StockCountInput)?;
    service::fill_opname(&mut *state.db()?, &user, &input)
}

#[tauri::command]
pub async fn opname_submit(state: State<'_, AppState>, id: i64) -> AppResult<OpnameDetail> {
    let user = state.require(Permission::StockCountInput)?;
    service::submit_opname(&mut *state.db()?, &user, id)
}

#[tauri::command]
pub async fn opname_reopen(state: State<'_, AppState>, id: i64) -> AppResult<OpnameDetail> {
    let user = state.require(Permission::StockCountInput)?;
    service::reopen_opname(&mut *state.db()?, &user, id)
}

#[tauri::command]
pub async fn opname_cancel(state: State<'_, AppState>, id: i64, reason: Option<String>) -> AppResult<OpnameDetail> {
    let user = state.require(Permission::StockCountInput)?;
    service::cancel_opname(&mut *state.db()?, &user, id, reason.as_deref())
}

#[tauri::command]
pub async fn opname_approve(state: State<'_, AppState>, id: i64) -> AppResult<OpnameDetail> {
    let user = state.require(Permission::StockCountApprove)?;
    service::approve_opname(&mut *state.db()?, &user, id)
}

#[tauri::command]
pub async fn stock_opening_lock(state: State<'_, AppState>) -> AppResult<()> {
    let user = state.require(Permission::StockCountApprove)?;
    service::lock_opening(&mut *state.db()?, &user)
}
