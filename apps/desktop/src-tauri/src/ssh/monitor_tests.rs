//! End-to-end monitor tests against the in-process SSH server (see client_tests.rs).

use super::client::{AuthSecret, TestParams};
use super::client_tests::{
    Running, fingerprint, new_key, params, start_server, start_server_with_exec, stored,
};
use super::monitor::parse::COMMAND;
use super::monitor::{MonitorConfig, MonitorEvent, MonitorHandle, MonitorState, spawn};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

const PASSWORD: &str = "correct-pw";

fn fast() -> MonitorConfig {
    MonitorConfig {
        interval: Duration::from_millis(40),
        retry_base: Duration::from_millis(20),
        retry_max: Duration::from_millis(80),
        poll_timeout: Duration::from_secs(5),
    }
}

fn trusted(s: &Running) -> TestParams {
    params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&s.host_key)),
    )
}

type Events = mpsc::Receiver<MonitorEvent>;

fn start(p: TestParams) -> (MonitorHandle, Events) {
    let (tx, rx) = mpsc::channel(256);
    (spawn(p, fast(), tx).expect("monitor starts"), rx)
}

async fn next(rx: &mut Events) -> Option<MonitorEvent> {
    timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for a monitor event")
}

fn is_status(e: &MonitorEvent, state: MonitorState) -> bool {
    matches!(e, MonitorEvent::Status { state: s, .. } if *s == state)
}

#[tokio::test]
async fn polls_repeatedly_over_one_login_running_only_the_fixed_command() {
    let s = start_server(None).await;
    let (handle, mut rx) = start(trusted(&s));

    let mut seen = Vec::new();
    let mut samples = 0;
    while samples < 3 {
        let e = next(&mut rx).await.expect("event");
        if let MonitorEvent::Sample { metrics } = &e {
            samples += 1;
            assert_eq!(metrics.cpu_percent, 60.0);
            assert_eq!(metrics.mem_used_percent, 50.0);
            assert_eq!(metrics.cores, 2);
        }
        seen.push(e);
    }
    // connecting -> online comes before the first sample
    assert!(is_status(&seen[0], MonitorState::Connecting));
    assert!(is_status(&seen[1], MonitorState::Online));
    assert!(matches!(seen[2], MonitorEvent::Sample { .. }));

    handle.stop("test over");

    let log = s.log.lock().unwrap().clone();
    let execs: Vec<_> = log.iter().filter(|l| l.starts_with("exec:")).collect();
    assert!(execs.len() >= 3);
    for e in execs {
        assert_eq!(
            e,
            &format!("exec:{COMMAND}"),
            "only the fixed command may ever run"
        );
    }
    assert_eq!(
        s.attempts.load(Ordering::SeqCst),
        1,
        "one persistent login, not one per poll"
    );
    assert!(
        !log.iter().any(|l| l.starts_with("pty")),
        "no terminal is allocated"
    );
}

#[tokio::test]
async fn stop_ends_the_task_with_a_stopped_event_and_no_more_samples() {
    let s = start_server(None).await;
    let (handle, mut rx) = start(trusted(&s));
    loop {
        if matches!(next(&mut rx).await, Some(MonitorEvent::Sample { .. })) {
            break;
        }
    }
    handle.stop("Server was removed.");

    let mut stopped = None;
    while let Some(e) = next(&mut rx).await {
        if let MonitorEvent::Status {
            state: MonitorState::Stopped,
            message,
        } = e
        {
            stopped = Some(message);
        }
    }
    assert_eq!(stopped, Some(Some("Server was removed.".to_string())));
    // the channel closed, so the task has ended and cannot produce more samples
}

#[tokio::test]
async fn dropping_the_handle_stops_monitoring() {
    let s = start_server(None).await;
    let (handle, mut rx) = start(trusted(&s));
    loop {
        if matches!(next(&mut rx).await, Some(MonitorEvent::Sample { .. })) {
            break;
        }
    }
    drop(handle);
    while next(&mut rx).await.is_some() {} // ends once the task exits
}

