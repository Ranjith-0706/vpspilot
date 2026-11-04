//! End-to-end Docker listing tests against the in-process SSH server (see client_tests.rs).

use super::client::{AuthSecret, TestParams};
use super::client_tests::{
    DockerReply, Running, fingerprint, new_key, params, start_server_with_docker,
    start_server_with_logs, stored,
};
use super::docker::logs::{self, LogsRequest, LogsResult};
use super::docker::parse::{COMMAND, ContainerState, Health, MAX_CONTAINERS};
use super::docker::{DockerOutcome, DockerResult, list_containers};
use std::sync::atomic::Ordering;
use tokio::net::TcpListener;

const PASSWORD: &str = "correct-pw";

fn trusted(s: &Running) -> TestParams {
    params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&s.host_key)),
    )
}

async fn list(reply: DockerReply) -> (DockerResult, Running) {
    let s = start_server_with_docker(reply).await;
    let result = list_containers(trusted(&s)).await.expect("reaches docker");
    (result, s)
}

fn execs(s: &Running) -> Vec<String> {
    s.log
        .lock()
        .unwrap()
        .iter()
        .filter(|l| l.starts_with("exec:"))
        .cloned()
        .collect()
}

#[tokio::test]
async fn lists_containers_with_one_login_and_only_the_fixed_command() {
    let (result, s) = list(DockerReply::default()).await;

    assert_eq!(result.outcome, DockerOutcome::Ok);
    assert_eq!(result.message, None);
    assert!(!result.truncated);
    assert!(result.fetched_at > 0);

    let names: Vec<_> = result.containers.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["web", "db", "old-job"]);
    assert_eq!(result.containers[0].state, ContainerState::Running);
    assert_eq!(result.containers[0].health, Some(Health::Healthy));
    assert_eq!(result.containers[1].health, Some(Health::Unhealthy));
    assert_eq!(result.containers[2].state, ContainerState::Exited);

    assert_eq!(execs(&s), vec![format!("exec:{COMMAND}")]);
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_server_with_no_containers_is_a_normal_empty_result() {
    let (result, _s) = list(DockerReply::ok("")).await;
    assert_eq!(result.outcome, DockerOutcome::Ok);
    assert!(result.containers.is_empty());
}

#[tokio::test]
async fn docker_not_installed_is_reported() {
    let (result, _s) = list(DockerReply::failing(127, "")).await;
    assert_eq!(result.outcome, DockerOutcome::NotInstalled);
    assert!(result.containers.is_empty());
    assert!(result.message.unwrap().contains("not installed"));
}

#[tokio::test]
async fn permission_denied_is_reported_without_echoing_remote_text() {
    let (result, _s) = list(DockerReply::failing(
        1,
        "permission denied while trying to connect to the Docker daemon socket at unix:///var/run/docker.sock SECRET-DETAIL",
    ))
    .await;
    assert_eq!(result.outcome, DockerOutcome::PermissionDenied);
    let message = result.message.unwrap();
    assert!(message.contains("docker group"));
    assert!(message.contains("never uses sudo"));
    assert!(!message.contains("SECRET") && !message.contains("/var/run"));
}

#[tokio::test]
async fn a_stopped_daemon_is_reported() {
    let (result, _s) = list(DockerReply::failing(
        1,
        "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?",
    ))
    .await;
    assert_eq!(result.outcome, DockerOutcome::DaemonUnavailable);
    assert!(!result.message.unwrap().contains("/var/run"));
}

#[tokio::test]
async fn an_unknown_docker_error_gets_a_generic_message() {
    let (result, _s) = list(DockerReply::failing(2, "boom SECRET-DETAIL")).await;
    assert_eq!(result.outcome, DockerOutcome::Error);
    let message = result.message.unwrap();
    assert!(message.contains("exit status 2"));
    assert!(!message.contains("SECRET"));
}

#[tokio::test]
async fn unreadable_output_is_an_error_that_does_not_echo_it() {
    let (result, _s) = list(DockerReply::ok("garbage SECRET-DETAIL\n")).await;
    assert_eq!(result.outcome, DockerOutcome::Error);
    assert!(result.containers.is_empty());
    assert!(!result.message.unwrap().contains("SECRET"));
}

#[tokio::test]
async fn very_many_containers_are_cut_and_flagged() {
    let line = r#"{"ID":"abc123","Names":"c","State":"running","Status":"Up 1 hour"}"#;
    let many = std::iter::repeat_n(line, MAX_CONTAINERS + 100)
        .collect::<Vec<_>>()
        .join("\n");
    let (result, _s) = list(DockerReply::ok(&many)).await;
    assert_eq!(result.outcome, DockerOutcome::Ok);
    assert_eq!(result.containers.len(), MAX_CONTAINERS);
    assert!(result.truncated);
}

