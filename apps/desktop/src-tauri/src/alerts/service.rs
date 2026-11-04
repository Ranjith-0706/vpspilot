//! Alert use cases: rule CRUD, and evaluating monitoring samples into persisted events.

use super::engine::{AlertEngine, Transition};
use super::model::{AlertEvent, AlertRule, MAX_RULES, RuleInput};
use super::repo;
use crate::error::AppError;
use crate::servers::repo as servers_repo;
use crate::ssh::monitor::parse::Metrics;
use rusqlite::Connection;

pub const DEFAULT_EVENT_LIMIT: i64 = 200;

fn check_server(conn: &Connection, input: &RuleInput) -> Result<(), AppError> {
    if let Some(id) = &input.server_id
        && servers_repo::get(conn, id)?.is_none()
    {
        return Err(AppError::validation(
            "serverId",
            "That server no longer exists.",
        ));
    }
    Ok(())
}

pub fn list_rules(conn: &Connection) -> Result<Vec<AlertRule>, AppError> {
    repo::list_rules(conn)
}

pub fn add_rule(conn: &Connection, input: &RuleInput, now: i64) -> Result<AlertRule, AppError> {
    input.validate()?;
    check_server(conn, input)?;
    if repo::count_rules(conn)? >= MAX_RULES {
        return Err(AppError::validation(
            "metric",
            format!("You can have at most {MAX_RULES} alert rules."),
        ));
    }
    repo::insert_rule(conn, &uuid::Uuid::new_v4().to_string(), input, now)
}

pub fn update_rule(
    conn: &Connection,
    id: &str,
    input: &RuleInput,
    now: i64,
) -> Result<AlertRule, AppError> {
    input.validate()?;
    check_server(conn, input)?;
    if !repo::update_rule(conn, id, input, now)? {
        return Err(AppError::not_found("Alert rule"));
    }
    repo::get_rule(conn, id)?.ok_or_else(|| AppError::not_found("Alert rule"))
}

pub fn delete_rule(conn: &Connection, id: &str) -> Result<(), AppError> {
    if repo::delete_rule(conn, id)? {
        Ok(())
    } else {
        Err(AppError::not_found("Alert rule"))
    }
}

pub fn list_events(conn: &Connection, limit: Option<u32>) -> Result<Vec<AlertEvent>, AppError> {
    let limit = limit.map_or(DEFAULT_EVENT_LIMIT, i64::from);
    repo::list_events(conn, limit.clamp(1, repo::MAX_EVENTS))
}

fn persist(conn: &Connection, transitions: Vec<Transition>) -> Result<Vec<AlertEvent>, AppError> {
    transitions
        .iter()
        .map(|t| repo::insert_event(conn, t))
        .collect()
}

/// Evaluates one sample and records any alerts that fired or resolved because of it.
pub fn process_sample(
    conn: &Connection,
    engine: &mut AlertEngine,
    server_id: &str,
    metrics: &Metrics,
    now: i64,
) -> Result<Vec<AlertEvent>, AppError> {
    let rules = repo::enabled_rules_for(conn, server_id)?;
    persist(conn, engine.evaluate(server_id, &rules, metrics, now))
}

