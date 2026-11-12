use super::model::*;
use super::repo;
use super::service::{self, HistoryRecorder, PRUNE_BATCH};
use crate::db;
use crate::ssh::monitor::parse::Metrics;
use rusqlite::{Connection, params};

const MIN: i64 = 60_000;
const NOW: i64 = 1_800_000_000_000;

fn metrics(at: i64, cpu: f64) -> Metrics {
    Metrics {
        cpu_percent: cpu,
        cores: 4,
        load1: 0.5,
        load5: 0.4,
        load15: 0.3,
        mem_total_kib: 8_000_000,
        mem_available_kib: 4_000_000,
        mem_used_percent: 50.0,
        swap_total_kib: 0,
        swap_used_kib: 0,
        disk_total_kib: 100_000_000,
        disk_used_kib: 40_000_000,
        disk_used_percent: 40.0,
        uptime_secs: 1234,
        sampled_at: at,
    }
}

fn add_server(conn: &Connection, id: &str) {
    conn.execute(
        "INSERT INTO servers (id,name,host,port,username,auth_method,created_at,updated_at)
         VALUES (?1,?1,'h',22,'u','password',0,0)",
        params![id],
    )
    .unwrap();
}

fn db_with(servers: &[&str]) -> Connection {
    let conn = db::open_in_memory();
    for s in servers {
        add_server(&conn, s);
    }
    conn
}

fn count(conn: &Connection, server: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM metric_samples WHERE server_id = ?1",
        params![server],
        |r| r.get(0),
    )
    .unwrap()
}

fn total(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM metric_samples", [], |r| r.get(0))
        .unwrap()
}

/// Inserts one sample per minute ending at `NOW`, oldest first.
fn fill(conn: &Connection, server: &str, minutes: i64, cpu: f64) {
    for i in 0..minutes {
        let at = NOW - (minutes - i) * MIN;
        service::record_sample(conn, server, &metrics(at, cpu)).unwrap();
    }
}

// ---- migration -------------------------------------------------------------------------

#[test]
fn migration_creates_tables_indexes_and_keeps_secrets_out() {
    let conn = db::open_in_memory();
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(metric_samples)")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for c in [
        "id",
        "server_id",
        "at",
        "cpu_percent",
        "mem_used_percent",
        "disk_used_percent",
        "load1",
        "load5",
        "load15",
        "uptime_secs",
        "cores",
        "mem_total_kib",
        "disk_total_kib",
    ] {
        assert!(cols.contains(&c.to_string()), "missing {c}");
    }
    assert!(!cols.iter().any(|c| c.contains("host")
        || c.contains("secret")
        || c.contains("password")
        || c.contains("output")));
    let indexes: Vec<String> = conn
        .prepare("PRAGMA index_list(metric_samples)")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(indexes.contains(&"idx_metric_samples_server_at".to_string()));
    assert!(indexes.contains(&"idx_metric_samples_at".to_string()));
}

#[test]
fn upgrading_from_v3_keeps_servers_credentials_alerts_and_host_keys() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    assert_eq!(
        db::migrate_with(&mut conn, &db::MIGRATIONS[..3], 0).unwrap(),
        3
    );
    conn.execute_batch(
        "INSERT INTO servers (id,name,host,port,username,auth_method,credential_ref,created_at,updated_at)
           VALUES ('s','name','h',22,'u','password','vpspilot:s',1,1);
         INSERT INTO known_host_keys (server_id,key_type,fingerprint,trusted_at)
           VALUES ('s','ssh-ed25519','SHA256:abc',1);
         INSERT INTO alert_rules (id,server_id,metric,threshold,consecutive,enabled,created_at,updated_at)
           VALUES ('r','s','cpu',90,3,1,1,1);
         INSERT INTO alert_events (rule_id,server_id,metric,threshold,state,value,at)
           VALUES ('r','s','cpu',90,'firing',95.0,1);",
    )
    .unwrap();

    assert_eq!(db::migrate(&mut conn).unwrap(), 4);
    let one = |sql: &str| -> String { conn.query_row(sql, [], |r| r.get::<_, String>(0)).unwrap() };
    assert_eq!(
        one("SELECT credential_ref FROM servers WHERE id='s'"),
        "vpspilot:s"
    );
    assert_eq!(one("SELECT fingerprint FROM known_host_keys"), "SHA256:abc");
    assert_eq!(one("SELECT id FROM alert_rules"), "r");
    assert_eq!(one("SELECT state FROM alert_events"), "firing");
    assert_eq!(total(&conn), 0);
}