#[tokio::test]
async fn untrusted_host_key_refuses_without_touching_the_network() {
    let s = start_server_with_docker(DockerReply::default()).await;
    let p = params(s.port, AuthSecret::Password(PASSWORD.into()), None);
    let err = list_containers(p).await.unwrap_err();
    assert!(err.contains("not trusted"));
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
    assert!(execs(&s).is_empty());
}

#[tokio::test]
async fn missing_credential_refuses_without_touching_the_network() {
    let s = start_server_with_docker(DockerReply::default()).await;
    let p = params(s.port, AuthSecret::None, stored(&fingerprint(&s.host_key)));
    let err = list_containers(p).await.unwrap_err();
    assert!(err.contains("No credential"));
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn wrong_password_fails_after_one_attempt_and_runs_nothing() {
    let s = start_server_with_docker(DockerReply::default()).await;
    let p = params(
        s.port,
        AuthSecret::Password("wrong".into()),
        stored(&fingerprint(&s.host_key)),
    );
    let err = list_containers(p).await.unwrap_err();
    assert!(!err.contains("wrong"), "the password must not be echoed");
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1, "no retry");
    assert!(execs(&s).is_empty());
}

#[tokio::test]
async fn changed_host_key_fails_before_any_credential_is_sent() {
    let s = start_server_with_docker(DockerReply::default()).await;
    let p = params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&new_key())),
    );
    let err = list_containers(p).await.unwrap_err();
    assert!(err.to_lowercase().contains("host key"), "{err}");
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
    assert!(execs(&s).is_empty());
}

#[tokio::test]
async fn an_unreachable_server_is_an_error() {
    // Bind to get a free port, then close it so nothing is listening.
    let port = {
        let l = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        l.local_addr().unwrap().port()
    };
    let p = params(
        port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&new_key())),
    );
    assert!(list_containers(p).await.is_err());
}

#[tokio::test]
async fn result_serializes_with_the_shape_the_frontend_expects() {
    let (result, _s) = list(DockerReply::default()).await;
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["outcome"], "ok");
    assert!(json.get("message").is_none());
    assert_eq!(json["truncated"], false);
    assert!(json["fetchedAt"].is_i64());
    let web = &json["containers"][0];
    assert_eq!(web["name"], "web");
    assert_eq!(web["state"], "running");
    assert_eq!(web["health"], "healthy");
    assert!(json["containers"][2].get("health").is_none());

    let (denied, _s) = list(DockerReply::failing(1, "permission denied")).await;
    assert_eq!(
        serde_json::to_value(&denied).unwrap()["outcome"],
        "permission_denied"
    );
}

// ---- container logs -------------------------------------------------------------------------

const CONTAINER: &str = "a1b2c3d4e5f6";

fn request() -> LogsRequest {
    LogsRequest::new(CONTAINER, 200).unwrap()
}

async fn fetch_logs(reply: DockerReply) -> (LogsResult, Running) {
    let s = start_server_with_logs(reply).await;
    let result = logs::fetch(trusted(&s), &request())
        .await
        .expect("reaches docker");
    (result, s)
}

#[tokio::test]
async fn fetches_logs_with_one_login_and_exactly_the_built_command() {
    let (result, s) = fetch_logs(DockerReply::ok(logs::SAMPLE_LOGS)).await;

    assert_eq!(result.outcome, DockerOutcome::Ok);
    assert_eq!(result.message, None);
    assert!(!result.truncated);
    assert_eq!(result.lines.len(), 3);
    assert_eq!(result.lines[0].message, "server listening on :8080");
    assert_eq!(result.lines[1].message, "ERROR failed to reach db");

    assert_eq!(execs(&s), vec![format!("exec:{}", request().command())]);
    assert!(execs(&s)[0].contains(&format!("--tail 200 {CONTAINER} ")));
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_container_with_no_logs_is_a_normal_empty_result() {
    let (result, _s) = fetch_logs(DockerReply::ok("")).await;
    assert_eq!(result.outcome, DockerOutcome::Ok);
    assert!(result.lines.is_empty());
}

#[tokio::test]
async fn terminal_escape_sequences_in_logs_never_reach_the_result() {
    let evil =
        "2026-11-04T10:00:00Z \u{1b}]0;pwned\u{7}\u{1b}[2J\u{1b}[31mred\u{1b}[0m\u{8}\u{7} ok\n";
    let (result, _s) = fetch_logs(DockerReply::ok(evil)).await;
    assert_eq!(result.lines[0].message, "red ok");
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("pwned") && !json.contains("\\u001b"));
}

