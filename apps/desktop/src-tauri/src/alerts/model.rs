//! Alert rule and event types, plus input validation.

use crate::error::AppError;
use crate::ssh::monitor::parse::Metrics;
use serde::{Deserialize, Serialize};

pub const MAX_RULES: usize = 100;
pub const MIN_THRESHOLD: u32 = 1;
pub const MAX_THRESHOLD: u32 = 100;
pub const MIN_CONSECUTIVE: u32 = 1;
pub const MAX_CONSECUTIVE: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    Cpu,
    Memory,
    Disk,
}

impl Metric {
    pub fn as_str(self) -> &'static str {
        match self {
            Metric::Cpu => "cpu",
            Metric::Memory => "memory",
            Metric::Disk => "disk",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cpu" => Some(Metric::Cpu),
            "memory" => Some(Metric::Memory),
            "disk" => Some(Metric::Disk),
            _ => None,
        }
    }

    /// The sampled percentage this metric watches.
    pub fn value(self, m: &Metrics) -> f64 {
        match self {
            Metric::Cpu => m.cpu_percent,
            Metric::Memory => m.mem_used_percent,
            Metric::Disk => m.disk_used_percent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertState {
    Firing,
    Resolved,
}

impl AlertState {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertState::Firing => "firing",
            AlertState::Resolved => "resolved",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "firing" => Some(AlertState::Firing),
            "resolved" => Some(AlertState::Resolved),
            _ => None,
        }
    }
}

/// A saved threshold rule: "alert when `metric` is at or above `threshold` percent for
/// `consecutive` samples in a row". `server_id` of `None` applies to every server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRule {
    pub id: String,
    pub server_id: Option<String>,
    pub metric: Metric,
    pub threshold: u32,
    pub consecutive: u32,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Rule fields as sent by the frontend.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub server_id: Option<String>,
    pub metric: Metric,
    pub threshold: u32,
    pub consecutive: u32,
    pub enabled: bool,
}

impl RuleInput {
    pub fn validate(&self) -> Result<(), AppError> {
        if !(MIN_THRESHOLD..=MAX_THRESHOLD).contains(&self.threshold) {
            return Err(AppError::validation(
                "threshold",
                format!("Threshold must be between {MIN_THRESHOLD} and {MAX_THRESHOLD} percent."),
            ));
        }
        if !(MIN_CONSECUTIVE..=MAX_CONSECUTIVE).contains(&self.consecutive) {
            return Err(AppError::validation(
                "consecutive",
                format!("Checks in a row must be between {MIN_CONSECUTIVE} and {MAX_CONSECUTIVE}."),
            ));
        }
        if let Some(id) = &self.server_id
            && (id.is_empty() || id.len() > 64)
        {
            return Err(AppError::validation("serverId", "Choose a valid server."));
        }
        Ok(())
    }
}

/// One alert state change, kept as history. `metric` and `threshold` are a snapshot, so history
/// still reads correctly after the rule is edited or deleted. A resolved event with no `value`
/// means the alert ended because monitoring stopped or the rule was removed or changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertEvent {
    pub id: i64,
    pub rule_id: Option<String>,
    pub server_id: String,
    pub metric: Metric,
    pub threshold: u32,
    pub state: AlertState,
    pub value: Option<f64>,
    pub at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(threshold: u32, consecutive: u32) -> RuleInput {
        RuleInput {
            server_id: None,
            metric: Metric::Cpu,
            threshold,
            consecutive,
            enabled: true,
        }
    }

    #[test]
    fn accepts_bounds() {
        assert!(input(1, 1).validate().is_ok());
        assert!(input(100, 60).validate().is_ok());
    }

    #[test]
    fn rejects_out_of_range() {
        for (t, c) in [(0, 1), (101, 1), (50, 0), (50, 61), (u32::MAX, 1)] {
            assert!(input(t, c).validate().is_err(), "{t} {c}");
        }
    }

    #[test]
    fn rejects_bad_server_id() {
        let mut i = input(50, 1);
        i.server_id = Some(String::new());
        assert!(i.validate().is_err());
        i.server_id = Some("x".repeat(65));
        assert!(i.validate().is_err());
    }

    #[test]
    fn metric_and_state_round_trip() {
        for m in [Metric::Cpu, Metric::Memory, Metric::Disk] {
            assert_eq!(Metric::parse(m.as_str()), Some(m));
        }
        for s in [AlertState::Firing, AlertState::Resolved] {
            assert_eq!(AlertState::parse(s.as_str()), Some(s));
        }
        assert_eq!(Metric::parse("swap"), None);
    }
}