// ---- insertion and constraints ---------------------------------------------------------

#[test]
fn a_sample_is_stored_and_read_back() {
    let conn = db_with(&["a"]);
    service::record_sample(&conn, "a", &metrics(NOW - MIN, 42.5)).unwrap();
    let r = service::query(&conn, "a", 3600, None, NOW).unwrap();
    assert_eq!(r.sample_count, 1);
    assert_eq!(r.points.len(), 1);
    let p = &r.points[0];
    assert_eq!(p.at, NOW - MIN);
    assert_eq!(p.cpu_percent, 42.5);
    assert_eq!(p.mem_used_percent, 50.0);
    assert_eq!(p.disk_used_percent, 40.0);
    assert_eq!((p.load1, p.load5, p.load15), (0.5, 0.4, 0.3));
    assert_eq!(r.first_sample_at, Some(NOW - MIN));
    assert_eq!(r.last_sample_at, Some(NOW - MIN));
}

#[test]
fn invalid_samples_are_refused_and_never_stored() {
    let conn = db_with(&["a"]);
    let bad: Vec<Metrics> = vec![
        Metrics {
            cpu_percent: f64::NAN,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            cpu_percent: 100.1,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            cpu_percent: -0.1,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            mem_used_percent: f64::INFINITY,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            disk_used_percent: 101.0,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            load5: -1.0,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            load15: f64::NAN,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            cores: 0,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            sampled_at: 0,
            ..metrics(NOW, 1.0)
        },
        Metrics {
            uptime_secs: u64::MAX,
            ..metrics(NOW, 1.0)
        },
    ];
    let recorder = HistoryRecorder::default();
    for m in &bad {
        assert!(service::record_sample(&conn, "a", m).is_err(), "{m:?}");
        assert!(!recorder.due(m), "{m:?}");
    }
    assert_eq!(total(&conn), 0);
}

#[test]
fn the_database_itself_rejects_out_of_range_rows() {
    let conn = db_with(&["a"]);
    let bad = conn.execute(
        "INSERT INTO metric_samples (server_id,at,cpu_percent,mem_used_percent,disk_used_percent,
           load1,load5,load15,uptime_secs,cores,mem_total_kib,disk_total_kib)
         VALUES ('a',1,150,1,1,0,0,0,0,1,0,0)",
        [],
    );
    assert!(bad.is_err());
}

#[test]
fn history_cannot_reference_a_missing_server() {
    let conn = db_with(&[]);
    assert!(service::record_sample(&conn, "ghost", &metrics(NOW, 1.0)).is_err());
    assert_eq!(total(&conn), 0);
}

// ---- queries ---------------------------------------------------------------------------

#[test]
fn time_range_filtering_includes_only_the_requested_window() {
    let conn = db_with(&["a"]);
    fill(&conn, "a", 180, 10.0); // three hours of minutes
    let hour = service::query(&conn, "a", 3600, None, NOW).unwrap();
    assert_eq!(hour.sample_count, 60);
    assert!(hour.first_sample_at.unwrap() >= NOW - 3_600_000);
    let all = service::query(&conn, "a", 6 * 3600, None, NOW).unwrap();
    assert_eq!(all.sample_count, 180);
    // A sample after `now` (clock skew) is not part of any range.
    service::record_sample(&conn, "a", &metrics(NOW + 5 * MIN, 99.0)).unwrap();
    assert_eq!(
        service::query(&conn, "a", 6 * 3600, None, NOW)
            .unwrap()
            .sample_count,
        180
    );
}