#[tokio::test]
async fn log_problems_are_reported_without_echoing_remote_text() {
    let cases = [
        (127, "", DockerOutcome::NotInstalled),
        (
            1,
            "Error response from daemon: No such container: a1b2c3d4e5f6 SECRET-DETAIL",
            DockerOutcome::ContainerNotFound,
        ),
        (
            1,
            "permission denied while trying to connect SECRET-DETAIL",
            DockerOutcome::PermissionDenied,
        ),
        (
            1,
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock SECRET-DETAIL",
            DockerOutcome::DaemonUnavailable,
        ),
        (2, "boom SECRET-DETAIL", DockerOutcome::Error),
    ];
    for (exit, text, want) in cases {
        // With `2>&1` an error arrives on stdout, so the test server sends it there.
        let reply = DockerReply {
            stdout: text.into(),
            stderr: String::new(),
            exit,
        };
        let (result, _s) = fetch_logs(reply).await;
        assert_eq!(result.outcome, want, "exit {exit}");
        assert!(result.lines.is_empty());
        let message = result.message.expect("a message");
        assert!(!message.contains("SECRET"), "{message}");
        assert!(!message.contains("/var/run"), "{message}");
    }
}

#[tokio::test]
async fn oversized_logs_are_an_error_not_a_hang() {
    let line = format!("2026-11-04T10:00:00Z {}\n", "x".repeat(1000));
    let huge = line.repeat(5 * 1024);
    let s = start_server_with_logs(DockerReply::ok(&huge)).await;
    let err = logs::fetch(trusted(&s), &request()).await.unwrap_err();
    assert!(err.contains("too much output"), "{err}");
    assert!(!err.contains("xxxx"));
}

#[tokio::test]
async fn logs_refuse_an_untrusted_host_without_touching_the_network() {
    let s = start_server_with_logs(DockerReply::ok(logs::SAMPLE_LOGS)).await;
    let p = params(s.port, AuthSecret::Password(PASSWORD.into()), None);
    let err = logs::fetch(p, &request()).await.unwrap_err();
    assert!(err.contains("not trusted"));
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
    assert!(execs(&s).is_empty());
}

#[tokio::test]
async fn logs_stop_after_one_failed_login_and_run_nothing() {
    let s = start_server_with_logs(DockerReply::ok(logs::SAMPLE_LOGS)).await;
    let p = params(
        s.port,
        AuthSecret::Password("wrong".into()),
        stored(&fingerprint(&s.host_key)),
    );
    assert!(logs::fetch(p, &request()).await.is_err());
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1, "no retry");
    assert!(execs(&s).is_empty());
}

#[tokio::test]
async fn logs_fail_on_a_changed_host_key_before_any_credential_is_sent() {
    let s = start_server_with_logs(DockerReply::ok(logs::SAMPLE_LOGS)).await;
    let p = params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&new_key())),
    );
    assert!(logs::fetch(p, &request()).await.is_err());
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
    assert!(execs(&s).is_empty());
}

#[tokio::test]
async fn another_container_and_line_count_change_only_those_two_values() {
    let s = start_server_with_logs(DockerReply::ok(logs::SAMPLE_LOGS)).await;
    let other = LogsRequest::new("0123456789ab", 5).unwrap();
    let result = logs::fetch(trusted(&s), &other).await.unwrap();
    assert_eq!(result.outcome, DockerOutcome::Ok);
    let ran = execs(&s);
    assert_eq!(ran.len(), 1);
    assert!(ran[0].contains("--tail 5 0123456789ab "));
}

#[tokio::test]
async fn logs_result_serializes_with_the_shape_the_frontend_expects() {
    let (result, _s) = fetch_logs(DockerReply::ok(logs::SAMPLE_LOGS)).await;
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["outcome"], "ok");
    assert!(json.get("message").is_none());
    assert_eq!(json["truncated"], false);
    assert!(json["fetchedAt"].is_i64());
    assert_eq!(json["lines"][0]["time"], "2026-11-04T10:00:00.123456789Z");
    assert_eq!(json["lines"][0]["message"], "server listening on :8080");

    let (gone, _s) = fetch_logs(DockerReply {
        stdout: "No such container".into(),
        stderr: String::new(),
        exit: 1,
    })
    .await;
    assert_eq!(
        serde_json::to_value(&gone).unwrap()["outcome"],
        "container_not_found"
    );
}
