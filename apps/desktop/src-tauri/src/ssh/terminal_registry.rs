//! Registry of open terminal sessions, and the rules that keep it consistent.
//!
//! Three guarantees, all enforced under one lock:
//!
//! * **The session cap is atomic.** A slot is *reserved* before the (slow) SSH connect and only
//!   *committed* afterwards, so concurrent opens can never exceed the cap.
//! * **Revocation covers sessions that are still connecting.** Every server has a revision, and
//!   there is a global epoch. A reservation remembers both. Revoking a server (delete, credential
//!   change, trust change, identity edit) bumps its revision; closing everything (webview reload
//!   or window destroyed) bumps the epoch. Committing a slot whose revision or epoch moved on is
//!   refused, and the freshly opened shell is closed instead of registered.
//! * **Every session has an owner.** [`forward_events`] drops the session as soon as the frontend
//!   can no longer receive its output, even when the shell is idle.
//!
//! Independent of Tauri so all of it can be tested without a window.

use super::terminal::{TerminalEvent, TerminalHandle};
use crate::error::AppError;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::mpsc;

/// How often an idle session checks that the frontend can still receive output.
pub const OWNER_PROBE_INTERVAL: Duration = Duration::from_secs(30);

/// An open terminal, remembered with its server so it can be closed when that server's trust or
/// credentials change.
pub struct TerminalEntry {
    pub server_id: String,
    pub handle: TerminalHandle,
}

/// What a reservation remembers; a commit is valid only while this is still current.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Snapshot {
    epoch: u64,
    revision: u64,
}

#[derive(Default)]
pub struct TerminalRegistry {
    entries: HashMap<String, TerminalEntry>,
    /// Slots reserved by opens that are still connecting. They count toward the cap.
    reserved: usize,
    /// Bumped by [`TerminalRegistry::revoke_all`].
    epoch: u64,
    /// Source of unique, increasing revision numbers.
    counter: u64,
    /// Latest revision per server; a server never revoked has revision 0. Entries are kept even
    /// after a server is deleted, so an in-flight open can never match a recycled value.
    revisions: HashMap<String, u64>,
}

impl TerminalRegistry {
    fn snapshot(&self, server_id: &str) -> Snapshot {
        Snapshot {
            epoch: self.epoch,
            revision: self.revisions.get(server_id).copied().unwrap_or(0),
        }
    }

    fn bump(&mut self, server_id: &str) {
        self.counter += 1;
        self.revisions.insert(server_id.to_string(), self.counter);
    }

    /// Invalidates in-flight opens for a server and removes its sessions, returning them so the
    /// caller can close them after releasing the lock.
    fn revoke_server(&mut self, server_id: &str) -> Vec<TerminalEntry> {
        self.bump(server_id);
        let ids: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, e)| e.server_id == server_id)
            .map(|(k, _)| k.clone())
            .collect();
        ids.iter().filter_map(|k| self.entries.remove(k)).collect()
    }

    /// Invalidates every in-flight open and removes every session.
    fn revoke_all(&mut self) -> Vec<TerminalEntry> {
        self.epoch += 1;
        self.entries.drain().map(|(_, e)| e).collect()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn contains(&self, session_id: &str) -> bool {
        self.entries.contains_key(session_id)
    }
}

pub type TerminalSessions = Arc<Mutex<TerminalRegistry>>;

fn lock(sessions: &TerminalSessions) -> Result<MutexGuard<'_, TerminalRegistry>, AppError> {
    sessions
        .lock()
        .map_err(|_| AppError::internal("Terminal state lock was poisoned."))
}

/// The handle of a registered session, if it exists.
pub fn handle_of(
    sessions: &TerminalSessions,
    session_id: &str,
) -> Result<TerminalHandle, AppError> {
    lock(sessions)?
        .entries
        .get(session_id)
        .map(|e| e.handle.clone())
        .ok_or_else(|| AppError::not_found("Terminal session"))
}

/// Removes a finished session. Unknown ids are ignored.
pub fn forget(sessions: &TerminalSessions, session_id: &str) {
    if let Ok(mut reg) = sessions.lock() {
        reg.entries.remove(session_id);
    }
}

