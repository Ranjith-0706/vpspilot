//! Connection test and host key commands. Every command takes a saved server's id only:
//! the frontend can never supply a host, port, username or secret to connect with.

use crate::error::AppError;
use crate::servers::repo::now_ms;
use crate::ssh::client::{self, ConnectionTestResult, Outcome};
use crate::ssh::hostkey::HostKeyRecord;
use crate::ssh::{known_hosts, service};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn test_server_connection(
    state: State<'_, AppState>,
    id: String,
) -> Result<ConnectionTestResult, AppError> {
    // Load everything first; the database guard must not be held across the network await.
    let params = {
        let conn = state.conn()?;
        service::prepare_connection(&conn, state.credentials.as_ref(), &id)?
    };

    let result = client::run_test(params).await;

    let mut pending = state.pending()?;
    match (&result.outcome, &result.host_key) {
        (Outcome::HostKeyUnknown, Some(key)) => {
            pending.insert(id, key.clone());
        }
        _ => {
            pending.remove(&id);
        }
    }
    Ok(result)
}

/// Trusts the host key the server presented on its last test. The caller must echo the
/// fingerprint it displayed to the user; it is checked against what the backend saw.
#[tauri::command]
pub fn trust_host_key(
    state: State<'_, AppState>,
    id: String,
    fingerprint: String,
) -> Result<HostKeyRecord, AppError> {
    let conn = state.conn()?;
    let mut pending = state.pending()?;
    service::trust_pending(&conn, &mut pending, &id, &fingerprint, now_ms())
}

#[tauri::command]
pub fn forget_host_key(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let conn = state.conn()?;
    let mut pending = state.pending()?;
    service::forget(&conn, &mut pending, &id)
}

#[tauri::command]
pub fn list_host_keys(state: State<'_, AppState>) -> Result<Vec<HostKeyRecord>, AppError> {
    let conn = state.conn()?;
    known_hosts::list(&conn)
}
