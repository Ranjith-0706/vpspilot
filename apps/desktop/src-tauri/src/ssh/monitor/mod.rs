//! Agentless server monitoring over a single persistent SSH session.
//!
//! One session per monitored server is kept open and polled with a fixed, read-only command
//! (see [`parse::COMMAND`]), so the server's auth log shows one login rather than one per poll.
//! It uses the same host key rules as the terminal: it never connects to, or sends a credential
//! to, a host whose key is not already trusted. After an authentication or host key failure it
//! stops instead of retrying, so it cannot trigger lockouts (e.g. fail2ban); only genuine
//! network problems are retried, with exponential backoff.

pub mod parse;

use super::client::{
    AuthSecret, Failure, Outcome, Seen, Session, TestParams, connect_authenticated, explain,
};
use parse::{COMMAND, MAX_OUTPUT_BYTES, Metrics, parse_output};
use russh::{ChannelMsg, Disconnect};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// After this many consecutive polls that fail for a reason retrying cannot fix (the command
/// exits non-zero, prints something unparseable, or is refused), monitoring gives up instead of
/// logging in again forever. A successful sample resets the count, so one glitch under load
/// still recovers.
const MAX_DETERMINISTIC_FAILURES: u32 = 3;

/// Why a poll failed.
enum PollError {
    /// A network or timing problem: worth retrying indefinitely with backoff.
    Transient(String),
    /// The server answered, but not in a way retrying will change (e.g. not a Linux host).
    Deterministic(String),
}

impl PollError {
    fn message(&self) -> &str {
        match self {
            PollError::Transient(m) | PollError::Deterministic(m) => m,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorState {
    Connecting,
    Online,
    /// A network problem; will retry automatically.
    Retrying,
    /// Gave up (authentication / host key problem); will not retry.
    Failed,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MonitorEvent {
    Status {
        state: MonitorState,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Sample {
        metrics: Metrics,
    },
}

#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub interval: Duration,
    pub retry_base: Duration,
    pub retry_max: Duration,
    pub poll_timeout: Duration,
}

impl MonitorConfig {
    pub fn with_interval(interval: Duration) -> Self {
        Self {
            interval,
            retry_base: Duration::from_secs(5),
            retry_max: Duration::from_secs(60),
            poll_timeout: Duration::from_secs(15),
        }
    }
}

/// Stops the monitor when `stop` is called or when the handle is dropped.
pub struct MonitorHandle {
    stop: watch::Sender<Option<String>>,
}

impl MonitorHandle {
    pub fn stop(&self, reason: &str) {
        let _ = self.stop.send(Some(reason.to_string()));
    }
}

fn stop_reason(rx: &watch::Receiver<Option<String>>) -> Option<String> {
    rx.borrow().clone()
}

/// Sleeps for `d`, or returns early with the stop reason.
async fn sleep_or_stop(d: Duration, rx: &mut watch::Receiver<Option<String>>) -> Option<String> {
    if let Some(r) = stop_reason(rx) {
        return Some(r);
    }
    tokio::select! {
        _ = tokio::time::sleep(d) => None,
        // Err means the handle was dropped, which also means stop.
        _ = rx.changed() => Some(stop_reason(rx).unwrap_or_default()),
    }
}

async fn emit(
    events: &mpsc::Sender<MonitorEvent>,
    state: MonitorState,
    message: Option<String>,
) -> bool {
    events
        .send(MonitorEvent::Status { state, message })
        .await
        .is_ok()
}

/// Runs the fixed command once over `session` and parses its output.
async fn poll(session: &Session, timeout: Duration) -> Result<Metrics, PollError> {
    let work = async {
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|e| PollError::Transient(format!("Could not open a channel: {e}")))?;
        channel.exec(true, COMMAND).await.map_err(|e| {
            PollError::Transient(format!("Could not run the monitoring command: {e}"))
        })?;

        let mut out: Vec<u8> = Vec::new();
        let mut exit: Option<u32> = None;
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    if out.len() + data.len() > MAX_OUTPUT_BYTES {
                        return Err(PollError::Deterministic(
                            "The server returned too much output.".into(),
                        ));
                    }
                    out.extend_from_slice(&data);
                }
                ChannelMsg::ExitStatus { exit_status } => exit = Some(exit_status),
                ChannelMsg::Failure => {
                    return Err(PollError::Deterministic(
                        "The server refused to run the monitoring command.".into(),
                    ));
                }
                ChannelMsg::Eof if exit.is_some() => break,
                ChannelMsg::Close => break,
                _ => {}
            }
        }
        match exit {
            Some(0) => {}
            Some(code) => {
                return Err(PollError::Deterministic(format!(
                    "The monitoring command failed (exit status {code}). Monitoring supports Linux servers."
                )));
            }
            None => {
                return Err(PollError::Transient(
                    "The connection closed before the command finished.".into(),
                ));
            }
        }
        parse_output(
            &String::from_utf8_lossy(&out),
            crate::servers::repo::now_ms(),
        )
        .map_err(|e| PollError::Deterministic(e.to_string()))
    };
    match tokio::time::timeout(timeout, work).await {
        Ok(r) => r,
        Err(_) => Err(PollError::Transient(
            "The server took too long to respond.".into(),
        )),
    }
}

