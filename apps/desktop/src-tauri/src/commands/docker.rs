//! Docker commands. `list_docker_containers` takes a saved server's id only; the command that
//! runs on the server is a compile-time constant (see `ssh::docker::parse`) and takes no
//! container name, so the frontend cannot influence what runs.

use crate::error::AppError;
use crate::ssh::docker::logs::{self, LogsRequest, LogsResult};
use crate::ssh::docker::{self, DockerResult};
use crate::ssh::service;
use crate::state::AppState;
use tauri::State;

const BUSY: &str = "Docker is already being queried for this server. Try again in a moment.";

/// Recent logs of one container. `container_id` and `tail` are validated before anything else
/// happens (see `LogsRequest`): a container id is 12 to 64 lowercase hex characters and `tail` is
/// 1 to 1000, so neither can change the command that runs.
#[tauri::command]
pub async fn get_container_logs(
    state: State<'_, AppState>,
    id: String,
    container_id: String,
    tail: u32,
) -> Result<LogsResult, AppError> {
    // Reject bad input before touching the database or the network.
    let request = LogsRequest::new(&container_id, tail)
        .map_err(|e| AppError::validation(e.field, e.message))?;

    let params = {
        let conn = state.conn()?;
        service::prepare_connection(&conn, state.credentials.as_ref(), &id)?
    };

    let Some(_guard) = state.begin_docker(&id) else {
        return Err(AppError::connection(BUSY));
    };

    logs::fetch(params, &request)
        .await
        .map_err(AppError::connection)
}

#[tauri::command]
pub async fn list_docker_containers(
    state: State<'_, AppState>,
    id: String,
) -> Result<DockerResult, AppError> {
    // Load everything first; the database guard must not be held across the network await.
    let params = {
        let conn = state.conn()?;
        service::prepare_connection(&conn, state.credentials.as_ref(), &id)?
    };

    // One listing per server at a time, so a double click or a stuck request cannot pile up
    // logins on the server.
    let Some(_guard) = state.begin_docker(&id) else {
        return Err(AppError::connection(BUSY));
    };

    docker::list_containers(params)
        .await
        .map_err(AppError::connection)
}