#[tokio::test]
async fn refuses_to_start_for_an_untrusted_host_without_any_network_access() {
    let s = start_server(None).await;
    let (tx, _rx) = mpsc::channel(8);
    let p = params(s.port, AuthSecret::Password(PASSWORD.into()), None);
    let err = spawn(p, fast(), tx).err().expect("must refuse");
    assert!(err.contains("not trusted"), "{err}");
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn refuses_to_start_without_a_credential() {
    let s = start_server(None).await;
    let (tx, _rx) = mpsc::channel(8);
    let p = params(s.port, AuthSecret::None, stored(&fingerprint(&s.host_key)));
    assert!(
        spawn(p, fast(), tx)
            .err()
            .unwrap()
            .contains("No credential")
    );
}

#[tokio::test]
async fn wrong_password_fails_once_and_is_never_retried() {
    let s = start_server(None).await;
    let p = params(
        s.port,
        AuthSecret::Password("wrong".into()),
        stored(&fingerprint(&s.host_key)),
    );
    let (_handle, mut rx) = start(p);

    let mut failed = false;
    while let Some(e) = next(&mut rx).await {
        if is_status(&e, MonitorState::Failed) {
            failed = true;
        }
        assert!(
            !is_status(&e, MonitorState::Retrying),
            "must not retry a rejected credential"
        );
    }
    assert!(failed);
    tokio::time::sleep(Duration::from_millis(300)).await; // a retry would show up by now
    assert_eq!(
        s.attempts.load(Ordering::SeqCst),
        1,
        "exactly one login attempt"
    );
}

#[tokio::test]
async fn changed_host_key_fails_without_credentials_and_is_never_retried() {
    let s = start_server(None).await;
    let p = params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&new_key())),
    );
    let (_handle, mut rx) = start(p);
    let mut failed = false;
    while let Some(e) = next(&mut rx).await {
        if let MonitorEvent::Status {
            state: MonitorState::Failed,
            message,
        } = &e
        {
            failed = true;
            assert!(message.as_deref().unwrap().contains("does not match"));
        }
        assert!(!is_status(&e, MonitorState::Retrying));
    }
    assert!(failed);
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn unreachable_server_is_retried_with_backoff_until_stopped() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // nothing is listening
    let p = params(
        port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&new_key())),
    );
    let (handle, mut rx) = start(p);

    let mut retries = 0;
    while retries < 3 {
        if let Some(MonitorEvent::Status {
            state: MonitorState::Retrying,
            message,
        }) = next(&mut rx).await
        {
            assert!(message.unwrap().contains("Could not reach"));
            retries += 1;
        }
    }
    handle.stop("");
    while next(&mut rx).await.is_some() {}
}

#[tokio::test]
async fn unparseable_output_is_reported_safely_and_retried_without_echoing_it() {
    let s = start_server_with_exec(None, Some("SECRET-REMOTE-TEXT\n".into())).await;
    let (handle, mut rx) = start(trusted(&s));
    loop {
        match next(&mut rx).await.expect("event") {
            MonitorEvent::Status {
                state: MonitorState::Retrying,
                message,
            } => {
                let m = message.unwrap();
                assert!(m.contains("unexpected output"), "{m}");
                assert!(!m.contains("SECRET"));
                break;
            }
            MonitorEvent::Sample { .. } => panic!("garbage must not produce a sample"),
            _ => {}
        }
    }
    handle.stop("");
}

#[tokio::test]
async fn events_serialize_with_camel_case_and_a_type_tag() {
    let s = start_server(None).await;
    let (handle, mut rx) = start(trusted(&s));
    let json = loop {
        if let Some(e @ MonitorEvent::Sample { .. }) = next(&mut rx).await {
            break serde_json::to_string(&e).unwrap();
        }
    };
    handle.stop("");
    assert!(
        json.starts_with(r#"{"type":"sample","metrics":{"cpuPercent":60.0"#),
        "{json}"
    );
    for key in [
        "memUsedPercent",
        "diskUsedPercent",
        "uptimeSecs",
        "sampledAt",
        "load1",
    ] {
        assert!(json.contains(key), "{key} in {json}");
    }
    let status = serde_json::to_string(&MonitorEvent::Status {
        state: MonitorState::Retrying,
        message: None,
    })
    .unwrap();
    assert_eq!(status, r#"{"type":"status","state":"retrying"}"#);
}
