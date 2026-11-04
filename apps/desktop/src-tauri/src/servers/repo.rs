//! Persistence for server profiles. All SQL is parameterized.

use super::model::{AuthMethod, ServerProfile, ValidatedServer};
use crate::error::AppError;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

const COLUMNS: &str = "id, name, host, port, username, auth_method, credential_ref, description, created_at, updated_at";

fn from_row(row: &Row<'_>) -> rusqlite::Result<ServerProfile> {
    let auth: String = row.get(5)?;
    let auth_method = AuthMethod::parse(&auth).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            format!("unknown auth_method '{auth}'").into(),
        )
    })?;
    let port: i64 = row.get(3)?;
    Ok(ServerProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        host: row.get(2)?,
        port: u16::try_from(port).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, port))?,
        username: row.get(4)?,
        auth_method,
        credential_ref: row.get(6)?,
        description: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn insert(
    conn: &Connection,
    id: &str,
    v: &ValidatedServer,
    now: i64,
) -> Result<ServerProfile, AppError> {
    conn.execute(
        "INSERT INTO servers (id, name, host, port, username, auth_method, credential_ref, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?8)",
        params![id, v.name, v.host, v.port, v.username, v.auth_method.as_str(), v.description, now],
    )?;
    get(conn, id)?.ok_or_else(|| AppError::internal("Inserted server could not be read back."))
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<ServerProfile>, AppError> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM servers WHERE id = ?1"),
            params![id],
            from_row,
        )
        .optional()?)
}

pub fn list(conn: &Connection) -> Result<Vec<ServerProfile>, AppError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM servers ORDER BY name COLLATE NOCASE, created_at"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Updates the editable fields. Returns false if the server does not exist.
pub fn update(
    conn: &Connection,
    id: &str,
    v: &ValidatedServer,
    now: i64,
) -> Result<bool, AppError> {
    let n = conn.execute(
        "UPDATE servers SET name = ?2, host = ?3, port = ?4, username = ?5, auth_method = ?6,
                description = ?7, updated_at = ?8
         WHERE id = ?1",
        params![
            id,
            v.name,
            v.host,
            v.port,
            v.username,
            v.auth_method.as_str(),
            v.description,
            now
        ],
    )?;
    Ok(n > 0)
}

pub fn set_credential_ref(
    conn: &Connection,
    id: &str,
    credential_ref: Option<&str>,
    now: i64,
) -> Result<bool, AppError> {
    let n = conn.execute(
        "UPDATE servers SET credential_ref = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, credential_ref, now],
    )?;
    Ok(n > 0)
}

pub fn delete(conn: &Connection, id: &str) -> Result<bool, AppError> {
    let n = conn.execute("DELETE FROM servers WHERE id = ?1", params![id])?;
    Ok(n > 0)
}
