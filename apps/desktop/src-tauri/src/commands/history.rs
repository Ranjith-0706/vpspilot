//! Thin Tauri command wrappers for monitoring history. Logic lives in `history::service`.
//! Nothing here connects to a server: it only reads what was stored earlier.

use crate::error::AppError;
use crate::history::model::HistoryRange;
use crate::history::service::{self, PRUNE_BATCH};
use crate::servers::repo::now_ms;
use crate::state::AppState;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

const PRUNE_EVERY: Duration = Duration::from_secs(3600);

/// Stored samples of one server for the last `range_secs`, averaged into at most `max_points`.
#[tauri::command]
pub async fn get_metric_history(
    state: State<'_, AppState>,
    server_id: String,
    range_secs: u32,
    max_points: Option<u32>,
) -> Result<HistoryRange, AppError> {
    let conn = state.conn()?;
    service::query(&conn, &server_id, range_secs, max_points, now_ms())
}

#[tauri::command]
pub async fn get_history_retention_days(state: State<'_, AppState>) -> Result<u32, AppError> {
    let conn = state.conn()?;
    service::retention_days(&conn)
}

/// Saves the retention period and starts trimming right away in the background.
#[tauri::command]
pub async fn set_history_retention_days(
    app: AppHandle,
    state: State<'_, AppState>,
    days: u32,
) -> Result<u32, AppError> {
    let saved = {
        let conn = state.conn()?;
        service::set_retention_days(&conn, days)?
    };
    tauri::async_runtime::spawn(async move { prune_now(&app).await });
    Ok(saved)
}

/// Deletes expired samples in small batches, releasing the database between batches.
async fn prune_now(app: &AppHandle) {
    loop {
        let app = app.clone();
        let batch = tokio::task::spawn_blocking(move || {
            let state = app.state::<AppState>();
            let conn = state.conn()?;
            service::prune_batch(&conn, now_ms())
        })
        .await;
        match batch {
            Ok(Ok(n)) if n >= PRUNE_BATCH as usize => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Ok(Ok(_)) => return,
            Ok(Err(e)) => {
                eprintln!("history cleanup failed: {e}");
                return;
            }
            Err(e) => {
                eprintln!("history cleanup task failed: {e}");
                return;
            }
        }
    }
}

/// Runs for the life of the app: trims at startup, then every hour.
pub async fn prune_loop(app: AppHandle) {
    loop {
        prune_now(&app).await;
        tokio::time::sleep(PRUNE_EVERY).await;
    }
}
