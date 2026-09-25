use tauri::State;

use super::model::{
    Category, CategoryInput, CategoryPage, MasterKind, MasterPageQuery, NamedItem, NamedItemInput, NamedItemPage, ProductDetail, ProductInput, ProductListQuery, ProductListResult,
    ProductPricesInput, ProductSaveResult, Unit,
};
use super::repo::NamedTable;
use super::service::{self, PriceAccess};
use crate::auth::Permission;
use crate::error::AppResult;
use crate::state::AppState;

// Membaca master (daftar obat, kategori, satuan) cukup login: kasir juga butuh untuk mencari obat.
// `*_list` mengembalikan semua data (untuk dropdown), `*_page` per halaman (untuk menu Master Data).

#[tauri::command]
pub async fn category_list(state: State<'_, AppState>) -> AppResult<Vec<Category>> {
    let user = state.current_user()?;
    service::list_categories(&*state.db()?, PriceAccess::of(&user))
}

#[tauri::command]
pub async fn category_page(state: State<'_, AppState>, query: MasterPageQuery) -> AppResult<CategoryPage> {
    let user = state.current_user()?;
    service::page_categories(&*state.db()?, PriceAccess::of(&user), &query)
}

#[tauri::command]
pub async fn category_save(state: State<'_, AppState>, input: CategoryInput) -> AppResult<Category> {
    let user = state.require(Permission::ProductManage)?;
    service::save_category(&mut *state.db()?, &user, &input)
}

#[tauri::command]
pub async fn rack_list(state: State<'_, AppState>) -> AppResult<Vec<NamedItem>> {
    state.current_user()?;
    service::list_named(&*state.db()?, NamedTable::Racks)
}

#[tauri::command]
pub async fn rack_page(state: State<'_, AppState>, query: MasterPageQuery) -> AppResult<NamedItemPage> {
    state.current_user()?;
    service::page_named(&*state.db()?, NamedTable::Racks, &query)
}

#[tauri::command]
pub async fn rack_save(state: State<'_, AppState>, input: NamedItemInput) -> AppResult<NamedItem> {
    let user = state.require(Permission::ProductManage)?;
    service::save_named(&*state.db()?, &user, NamedTable::Racks, &input)
}

#[tauri::command]
pub async fn manufacturer_list(state: State<'_, AppState>) -> AppResult<Vec<NamedItem>> {
    state.current_user()?;
    service::list_named(&*state.db()?, NamedTable::Manufacturers)
}

#[tauri::command]
pub async fn manufacturer_page(state: State<'_, AppState>, query: MasterPageQuery) -> AppResult<NamedItemPage> {
    state.current_user()?;
    service::page_named(&*state.db()?, NamedTable::Manufacturers, &query)
}

#[tauri::command]
pub async fn manufacturer_save(state: State<'_, AppState>, input: NamedItemInput) -> AppResult<NamedItem> {
    let user = state.require(Permission::ProductManage)?;
    service::save_named(&*state.db()?, &user, NamedTable::Manufacturers, &input)
}

#[tauri::command]
pub async fn master_set_active(state: State<'_, AppState>, kind: MasterKind, id: i64, active: bool) -> AppResult<()> {
    let user = state.require(Permission::ProductManage)?;
    service::set_master_active(&*state.db()?, &user, kind, id, active)
}

#[tauri::command]
pub async fn master_delete(state: State<'_, AppState>, kind: MasterKind, id: i64) -> AppResult<()> {
    let user = state.require(Permission::ProductManage)?;
    service::delete_master(&mut *state.db()?, &user, kind, id)
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
