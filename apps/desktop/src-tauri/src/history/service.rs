//! History rules: what is stored, how long, and how it is queried.

use super::model::*;
use super::repo;
use crate::error::AppError;
use crate::ssh::monitor::parse::Metrics;
use rusqlite::Connection;

const RETENTION_KEY: &str = "history_retention_days";

/// Rows removed per transaction by the cleanup, so it never holds the database for long.
pub const PRUNE_BATCH: u32 = 5_000;

/// Decides which samples of one running monitor are stored. Lives only as long as the monitor.
#[derive(Debug, Default)]
pub struct HistoryRecorder {
    last_stored_at: Option<i64>,
    failing: bool,
}

impl HistoryRecorder {
    /// Whether this sample should be stored: valid, and not within a minute of the last stored
    /// one. A clock that jumps backwards is stored rather than silently dropped for hours.
    pub fn due(&self, m: &Metrics) -> bool {
        if repo::validate(m).is_err() {
            return false;
        }
        match self.last_stored_at {
            Some(last) => !(0..MIN_SAMPLE_GAP_MS).contains(&(m.sampled_at - last)),
            None => true,
        }
    }

    /// Records that a sample was stored. Returns true if writes had been failing before.
    pub fn stored(&mut self, at: i64) -> bool {
        self.last_stored_at = Some(at);
        std::mem::take(&mut self.failing)
    }

    /// Records a failed write (the sample is retried with the next one). Returns true only for
    /// the first failure of a run, so the caller can log once instead of on every poll.
    pub fn failed(&mut self) -> bool {
        !std::mem::replace(&mut self.failing, true)
    }
}

/// Stores one successful sample. Invalid samples are refused, not stored.
pub fn record_sample(conn: &Connection, server_id: &str, m: &Metrics) -> Result<(), AppError> {
    repo::validate(m).map_err(|why| AppError::internal(format!("Sample not stored: {why}.")))?;
    repo::insert_sample(conn, server_id, m)
}

pub fn retention_days(conn: &Connection) -> Result<u32, AppError> {
    Ok(repo::get_setting(conn, RETENTION_KEY)?
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|d| (MIN_RETENTION_DAYS..=MAX_RETENTION_DAYS).contains(d))
        .unwrap_or(DEFAULT_RETENTION_DAYS))
}

pub fn set_retention_days(conn: &Connection, days: u32) -> Result<u32, AppError> {
    if !(MIN_RETENTION_DAYS..=MAX_RETENTION_DAYS).contains(&days) {
        return Err(AppError::validation(
            "days",
            format!("History can be kept for {MIN_RETENTION_DAYS} to {MAX_RETENTION_DAYS} days."),
        ));
    }
    repo::set_setting(conn, RETENTION_KEY, &days.to_string())?;
    Ok(days)
}

/// Deletes one batch of samples older than the retention period. A full batch means there may
/// be more, so callers repeat until it returns fewer than [`PRUNE_BATCH`].
pub fn prune_batch(conn: &Connection, now: i64) -> Result<usize, AppError> {
    let cutoff = now - i64::from(retention_days(conn)?) * 86_400_000;
    repo::delete_older_than(conn, cutoff, PRUNE_BATCH)
}

/// Samples of one server for the last `range_secs`, averaged into at most `max_points` points.
/// The result is history: it carries no claim about the server's current state.
pub fn query(
    conn: &Connection,
    server_id: &str,
    range_secs: u32,
    max_points: Option<u32>,
    now: i64,
) -> Result<HistoryRange, AppError> {
    if !(MIN_RANGE_SECS..=MAX_RANGE_SECS).contains(&range_secs) {
        return Err(AppError::validation(
            "rangeSecs",
            "That time range is not supported.",
        ));
    }
    if !repo::server_exists(conn, server_id)? {
        return Err(AppError::not_found("server"));
    }
    let max_points = max_points
        .unwrap_or(MAX_POINTS)
        .clamp(MIN_POINTS, MAX_POINTS);
    let to = now;
    let from = to - i64::from(range_secs) * 1000;
    // Never finer than the storage interval, so a line has at most one point per sample.
    let bucket_ms = (i64::from(range_secs) * 1000 / i64::from(max_points)).max(MIN_SAMPLE_GAP_MS);

    let points = repo::bucketed(conn, server_id, from, to, bucket_ms)?;
    let (sample_count, first_sample_at, last_sample_at) =
        repo::range_stats(conn, server_id, from, to)?;
    Ok(HistoryRange {
        server_id: server_id.to_string(),
        from,
        to,
        bucket_ms,
        points,
        sample_count,
        first_sample_at,
        last_sample_at,
        retention_days: retention_days(conn)?,
    })
}
