//! Monitoring commands. `start_monitoring` takes a saved server's id and a poll interval only;
//! the command that runs on the server is a compile-time constant (see `ssh::monitor::parse`).

use crate::alerts::model::AlertEvent;
use crate::alerts::service as alerts;
use crate::error::AppError;
use crate::servers::repo::now_ms;
use crate::ssh::monitor::{self, MonitorConfig, MonitorEvent, MonitorState};
use crate::ssh::service;
use crate::state::{AppState, MonitorEntry};
use serde::Serialize;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
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

/// What the frontend receives: the monitor's own events, plus alert state changes found by
/// evaluating its samples. Monitor events keep their shape; alerts add `{"type":"alert", ...}`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum StreamEvent {
    Monitor(MonitorEvent),
    Alert(AlertNotice),
}

#[derive(Debug, Serialize)]
pub struct AlertNotice {
    /// Always `"alert"`.
    #[serde(rename = "type")]
    kind: &'static str,
    event: AlertEvent,
}

impl StreamEvent {
    fn alert(event: AlertEvent) -> Self {
        StreamEvent::Alert(AlertNotice {
            kind: "alert",
            event,
        })
    }
}

/// Evaluates alert rules for a new sample. A failure here must never disturb monitoring, so it
/// is logged and treated as "no alerts".
fn alerts_for_sample(app: &AppHandle, server_id: &str, event: &MonitorEvent) -> Vec<AlertEvent> {
    let MonitorEvent::Sample { metrics } = event else {
        return Vec::new();
    };
    let state = app.state::<AppState>();
    let result = (|| {
        let conn = state.conn()?;
        let mut engine = state.engine()?;
        alerts::process_sample(&conn, &mut engine, server_id, metrics, now_ms())
    })();
    result.unwrap_or_else(|e| {
        eprintln!("alert evaluation failed: {e}");
        Vec::new()
    })
}

/// Whether an event means samples have stopped arriving (connecting or reconnecting).
fn is_gap(event: &MonitorEvent) -> bool {
    matches!(
        event,
        MonitorEvent::Status {
            state: MonitorState::Connecting | MonitorState::Retrying,
            ..
        }
    )
}

/// A gap in samples ends any run of "consecutive" checks. Alerts already firing are left alone.
fn note_gap(app: &AppHandle, server_id: &str, event: &MonitorEvent) {
    if !is_gap(event) {
        return;
    }
    match app.state::<AppState>().engine() {
        Ok(mut engine) => alerts::note_gap(&mut engine, server_id),
        Err(e) => eprintln!("alert gap handling failed: {e}"),
    }
}

/// Monitoring of a server ended: resolve whatever it had firing.
fn close_alerts(app: &AppHandle, server_id: &str) -> Vec<AlertEvent> {
    let state = app.state::<AppState>();
    let result = (|| {
        let conn = state.conn()?;
        let mut engine = state.engine()?;
        alerts::close_server(&conn, &mut engine, server_id, now_ms())
    })();
    result.unwrap_or_else(|e| {
        eprintln!("closing alerts failed: {e}");
        Vec::new()
    })
}

/// Starts (or restarts) monitoring a server. Events stream back over `on_event`.
#[tauri::command]
pub async fn start_monitoring(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    interval_secs: u32,
    on_event: Channel<StreamEvent>,
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
        'events: while let Some(event) = rx.recv().await {
            let ends = matches!(
                &event,
                MonitorEvent::Status {
                    state: MonitorState::Failed | MonitorState::Stopped,
                    ..
                }
            );
            note_gap(&app, &id, &event);
            for alert in alerts_for_sample(&app, &id, &event) {
                if on_event.send(StreamEvent::alert(alert)).is_err() {
                    break 'events;
                }
            }
            if on_event.send(StreamEvent::Monitor(event)).is_err() {
                break; // the frontend is gone; dropping `rx` makes the monitor stop
            }
            if ends {
                break;
            }
        }
        // However it ended, nothing is firing for a server that is no longer monitored. If a
        // restart already replaced us, the newer monitor owns the alert state: leave it alone.
        // (The registry lock is released before the database is touched; other code takes the
        // database first and the registry second, so holding both here could deadlock.)
        let superseded = monitors
            .lock()
            .is_ok_and(|map| map.get(&id).is_some_and(|e| e.token != token));
        if !superseded {
            for alert in close_alerts(&app, &id) {
                let _ = on_event.send(StreamEvent::alert(alert));
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
    fn only_connecting_and_retrying_are_gaps() {
        let status = |state| MonitorEvent::Status {
            state,
            message: None,
        };
        assert!(is_gap(&status(MonitorState::Connecting)));
        assert!(is_gap(&status(MonitorState::Retrying)));
        assert!(!is_gap(&status(MonitorState::Online)));
        assert!(!is_gap(&status(MonitorState::Failed)));
        assert!(!is_gap(&status(MonitorState::Stopped)));
    }

    #[test]
    fn interval_bounds() {
        assert_eq!(validate_interval(2).unwrap(), Duration::from_secs(2));
        assert_eq!(validate_interval(300).unwrap(), Duration::from_secs(300));
        for bad in [0, 1, 301, u32::MAX] {
            assert!(validate_interval(bad).is_err(), "{bad}");
        }
    }
}
