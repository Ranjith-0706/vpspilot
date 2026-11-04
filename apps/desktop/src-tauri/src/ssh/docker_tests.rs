//! End-to-end Docker listing tests against the in-process SSH server (see client_tests.rs).

use super::client::{AuthSecret, TestParams};
use super::client_tests::{
    DockerReply, Running, fingerprint, new_key, params, start_server_with_docker, stored,
};
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
