//! Thin Tauri command wrappers. Logic lives in `alerts::service`.
//! Rules are plain thresholds on fixed metrics; nothing here runs anything on a server.

use crate::alerts::engine::ActiveAlert;
use crate::alerts::model::{AlertEvent, AlertRule, RuleInput};
use crate::alerts::{repo, service};
use crate::error::AppError;
use crate::servers::repo::now_ms;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn list_alert_rules(state: State<'_, AppState>) -> Result<Vec<AlertRule>, AppError> {
    let conn = state.conn()?;
    service::list_rules(&conn)
}

#[tauri::command]
pub fn add_alert_rule(state: State<'_, AppState>, input: RuleInput) -> Result<AlertRule, AppError> {
    let conn = state.conn()?;
    service::add_rule(&conn, &input, now_ms())
}

#[tauri::command]
pub fn update_alert_rule(
    state: State<'_, AppState>,
    id: String,
    input: RuleInput,
) -> Result<AlertRule, AppError> {
    let conn = state.conn()?;
    service::update_rule(&conn, &id, &input, now_ms())
}

#[tauri::command]
pub fn delete_alert_rule(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let conn = state.conn()?;
    service::delete_rule(&conn, &id)
}

/// Newest first. `limit` is clamped by the service.
#[tauri::command]
pub fn list_alert_events(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<AlertEvent>, AppError> {
    let conn = state.conn()?;
    service::list_events(&conn, limit)
}

#[tauri::command]
pub fn clear_alert_events(state: State<'_, AppState>) -> Result<(), AppError> {
    let conn = state.conn()?;
    repo::clear_events(&conn)
}

/// Alerts firing right now.
#[tauri::command]
pub fn list_active_alerts(state: State<'_, AppState>) -> Result<Vec<ActiveAlert>, AppError> {
    Ok(state.engine()?.active())
}