/// Monitoring of a server ended: resolve its alerts. A server that was just deleted has no
/// history to write to, so its alerts are simply dropped.
pub fn close_server(
    conn: &Connection,
    engine: &mut AlertEngine,
    server_id: &str,
    now: i64,
) -> Result<Vec<AlertEvent>, AppError> {
    let transitions = engine.close_server(server_id, now);
    if transitions.is_empty() || servers_repo::get(conn, server_id)?.is_none() {
        return Ok(Vec::new());
    }
    persist(conn, transitions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::model::{AlertState, Metric};
    use crate::db::open_in_memory;
    use crate::servers::model::ValidatedServer;

    fn server(conn: &Connection, id: &str) {
        let v = ValidatedServer {
            name: id.into(),
            host: "h".into(),
            port: 22,
            username: "u".into(),
            auth_method: crate::servers::model::AuthMethod::Password,
            description: None,
        };
        servers_repo::insert(conn, id, &v, 0).unwrap();
    }

    fn input(server_id: Option<&str>, metric: Metric, threshold: u32, n: u32) -> RuleInput {
        RuleInput {
            server_id: server_id.map(String::from),
            metric,
            threshold,
            consecutive: n,
            enabled: true,
        }
    }

    fn sample(cpu: f64) -> Metrics {
        Metrics {
            cpu_percent: cpu,
            cores: 1,
            load1: 0.0,
            load5: 0.0,
            load15: 0.0,
            mem_total_kib: 1,
            mem_available_kib: 1,
            mem_used_percent: 0.0,
            swap_total_kib: 0,
            swap_used_kib: 0,
            disk_total_kib: 1,
            disk_used_kib: 1,
            disk_used_percent: 0.0,
            uptime_secs: 1,
            sampled_at: 0,
        }
    }

    #[test]
    fn rule_crud_round_trip() {
        let conn = open_in_memory();
        let r = add_rule(&conn, &input(None, Metric::Cpu, 90, 3), 10).unwrap();
        assert_eq!((r.threshold, r.consecutive, r.enabled), (90, 3, true));
        assert_eq!(r.created_at, 10);

        let u = update_rule(&conn, &r.id, &input(None, Metric::Disk, 95, 1), 20).unwrap();
        assert_eq!(u.metric, Metric::Disk);
        assert_eq!(u.updated_at, 20);
        assert_eq!(u.created_at, 10);
        assert_eq!(list_rules(&conn).unwrap().len(), 1);

        delete_rule(&conn, &r.id).unwrap();
        assert!(list_rules(&conn).unwrap().is_empty());
        assert!(delete_rule(&conn, &r.id).is_err());
        assert!(update_rule(&conn, &r.id, &input(None, Metric::Cpu, 50, 1), 30).is_err());
    }

    #[test]
    fn rejects_invalid_input_and_unknown_server() {
        let conn = open_in_memory();
        assert!(add_rule(&conn, &input(None, Metric::Cpu, 0, 1), 0).is_err());
        assert!(add_rule(&conn, &input(Some("nope"), Metric::Cpu, 50, 1), 0).is_err());
        assert!(list_rules(&conn).unwrap().is_empty());
    }

    #[test]
    fn caps_the_number_of_rules() {
        let conn = open_in_memory();
        for _ in 0..MAX_RULES {
            add_rule(&conn, &input(None, Metric::Cpu, 50, 1), 0).unwrap();
        }
        assert!(add_rule(&conn, &input(None, Metric::Cpu, 50, 1), 0).is_err());
    }

    #[test]
    fn deleting_a_server_removes_its_rules_and_events() {
        let conn = open_in_memory();
        server(&conn, "s1");
        add_rule(&conn, &input(Some("s1"), Metric::Cpu, 50, 1), 0).unwrap();
        add_rule(&conn, &input(None, Metric::Cpu, 50, 1), 0).unwrap();
        let mut e = AlertEngine::default();
        process_sample(&conn, &mut e, "s1", &sample(99.0), 1).unwrap();
        assert!(!list_events(&conn, None).unwrap().is_empty());

        servers_repo::delete(&conn, "s1").unwrap();
        assert_eq!(list_rules(&conn).unwrap().len(), 1);
        assert!(list_events(&conn, None).unwrap().is_empty());
    }

    #[test]
    fn sample_fires_persists_and_resolves() {
        let conn = open_in_memory();
        server(&conn, "s1");
        add_rule(&conn, &input(None, Metric::Cpu, 90, 2), 0).unwrap();
        let mut e = AlertEngine::default();

        assert!(
            process_sample(&conn, &mut e, "s1", &sample(95.0), 1)
                .unwrap()
                .is_empty()
        );
        let fired = process_sample(&conn, &mut e, "s1", &sample(95.0), 2).unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].state, AlertState::Firing);
        assert!(fired[0].id > 0);

        let done = process_sample(&conn, &mut e, "s1", &sample(10.0), 3).unwrap();
        assert_eq!(done[0].state, AlertState::Resolved);

        let history = list_events(&conn, None).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].state, AlertState::Resolved, "newest first");
    }

    #[test]
    fn server_scoped_rule_ignores_other_servers() {
        let conn = open_in_memory();
        server(&conn, "s1");
        server(&conn, "s2");
        add_rule(&conn, &input(Some("s1"), Metric::Cpu, 50, 1), 0).unwrap();
        let mut e = AlertEngine::default();
        assert!(
            process_sample(&conn, &mut e, "s2", &sample(99.0), 1)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            process_sample(&conn, &mut e, "s1", &sample(99.0), 1)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn disabled_rule_does_not_fire_and_ends_an_active_alert() {
        let conn = open_in_memory();
        server(&conn, "s1");
        let r = add_rule(&conn, &input(None, Metric::Cpu, 50, 1), 0).unwrap();
        let mut e = AlertEngine::default();
        assert_eq!(
            process_sample(&conn, &mut e, "s1", &sample(99.0), 1)
                .unwrap()
                .len(),
            1
        );

        let mut off = input(None, Metric::Cpu, 50, 1);
        off.enabled = false;
        update_rule(&conn, &r.id, &off, 2).unwrap();
        let ended = process_sample(&conn, &mut e, "s1", &sample(99.0), 3).unwrap();
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].state, AlertState::Resolved);
        assert_eq!(ended[0].value, None);
        assert!(
            process_sample(&conn, &mut e, "s1", &sample(99.0), 4)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn history_survives_rule_deletion() {
        let conn = open_in_memory();
        server(&conn, "s1");
        let r = add_rule(&conn, &input(None, Metric::Cpu, 50, 1), 0).unwrap();
        let mut e = AlertEngine::default();
        process_sample(&conn, &mut e, "s1", &sample(99.0), 1).unwrap();
        delete_rule(&conn, &r.id).unwrap();
        let history = list_events(&conn, None).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].threshold, 50);
    }

    #[test]
    fn close_server_resolves_and_persists() {
        let conn = open_in_memory();
        server(&conn, "s1");
        add_rule(&conn, &input(None, Metric::Cpu, 50, 1), 0).unwrap();
        let mut e = AlertEngine::default();
        process_sample(&conn, &mut e, "s1", &sample(99.0), 1).unwrap();
        let closed = close_server(&conn, &mut e, "s1", 5).unwrap();
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].value, None);
        assert!(e.active().is_empty());
    }

    #[test]
    fn close_server_after_the_server_was_deleted_is_harmless() {
        let conn = open_in_memory();
        server(&conn, "s1");
        add_rule(&conn, &input(None, Metric::Cpu, 50, 1), 0).unwrap();
        let mut e = AlertEngine::default();
        process_sample(&conn, &mut e, "s1", &sample(99.0), 1).unwrap();
        servers_repo::delete(&conn, "s1").unwrap();
        assert!(close_server(&conn, &mut e, "s1", 5).unwrap().is_empty());
        assert!(e.active().is_empty());
    }

    #[test]
    fn history_is_trimmed() {
        let conn = open_in_memory();
        server(&conn, "s1");
        for i in 0..(repo::MAX_EVENTS + 25) {
            repo::insert_event(
                &conn,
                &Transition {
                    rule_id: "r".into(),
                    server_id: "s1".into(),
                    metric: Metric::Cpu,
                    threshold: 50,
                    state: AlertState::Firing,
                    value: Some(1.0),
                    at: i,
                },
            )
            .unwrap();
        }
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM alert_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, repo::MAX_EVENTS);
        // The newest survive.
        let newest = list_events(&conn, Some(1)).unwrap();
        assert_eq!(newest[0].at, repo::MAX_EVENTS + 24);
    }

    #[test]
    fn event_limit_is_clamped() {
        let conn = open_in_memory();
        assert!(list_events(&conn, Some(0)).unwrap().is_empty());
        assert!(list_events(&conn, Some(u32::MAX)).unwrap().is_empty());
    }
}
