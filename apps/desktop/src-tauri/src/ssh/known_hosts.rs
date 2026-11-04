//! Persistence of trusted host keys (one per server profile). All SQL is parameterized.

use super::hostkey::{HostKeyRecord, ObservedKey};
use crate::error::AppError;
use rusqlite::{Connection, OptionalExtension, Row, params};

const COLUMNS: &str = "server_id, key_type, fingerprint, trusted_at";

fn from_row(row: &Row<'_>) -> rusqlite::Result<HostKeyRecord> {
    Ok(HostKeyRecord {
        server_id: row.get(0)?,
        key_type: row.get(1)?,
        fingerprint: row.get(2)?,
        trusted_at: row.get(3)?,
    })
}

pub fn get(conn: &Connection, server_id: &str) -> Result<Option<HostKeyRecord>, AppError> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM known_host_keys WHERE server_id = ?1"),
            params![server_id],
            from_row,
        )
        .optional()?)
}

pub fn list(conn: &Connection) -> Result<Vec<HostKeyRecord>, AppError> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM known_host_keys"))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Trusts `key` for the server, replacing any previous key.
pub fn upsert(
    conn: &Connection,
    server_id: &str,
    key: &ObservedKey,
    now: i64,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO known_host_keys (server_id, key_type, fingerprint, trusted_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(server_id) DO UPDATE SET
            key_type = excluded.key_type,
            fingerprint = excluded.fingerprint,
            trusted_at = excluded.trusted_at",
        params![server_id, key.key_type, key.fingerprint, now],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, server_id: &str) -> Result<bool, AppError> {
    let n = conn.execute(
        "DELETE FROM known_host_keys WHERE server_id = ?1",
        params![server_id],
    )?;
    Ok(n > 0)
}
