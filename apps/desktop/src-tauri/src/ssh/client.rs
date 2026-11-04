//! SSH connection test.
//!
//! Connects, verifies the host key against the stored trusted key, authenticates, and
//! disconnects. It never opens a channel, shell or exec request, so it cannot run anything on
//! the server. The host key decision is made *before* any credential is sent.

use super::hostkey::{HostKeyDecision, HostKeyRecord, ObservedKey, evaluate, observe};
use russh::Disconnect;
use russh::client;
use russh::keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate, decode_secret_key};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
/// How often an interactive session pings an idle server.
const KEEPALIVE: Duration = Duration::from_secs(30);

/// Secret material used for authentication. Never printed or serialized.
pub enum AuthSecret {
    None,
    Password(String),
    PrivateKey(String),
}

impl std::fmt::Debug for AuthSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AuthSecret::None => "AuthSecret::None",
            AuthSecret::Password(_) => "AuthSecret::Password(<redacted>)",
            AuthSecret::PrivateKey(_) => "AuthSecret::PrivateKey(<redacted>)",
        })
    }
}

#[derive(Debug)]
pub struct TestParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthSecret,
    pub stored: Option<HostKeyRecord>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    HostKeyUnknown,
    HostKeyChanged,
    AuthFailed,
    NoCredential,
    Unreachable,
    Timeout,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub outcome: Outcome,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_key: Option<ObservedKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    /// Unix epoch milliseconds.
    pub tested_at: i64,
}

#[derive(Default)]
pub(super) struct Seen {
    pub(super) observed: Option<ObservedKey>,
    pub(super) decision: Option<HostKeyDecision>,
    pub(super) unsupported_cert: bool,
}

pub(super) struct Handler {
    stored: Option<HostKeyRecord>,
    seen: Arc<Mutex<Seen>>,
}

impl client::Handler for Handler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let mut seen = self.seen.lock().map_err(|_| russh::Error::Disconnect)?;
        match observe(server_public_key) {
            None => {
                seen.unsupported_cert = true;
                Ok(false)
            }
            Some(observed) => {
                let decision = evaluate(self.stored.as_ref(), &observed);
                let trusted = decision == HostKeyDecision::Trusted;
                seen.observed = Some(observed);
                seen.decision = Some(decision);
                Ok(trusted) // false aborts the handshake before any credential is sent
            }
        }
    }
}

pub(super) type Session = client::Handle<Handler>;

