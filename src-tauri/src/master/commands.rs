use tauri::State;

use super::model::{
    Category, CategoryInput, ProductDetail, ProductInput, ProductListQuery, ProductListResult,
    ProductPricesInput, ProductSaveResult, Unit,
};
use super::service::{self, PriceAccess};
use crate::auth::Permission;
use crate::error::AppResult;
use crate::state::AppState;

// Membaca master (daftar obat, kategori, satuan) cukup login: kasir juga butuh untuk mencari obat.

#[tauri::command]
pub async fn category_list(state: State<'_, AppState>) -> AppResult<Vec<Category>> {
    let user = state.current_user()?;
    service::list_categories(&*state.db()?, PriceAccess::of(&user))
}

#[tauri::command]
pub async fn category_save(state: State<'_, AppState>, input: CategoryInput) -> AppResult<Category> {
    let user = state.require(Permission::ProductManage)?;
    service::save_category(&mut *state.db()?, &user, &input)
}

#[tauri::command]
pub async fn unit_list(state: State<'_, AppState>) -> AppResult<Vec<Unit>> {
    state.current_user()?;
    service::list_units(&*state.db()?)
}

#[tauri::command]
pub async fn unit_create(state: State<'_, AppState>, name: String) -> AppResult<Unit> {
    state.require(Permission::ProductManage)?;
    service::create_unit(&*state.db()?, &name)
}

#[tauri::command]
pub async fn product_list(state: State<'_, AppState>, query: ProductListQuery) -> AppResult<ProductListResult> {
    state.current_user()?;
    service::list_products(&*state.db()?, &query)
}

#[tauri::command]
pub async fn product_get(state: State<'_, AppState>, id: i64) -> AppResult<ProductDetail> {
    let user = state.current_user()?;
    service::get_product(&*state.db()?, id, PriceAccess::of(&user))
}

#[tauri::command]
pub async fn product_save(state: State<'_, AppState>, input: ProductInput) -> AppResult<ProductSaveResult> {
    let user = state.require(Permission::ProductManage)?;
    service::save_product(&mut *state.db()?, &user, &input)
}

#[tauri::command]
pub async fn product_prices_save(state: State<'_, AppState>, input: ProductPricesInput) -> AppResult<ProductSaveResult> {
    let user = state.require(Permission::PriceManage)?;
    service::save_prices(&mut *state.db()?, &user, &input)
}

#[tauri::command]
pub async fn product_set_active(state: State<'_, AppState>, id: i64, active: bool) -> AppResult<()> {
    let user = state.require(Permission::ProductManage)?;
    service::set_product_active(&*state.db()?, &user, id, active)
}