#[test]
fn long_ranges_are_averaged_into_a_bounded_number_of_points() {
    let conn = db_with(&["a"]);
    for i in 0..1440 {
        // CPU alternates 20 / 40, so any bucket of two or more samples averages near 30.
        let cpu = if i % 2 == 0 { 20.0 } else { 40.0 };
        service::record_sample(&conn, "a", &metrics(NOW - (1440 - i) * MIN, cpu)).unwrap();
    }
    let r = service::query(&conn, "a", 86_400, Some(50), NOW).unwrap();
    assert!(r.points.len() <= 50, "{}", r.points.len());
    assert_eq!(r.sample_count, 1440);
    assert_eq!(r.points.iter().map(|p| p.samples).sum::<u32>(), 1440);
    for p in &r.points {
        assert!((p.cpu_percent - 30.0).abs() <= 10.0);
        assert!(p.samples >= 1);
    }
    // Buckets are returned oldest first.
    assert!(r.points.windows(2).all(|w| w[0].at < w[1].at));
}

#[test]
fn a_point_budget_outside_the_allowed_range_is_clamped() {
    let conn = db_with(&["a"]);
    fill(&conn, "a", 100, 10.0);
    let huge = service::query(&conn, "a", 86_400, Some(1_000_000), NOW).unwrap();
    assert!(huge.points.len() <= MAX_POINTS as usize);
    let tiny = service::query(&conn, "a", 86_400, Some(1), NOW).unwrap();
    assert!(tiny.points.len() <= MIN_POINTS as usize);
}

#[test]
fn unsupported_ranges_and_unknown_servers_are_rejected() {
    let conn = db_with(&["a"]);
    assert!(service::query(&conn, "a", 0, None, NOW).is_err());
    assert!(service::query(&conn, "a", MIN_RANGE_SECS - 1, None, NOW).is_err());
    assert!(service::query(&conn, "a", MAX_RANGE_SECS + 1, None, NOW).is_err());
    assert!(service::query(&conn, "a", MAX_RANGE_SECS, None, NOW).is_ok());
    assert!(service::query(&conn, "nope", 3600, None, NOW).is_err());
}

#[test]
fn missing_data_is_reported_as_empty_not_as_zero() {
    let conn = db_with(&["a"]);
    let r = service::query(&conn, "a", 3600, None, NOW).unwrap();
    assert!(r.points.is_empty());
    assert_eq!(r.sample_count, 0);
    assert_eq!((r.first_sample_at, r.last_sample_at), (None, None));
}

#[test]
fn a_gap_in_collection_leaves_a_gap_in_the_points() {
    let conn = db_with(&["a"]);
    for i in (0..10).chain(40..50) {
        service::record_sample(&conn, "a", &metrics(NOW - 60 * MIN + i * MIN, 10.0)).unwrap();
    }
    let r = service::query(&conn, "a", 3600, None, NOW).unwrap();
    let biggest = r
        .points
        .windows(2)
        .map(|w| w[1].at - w[0].at)
        .max()
        .unwrap();
    assert!(biggest >= 30 * MIN, "no gap found: {biggest}");
}

#[test]
fn the_range_query_uses_the_server_time_index() {
    let conn = db_with(&["a"]);
    let plan: String = conn
        .prepare(
            "EXPLAIN QUERY PLAN SELECT COUNT(*) FROM metric_samples
              WHERE server_id = 'a' AND at >= 1 AND at < 2",
        )
        .unwrap()
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(plan.contains("idx_metric_samples_server_at"), "{plan}");
}

// ---- multiple servers and deletion -----------------------------------------------------

#[test]
fn servers_never_see_each_others_samples() {
    let conn = db_with(&["a", "b"]);
    fill(&conn, "a", 30, 10.0);
    fill(&conn, "b", 20, 90.0);
    let a = service::query(&conn, "a", 3600, None, NOW).unwrap();
    let b = service::query(&conn, "b", 3600, None, NOW).unwrap();
    assert_eq!((a.sample_count, b.sample_count), (30, 20));
    assert!(a.points.iter().all(|p| p.cpu_percent == 10.0));
    assert!(b.points.iter().all(|p| p.cpu_percent == 90.0));
    assert_eq!((count(&conn, "a"), count(&conn, "b")), (30, 20));
}

