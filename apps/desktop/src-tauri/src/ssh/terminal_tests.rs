//! End-to-end terminal tests against the in-process SSH server (see client_tests.rs).

use super::client::{AuthSecret, TestParams};
use super::client_tests::{Running, fingerprint, new_key, params, start_server, stored};
use super::terminal::{TerminalEvent, TerminalHandle, start};
use super::terminal_registry::{TerminalSessions, forget, handle_of, open_session, revoke_server};
use crate::error::AppError;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

const PASSWORD: &str = "correct-pw";

fn trusted_params(s: &Running) -> TestParams {
    params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&s.host_key)),
    )
}

async fn open(s: &Running) -> (TerminalHandle, mpsc::Receiver<TerminalEvent>) {
    let (tx, rx) = mpsc::channel(64);
    let handle = start(trusted_params(s), 80, 24, tx)
        .await
        .expect("terminal starts");
    (handle, rx)
}

/// Collects output until `needle` appears, or fails after a few seconds.
async fn output_until(rx: &mut mpsc::Receiver<TerminalEvent>, needle: &str) -> String {
    let mut out = String::new();
    loop {
        match timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(TerminalEvent::Data { data })) => {
                out.push_str(&String::from_utf8_lossy(&data));
                if out.contains(needle) {
                    return out;
                }
            }
            other => panic!("did not see {needle:?}; got {other:?} after output {out:?}"),
        }
    }
}

async fn wait_for_log(s: &Running, entry: &str) {
    for _ in 0..50 {
        if s.log.lock().unwrap().iter().any(|l| l == entry) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "server never logged {entry:?}; log = {:?}",
        s.log.lock().unwrap()
    );
}

#[tokio::test]
async fn opens_a_shell_with_the_requested_pty_and_streams_io() {
    let s = start_server(None).await;
    let (handle, mut rx) = open(&s).await;

    output_until(&mut rx, "welcome").await;
    wait_for_log(&s, "pty xterm-256color 80x24").await;

    handle.write(b"ls\n".to_vec()).unwrap();
    output_until(&mut rx, "echo:ls").await;
    assert_eq!(s.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn resize_is_forwarded_to_the_server() {
    let s = start_server(None).await;
    let (handle, mut rx) = open(&s).await;
    output_until(&mut rx, "welcome").await;

    handle.resize(120, 40).unwrap();
    wait_for_log(&s, "resize 120x40").await;
}

#[tokio::test]
async fn client_close_emits_closed_and_ends_the_session() {
    let s = start_server(None).await;
    let (handle, mut rx) = open(&s).await;
    output_until(&mut rx, "welcome").await;

    handle.close();
    loop {
        match timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(TerminalEvent::Closed { .. })) => break,
            Ok(Some(_)) => continue,
            other => panic!("expected Closed, got {other:?}"),
        }
    }
    // Once ended, writes are rejected rather than silently dropped.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(handle.write(b"x".to_vec()).is_err());
}

