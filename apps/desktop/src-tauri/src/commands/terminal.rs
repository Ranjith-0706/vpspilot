//! Terminal commands. `open_terminal` takes a saved server's id only: the frontend can never
//! supply a host, port, username or secret. Sessions are addressed by an unguessable id.

use crate::error::AppError;
use crate::ssh::service;
use crate::ssh::terminal::{self, TerminalEvent, TerminalHandle};
use crate::ssh::terminal_registry::{self, OWNER_PROBE_INTERVAL};
use crate::state::AppState;
use tauri::State;
use tauri::ipc::Channel;
use tokio::sync::mpsc;

pub const MAX_SESSIONS: usize = 8;

fn session(state: &AppState, session_id: &str) -> Result<TerminalHandle, AppError> {
    terminal_registry::handle_of(&state.terminals, session_id)
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
    let (tx, rx) = mpsc::channel::<TerminalEvent>(64);

    // The place in the registry is reserved first (atomically with the session cap) and the
    // session is registered last, only if the server was not revoked in between.
    let (session_id, handle) =
        terminal_registry::open_session(&state.terminals, &id, MAX_SESSIONS, || async {
            // Load everything first; the database guard must not be held across the network await.
            let params = {
                let conn = state.conn()?;
                service::prepare_connection(&conn, state.credentials.as_ref(), &id)?
            };
            terminal::start(params, cols, rows, tx)
                .await
                .map_err(AppError::connection)
        })
        .await?;

    // Forward events to the frontend; when the session ends, forget it. An idle session whose
    // frontend has gone away (reloaded or destroyed webview) is closed by the periodic probe.
    let sessions = state.terminals.clone();
    let sid = session_id.clone();
    tokio::spawn(async move {
        terminal_registry::forward_events(
            rx,
            handle,
            move |event| on_event.send(event).is_ok(),
            OWNER_PROBE_INTERVAL,
        )
        .await;
        terminal_registry::forget(&sessions, &sid);
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
