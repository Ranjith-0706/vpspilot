//! Interactive SSH terminal session (a remote shell with a PTY).
//!
//! This gives the user a shell on *their own saved server*; nothing runs on the local machine.
//! It uses the same connect/verify/authenticate path as the connection test, and additionally
//! refuses to start unless the host key is already trusted and a credential is stored.
//!
//! Independent of Tauri: output is delivered through an `mpsc` channel, so it can be tested
//! against an in-process SSH server.

use super::client::{AuthSecret, Seen, TestParams, connect_authenticated, explain};
use crate::error::AppError;
use base64::Engine;
use russh::{ChannelMsg, Disconnect};
use serde::{Serialize, Serializer};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Notify, mpsc};

pub const MAX_COLS: u32 = 1000;
pub const MAX_ROWS: u32 = 1000;
/// Largest single write accepted from the frontend (a big paste is split by the caller).
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const QUEUE: usize = 256;

fn as_base64<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Events streamed to the frontend. Output bytes are base64 so that multi-byte characters
/// split across packets are never corrupted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalEvent {
    Data {
        #[serde(serialize_with = "as_base64")]
        data: Vec<u8>,
    },
    Closed {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

pub(crate) enum Command {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Close,
}

#[derive(Clone)]
pub struct TerminalHandle {
    tx: mpsc::Sender<Command>,
    /// Wakes the session even when the input queue is full, so a close request is never lost.
    closer: Arc<Notify>,
}

fn send(tx: &mpsc::Sender<Command>, cmd: Command) -> Result<(), AppError> {
    tx.try_send(cmd).map_err(|e| match e {
        TrySendError::Full(_) => AppError::connection("The terminal is busy. Try again."),
        TrySendError::Closed(_) => AppError::connection("The terminal session has ended."),
    })
}

impl TerminalHandle {
    pub fn write(&self, bytes: Vec<u8>) -> Result<(), AppError> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(AppError::validation("data", "Input is too large."));
        }
        send(&self.tx, Command::Input(bytes))
    }

    pub fn resize(&self, cols: u32, rows: u32) -> Result<(), AppError> {
        let (cols, rows) = validate_size(cols, rows)?;
        send(&self.tx, Command::Resize { cols, rows })
    }

    /// Asks the session to end. Safe to call on an already-ended session.
    pub fn close(&self) {
        let _ = self.tx.try_send(Command::Close);
        self.closer.notify_one();
    }

    /// A handle that is not connected to any session, for tests of code that holds handles.
    #[cfg(test)]
    pub(crate) fn detached() -> (Self, mpsc::Receiver<Command>, Arc<Notify>) {
        let (tx, rx) = mpsc::channel(QUEUE);
        let closer = Arc::new(Notify::new());
        (
            Self {
                tx,
                closer: closer.clone(),
            },
            rx,
            closer,
        )
    }
}

pub fn validate_size(cols: u32, rows: u32) -> Result<(u16, u16), AppError> {
    if !(1..=MAX_COLS).contains(&cols) || !(1..=MAX_ROWS).contains(&rows) {
        return Err(AppError::validation(
            "size",
            "Terminal size is out of range.",
        ));
    }
    Ok((cols as u16, rows as u16))
}