/// Whether a connection failure is a network problem worth retrying.
///
/// Credential and host key problems are never retried (that could trigger lockouts), and neither
/// are deterministic protocol errors. Only unreachable/timeout failures, and connections that
/// drop mid-login, are retried.
fn retryable(failure: &Failure, outcome: Outcome) -> bool {
    match failure {
        Failure::AuthFailed | Failure::NoCredential | Failure::BadKey(_) => false,
        Failure::Connect(_) => matches!(outcome, Outcome::Unreachable | Outcome::Timeout),
        Failure::Other(_) => true,
    }
}

/// Starts monitoring. Fails immediately (without any network access) if the host key is not
/// already trusted or there is no credential.
pub fn spawn(
    params: TestParams,
    cfg: MonitorConfig,
    events: mpsc::Sender<MonitorEvent>,
) -> Result<MonitorHandle, String> {
    if params.stored.is_none() {
        return Err("This server's host key is not trusted yet. Run a connection test on the Servers page and trust the host key first.".into());
    }
    if matches!(params.auth, AuthSecret::None) {
        return Err("No credential is stored for this server. Edit the server to add one.".into());
    }

    let (stop_tx, mut stop) = watch::channel::<Option<String>>(None);
    tokio::spawn(async move {
        let mut backoff = cfg.retry_base;
        let mut deterministic_failures: u32 = 0;
        let final_reason: Option<String>;

        'outer: loop {
            if !emit(&events, MonitorState::Connecting, None).await {
                return;
            }
            let seen = Arc::new(Mutex::new(Seen::default()));
            let connected = tokio::select! {
                r = tokio::time::timeout(CONNECT_TIMEOUT, connect_authenticated(&params, seen.clone(), true)) => r,
                _ = stop.changed() => {
                    final_reason = Some(stop_reason(&stop).unwrap_or_default());
                    break 'outer;
                }
            };

            let session = match connected {
                Ok(Ok(session)) => session,
                Ok(Err(failure)) => {
                    let (outcome, message, _) = explain(&failure, &params, &seen);
                    if !retryable(&failure, outcome) {
                        let _ = emit(&events, MonitorState::Failed, Some(message)).await;
                        return; // no Stopped event: this is a failure, not a stop
                    }
                    if !emit(&events, MonitorState::Retrying, Some(message)).await {
                        return;
                    }
                    if let Some(r) = sleep_or_stop(backoff, &mut stop).await {
                        final_reason = Some(r);
                        break 'outer;
                    }
                    backoff = (backoff * 2).min(cfg.retry_max);
                    continue 'outer;
                }
                Err(_) => {
                    let message = format!(
                        "No response from {}:{} within {} seconds.",
                        params.host,
                        params.port,
                        CONNECT_TIMEOUT.as_secs()
                    );
                    if !emit(&events, MonitorState::Retrying, Some(message)).await {
                        return;
                    }
                    if let Some(r) = sleep_or_stop(backoff, &mut stop).await {
                        final_reason = Some(r);
                        break 'outer;
                    }
                    backoff = (backoff * 2).min(cfg.retry_max);
                    continue 'outer;
                }
            };
            backoff = cfg.retry_base;

            let mut online = false;
            loop {
                match poll(&session, cfg.poll_timeout).await {
                    Ok(metrics) => {
                        deterministic_failures = 0;
                        if !online {
                            online = true;
                            if !emit(&events, MonitorState::Online, None).await {
                                return;
                            }
                        }
                        if events.send(MonitorEvent::Sample { metrics }).await.is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        // Session is suspect: drop it and reconnect after a pause.
                        let _ = session
                            .disconnect(Disconnect::ByApplication, "reconnecting", "en")
                            .await;
                        if matches!(error, PollError::Deterministic(_)) {
                            deterministic_failures += 1;
                            if deterministic_failures >= MAX_DETERMINISTIC_FAILURES {
                                let message = format!(
                                    "{} Monitoring stopped after {MAX_DETERMINISTIC_FAILURES} failed attempts.",
                                    error.message()
                                );
                                let _ = emit(&events, MonitorState::Failed, Some(message)).await;
                                return; // a failure, not a stop: no Stopped event
                            }
                        } else {
                            deterministic_failures = 0;
                        }
                        let message = error.message().to_string();
                        if !emit(&events, MonitorState::Retrying, Some(message)).await {
                            return;
                        }
                        if let Some(r) = sleep_or_stop(backoff, &mut stop).await {
                            final_reason = Some(r);
                            break 'outer;
                        }
                        backoff = (backoff * 2).min(cfg.retry_max);
                        continue 'outer;
                    }
                }
                if let Some(r) = sleep_or_stop(cfg.interval, &mut stop).await {
                    final_reason = Some(r);
                    let _ = session
                        .disconnect(Disconnect::ByApplication, "monitoring stopped", "en")
                        .await;
                    break 'outer;
                }
            }
        }

        let message = final_reason.filter(|r| !r.is_empty());
        let _ = emit(&events, MonitorState::Stopped, message).await;
    });

    Ok(MonitorHandle { stop: stop_tx })
}
