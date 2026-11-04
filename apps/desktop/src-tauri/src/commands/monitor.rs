//! Monitoring commands. `start_monitoring` takes a saved server's id and a poll interval only;
//! the command that runs on the server is a compile-time constant (see `ssh::monitor::parse`).

use crate::error::AppError;
use crate::ssh::monitor::{self, MonitorConfig, MonitorEvent};
use crate::ssh::service;
use crate::state::{AppState, MonitorEntry};
use std::time::Duration;
use tauri::State;
use tauri::ipc::Channel;
use tokio::sync::mpsc;

pub const MIN_INTERVAL_SECS: u32 = 2;
pub const MAX_INTERVAL_SECS: u32 = 300;
pub const MAX_MONITORS: usize = 20;

pub fn validate_interval(secs: u32) -> Result<Duration, AppError> {
    if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&secs) {
        return Err(AppError::validation(
            "intervalSecs",
            format!(
                "Interval must be between {MIN_INTERVAL_SECS} and {MAX_INTERVAL_SECS} seconds."
            ),
        ));
    }
    Ok(Duration::from_secs(secs.into()))
}

/// Starts (or restarts) monitoring a server. Events stream back over `on_event`.
#[tauri::command]
pub async fn start_monitoring(
    state: State<'_, AppState>,
    id: String,
    interval_secs: u32,
    on_event: Channel<MonitorEvent>,
) -> Result<(), AppError> {
    let interval = validate_interval(interval_secs)?;

    // Load everything first; the database guard must not be held across any await.
    let params = {
        let conn = state.conn()?;
        service::prepare_connection(&conn, state.credentials.as_ref(), &id)?
    };

    // Restarting replaces the previous monitor (dropping its handle stops it).
    {
        let mut monitors = state.monitors()?;
        monitors.remove(&id);
        if monitors.len() >= MAX_MONITORS {
            return Err(AppError::connection(format!(
                "At most {MAX_MONITORS} servers can be monitored at once."
            )));
        }
    }

    let (tx, mut rx) = mpsc::channel::<MonitorEvent>(64);
    let handle = monitor::spawn(params, MonitorConfig::with_interval(interval), tx)
        .map_err(AppError::connection)?;

    let token = uuid::Uuid::new_v4().to_string();
    state.monitors()?.insert(
        id.clone(),
        MonitorEntry {
            token: token.clone(),
            handle,
        },
    );

    // Forward events to the frontend. When the monitor ends, forget it, but only if the entry
    // is still ours (a restart may have replaced it).
    let monitors = state.monitor_registry();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if on_event.send(event).is_err() {
                break; // the frontend is gone; dropping `rx` makes the monitor stop
            }
        }
        if let Ok(mut map) = monitors.lock()
            && map.get(&id).is_some_and(|e| e.token == token)
        {
            map.remove(&id);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn stop_monitoring(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.stop_monitor(&id, "Monitoring stopped.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_bounds() {
        assert_eq!(validate_interval(2).unwrap(), Duration::from_secs(2));
        assert_eq!(validate_interval(300).unwrap(), Duration::from_secs(300));
        for bad in [0, 1, 301, u32::MAX] {
            assert!(validate_interval(bad).is_err(), "{bad}");
        }
    }
}
