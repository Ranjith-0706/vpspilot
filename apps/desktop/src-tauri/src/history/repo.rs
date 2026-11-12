//! SQL for metric history. Everything is parameterized.

use super::model::HistoryPoint;
use crate::error::AppError;
use crate::ssh::monitor::parse::Metrics;
use rusqlite::{Connection, OptionalExtension, params};

/// Whether a reading is complete and plausible enough to store as a valid sample. Anything
/// else is dropped rather than recorded as data.
pub fn validate(m: &Metrics) -> Result<(), &'static str> {
    let pct = |v: f64| v.is_finite() && (0.0..=100.0).contains(&v);
    let load = |v: f64| v.is_finite() && v >= 0.0;
    if !pct(m.cpu_percent) || !pct(m.mem_used_percent) || !pct(m.disk_used_percent) {
        return Err("a usage percentage is out of range");
    }
    if !load(m.load1) || !load(m.load5) || !load(m.load15) {
        return Err("a load average is invalid");
    }
    if m.cores == 0 || m.sampled_at <= 0 || m.uptime_secs > i64::MAX as u64 {
        return Err("the sample is incomplete");
    }
    if m.mem_total_kib > i64::MAX as u64 || m.disk_total_kib > i64::MAX as u64 {
        return Err("a size is out of range");
    }
    Ok(())
}

pub fn insert_sample(conn: &Connection, server_id: &str, m: &Metrics) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO metric_samples
           (server_id, at, cpu_percent, mem_used_percent, disk_used_percent,
            load1, load5, load15, uptime_secs, cores, mem_total_kib, disk_total_kib)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            server_id,
            m.sampled_at,
            m.cpu_percent,
            m.mem_used_percent,
            m.disk_used_percent,
            m.load1,
            m.load5,
            m.load15,
            m.uptime_secs as i64,
            m.cores,
            m.mem_total_kib as i64,
            m.disk_total_kib as i64,
        ],
    )?;
    Ok(())
}

pub fn server_exists(conn: &Connection, server_id: &str) -> Result<bool, AppError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM servers WHERE id = ?1",
            params![server_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Averages the samples of `[from, to)` into buckets of `bucket_ms`, oldest first.
pub fn bucketed(
    conn: &Connection,
    server_id: &str,
    from: i64,
    to: i64,
    bucket_ms: i64,
) -> Result<Vec<HistoryPoint>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT CAST(AVG(at) AS INTEGER), AVG(cpu_percent), AVG(mem_used_percent),
                AVG(disk_used_percent), AVG(load1), AVG(load5), AVG(load15), COUNT(*)
           FROM metric_samples
          WHERE server_id = ?1 AND at >= ?2 AND at < ?3
          GROUP BY (at - ?2) / ?4
          ORDER BY 1",
    )?;
    let rows = stmt.query_map(params![server_id, from, to, bucket_ms], |r| {
        Ok(HistoryPoint {
            at: r.get(0)?,
            cpu_percent: r.get(1)?,
            mem_used_percent: r.get(2)?,
            disk_used_percent: r.get(3)?,
            load1: r.get(4)?,
            load5: r.get(5)?,
            load15: r.get(6)?,
            samples: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// `(count, first, last)` of the stored samples in `[from, to)`.
pub fn range_stats(
    conn: &Connection,
    server_id: &str,
    from: i64,
    to: i64,
) -> Result<(u32, Option<i64>, Option<i64>), AppError> {
    Ok(conn.query_row(
        "SELECT COUNT(*), MIN(at), MAX(at) FROM metric_samples
          WHERE server_id = ?1 AND at >= ?2 AND at < ?3",
        params![server_id, from, to],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?)
}

/// Deletes up to `limit` samples older than `cutoff`. Returns how many were deleted.
pub fn delete_older_than(conn: &Connection, cutoff: i64, limit: u32) -> Result<usize, AppError> {
    Ok(conn.execute(
        "DELETE FROM metric_samples WHERE id IN
           (SELECT id FROM metric_samples WHERE at < ?1 LIMIT ?2)",
        params![cutoff, limit],
    )?)
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
