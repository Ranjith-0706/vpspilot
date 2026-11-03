//! End-to-end tests of the connection test against an in-process SSH server on localhost.

use super::client::{AuthSecret, ConnectionTestResult, Outcome, TestParams, run_test};
use super::hostkey::HostKeyRecord;
use russh::keys::ssh_key::LineEnding;
use russh::keys::{Algorithm, HashAlg, PrivateKey, PublicKey};
use russh::server::{self, Auth, Server as _, VerifiedPublicKey};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;

const USER: &str = "deploy";
const PASSWORD: &str = "correct-pw";

#[derive(Clone)]
struct TestServer {
    allowed_key: Option<PublicKey>,
    attempts: Arc<AtomicUsize>,
}

impl server::Server for TestServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self {
        self.clone()
    }
}

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Ok(if user == USER && password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: VerifiedPublicKey<'_>,
    ) -> Result<Auth, Self::Error> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        let ok = user == USER && self.allowed_key.as_ref() == Some(key.public_key);
        Ok(if ok { Auth::Accept } else { Auth::reject() })
    }
}

fn new_key() -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()
}

fn fingerprint(key: &PrivateKey) -> String {
    key.public_key().fingerprint(HashAlg::Sha256).to_string()
}

fn openssh_pem(key: &PrivateKey) -> String {
    key.to_openssh(LineEnding::LF).unwrap().to_string()
}

struct Running {
    port: u16,
    host_key: PrivateKey,
    attempts: Arc<AtomicUsize>,
}

async fn start_server(allowed_key: Option<PublicKey>) -> Running {
    let host_key = new_key();
    let attempts = Arc::new(AtomicUsize::new(0));
    let config = Arc::new(server::Config {
        keys: vec![host_key.clone()],
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    });
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut srv = TestServer {
        allowed_key,
        attempts: attempts.clone(),
    };
    tokio::spawn(async move {
        let running = srv.run_on_socket(config, &listener);
        let _ = running.await;
    });
    Running {
        port,
        host_key,
        attempts,
    }
}

fn stored(fp: &str) -> Option<HostKeyRecord> {
    Some(HostKeyRecord {
        server_id: "s".into(),
        key_type: "ssh-ed25519".into(),
        fingerprint: fp.into(),
        trusted_at: 0,
    })
}

fn params(port: u16, auth: AuthSecret, stored: Option<HostKeyRecord>) -> TestParams {
    TestParams {
        host: "127.0.0.1".into(),
        port,
        username: USER.into(),
        auth,
        stored,
        timeout: Duration::from_secs(10),
    }
}

async fn run(p: TestParams) -> ConnectionTestResult {
    run_test(p).await
}

#[tokio::test]
async fn unknown_host_key_stops_before_any_credential_is_sent() {
    let s = start_server(None).await;
    let r = run(params(s.port, AuthSecret::Password(PASSWORD.into()), None)).await;
    assert_eq!(r.outcome, Outcome::HostKeyUnknown);
    let seen = r.host_key.expect("fingerprint is reported");
    assert_eq!(seen.fingerprint, fingerprint(&s.host_key));
    assert_eq!(seen.key_type, "ssh-ed25519");
    assert_eq!(
        s.attempts.load(Ordering::SeqCst),
        0,
        "no credential may be sent to an untrusted host"
    );
}

#[tokio::test]
async fn trusted_key_and_correct_password_succeeds() {
    let s = start_server(None).await;
    let fp = fingerprint(&s.host_key);
    let r = run(params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fp),
    ))
    .await;
    assert_eq!(r.outcome, Outcome::Success, "{}", r.message);
    assert!(r.latency_ms.is_some());
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn wrong_password_is_auth_failed() {
    let s = start_server(None).await;
    let fp = fingerprint(&s.host_key);
    let r = run(params(
        s.port,
        AuthSecret::Password("nope".into()),
        stored(&fp),
    ))
    .await;
    assert_eq!(r.outcome, Outcome::AuthFailed);
    assert!(r.latency_ms.is_none());
}

#[tokio::test]
async fn changed_host_key_is_refused_before_authentication() {
    let s = start_server(None).await;
    let old = fingerprint(&new_key()); // the key we "trusted" earlier
    let r = run(params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&old),
    ))
    .await;
    assert_eq!(r.outcome, Outcome::HostKeyChanged);
    assert_eq!(r.expected_fingerprint.as_deref(), Some(old.as_str()));
    assert_eq!(r.host_key.unwrap().fingerprint, fingerprint(&s.host_key));
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn private_key_auth_succeeds_and_wrong_key_fails() {
    let client_key = new_key();
    let s = start_server(Some(client_key.public_key().clone())).await;
    let fp = fingerprint(&s.host_key);

    let ok = run(params(
        s.port,
        AuthSecret::PrivateKey(openssh_pem(&client_key)),
        stored(&fp),
    ))
    .await;
    assert_eq!(ok.outcome, Outcome::Success, "{}", ok.message);

    let bad = run(params(
        s.port,
        AuthSecret::PrivateKey(openssh_pem(&new_key())),
        stored(&fp),
    ))
    .await;
    assert_eq!(bad.outcome, Outcome::AuthFailed);
}

#[tokio::test]
async fn unreadable_private_key_reports_error_without_echoing_it() {
    let s = start_server(None).await;
    let fp = fingerprint(&s.host_key);
    let garbage = "not-a-key-SECRETMARKER";
    let r = run(params(
        s.port,
        AuthSecret::PrivateKey(garbage.into()),
        stored(&fp),
    ))
    .await;
    assert_eq!(r.outcome, Outcome::Error);
    assert!(!r.message.contains("SECRETMARKER"));
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn trusted_host_without_credential_reports_no_credential() {
    let s = start_server(None).await;
    let fp = fingerprint(&s.host_key);
    let r = run(params(s.port, AuthSecret::None, stored(&fp))).await;
    assert_eq!(r.outcome, Outcome::NoCredential);
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn closed_port_is_unreachable() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let r = run(params(port, AuthSecret::Password("x".into()), None)).await;
    assert_eq!(r.outcome, Outcome::Unreachable, "{}", r.message);
}

#[tokio::test]
async fn silent_server_times_out() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _held = listener.accept().await; // accept, then never speak
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    let mut p = params(port, AuthSecret::Password("x".into()), None);
    p.timeout = Duration::from_millis(400);
    let r = run(p).await;
    assert_eq!(r.outcome, Outcome::Timeout);
}

#[tokio::test]
async fn result_json_is_camel_case_and_contains_no_secret() {
    let s = start_server(None).await;
    let fp = fingerprint(&s.host_key);
    let r = run(params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fp),
    ))
    .await;
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("\"outcome\":\"success\""));
    assert!(json.contains("\"latencyMs\""));
    assert!(json.contains("\"testedAt\""));
    assert!(!json.contains(PASSWORD));
}
