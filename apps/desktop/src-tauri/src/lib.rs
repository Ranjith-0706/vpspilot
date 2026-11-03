//! VPSPilot native layer.
//!
//! Only narrowly-scoped, typed commands are exposed to the frontend.
//! No generic shell execution is, or must ever be, exposed.

mod commands;
mod credentials;
mod db;
mod error;
mod servers;
mod ssh;
mod state;

use std::sync::Mutex;
use tauri::Manager;

const DB_FILE: &str = "vpspilot.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let db = app
                .path()
                .app_data_dir()
                .map_err(|e| {
                    error::AppError::database(format!("Cannot resolve app data directory: {e}"))
                })
                .and_then(|dir| {
                    std::fs::create_dir_all(&dir).map_err(|e| {
                        error::AppError::database(format!("Cannot create app data directory: {e}"))
                    })?;
                    db::open(&dir.join(DB_FILE))
                })
                .map(Mutex::new);
            if let Err(e) = &db {
                eprintln!("database initialization failed: {e}");
            }
            app.manage(state::AppState {
                db,
                credentials: Box::new(credentials::OsCredentialStore),
                pending_host_keys: Mutex::new(Default::default()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info::get_app_info,
            commands::servers::list_servers,
            commands::servers::add_server,
            commands::servers::update_server,
            commands::servers::delete_server,
            commands::servers::set_server_credential,
            commands::servers::clear_server_credential,
            commands::ssh::test_server_connection,
            commands::ssh::trust_host_key,
            commands::ssh::forget_host_key,
            commands::ssh::list_host_keys,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VPSPilot");
}
