use tauri::State;

use super::model::{
    PosPrescription, PosProduct, PosState, PrescriptionQuote, SaleDetail, SaleInput, SalePage, SaleQuery,
    SaleVoidInput, ShiftCloseInput, ShiftOpenInput, ShiftSummary,
};
use super::service::{self, Clock};
use crate::auth::Permission;
use crate::error::AppResult;
use crate::state::AppState;

// Urutan kunci: `state.require(..)` (sesi & license, langsung dilepas) SELALU sebelum `state.db()`.
// Guard DB dipegang selama satu pemanggilan service = satu transaksi. Lihat `sales/mod.rs`.

#[tauri::command]
pub async fn pos_state(state: State<'_, AppState>) -> AppResult<PosState> {
    let user = state.require(Permission::SaleCreate)?;
    service::pos_state(&*state.db()?, &user)
}

#[tauri::command]
pub async fn shift_open(state: State<'_, AppState>, input: ShiftOpenInput) -> AppResult<ShiftSummary> {
    let user = state.require(Permission::ShiftManage)?;
    service::open_shift(&mut *state.db()?, &user, &input, &Clock::now())
}

#[tauri::command]
pub async fn shift_close(state: State<'_, AppState>, input: ShiftCloseInput) -> AppResult<ShiftSummary> {
    let user = state.require(Permission::ShiftManage)?;
    service::close_shift(&mut *state.db()?, &user, &input, &Clock::now())
}

#[tauri::command]
pub async fn pos_search(state: State<'_, AppState>, q: String) -> AppResult<Vec<PosProduct>> {
    state.require(Permission::SaleCreate)?;
    service::search(&*state.db()?, &q, &Clock::now())
}

#[tauri::command]
pub async fn sale_create(state: State<'_, AppState>, input: SaleInput) -> AppResult<SaleDetail> {
    let user = state.require(Permission::SaleCreate)?;
    service::create_sale(&mut *state.db()?, &user, &input, &Clock::now())
}

#[tauri::command]
pub async fn sale_get(state: State<'_, AppState>, id: i64) -> AppResult<SaleDetail> {
    let user = state.require(Permission::SaleCreate)?;
    service::get_sale(&*state.db()?, &user, id)
}

#[tauri::command]
pub async fn sale_page(state: State<'_, AppState>, query: SaleQuery) -> AppResult<SalePage> {
    let user = state.require(Permission::SaleCreate)?;
    service::page_sales(&*state.db()?, &user, &query)
}

#[tauri::command]
pub async fn sale_void(state: State<'_, AppState>, input: SaleVoidInput) -> AppResult<SaleDetail> {
    let user = state.require(Permission::SaleCreate)?;
    service::void_sale(&mut *state.db()?, &user, &input, &Clock::now())
}

#[tauri::command]
pub async fn pos_prescriptions(state: State<'_, AppState>) -> AppResult<Vec<PosPrescription>> {
    state.require(Permission::SaleCreate)?;
    service::ready_prescriptions(&*state.db()?)
}

#[tauri::command]
pub async fn pos_prescription_quote(state: State<'_, AppState>, id: i64) -> AppResult<PrescriptionQuote> {
    state.require(Permission::SaleCreate)?;
    service::prescription_quote(&*state.db()?, id, &Clock::now())
}