/// Ends every session of a server and invalidates opens still connecting to it.
pub fn revoke_server(sessions: &TerminalSessions, server_id: &str) {
    // Remove first so nothing more can be written to a revoked session, then ask each to end.
    let closing = match sessions.lock() {
        Ok(mut reg) => reg.revoke_server(server_id),
        Err(_) => return,
    };
    for entry in closing {
        entry.handle.close();
    }
}

/// Ends every session and invalidates every open still connecting.
pub fn revoke_all(sessions: &TerminalSessions) {
    let closing = match sessions.lock() {
        Ok(mut reg) => reg.revoke_all(),
        Err(_) => return,
    };
    for entry in closing {
        entry.handle.close();
    }
}

/// A reserved place in the registry. Dropping it without committing (connect failed, or the
/// command was cancelled) gives the place back.
pub struct TerminalSlot {
    sessions: TerminalSessions,
    server_id: String,
    snapshot: Snapshot,
    released: bool,
}

/// Reserves a place for a new session, or refuses when the cap is reached. Check and reservation
/// happen under one lock.
pub fn reserve(
    sessions: &TerminalSessions,
    server_id: &str,
    max: usize,
) -> Result<TerminalSlot, AppError> {
    let mut reg = lock(sessions)?;
    if reg.entries.len() + reg.reserved >= max {
        return Err(AppError::connection(format!(
            "At most {max} terminals can be open at once."
        )));
    }
    reg.reserved += 1;
    Ok(TerminalSlot {
        sessions: sessions.clone(),
        server_id: server_id.to_string(),
        snapshot: reg.snapshot(server_id),
        released: false,
    })
}

impl TerminalSlot {
    /// Registers the opened session. Fails, handing the handle back so the caller can close it,
    /// if the server was revoked (or everything was closed) since the slot was reserved.
    pub fn commit(
        mut self,
        session_id: String,
        handle: TerminalHandle,
    ) -> Result<(), TerminalHandle> {
        self.released = true;
        let Ok(mut reg) = self.sessions.lock() else {
            return Err(handle);
        };
        reg.reserved = reg.reserved.saturating_sub(1);
        if reg.snapshot(&self.server_id) != self.snapshot {
            return Err(handle);
        }
        reg.entries.insert(
            session_id,
            TerminalEntry {
                server_id: self.server_id.clone(),
                handle,
            },
        );
        Ok(())
    }
}

impl Drop for TerminalSlot {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        if let Ok(mut reg) = self.sessions.lock() {
            reg.reserved = reg.reserved.saturating_sub(1);
        }
    }
}

/// Reserves a slot, runs `connect` (which loads the server's details and opens the shell), and
/// registers the result under a new unguessable id.
///
/// The slot, and with it the revision, is taken *before* `connect` reads anything, so a
/// revocation that lands at any point afterwards, even immediately before registration, is
/// noticed. In that case the new shell is closed and an error is returned.
pub async fn open_session<F, Fut>(
    sessions: &TerminalSessions,
    server_id: &str,
    max: usize,
    connect: F,
) -> Result<(String, TerminalHandle), AppError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<TerminalHandle, AppError>>,
{
    let slot = reserve(sessions, server_id, max)?;
    let handle = connect().await?;
    let session_id = uuid::Uuid::new_v4().to_string();
    match slot.commit(session_id.clone(), handle.clone()) {
        Ok(()) => Ok((session_id, handle)),
        Err(handle) => {
            handle.close();
            Err(AppError::connection(
                "This server's settings or access changed while connecting. Open the terminal again.",
            ))
        }
    }
}

