mod audit;
mod auth;
mod dashboard;
mod db;
mod error;
mod inventory;
mod license;
mod master;
mod prescription;
mod purchasing;
mod reports;
mod sales;
mod settings;
mod state;
mod users;

use std::path::PathBuf;

use tauri::Manager;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

use crate::license::License;
use crate::state::AppState;

/// Folder data: `%APPDATA%\GeoPOSTik` (Windows). Build debug memakai folder terpisah
/// agar data pengembangan tidak tercampur dengan data produksi.
fn data_dir(app: &tauri::App) -> tauri::Result<PathBuf> {
    let name = if cfg!(debug_assertions) { "GeoPOSTik-dev" } else { "GeoPOSTik" };
    Ok(app.path().data_dir()?.join(name))
}

fn init_logging(dir: &std::path::Path) -> WorkerGuard {
    let appender = tracing_appender::rolling::daily(dir.join("logs"), "geopostik.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_writer(writer)
        .with_ansi(false)
        .init();
    guard
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Hanya satu jendela aplikasi yang boleh berjalan; instance kedua memfokuskan yang pertama.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = data_dir(app)?;
            std::fs::create_dir_all(dir.join("backups"))?;
            let guard = init_logging(&dir);
            app.manage(guard);

            let db_path = dir.join("geopostik.db");
            tracing::info!(path = %db_path.display(), "membuka database");
            let conn = db::open(&db_path).map_err(|e| {
                tracing::error!(error = %e, "gagal membuka database");
                e.to_string()
            })?;
            let license = License::load(dir.join("license.json"));
            app.manage(AppState::new(conn, license));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::commands::app_status,
            auth::commands::setup_owner,
            auth::commands::login,
            auth::commands::logout,
            license::commands::license_status,
            license::commands::license_activate,
            license::commands::license_revalidate,
            master::commands::category_list,
            master::commands::category_page,
            master::commands::category_save,
            master::commands::rack_list,
            master::commands::rack_page,
            master::commands::rack_save,
            master::commands::manufacturer_list,
            master::commands::manufacturer_page,
            master::commands::manufacturer_save,
            master::commands::master_set_active,
            master::commands::master_delete,
            master::commands::unit_list,
            master::commands::unit_create,
            master::commands::product_list,
            master::commands::product_get,
            master::commands::product_save,
            master::commands::product_prices_save,
            master::commands::product_set_active,
            master::commands::product_delete,
            master::commands::product_set_active_many,
            master::commands::product_delete_many,
            dashboard::commands::dashboard_get,
            reports::commands::report_sales,
            reports::commands::report_products,
            reports::commands::report_inventory,
            reports::commands::report_expiry,
            reports::commands::report_purchases,
            reports::commands::report_sipnap,
            audit::commands::audit_page,
            audit::commands::audit_users,
            prescription::commands::doctor_list,
            prescription::commands::doctor_page,
            prescription::commands::doctor_save,
            prescription::commands::doctor_set_active,
            prescription::commands::doctor_delete,
            prescription::commands::patient_page,
            prescription::commands::patient_save,
            prescription::commands::patient_set_active,
            prescription::commands::patient_delete,
            prescription::commands::prescription_page,
            prescription::commands::prescription_get,
            prescription::commands::prescription_save,
            prescription::commands::prescription_screen,
            prescription::commands::prescription_cancel,
            inventory::commands::stock_list,
            inventory::commands::stock_batches,
            inventory::commands::batch_set_locked,
            inventory::commands::stock_card,
            inventory::commands::opname_page,
            inventory::commands::opname_get,
            inventory::commands::opname_create,
            inventory::commands::opname_item_save,
            inventory::commands::opname_item_delete,
            inventory::commands::opname_fill,
            inventory::commands::opname_submit,
            inventory::commands::opname_reopen,
            inventory::commands::opname_cancel,
            inventory::commands::opname_approve,
            inventory::commands::stock_opening_lock,
            purchasing::commands::supplier_list,
            purchasing::commands::supplier_page,
            purchasing::commands::supplier_save,
            purchasing::commands::supplier_set_active,
            purchasing::commands::supplier_delete,
            purchasing::commands::purchase_defaults,
            purchasing::commands::purchase_page,
            purchasing::commands::purchase_get,
            purchasing::commands::purchase_product_search,
            purchasing::commands::purchase_save,
            purchasing::commands::purchase_post,
            purchasing::commands::purchase_void,
            purchasing::commands::debt_page,
            purchasing::commands::supplier_payment_create,
            purchasing::commands::supplier_payment_void,
            settings::commands::tax_settings_get,
            settings::commands::tax_settings_save,
            sales::commands::pos_state,
            sales::commands::shift_open,
            sales::commands::shift_close,
            sales::commands::pos_search,
            sales::commands::sale_create,
            sales::commands::sale_get,
            sales::commands::sale_page,
            sales::commands::sale_void,
            sales::commands::pos_prescriptions,
            sales::commands::pos_prescription_quote,
            users::commands::user_list,
            users::commands::user_get,
            users::commands::user_save,
            users::commands::user_set_active,
            users::commands::user_delete,
            users::commands::user_set_active_many,
            users::commands::user_delete_many,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
