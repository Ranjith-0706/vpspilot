//! Thin Tauri command wrappers. Logic lives in `servers::service`.
//! No command returns secret material.

use crate::error::AppError;
use crate::servers::model::{ServerInput, ServerProfile};
use crate::servers::{repo::now_ms, service};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn list_servers(state: State<'_, AppState>) -> Result<Vec<ServerProfile>, AppError> {
    let conn = state.conn()?;
    service::list_servers(&conn)
}

#[tauri::command]
pub fn add_server(
    state: State<'_, AppState>,
    input: ServerInput,
) -> Result<ServerProfile, AppError> {
    let conn = state.conn()?;
    service::add_server(&conn, &input, now_ms())
}

#[tauri::command]
pub fn update_server(
    state: State<'_, AppState>,
    id: String,
    input: ServerInput,
) -> Result<ServerProfile, AppError> {
    let conn = state.conn()?;
    service::update_server(&conn, state.credentials.as_ref(), &id, &input, now_ms())
}

#[tauri::command]
pub fn delete_server(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let conn = state.conn()?;
    service::delete_server(&conn, state.credentials.as_ref(), &id)
}

/// Write-only: stores a secret in the OS credential store and records only its reference.
#[tauri::command]
pub fn set_server_credential(
    state: State<'_, AppState>,
    id: String,
    secret: String,
) -> Result<ServerProfile, AppError> {
    let conn = state.conn()?;
    service::set_credential(&conn, state.credentials.as_ref(), &id, &secret, now_ms())
}

#[tauri::command]
pub fn clear_server_credential(
    state: State<'_, AppState>,
    id: String,
) -> Result<ServerProfile, AppError> {
    let conn = state.conn()?;
    service::clear_credential(&conn, state.credentials.as_ref(), &id, now_ms())
}
