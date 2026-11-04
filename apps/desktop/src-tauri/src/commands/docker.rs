//! Docker commands. `list_docker_containers` takes a saved server's id only; the command that
//! runs on the server is a compile-time constant (see `ssh::docker::parse`) and takes no
//! container name, so the frontend cannot influence what runs.

use crate::error::AppError;
use crate::ssh::docker::{self, DockerResult};
use crate::ssh::service;
use crate::state::AppState;
use tauri::State;

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
        return Err(AppError::connection(
            "Docker containers are already being loaded for this server.",
        ));
    };

    docker::list_containers(params)
        .await
        .map_err(AppError::connection)
}
