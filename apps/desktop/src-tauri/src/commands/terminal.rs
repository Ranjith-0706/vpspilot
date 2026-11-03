//! Terminal commands. `open_terminal` takes a saved server's id only: the frontend can never
//! supply a host, port, username or secret. Sessions are addressed by an unguessable id.

use crate::error::AppError;
use crate::ssh::service;
use crate::ssh::terminal::{self, TerminalEvent, TerminalHandle};
use crate::state::AppState;
use tauri::State;
use tauri::ipc::Channel;
use tokio::sync::mpsc;

pub const MAX_SESSIONS: usize = 8;

fn session(state: &AppState, session_id: &str) -> Result<TerminalHandle, AppError> {
    state
        .terminals()?
        .get(session_id)
        .cloned()
        .ok_or_else(|| AppError::not_found("Terminal session"))
}

#[tauri::command]
pub async fn open_terminal(
    state: State<'_, AppState>,
    id: String,
    cols: u32,
    rows: u32,
    on_event: Channel<TerminalEvent>,
) -> Result<String, AppError> {
    let (cols, rows) = terminal::validate_size(cols, rows)?;
    if state.terminals()?.len() >= MAX_SESSIONS {
        return Err(AppError::connection(format!(
            "At most {MAX_SESSIONS} terminals can be open at once."
        )));
    }

    // Load everything first; the database guard must not be held across the network await.
    let params = {
        let conn = state.conn()?;
        service::prepare_connection(&conn, state.credentials.as_ref(), &id)?
    };

    let (tx, mut rx) = mpsc::channel::<TerminalEvent>(64);
    let handle = terminal::start(params, cols, rows, tx)
        .await
        .map_err(AppError::connection)?;

    let session_id = uuid::Uuid::new_v4().to_string();
    state
        .terminals()?
        .insert(session_id.clone(), handle.clone());

    // Forward events to the frontend; when the session ends, forget it.
    let sessions = state.terminal_sessions();
    let sid = session_id.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if on_event.send(event).is_err() {
                handle.close(); // the frontend is gone; end the remote shell
            }
        }
        if let Ok(mut map) = sessions.lock() {
            map.remove(&sid);
        }
    });

    Ok(session_id)
}

/// Sends keyboard input (as typed in the terminal) to the remote shell.
#[tauri::command]
pub fn terminal_write(
    state: State<'_, AppState>,
    session_id: String,
    data: String,
) -> Result<(), AppError> {
    session(&state, &session_id)?.write(data.into_bytes())
}

#[tauri::command]
pub fn terminal_resize(
    state: State<'_, AppState>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), AppError> {
    session(&state, &session_id)?.resize(cols, rows)
}

#[tauri::command]
pub fn close_terminal(state: State<'_, AppState>, session_id: String) -> Result<(), AppError> {
    // Closing an unknown or already-ended session is not an error.
    if let Ok(handle) = session(&state, &session_id) {
        handle.close();
    }
    Ok(())
}