pub(super) enum Failure {
    Connect(russh::Error),
    AuthFailed,
    NoCredential,
    BadKey(&'static str),
    Other(String),
}

/// Connects, verifies the host key against the stored one, and authenticates.
///
/// This is the single path used by both the connection test and the terminal, so the host key
/// rules apply identically: nothing is sent to a host whose key is not already trusted.
/// `interactive` sessions stay open indefinitely (with keepalives) instead of timing out.
pub(super) async fn connect_authenticated(
    params: &TestParams,
    seen: Arc<Mutex<Seen>>,
    interactive: bool,
) -> Result<Session, Failure> {
    let config = Arc::new(if interactive {
        client::Config {
            inactivity_timeout: None,
            keepalive_interval: Some(KEEPALIVE),
            keepalive_max: 3,
            ..Default::default()
        }
    } else {
        client::Config {
            inactivity_timeout: Some(params.timeout),
            ..Default::default()
        }
    });
    let handler = Handler {
        stored: params.stored.clone(),
        seen,
    };
    let mut session = client::connect(config, (params.host.as_str(), params.port), handler)
        .await
        .map_err(Failure::Connect)?;

    // Host key is trusted at this point (otherwise connect() failed above).
    let authenticated = match &params.auth {
        AuthSecret::None => return Err(Failure::NoCredential),
        AuthSecret::Password(pw) => session
            .authenticate_password(params.username.clone(), pw.clone())
            .await
            .map_err(|e| Failure::Other(format!("Authentication error: {e}")))?
            .success(),
        AuthSecret::PrivateKey(pem) => {
            let key = decode_secret_key(pem, None).map_err(|e| match e {
                russh::keys::Error::KeyIsEncrypted => Failure::BadKey(
                    "This private key is passphrase-protected, which is not supported yet.",
                ),
                _ => Failure::BadKey(
                    "The stored private key could not be read. Check that it is a valid, unencrypted private key.",
                ),
            })?;
            let hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(|e| Failure::Other(format!("Authentication error: {e}")))?
                .flatten();
            session
                .authenticate_publickey(
                    params.username.clone(),
                    PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                )
                .await
                .map_err(|e| Failure::Other(format!("Authentication error: {e}")))?
                .success()
        }
    };

    if authenticated {
        Ok(session)
    } else {
        let _ = session
            .disconnect(Disconnect::ByApplication, "authentication failed", "en")
            .await;
        Err(Failure::AuthFailed)
    }
}

async fn attempt(params: &TestParams, seen: Arc<Mutex<Seen>>) -> Result<(), Failure> {
    let session = connect_authenticated(params, seen, false).await?;
    // Best effort: we're done either way. No channel is ever opened.
    let _ = session
        .disconnect(Disconnect::ByApplication, "connection test complete", "en")
        .await;
    Ok(())
}

fn describe_io(err: &std::io::Error) -> String {
    use std::io::ErrorKind::*;
    match err.kind() {
        ConnectionRefused => "the connection was refused (is SSH running on that port?)".into(),
        TimedOut => "the connection timed out".into(),
        NotFound | AddrNotAvailable => "the host name could not be resolved".into(),
        _ => format!("{}", err.kind()),
    }
}

/// Turns a failure into an outcome, a user-facing message and (for a changed key) the
/// previously trusted fingerprint. Messages never contain secrets.
pub(super) fn explain(
    failure: &Failure,
    params: &TestParams,
    seen: &Arc<Mutex<Seen>>,
) -> (Outcome, String, Option<String>) {
    match failure {
        Failure::AuthFailed => (
            Outcome::AuthFailed,
            "The server rejected the username or credential.".into(),
            None,
        ),
        Failure::NoCredential => (
            Outcome::NoCredential,
            "Host key verified, but no credential is stored for this server. Add one by editing the server.".into(),
            None,
        ),
        Failure::BadKey(msg) => (Outcome::Error, (*msg).into(), None),
        Failure::Other(msg) => (Outcome::Error, msg.clone(), None),
        Failure::Connect(err) => {
            let (decision, unsupported) = {
                let s = seen.lock().ok();
                (
                    s.as_ref().and_then(|s| s.decision.clone()),
                    s.as_ref().is_some_and(|s| s.unsupported_cert),
                )
            };
            match decision {
                Some(HostKeyDecision::Unknown) => (
                    Outcome::HostKeyUnknown,
                    "This server's host key is not trusted yet. Verify the fingerprint below, then trust it.".into(),
                    None,
                ),
                Some(HostKeyDecision::Changed { expected }) => (
                    Outcome::HostKeyChanged,
                    "WARNING: the server's host key does not match the one you trusted. This could mean the server was rebuilt, or that someone is impersonating it. No credentials were sent.".into(),
                    Some(expected),
                ),
                _ if unsupported => (
                    Outcome::Error,
                    "The server presented a host certificate, which is not supported yet.".into(),
                    None,
                ),
                _ => match err {
                    russh::Error::IO(io) => (
                        Outcome::Unreachable,
                        format!(
                            "Could not reach {}:{}: {}.",
                            params.host,
                            params.port,
                            describe_io(io)
                        ),
                        None,
                    ),
                    other => (Outcome::Error, format!("SSH connection failed: {other}"), None),
                },
            }
        }
    }
}

/// Runs the test. Never returns secrets in the result.
pub async fn run_test(params: TestParams) -> ConnectionTestResult {
    let started = Instant::now();
    let seen = Arc::new(Mutex::new(Seen::default()));

    let failure = match tokio::time::timeout(params.timeout, attempt(&params, seen.clone())).await {
        Ok(Ok(())) => {
            return finish(
                &seen,
                Outcome::Success,
                "Host key verified and authentication succeeded. The connection was closed without running anything.".into(),
                None,
                started,
            );
        }
        Ok(Err(failure)) => failure,
        Err(_) => {
            return finish(
                &seen,
                Outcome::Timeout,
                format!(
                    "No response from {}:{} within {} seconds.",
                    params.host,
                    params.port,
                    params.timeout.as_secs()
                ),
                None,
                started,
            );
        }
    };
    let (outcome, message, expected) = explain(&failure, &params, &seen);
    finish(&seen, outcome, message, expected, started)
}

fn finish(
    seen: &Arc<Mutex<Seen>>,
    outcome: Outcome,
    message: String,
    expected_fingerprint: Option<String>,
    started: Instant,
) -> ConnectionTestResult {
    let host_key = seen.lock().ok().and_then(|s| s.observed.clone());
    let latency_ms = (outcome == Outcome::Success).then(|| started.elapsed().as_millis() as u64);
    ConnectionTestResult {
        outcome,
        message,
        host_key,
        expected_fingerprint,
        latency_ms,
        tested_at: crate::servers::repo::now_ms(),
    }
}
