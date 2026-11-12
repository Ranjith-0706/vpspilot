//! VPSPilot native layer.
//!
//! Only narrowly-scoped, typed commands are exposed to the frontend.
//! No generic shell execution is, or must ever be, exposed.

mod alerts;
mod commands;
mod credentials;
mod db;
mod error;
mod history;
mod servers;
mod ssh;
mod state;

use std::sync::Mutex;
use tauri::Manager;

const DB_FILE: &str = "vpspilot.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Terminals belong to the webview that opened them. A reload starts a new page that
        // knows nothing about them, and a destroyed window can never receive their output, so
        // end them (and any still connecting) rather than leave them running unowned.
        .on_page_load(|webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Started)
                && let Some(state) = webview.try_state::<state::AppState>()
            {
                state.close_all_terminals();
            }
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed)
                && let Some(state) = window.try_state::<state::AppState>()
            {
                state.close_all_terminals();
            }
        })
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
                terminals: Default::default(),
                monitors: Default::default(),
                alerts: Default::default(),
                docker_busy: Default::default(),
            });
            // Trim old history now and then hourly. Failures are logged and retried next time.
            tauri::async_runtime::spawn(commands::history::prune_loop(app.handle().clone()));
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
            commands::terminal::open_terminal,
            commands::terminal::terminal_write,
            commands::terminal::terminal_resize,
            commands::terminal::close_terminal,
            commands::monitor::start_monitoring,
            commands::monitor::stop_monitoring,
            commands::alerts::list_alert_rules,
            commands::alerts::add_alert_rule,
            commands::alerts::update_alert_rule,
            commands::alerts::delete_alert_rule,
            commands::alerts::list_alert_events,
            commands::alerts::clear_alert_events,
            commands::alerts::list_active_alerts,
            commands::docker::list_docker_containers,
            commands::docker::get_container_logs,
            commands::history::get_metric_history,
            commands::history::get_history_retention_days,
            commands::history::set_history_retention_days,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VPSPilot");
}
