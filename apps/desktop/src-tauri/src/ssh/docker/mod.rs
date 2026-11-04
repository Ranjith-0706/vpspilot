//! Read-only Docker access over SSH: the container list and container logs.
//!
//! Each request opens one login, runs a single command, and disconnects. It follows the same
//! rules as the terminal and the monitor: it never connects to, or sends a credential to, a host
//! whose key is not already trusted, and it makes exactly one attempt (no retries, so it cannot
//! trigger lockouts such as fail2ban).
//!
//! It never uses `sudo`. If the account cannot use Docker the result says so, and the user fixes
//! that on the server.

pub mod logs;
pub mod parse;

use super::client::{AuthSecret, Seen, Session, TestParams, connect_authenticated, explain};
use parse::{COMMAND, Container, MAX_OUTPUT_BYTES, MAX_STDERR_BYTES, parse_output};
use russh::{ChannelMsg, Disconnect};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
/// The SSH extended-data type for standard error.
const STDERR: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DockerOutcome {
    Ok,
    NotInstalled,
    PermissionDenied,
    DaemonUnavailable,
    /// Logs only: the container is gone (for example it was removed since the list was loaded).
    ContainerNotFound,
    Error,
}

const MSG_NOT_INSTALLED: &str =
    "Docker is not installed for this account (the docker command was not found).";
const MSG_PERMISSION: &str = "This account is not allowed to use Docker. Add it to the docker group on the server and try again. VPSPilot never uses sudo.";
const MSG_DAEMON: &str = "The Docker service is not running on this server.";

/// Problems recognized from Docker's error text. The text itself is never shown.
fn known_problem(error_text: &str) -> Option<(DockerOutcome, &'static str)> {
    let t = error_text.to_ascii_lowercase();
    if t.contains("permission denied") {
        Some((DockerOutcome::PermissionDenied, MSG_PERMISSION))
    } else if t.contains("docker daemon") || t.contains("cannot connect to") {
        Some((DockerOutcome::DaemonUnavailable, MSG_DAEMON))
    } else {
        None
    }
}

/// What a listing produced. Connection and login problems are not here: they are returned as
/// errors, like every other SSH feature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerResult {
    pub outcome: DockerOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub containers: Vec<Container>,
    /// More containers existed than are listed.
    pub truncated: bool,
    /// Unix epoch milliseconds, set by VPSPilot (not the remote clock).
    pub fetched_at: i64,
}

pub(super) struct ExecOutput {
    pub(super) stdout: Vec<u8>,
    /// Only used to recognize known problems. Never shown or logged.
    pub(super) stderr: Vec<u8>,
    pub(super) exit: u32,
}

/// Opens a login after the same safety checks as every other SSH feature. Returns a user-safe
/// message for anything that stops it getting a session.
pub(super) async fn connect(params: &TestParams) -> Result<Session, String> {
    if params.stored.is_none() {
        return Err("This server's host key is not trusted yet. Run a connection test on the Servers page and trust the host key first.".into());
    }
    if matches!(params.auth, AuthSecret::None) {
        return Err("No credential is stored for this server. Edit the server to add one.".into());
    }

    let seen = Arc::new(Mutex::new(Seen::default()));
    match tokio::time::timeout(
        CONNECT_TIMEOUT,
        connect_authenticated(params, seen.clone(), true),
    )
    .await
    {
        Ok(Ok(session)) => Ok(session),
        Ok(Err(failure)) => Err(explain(&failure, params, &seen).1),
        Err(_) => Err(format!(
            "No response from {}:{} within {} seconds.",
            params.host,
            params.port,
            CONNECT_TIMEOUT.as_secs()
        )),
    }
}

/// Runs one command over `session` and collects its output. `command` must be a constant or be
/// built only from validated parts.
pub(super) async fn run_command(
    session: &Session,
    command: &str,
    timeout: Duration,
) -> Result<ExecOutput, String> {
    let work = async {
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|e| format!("Could not open a channel: {e}"))?;
        channel
            .exec(true, command)
            .await
            .map_err(|e| format!("Could not run the Docker command: {e}"))?;

        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let mut exit: Option<u32> = None;
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    if stdout.len() + data.len() > MAX_OUTPUT_BYTES {
                        return Err("Docker returned too much output.".to_string());
                    }
                    stdout.extend_from_slice(&data);
                }
                ChannelMsg::ExtendedData { data, ext } if ext == STDERR => {
                    let room = MAX_STDERR_BYTES.saturating_sub(stderr.len());
                    stderr.extend_from_slice(&data[..data.len().min(room)]);
                }
                ChannelMsg::ExitStatus { exit_status } => exit = Some(exit_status),
                ChannelMsg::Failure => {
                    return Err("The server refused to run the Docker command.".into());
                }
                ChannelMsg::Eof if exit.is_some() => break,
                ChannelMsg::Close => break,
                _ => {}
            }
        }
        let exit = exit.ok_or("The connection closed before the command finished.")?;
        Ok(ExecOutput {
            stdout,
            stderr,
            exit,
        })
    };
    match tokio::time::timeout(timeout, work).await {
        Ok(r) => r,
        Err(_) => Err("The server took too long to respond.".into()),
    }
}

/// Connects, runs one command, and disconnects.
pub(super) async fn run_once(params: &TestParams, command: &str) -> Result<ExecOutput, String> {
    let session = connect(params).await?;
    let out = run_command(&session, command, COMMAND_TIMEOUT).await;
    let _ = session
        .disconnect(Disconnect::ByApplication, "done", "en")
        .await;
    out
}

fn result(outcome: DockerOutcome, message: Option<&str>, now: i64) -> DockerResult {
    DockerResult {
        outcome,
        message: message.map(String::from),
        containers: Vec::new(),
        truncated: false,
        fetched_at: now,
    }
}

/// Turns what the command printed into a result. Only known problems are recognized from the
/// error text; anything else gets a generic message so remote text is never shown.
fn classify(out: &ExecOutput, now: i64) -> DockerResult {
    match out.exit {
        0 => match parse_output(&String::from_utf8_lossy(&out.stdout)) {
            Ok(parsed) => DockerResult {
                outcome: DockerOutcome::Ok,
                message: None,
                containers: parsed.containers,
                truncated: parsed.truncated,
                fetched_at: now,
            },
            Err(e) => result(DockerOutcome::Error, Some(&e.to_string()), now),
        },
        127 => result(DockerOutcome::NotInstalled, Some(MSG_NOT_INSTALLED), now),
        code => match known_problem(&String::from_utf8_lossy(&out.stderr)) {
            Some((outcome, message)) => result(outcome, Some(message), now),
            None => result(
                DockerOutcome::Error,
                Some(&format!("Docker reported an error (exit status {code}).")),
                now,
            ),
        },
    }
}

/// Lists the containers on a server. Returns `Err` with a user-safe message for anything that
/// stops it reaching Docker at all (untrusted host key, login failure, network problem).
pub async fn list_containers(params: TestParams) -> Result<DockerResult, String> {
    let out = run_once(&params, COMMAND).await?;
    Ok(classify(&out, crate::servers::repo::now_ms()))
}
