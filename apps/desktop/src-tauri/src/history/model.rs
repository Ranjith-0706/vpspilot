use serde::Serialize;

pub const DEFAULT_RETENTION_DAYS: u32 = 7;
pub const MIN_RETENTION_DAYS: u32 = 1;
pub const MAX_RETENTION_DAYS: u32 = 90;

/// At most one sample per server is stored in this window, however fast the monitor polls
/// (the default poll is every 5 s, which would be ~17,000 rows per server per day).
pub const MIN_SAMPLE_GAP_MS: i64 = 60_000;

/// A history query returns at most this many points; longer ranges are averaged into buckets.
pub const MAX_POINTS: u32 = 300;
pub const MIN_POINTS: u32 = 10;

/// Query ranges, in seconds.
pub const MIN_RANGE_SECS: u32 = 60;
pub const MAX_RANGE_SECS: u32 = MAX_RETENTION_DAYS * 86_400;

/// One point of a chart: the average of the samples that fall in its time bucket.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPoint {
    /// Unix epoch milliseconds: the mean sample time of the bucket.
    pub at: i64,
    pub cpu_percent: f64,
    pub mem_used_percent: f64,
    pub disk_used_percent: f64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    /// How many stored samples were averaged into this point.
    pub samples: u32,
}

/// The answer to one history query. Always historical data: it says nothing about whether the
/// server is reachable now.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRange {
    pub server_id: String,
    pub from: i64,
    pub to: i64,
    pub bucket_ms: i64,
    pub points: Vec<HistoryPoint>,
    /// Stored samples inside the range.
    pub sample_count: u32,
    pub first_sample_at: Option<i64>,
    pub last_sample_at: Option<i64>,
    /// How far back data is kept; a range longer than this simply has no older data.
    pub retention_days: u32,
}