#[test]
fn deleting_a_server_deletes_only_its_history() {
    let conn = db_with(&["a", "b"]);
    fill(&conn, "a", 30, 10.0);
    fill(&conn, "b", 20, 90.0);
    conn.execute("DELETE FROM servers WHERE id = 'a'", [])
        .unwrap();
    assert_eq!(count(&conn, "a"), 0);
    assert_eq!(count(&conn, "b"), 20);
    assert!(service::query(&conn, "a", 3600, None, NOW).is_err());
}

// ---- retention -------------------------------------------------------------------------

#[test]
fn retention_defaults_to_seven_days_and_can_be_changed() {
    let conn = db_with(&[]);
    assert_eq!(
        service::retention_days(&conn).unwrap(),
        DEFAULT_RETENTION_DAYS
    );
    assert_eq!(service::set_retention_days(&conn, 30).unwrap(), 30);
    assert_eq!(service::retention_days(&conn).unwrap(), 30);
    assert_eq!(service::set_retention_days(&conn, 30).unwrap(), 30); // idempotent upsert
}

#[test]
fn invalid_retention_is_rejected_and_a_corrupt_setting_falls_back() {
    let conn = db_with(&[]);
    assert!(service::set_retention_days(&conn, 0).is_err());
    assert!(service::set_retention_days(&conn, MAX_RETENTION_DAYS + 1).is_err());
    assert_eq!(
        service::retention_days(&conn).unwrap(),
        DEFAULT_RETENTION_DAYS
    );
    for junk in ["abc", "-4", "0", "9999", ""] {
        repo::set_setting(&conn, "history_retention_days", junk).unwrap();
        assert_eq!(
            service::retention_days(&conn).unwrap(),
            DEFAULT_RETENTION_DAYS,
            "{junk}"
        );
    }
}

#[test]
fn cleanup_removes_only_samples_older_than_the_retention_period() {
    let conn = db_with(&["a", "b"]);
    let day = 86_400_000;
    for server in ["a", "b"] {
        for age_days in [10, 8, 6, 1] {
            service::record_sample(&conn, server, &metrics(NOW - age_days * day, 5.0)).unwrap();
        }
    }
    assert_eq!(service::prune_batch(&conn, NOW).unwrap(), 4); // default 7 days: 10d and 8d, both servers
    assert_eq!((count(&conn, "a"), count(&conn, "b")), (2, 2));

    service::set_retention_days(&conn, 3).unwrap();
    assert_eq!(service::prune_batch(&conn, NOW).unwrap(), 2); // the 6-day-old pair
    assert_eq!(total(&conn), 2);
    assert_eq!(service::prune_batch(&conn, NOW).unwrap(), 0); // nothing left to remove
}

#[test]
fn cleanup_works_in_bounded_batches() {
    let conn = db_with(&["a"]);
    let tx = conn.unchecked_transaction().unwrap();
    let total_rows = i64::from(PRUNE_BATCH) * 2 + 7;
    for i in 0..total_rows {
        service::record_sample(&tx, "a", &metrics(1_000 + i, 5.0)).unwrap(); // ancient
    }
    service::record_sample(&tx, "a", &metrics(NOW - MIN, 5.0)).unwrap(); // recent
    tx.commit().unwrap();

    let mut removed = Vec::new();
    loop {
        let n = service::prune_batch(&conn, NOW).unwrap();
        removed.push(n);
        if n < PRUNE_BATCH as usize {
            break;
        }
    }
    assert_eq!(removed, vec![PRUNE_BATCH as usize, PRUNE_BATCH as usize, 7]);
    assert_eq!(count(&conn, "a"), 1);
}

// ---- recorder policy (what the monitor loop stores) ------------------------------------

/// Mirrors the forwarder in `commands::monitor`: only valid samples that are due are stored.
fn drive(conn: &Connection, rec: &mut HistoryRecorder, server: &str, m: &Metrics) -> bool {
    if !rec.due(m) {
        return false;
    }
    match service::record_sample(conn, server, m) {
        Ok(()) => {
            rec.stored(m.sampled_at);
            true
        }
        Err(_) => {
            rec.failed();
            false
        }
    }
}