/// Delivers a session's events to its owner (the frontend) and unregisters the session when it
/// ends. `send` returns false when the owner can no longer receive.
///
/// While the shell is idle nothing would be sent, so a vanished owner (reloaded, crashed or
/// destroyed webview) would go unnoticed. A harmless empty-output event is therefore sent every
/// `probe` interval; if it cannot be delivered the shell is closed.
pub async fn forward_events(
    mut rx: mpsc::Receiver<TerminalEvent>,
    handle: TerminalHandle,
    mut send: impl FnMut(TerminalEvent) -> bool,
    probe: Duration,
) {
    let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + probe, probe);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Some(event) => {
                    if !send(event) {
                        handle.close(); // the frontend is gone; end the remote shell
                    }
                }
                None => break,
            },
            _ = ticker.tick() => {
                if !send(TerminalEvent::Data { data: Vec::new() }) {
                    handle.close();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::sync::Notify;

    fn sessions() -> TerminalSessions {
        TerminalSessions::default()
    }

    fn detached() -> (TerminalHandle, Arc<Notify>) {
        let (handle, _rx, closer) = TerminalHandle::detached();
        (handle, closer)
    }

    async fn signalled(n: &Arc<Notify>) -> bool {
        tokio::time::timeout(Duration::from_millis(100), n.notified())
            .await
            .is_ok()
    }

    #[tokio::test]
    async fn revoking_a_server_affects_only_that_server() {
        let s = sessions();
        let (a, closed_a) = detached();
        let (b, closed_b) = detached();
        for (sid, server, handle) in [("t1", "s1", a), ("t2", "s2", b)] {
            let slot = reserve(&s, server, 8).unwrap();
            assert!(slot.commit(sid.into(), handle).is_ok());
        }
        revoke_server(&s, "s1");
        {
            let reg = s.lock().unwrap();
            assert!(!reg.contains("t1") && reg.contains("t2"));
        }
        assert!(signalled(&closed_a).await, "s1 terminal was told to close");
        assert!(!signalled(&closed_b).await, "s2 terminal was left alone");
        revoke_server(&s, "nobody"); // no-op
    }

    #[test]
    fn the_cap_counts_reservations_so_concurrent_opens_cannot_exceed_it() {
        let s = sessions();
        let a = reserve(&s, "s1", 2).unwrap();
        let _b = reserve(&s, "s2", 2).unwrap();
        // Neither has connected yet, but both places are taken.
        assert!(reserve(&s, "s3", 2).is_err());
        drop(a); // a failed or cancelled open gives its place back
        let c = reserve(&s, "s3", 2).unwrap();
        let (h, _) = detached();
        assert!(c.commit("t".into(), h).is_ok());
        assert!(
            reserve(&s, "s4", 2).is_err(),
            "one session plus one reservation"
        );
    }

    #[test]
    fn many_threads_racing_for_the_last_places_never_exceed_the_cap() {
        let s = sessions();
        let granted = Arc::new(AtomicUsize::new(0));
        let slots = Arc::new(Mutex::new(Vec::new()));
        let threads: Vec<_> = (0..32)
            .map(|i| {
                let (s, granted, slots) = (s.clone(), granted.clone(), slots.clone());
                std::thread::spawn(move || {
                    if let Ok(slot) = reserve(&s, &format!("server-{i}"), 8) {
                        granted.fetch_add(1, Ordering::SeqCst);
                        slots.lock().unwrap().push(slot);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(granted.load(Ordering::SeqCst), 8);
    }

    #[test]
    fn a_session_that_ends_frees_its_place() {
        let s = sessions();
        let (h, _) = detached();
        reserve(&s, "s1", 1)
            .unwrap()
            .commit("t1".into(), h)
            .unwrap_or_else(|_| panic!("commit"));
        assert!(reserve(&s, "s2", 1).is_err());
        forget(&s, "t1");
        assert!(reserve(&s, "s2", 1).is_ok());
    }

    #[tokio::test]
    async fn revocation_while_connecting_stops_the_new_session_from_registering() {
        let s = sessions();
        let (handle, closed) = detached();
        let s2 = s.clone();
        let result = open_session(&s, "s1", 8, || async move {
            // The server is revoked while the SSH connection is still being established.
            revoke_server(&s2, "s1");
            Ok(handle)
        })
        .await;
        assert!(result.is_err());
        assert_eq!(s.lock().unwrap().len(), 0, "nothing was registered");
        assert!(signalled(&closed).await, "the opened shell was closed");
        assert!(reserve(&s, "s1", 1).is_ok(), "the reservation was released");
    }

    #[tokio::test]
    async fn revocation_just_before_insertion_is_caught() {
        // Same race, at its narrowest: everything has finished and only the commit is left.
        let s = sessions();
        let slot = reserve(&s, "s1", 8).unwrap();
        let (handle, _) = detached();
        revoke_server(&s, "s1");
        assert!(slot.commit("t1".into(), handle).is_err());
        assert_eq!(s.lock().unwrap().len(), 0);
        assert_eq!(s.lock().unwrap().reserved, 0);
    }

    #[tokio::test]
    async fn revoking_one_server_does_not_cancel_opens_for_another() {
        let s = sessions();
        let (handle, _) = detached();
        let s2 = s.clone();
        let result = open_session(&s, "s1", 8, || async move {
            revoke_server(&s2, "other");
            Ok(handle)
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(s.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_revision_is_not_reused_after_the_server_is_revoked_twice() {
        let s = sessions();
        let slot = reserve(&s, "s1", 8).unwrap();
        revoke_server(&s, "s1");
        revoke_server(&s, "s1");
        let (h, _) = detached();
        assert!(slot.commit("t".into(), h).is_err());
        // A reservation taken after the revocations is valid.
        let slot = reserve(&s, "s1", 8).unwrap();
        let (h, _) = detached();
        assert!(slot.commit("t".into(), h).is_ok());
    }

    #[tokio::test]
    async fn destroying_the_window_closes_sessions_and_in_flight_opens() {
        let s = sessions();
        let (open, closed_open) = detached();
        reserve(&s, "s1", 8)
            .unwrap()
            .commit("t1".into(), open)
            .unwrap_or_else(|_| panic!("commit"));

        let (late, closed_late) = detached();
        let s2 = s.clone();
        let result = open_session(&s, "s2", 8, || async move {
            revoke_all(&s2); // the window goes away mid-connect
            Ok(late)
        })
        .await;

        assert!(result.is_err());
        assert_eq!(s.lock().unwrap().len(), 0);
        assert!(signalled(&closed_open).await, "the open session was closed");
        assert!(
            signalled(&closed_late).await,
            "the connecting one was closed"
        );
        // The app keeps working afterwards.
        assert!(reserve(&s, "s1", 8).is_ok());
    }

    #[tokio::test]
    async fn an_idle_session_is_closed_when_its_owner_vanishes() {
        let (handle, _rx_unused, closed) = TerminalHandle::detached();
        let (_tx, rx) = mpsc::channel::<TerminalEvent>(4);
        let alive = Arc::new(AtomicBool::new(true));
        let probe_alive = alive.clone();
        let task = tokio::spawn(forward_events(
            rx,
            handle,
            move |_| probe_alive.load(Ordering::SeqCst),
            Duration::from_millis(20),
        ));

        // Owner present: probes succeed and the session stays open.
        assert!(!signalled(&closed).await);
        // Owner gone (reloaded or destroyed webview) while the shell is idle: nothing is sent
        // by the shell, yet the session is still closed.
        alive.store(false, Ordering::SeqCst);
        assert!(signalled(&closed).await);
        task.abort();
    }

    #[tokio::test]
    async fn events_are_forwarded_in_order_and_the_loop_ends_with_the_session() {
        let (handle, _rx_unused, _closed) = TerminalHandle::detached();
        let (tx, rx) = mpsc::channel(4);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let task = tokio::spawn(forward_events(
            rx,
            handle,
            move |e| {
                sink.lock().unwrap().push(e);
                true
            },
            Duration::from_secs(3600),
        ));
        tx.send(TerminalEvent::Data {
            data: b"a".to_vec(),
        })
        .await
        .unwrap();
        tx.send(TerminalEvent::Closed { reason: None })
            .await
            .unwrap();
        drop(tx);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("loop ended")
            .unwrap();
        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                TerminalEvent::Data {
                    data: b"a".to_vec()
                },
                TerminalEvent::Closed { reason: None },
            ]
        );
    }
}