#[tokio::test]
async fn shell_exit_on_the_server_emits_closed() {
    let s = start_server(None).await;
    let (handle, mut rx) = open(&s).await;
    output_until(&mut rx, "welcome").await;

    handle.write(b"exit\n".to_vec()).unwrap();
    loop {
        match timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(TerminalEvent::Closed { .. })) => break,
            Ok(Some(_)) => continue,
            other => panic!("expected Closed, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn refuses_an_untrusted_host_without_sending_anything() {
    let s = start_server(None).await;
    let (tx, _rx) = mpsc::channel(8);
    let p = params(s.port, AuthSecret::Password(PASSWORD.into()), None);
    let err = start(p, 80, 24, tx).await.err().expect("must refuse");
    assert!(err.contains("not trusted"), "{err}");
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn refuses_a_changed_host_key_before_authentication() {
    let s = start_server(None).await;
    let (tx, _rx) = mpsc::channel(8);
    let p = params(
        s.port,
        AuthSecret::Password(PASSWORD.into()),
        stored(&fingerprint(&new_key())),
    );
    let err = start(p, 80, 24, tx).await.err().expect("must refuse");
    assert!(err.contains("does not match"), "{err}");
    assert_eq!(s.attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn refuses_when_no_credential_is_stored() {
    let s = start_server(None).await;
    let (tx, _rx) = mpsc::channel(8);
    let p = params(s.port, AuthSecret::None, stored(&fingerprint(&s.host_key)));
    let err = start(p, 80, 24, tx).await.err().expect("must refuse");
    assert!(err.contains("No credential"), "{err}");
}

#[tokio::test]
async fn wrong_password_is_reported_and_no_shell_opens() {
    let s = start_server(None).await;
    let (tx, mut rx) = mpsc::channel(8);
    let p = params(
        s.port,
        AuthSecret::Password("wrong".into()),
        stored(&fingerprint(&s.host_key)),
    );
    let err = start(p, 80, 24, tx).await.err().expect("must fail");
    assert!(err.contains("rejected"), "{err}");
    assert!(rx.try_recv().is_err(), "no output for a failed login");
}

async fn expect_closed(rx: &mut mpsc::Receiver<TerminalEvent>) {
    loop {
        match timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(TerminalEvent::Closed { .. })) => return,
            Ok(Some(_)) => continue,
            other => panic!("expected Closed, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn registered_terminal_works_normally_and_is_forgotten_when_it_ends() {
    let s = start_server(None).await;
    let sessions = TerminalSessions::default();
    let (tx, mut rx) = mpsc::channel(64);
    let params = trusted_params(&s);
    let (sid, _handle) = open_session(&sessions, "srv", 8, || async {
        start(params, 80, 24, tx)
            .await
            .map_err(AppError::connection)
    })
    .await
    .expect("opens and registers");

    output_until(&mut rx, "welcome").await;
    handle_of(&sessions, &sid)
        .unwrap()
        .write(
            b"ls
"
            .to_vec(),
        )
        .unwrap();
    output_until(&mut rx, "echo:ls").await;

    handle_of(&sessions, &sid).unwrap().close();
    expect_closed(&mut rx).await;
    forget(&sessions, &sid);
    assert!(handle_of(&sessions, &sid).is_err());
}

#[tokio::test]
async fn revoking_during_a_real_connection_leaves_no_session_behind() {
    let s = start_server(None).await;
    let sessions = TerminalSessions::default();
    let (tx, mut rx) = mpsc::channel(64);
    let params = trusted_params(&s);
    let revoker = sessions.clone();
    let result = open_session(&sessions, "srv", 8, || async move {
        // Revoked after the open began but before the SSH login completes.
        revoke_server(&revoker, "srv");
        start(params, 80, 24, tx)
            .await
            .map_err(AppError::connection)
    })
    .await;

    assert!(result.is_err(), "the revoked open must be refused");
    // The shell that did get opened on the server is ended, not leaked.
    expect_closed(&mut rx).await;
    assert!(
        open_session(&sessions, "srv", 1, || async {
            Ok(TerminalHandle::detached().0)
        })
        .await
        .is_ok(),
        "the place was released and a later open works"
    );
}

#[tokio::test]
async fn revoking_right_after_login_but_before_registration_closes_the_shell() {
    let s = start_server(None).await;
    let sessions = TerminalSessions::default();
    let (tx, mut rx) = mpsc::channel(64);
    let params = trusted_params(&s);
    let revoker = sessions.clone();
    let result = open_session(&sessions, "srv", 8, || async move {
        let handle = start(params, 80, 24, tx)
            .await
            .map_err(AppError::connection)?;
        revoke_server(&revoker, "srv"); // lands just before the insert
        Ok(handle)
    })
    .await;

    assert!(result.is_err());
    expect_closed(&mut rx).await;
}
