//! Persistence for alert rules and event history. All SQL is parameterized.

use super::engine::Transition;
use super::model::{AlertEvent, AlertRule, AlertState, Metric, RuleInput};
use crate::error::AppError;
use rusqlite::{Connection, OptionalExtension, Row, params};

/// History is trimmed to the newest rows beyond this.
pub const MAX_EVENTS: i64 = 1000;

const RULE_COLUMNS: &str =
    "id, server_id, metric, threshold, consecutive, enabled, created_at, updated_at";

fn conversion(idx: usize, what: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, what.into())
}

fn rule_from_row(row: &Row<'_>) -> rusqlite::Result<AlertRule> {
    let metric: String = row.get(2)?;
    Ok(AlertRule {
        id: row.get(0)?,
        server_id: row.get(1)?,
        metric: Metric::parse(&metric)
            .ok_or_else(|| conversion(2, format!("metric '{metric}'")))?,
        threshold: row.get(3)?,
        consecutive: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub fn insert_rule(
    conn: &Connection,
    id: &str,
    input: &RuleInput,
    now: i64,
) -> Result<AlertRule, AppError> {
    conn.execute(
        "INSERT INTO alert_rules (id, server_id, metric, threshold, consecutive, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            id,
            input.server_id,
            input.metric.as_str(),
            input.threshold,
            input.consecutive,
            input.enabled,
            now
        ],
    )?;
    get_rule(conn, id)?.ok_or_else(|| AppError::internal("Inserted rule could not be read back."))
}

pub fn get_rule(conn: &Connection, id: &str) -> Result<Option<AlertRule>, AppError> {
    Ok(conn
        .query_row(
            &format!("SELECT {RULE_COLUMNS} FROM alert_rules WHERE id = ?1"),
            params![id],
            rule_from_row,
        )
        .optional()?)
}

pub fn list_rules(conn: &Connection) -> Result<Vec<AlertRule>, AppError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {RULE_COLUMNS} FROM alert_rules ORDER BY created_at, id"
    ))?;
    let rows = stmt.query_map([], rule_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn count_rules(conn: &Connection) -> Result<usize, AppError> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM alert_rules", [], |r| r.get(0))?;
    Ok(usize::try_from(n).unwrap_or(0))
}

/// Enabled rules that apply to a server: its own plus the ones for all servers.
pub fn enabled_rules_for(conn: &Connection, server_id: &str) -> Result<Vec<AlertRule>, AppError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {RULE_COLUMNS} FROM alert_rules
         WHERE enabled = 1 AND (server_id IS NULL OR server_id = ?1)
         ORDER BY created_at, id"
    ))?;
    let rows = stmt.query_map(params![server_id], rule_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Returns false if the rule does not exist.
pub fn update_rule(
    conn: &Connection,
    id: &str,
    input: &RuleInput,
    now: i64,
) -> Result<bool, AppError> {
    let n = conn.execute(
        "UPDATE alert_rules SET server_id = ?2, metric = ?3, threshold = ?4, consecutive = ?5,
                enabled = ?6, updated_at = ?7
         WHERE id = ?1",
        params![
            id,
            input.server_id,
            input.metric.as_str(),
            input.threshold,
            input.consecutive,
            input.enabled,
            now
        ],
    )?;
    Ok(n > 0)
}

pub fn delete_rule(conn: &Connection, id: &str) -> Result<bool, AppError> {
    let n = conn.execute("DELETE FROM alert_rules WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<AlertEvent> {
    let metric: String = row.get(3)?;
    let state: String = row.get(5)?;
    Ok(AlertEvent {
        id: row.get(0)?,
        rule_id: row.get(1)?,
        server_id: row.get(2)?,
        metric: Metric::parse(&metric)
            .ok_or_else(|| conversion(3, format!("metric '{metric}'")))?,
        threshold: row.get(4)?,
        state: AlertState::parse(&state)
            .ok_or_else(|| conversion(5, format!("state '{state}'")))?,
        value: row.get(6)?,
        at: row.get(7)?,
    })
}

/// Records a state change and trims old history.
pub fn insert_event(conn: &Connection, t: &Transition) -> Result<AlertEvent, AppError> {
    conn.execute(
        "INSERT INTO alert_events (rule_id, server_id, metric, threshold, state, value, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            t.rule_id,
            t.server_id,
            t.metric.as_str(),
            t.threshold,
            t.state.as_str(),
            t.value,
            t.at
        ],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "DELETE FROM alert_events WHERE id <= (
             SELECT id FROM alert_events ORDER BY id DESC LIMIT 1 OFFSET ?1
         )",
        params![MAX_EVENTS],
    )?;
    Ok(AlertEvent {
        id,
        rule_id: Some(t.rule_id.clone()),
        server_id: t.server_id.clone(),
        metric: t.metric,
        threshold: t.threshold,
        state: t.state,
        value: t.value,
        at: t.at,
    })
}

/// Newest first.
pub fn list_events(conn: &Connection, limit: i64) -> Result<Vec<AlertEvent>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, rule_id, server_id, metric, threshold, state, value, at
         FROM alert_events ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], event_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn clear_events(conn: &Connection) -> Result<(), AppError> {
    conn.execute("DELETE FROM alert_events", [])?;
    Ok(())
}