/// Opens a shell. Returns a user-facing message on failure (never containing secrets).
pub async fn start(
    params: TestParams,
    cols: u16,
    rows: u16,
    events: mpsc::Sender<TerminalEvent>,
) -> Result<TerminalHandle, String> {
    if params.stored.is_none() {
        return Err("This server's host key is not trusted yet. Run a connection test on the Servers page and trust the host key first.".into());
    }
    if matches!(params.auth, AuthSecret::None) {
        return Err("No credential is stored for this server. Edit the server to add one.".into());
    }

    let seen = Arc::new(Mutex::new(Seen::default()));
    let session = match tokio::time::timeout(
        CONNECT_TIMEOUT,
        connect_authenticated(&params, seen.clone(), true),
    )
    .await
    {
        Ok(Ok(session)) => session,
        Ok(Err(failure)) => return Err(explain(&failure, &params, &seen).1),
        Err(_) => {
            return Err(format!(
                "No response from {}:{} within {} seconds.",
                params.host,
                params.port,
                CONNECT_TIMEOUT.as_secs()
            ));
        }
    };

    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("Could not open a session: {e}"))?;
    channel
        .request_pty(true, "xterm-256color", cols.into(), rows.into(), 0, 0, &[])
        .await
        .map_err(|e| format!("Could not allocate a terminal: {e}"))?;
    channel
        .request_shell(true)
        .await
        .map_err(|e| format!("Could not start a shell: {e}"))?;

    let (tx, mut rx) = mpsc::channel::<Command>(QUEUE);
    let closer = Arc::new(Notify::new());
    let closed = closer.clone();
    tokio::spawn(async move {
        let mut reason: Option<String> = None;
        loop {
            tokio::select! {
                _ = closed.notified() => break,
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        // Awaiting here applies backpressure to the remote when the UI is slow.
                        if events.send(TerminalEvent::Data { data: data.to_vec() }).await.is_err() {
                            break; // frontend went away
                        }
                    }
                    Some(ChannelMsg::Failure) => {
                        reason = Some("The server refused to start a shell.".into());
                        break;
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        reason = Some(format!("The shell exited with status {exit_status}."));
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                },
                cmd = rx.recv() => match cmd {
                    Some(Command::Input(bytes)) => {
                        if channel.data_bytes(bytes).await.is_err() {
                            reason = Some("The connection was lost.".into());
                            break;
                        }
                    }
                    Some(Command::Resize { cols, rows }) => {
                        let _ = channel.window_change(cols.into(), rows.into(), 0, 0).await;
                    }
                    Some(Command::Close) | None => break,
                }
            }
        }
        let _ = channel.close().await;
        let _ = session
            .disconnect(Disconnect::ByApplication, "terminal closed", "en")
            .await;
        let _ = events.send(TerminalEvent::Closed { reason }).await;
    });

    Ok(TerminalHandle { tx, closer })
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn size_limits() {
        assert_eq!(validate_size(80, 24).unwrap(), (80, 24));
        assert_eq!(validate_size(1, 1).unwrap(), (1, 1));
        for (c, r) in [(0, 24), (80, 0), (1001, 24), (80, 1001), (u32::MAX, 5)] {
            assert!(validate_size(c, r).is_err(), "{c}x{r}");
        }
    }

    #[test]
    fn events_serialize_as_tagged_base64_json() {
        let d = serde_json::to_string(&TerminalEvent::Data {
            data: b"hi".to_vec(),
        })
        .unwrap();
        assert_eq!(d, r#"{"type":"data","data":"aGk="}"#);
        let c = serde_json::to_string(&TerminalEvent::Closed { reason: None }).unwrap();
        assert_eq!(c, r#"{"type":"closed"}"#);
        let c = serde_json::to_string(&TerminalEvent::Closed {
            reason: Some("x".into()),
        })
        .unwrap();
        assert_eq!(c, r#"{"type":"closed","reason":"x"}"#);
    }

    #[tokio::test]
    async fn handle_rejects_oversized_input_and_reports_ended_session() {
        let (tx, rx) = mpsc::channel(1);
        let h = TerminalHandle {
            tx,
            closer: Arc::new(Notify::new()),
        };
        assert!(h.write(vec![0; MAX_INPUT_BYTES + 1]).is_err());
        assert!(h.write(vec![1]).is_ok());
        assert!(h.write(vec![1]).is_err(), "full queue reports busy");
        drop(rx);
        assert!(h.write(vec![1]).is_err(), "closed session reports ended");
        assert!(h.resize(0, 0).is_err());
        h.close(); // never panics
    }

    #[tokio::test]
    async fn close_is_delivered_even_when_the_input_queue_is_full() {
        let (h, _rx, closer) = TerminalHandle::detached();
        for _ in 0..QUEUE {
            h.write(vec![1]).unwrap();
        }
        assert!(h.write(vec![1]).is_err(), "queue is full");
        h.close();
        // notify_one stores a permit, so a session that checks later still sees the close.
        tokio::time::timeout(Duration::from_secs(1), closer.notified())
            .await
            .expect("close was signalled");
    }
}
