//! The alert state machine. Pure: no I/O, no clock. Feed it samples and the applicable rules,
//! get back the state changes.
//!
//! - An alert fires after `consecutive` samples in a row at or above the threshold.
//! - It resolves only once the value drops clearly below the threshold (a small margin), so a
//!   value hovering around the threshold does not flap.
//! - If its rule is deleted, disabled or its metric changes, or monitoring ends, it resolves
//!   without a value.

use super::model::{AlertRule, AlertState, Metric};
use crate::ssh::monitor::parse::Metrics;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Percentage points below the threshold needed to resolve (capped at half the threshold).
const RESOLVE_MARGIN: f64 = 5.0;

fn resolve_below(threshold: u32) -> f64 {
    let t = f64::from(threshold);
    t - RESOLVE_MARGIN.min(t / 2.0)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAlert {
    pub rule_id: String,
    pub server_id: String,
    pub metric: Metric,
    pub threshold: u32,
    /// Unix epoch milliseconds when it started firing.
    pub since: i64,
    /// The most recent value seen while firing.
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub rule_id: String,
    pub server_id: String,
    pub metric: Metric,
    pub threshold: u32,
    pub state: AlertState,
    pub value: Option<f64>,
    pub at: i64,
}

fn resolved(a: ActiveAlert, value: Option<f64>, at: i64) -> Transition {
    Transition {
        rule_id: a.rule_id,
        server_id: a.server_id,
        metric: a.metric,
        threshold: a.threshold,
        state: AlertState::Resolved,
        value,
        at,
    }
}

#[derive(Default)]
struct Track {
    /// Consecutive samples at or above the threshold while not firing.
    streak: u32,
    active: Option<ActiveAlert>,
}

/// State per (rule id, server id).
#[derive(Default)]
pub struct AlertEngine {
    tracks: HashMap<(String, String), Track>,
}

impl AlertEngine {
    /// Evaluates one sample from `server_id` against the rules that apply to it.
    pub fn evaluate(
        &mut self,
        server_id: &str,
        rules: &[AlertRule],
        metrics: &Metrics,
        now: i64,
    ) -> Vec<Transition> {
        let mut out = Vec::new();

        // Anything tracked for a rule that no longer applies (deleted or disabled) ends.
        let live: HashSet<&str> = rules.iter().map(|r| r.id.as_str()).collect();
        let gone: Vec<(String, String)> = self
            .tracks
            .keys()
            .filter(|(rule, server)| server == server_id && !live.contains(rule.as_str()))
            .cloned()
            .collect();
        for key in gone {
            if let Some(track) = self.tracks.remove(&key)
                && let Some(active) = track.active
            {
                out.push(resolved(active, None, now));
            }
        }

        for rule in rules {
            let key = (rule.id.clone(), server_id.to_string());
            let track = self.tracks.entry(key).or_default();

            // The rule now watches something else: the old alert is over.
            if track
                .active
                .as_ref()
                .is_some_and(|a| a.metric != rule.metric)
                && let Some(old) = track.active.take()
            {
                out.push(resolved(old, None, now));
                track.streak = 0;
            }

            let value = rule.metric.value(metrics);
            if let Some(active) = track.active.as_mut() {
                active.threshold = rule.threshold;
                if value < resolve_below(rule.threshold) {
                    if let Some(done) = track.active.take() {
                        out.push(resolved(done, Some(value), now));
                    }
                    track.streak = 0;
                } else {
                    active.value = value;
                }
            } else if value >= f64::from(rule.threshold) {
                track.streak += 1;
                if track.streak >= rule.consecutive {
                    track.active = Some(ActiveAlert {
                        rule_id: rule.id.clone(),
                        server_id: server_id.to_string(),
                        metric: rule.metric,
                        threshold: rule.threshold,
                        since: now,
                        value,
                    });
                    out.push(Transition {
                        rule_id: rule.id.clone(),
                        server_id: server_id.to_string(),
                        metric: rule.metric,
                        threshold: rule.threshold,
                        state: AlertState::Firing,
                        value: Some(value),
                        at: now,
                    });
                }
            } else {
                track.streak = 0;
            }
        }
        out
    }

    /// Samples from `server_id` stopped arriving (connecting, or reconnecting after a network
    /// problem). Checks on either side of the gap are not "in a row", so every pending streak
    /// starts again.
    ///
    /// Nothing else changes: an alert that is firing keeps firing, because an unreachable server
    /// has not recovered, and no transition is produced so history is untouched. The first
    /// sample after the gap decides whether it resolves.
    pub fn gap(&mut self, server_id: &str) {
        for ((_, server), track) in self.tracks.iter_mut() {
            if server == server_id {
                track.streak = 0;
            }
        }
    }

    /// Monitoring of `server_id` ended: resolve everything it had firing and forget its state.
    pub fn close_server(&mut self, server_id: &str, now: i64) -> Vec<Transition> {
        let keys: Vec<(String, String)> = self
            .tracks
            .keys()
            .filter(|(_, server)| server == server_id)
            .cloned()
            .collect();
        let mut out = Vec::new();
        for key in keys {
            if let Some(track) = self.tracks.remove(&key)
                && let Some(active) = track.active
            {
                out.push(resolved(active, None, now));
            }
        }
        out
    }

    /// Everything currently firing, oldest first.
    pub fn active(&self) -> Vec<ActiveAlert> {
        let mut all: Vec<ActiveAlert> = self
            .tracks
            .values()
            .filter_map(|t| t.active.clone())
            .collect();
        all.sort_by(|a, b| {
            a.since
                .cmp(&b.since)
                .then_with(|| a.rule_id.cmp(&b.rule_id))
        });
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(cpu: f64, mem: f64, disk: f64) -> Metrics {
        Metrics {
            cpu_percent: cpu,
            cores: 2,
            load1: 0.0,
            load5: 0.0,
            load15: 0.0,
            mem_total_kib: 1,
            mem_available_kib: 1,
            mem_used_percent: mem,
            swap_total_kib: 0,
            swap_used_kib: 0,
            disk_total_kib: 1,
            disk_used_kib: 1,
            disk_used_percent: disk,
            uptime_secs: 1,
            sampled_at: 0,
        }
    }

    fn rule(id: &str, metric: Metric, threshold: u32, consecutive: u32) -> AlertRule {
        AlertRule {
            id: id.into(),
            server_id: None,
            metric,
            threshold,
            consecutive,
            enabled: true,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn cpu(e: &mut AlertEngine, rules: &[AlertRule], v: f64, now: i64) -> Vec<Transition> {
        e.evaluate("s1", rules, &metrics(v, 0.0, 0.0), now)
    }

    #[test]
    fn fires_after_consecutive_samples_only() {
        let rules = [rule("r", Metric::Cpu, 90, 3)];
        let mut e = AlertEngine::default();
        assert!(cpu(&mut e, &rules, 95.0, 1).is_empty());
        assert!(cpu(&mut e, &rules, 95.0, 2).is_empty());
        let t = cpu(&mut e, &rules, 96.0, 3);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].state, AlertState::Firing);
        assert_eq!(t[0].value, Some(96.0));
        assert_eq!(t[0].at, 3);
        // Staying above does not fire again.
        assert!(cpu(&mut e, &rules, 97.0, 4).is_empty());
        assert_eq!(e.active().len(), 1);
        assert_eq!(e.active()[0].value, 97.0);
    }

    #[test]
    fn a_dip_resets_the_streak() {
        let rules = [rule("r", Metric::Cpu, 90, 3)];
        let mut e = AlertEngine::default();
        cpu(&mut e, &rules, 95.0, 1);
        cpu(&mut e, &rules, 95.0, 2);
        cpu(&mut e, &rules, 10.0, 3);
        assert!(cpu(&mut e, &rules, 95.0, 4).is_empty());
        assert!(cpu(&mut e, &rules, 95.0, 5).is_empty());
        assert_eq!(cpu(&mut e, &rules, 95.0, 6).len(), 1);
    }

    #[test]
    fn threshold_is_inclusive() {
        let rules = [rule("r", Metric::Cpu, 90, 1)];
        let mut e = AlertEngine::default();
        assert!(cpu(&mut e, &rules, 89.9, 1).is_empty());
        assert_eq!(cpu(&mut e, &rules, 90.0, 2).len(), 1);
    }

    #[test]
    fn resolves_only_below_the_margin() {
        let rules = [rule("r", Metric::Cpu, 90, 1)];
        let mut e = AlertEngine::default();
        cpu(&mut e, &rules, 95.0, 1);
        // Between 85 and 90 it stays firing: no flapping.
        assert!(cpu(&mut e, &rules, 88.0, 2).is_empty());
        assert!(cpu(&mut e, &rules, 85.0, 3).is_empty());
        let t = cpu(&mut e, &rules, 84.9, 4);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].state, AlertState::Resolved);
        assert_eq!(t[0].value, Some(84.9));
        assert!(e.active().is_empty());
    }

    #[test]
    fn can_fire_again_after_resolving() {
        let rules = [rule("r", Metric::Cpu, 90, 2)];
        let mut e = AlertEngine::default();
        cpu(&mut e, &rules, 95.0, 1);
        cpu(&mut e, &rules, 95.0, 2);
        cpu(&mut e, &rules, 10.0, 3);
        // The streak starts over after a resolve.
        assert!(cpu(&mut e, &rules, 95.0, 4).is_empty());
        assert_eq!(cpu(&mut e, &rules, 95.0, 5).len(), 1);
    }

    #[test]
    fn low_thresholds_still_resolve() {
        let rules = [rule("r", Metric::Cpu, 2, 1)];
        let mut e = AlertEngine::default();
        assert_eq!(cpu(&mut e, &rules, 5.0, 1).len(), 1);
        // The margin is capped at half the threshold, so 0.9 is clearly below 2.
        assert_eq!(cpu(&mut e, &rules, 0.9, 2).len(), 1);
    }

    #[test]
    fn rules_are_independent_per_metric_and_per_server() {
        let rules = [
            rule("c", Metric::Cpu, 90, 1),
            rule("m", Metric::Memory, 80, 1),
        ];
        let mut e = AlertEngine::default();
        let t = e.evaluate("s1", &rules, &metrics(95.0, 10.0, 0.0), 1);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].metric, Metric::Cpu);
        let t = e.evaluate("s2", &rules, &metrics(10.0, 85.0, 0.0), 2);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].metric, Metric::Memory);
        assert_eq!(t[0].server_id, "s2");
        assert_eq!(e.active().len(), 2);
    }

    #[test]
    fn deleted_or_disabled_rule_resolves_without_a_value() {
        let rules = [rule("r", Metric::Cpu, 90, 1)];
        let mut e = AlertEngine::default();
        cpu(&mut e, &rules, 95.0, 1);
        let t = cpu(&mut e, &[], 95.0, 2);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].state, AlertState::Resolved);
        assert_eq!(t[0].value, None);
        assert_eq!(t[0].threshold, 90);
        assert!(e.active().is_empty());
    }

    #[test]
    fn removing_a_rule_does_not_touch_other_servers() {
        let rules = [rule("r", Metric::Cpu, 90, 1)];
        let mut e = AlertEngine::default();
        e.evaluate("s1", &rules, &metrics(95.0, 0.0, 0.0), 1);
        e.evaluate("s2", &rules, &metrics(95.0, 0.0, 0.0), 1);
        let t = e.evaluate("s1", &[], &metrics(95.0, 0.0, 0.0), 2);
        assert_eq!(t.len(), 1);
        assert_eq!(e.active().len(), 1);
        assert_eq!(e.active()[0].server_id, "s2");
    }

    #[test]
    fn editing_the_metric_ends_the_old_alert() {
        let mut e = AlertEngine::default();
        cpu(&mut e, &[rule("r", Metric::Cpu, 90, 1)], 95.0, 1);
        let t = e.evaluate(
            "s1",
            &[rule("r", Metric::Disk, 90, 1)],
            &metrics(95.0, 0.0, 10.0),
            2,
        );
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].metric, Metric::Cpu);
        assert_eq!(t[0].value, None);
        assert!(e.active().is_empty());
    }

    #[test]
    fn editing_the_threshold_keeps_the_alert_and_uses_the_new_one() {
        let mut e = AlertEngine::default();
        cpu(&mut e, &[rule("r", Metric::Cpu, 90, 1)], 95.0, 1);
        // Raised to 99: 95 is still not clearly below 94, so it stays firing.
        assert!(cpu(&mut e, &[rule("r", Metric::Cpu, 99, 1)], 95.0, 2).is_empty());
        assert_eq!(e.active()[0].threshold, 99);
        let t = cpu(&mut e, &[rule("r", Metric::Cpu, 99, 1)], 90.0, 3);
        assert_eq!(t[0].state, AlertState::Resolved);
        assert_eq!(t[0].threshold, 99);
    }

    #[test]
    fn close_server_resolves_only_that_server() {
        let rules = [rule("r", Metric::Cpu, 90, 1)];
        let mut e = AlertEngine::default();
        e.evaluate("s1", &rules, &metrics(95.0, 0.0, 0.0), 1);
        e.evaluate("s2", &rules, &metrics(95.0, 0.0, 0.0), 1);
        let t = e.close_server("s1", 5);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].server_id, "s1");
        assert_eq!(t[0].value, None);
        assert_eq!(t[0].at, 5);
        assert!(e.close_server("s1", 6).is_empty());
        assert_eq!(e.active().len(), 1);
    }

    #[test]
    fn nan_never_fires_or_resolves() {
        let rules = [rule("r", Metric::Cpu, 90, 1)];
        let mut e = AlertEngine::default();
        assert!(cpu(&mut e, &rules, f64::NAN, 1).is_empty());
        cpu(&mut e, &rules, 95.0, 2);
        assert!(cpu(&mut e, &rules, f64::NAN, 3).is_empty());
        assert_eq!(e.active().len(), 1);
    }

    // ----- gaps in monitoring (disconnect, retry, reconnect, recovery) -----

    fn feed(e: &mut AlertEngine, server: &str, rules: &[AlertRule], cpu: f64, at: i64) -> usize {
        e.evaluate(server, rules, &metrics(cpu, 0.0, 0.0), at).len()
    }

    #[test]
    fn a_gap_restarts_the_streak_so_checks_either_side_are_not_in_a_row() {
        let rules = [rule("r1", Metric::Cpu, 90, 3)];
        let mut e = AlertEngine::default();
        assert_eq!(feed(&mut e, "s1", &rules, 95.0, 1), 0);
        assert_eq!(feed(&mut e, "s1", &rules, 95.0, 2), 0);
        e.gap("s1"); // disconnect + retry
        assert_eq!(
            feed(&mut e, "s1", &rules, 95.0, 3),
            0,
            "only one check since the gap"
        );
        assert_eq!(feed(&mut e, "s1", &rules, 95.0, 4), 0);
        assert_eq!(
            feed(&mut e, "s1", &rules, 95.0, 5),
            1,
            "three in a row after the gap"
        );
    }

    #[test]
    fn a_gap_does_not_resolve_or_duplicate_a_firing_alert() {
        let rules = [rule("r1", Metric::Cpu, 90, 1)];
        let mut e = AlertEngine::default();
        assert_eq!(feed(&mut e, "s1", &rules, 95.0, 1), 1);
        e.gap("s1");
        e.gap("s1"); // repeated retries
        assert_eq!(e.active().len(), 1, "still firing while unreachable");
        assert_eq!(e.active()[0].since, 1);
        // Reconnect with the problem still there: no new event, value refreshed.
        assert_eq!(feed(&mut e, "s1", &rules, 97.0, 2), 0);
        assert_eq!(e.active().len(), 1);
        assert_eq!(e.active()[0].value, 97.0);
        assert_eq!(
            e.active()[0].since,
            1,
            "it is the same alert, not a new one"
        );
    }

    #[test]
    fn recovery_after_a_gap_resolves_with_the_fresh_value() {
        let rules = [rule("r1", Metric::Cpu, 90, 1)];
        let mut e = AlertEngine::default();
        feed(&mut e, "s1", &rules, 95.0, 1);
        e.gap("s1");
        let out = e.evaluate("s1", &rules, &metrics(20.0, 0.0, 0.0), 9);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].state, AlertState::Resolved);
        assert_eq!(out[0].value, Some(20.0));
        assert!(e.active().is_empty());
    }

    #[test]
    fn a_gap_only_affects_the_server_it_names() {
        let rules = [rule("r1", Metric::Cpu, 90, 2)];
        let mut e = AlertEngine::default();
        feed(&mut e, "s1", &rules, 95.0, 1);
        feed(&mut e, "s2", &rules, 95.0, 1);
        e.gap("s1");
        assert_eq!(feed(&mut e, "s1", &rules, 95.0, 2), 0, "s1 restarted");
        assert_eq!(feed(&mut e, "s2", &rules, 95.0, 2), 1, "s2 unaffected");
        e.gap("nobody"); // no-op
    }
}