#[test]
fn polling_every_five_seconds_stores_one_sample_a_minute() {
    let conn = db_with(&["a"]);
    let mut rec = HistoryRecorder::default();
    let start = NOW - 10 * MIN;
    let stored: usize =
        (0..120) // ten minutes at 5 s
            .filter(|i| drive(&conn, &mut rec, "a", &metrics(start + i * 5_000, 10.0)))
            .count();
    assert_eq!(stored, 10);
    assert_eq!(count(&conn, "a"), 10);
}

#[test]
fn a_restarted_monitor_stores_its_first_sample_straight_away() {
    let conn = db_with(&["a"]);
    let mut first = HistoryRecorder::default();
    assert!(drive(&conn, &mut first, "a", &metrics(NOW - 100_000, 1.0)));
    // Monitoring stopped; a new monitor (a new recorder) starts 10 s later.
    let mut second = HistoryRecorder::default();
    assert!(drive(&conn, &mut second, "a", &metrics(NOW - 90_000, 1.0)));
    assert_eq!(count(&conn, "a"), 2);
}

#[test]
fn a_clock_that_jumps_backwards_does_not_silence_history() {
    let conn = db_with(&["a"]);
    let mut rec = HistoryRecorder::default();
    assert!(drive(&conn, &mut rec, "a", &metrics(NOW, 1.0)));
    assert!(!drive(&conn, &mut rec, "a", &metrics(NOW + 10_000, 1.0)));
    assert!(drive(&conn, &mut rec, "a", &metrics(NOW - 3_600_000, 1.0)));
}

// ---- database failures -----------------------------------------------------------------

#[test]
fn a_database_failure_is_an_error_not_a_crash_and_storage_recovers() {
    let conn = db_with(&["a"]);
    let mut rec = HistoryRecorder::default();
    conn.execute_batch(
        "CREATE TRIGGER fail_samples BEFORE INSERT ON metric_samples
         BEGIN SELECT RAISE(ABORT, 'disk full'); END;",
    )
    .unwrap();

    let t = NOW - 10 * MIN;
    assert!(!drive(&conn, &mut rec, "a", &metrics(t, 1.0)));
    // The failed sample did not use up the minute, so the next one is tried immediately.
    assert!(rec.due(&metrics(t + 5_000, 1.0)));
    assert!(!rec.failed(), "only the first failure is reported");
    assert_eq!(total(&conn), 0);

    conn.execute_batch("DROP TRIGGER fail_samples").unwrap();
    assert!(drive(&conn, &mut rec, "a", &metrics(t + 10_000, 1.0)));
    assert!(
        !rec.stored(t + 10_000),
        "already recovered by the first success"
    );
    assert_eq!(total(&conn), 1);
}

#[test]
fn the_first_failure_of_a_run_is_reported_once() {
    let mut rec = HistoryRecorder::default();
    assert!(rec.failed());
    assert!(!rec.failed());
    assert!(rec.stored(1), "recovery is reported once after failures");
    assert!(!rec.stored(2));
    assert!(rec.failed(), "a new run of failures is reported again");
}

#[test]
fn a_failed_cleanup_leaves_the_data_intact() {
    let conn = db_with(&["a"]);
    service::record_sample(&conn, "a", &metrics(1_000, 1.0)).unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_delete BEFORE DELETE ON metric_samples
         BEGIN SELECT RAISE(ABORT, 'locked'); END;",
    )
    .unwrap();
    assert!(service::prune_batch(&conn, NOW).is_err());
    assert_eq!(total(&conn), 1);
}

// ---- restart ---------------------------------------------------------------------------

#[test]
fn history_survives_closing_and_reopening_the_database_file() {
    let dir = std::env::temp_dir().join(format!("vpspilot-history-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    {
        let conn = db::open(&path).unwrap();
        add_server(&conn, "a");
        fill(&conn, "a", 30, 33.0);
        service::set_retention_days(&conn, 14).unwrap();
    } // closed, as when the app exits
    {
        let conn = db::open(&path).unwrap(); // a second launch: migrations do not reapply
        let r = service::query(&conn, "a", 3600, None, NOW).unwrap();
        assert_eq!(r.sample_count, 30);
        assert!(r.points.iter().all(|p| p.cpu_percent == 33.0));
        assert_eq!(r.retention_days, 14);
        assert_eq!(
            db::current_version(&conn).unwrap(),
            db::MIGRATIONS.last().unwrap().version
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
