mod audit;
mod auth;
mod db;
mod error;
mod license;
mod master;
mod settings;
mod state;

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
            audit::commands::audit_page,
            audit::commands::audit_users,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
