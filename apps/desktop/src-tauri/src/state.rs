use crate::alerts::engine::AlertEngine;
use crate::credentials::CredentialStore;
use crate::error::AppError;
use crate::ssh::monitor::MonitorHandle;
use crate::ssh::service::PendingKeys;
use crate::ssh::terminal::TerminalHandle;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

/// An open terminal, remembered with its server so it can be closed when that server's trust or
/// credentials change.
pub struct TerminalEntry {
    pub server_id: String,
    pub handle: TerminalHandle,
}

pub type TerminalSessions = Arc<Mutex<HashMap<String, TerminalEntry>>>;

/// A running monitor. `token` lets a finished monitor remove only its own registry entry.
pub struct MonitorEntry {
    pub token: String,
    pub handle: MonitorHandle,
}

pub type MonitorRegistry = Arc<Mutex<HashMap<String, MonitorEntry>>>;

/// Servers with a Docker listing in flight.
pub type DockerBusy = Arc<Mutex<HashSet<String>>>;

/// Held while a Docker listing runs for a server; releases it when dropped, however the request
/// ends (success, error or cancellation).
pub struct DockerGuard {
    busy: DockerBusy,
    id: String,
}

impl Drop for DockerGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = self.busy.lock() {
            set.remove(&self.id);
        }
    }
}

fn claim_docker(busy: &DockerBusy, id: &str) -> Option<DockerGuard> {
    if !busy.lock().ok()?.insert(id.to_string()) {
        return None;
    }
    Some(DockerGuard {
        busy: busy.clone(),
        id: id.to_string(),
    })
}

fn close_terminals_in(sessions: &TerminalSessions, server_id: &str) {
    // Remove first so nothing more can be written to a revoked session, then ask each to end.
    let closing: Vec<TerminalEntry> = match sessions.lock() {
        Ok(mut map) => {
            let ids: Vec<String> = map
                .iter()
                .filter(|(_, e)| e.server_id == server_id)
                .map(|(k, _)| k.clone())
                .collect();
            ids.iter().filter_map(|k| map.remove(k)).collect()
        }
        Err(_) => return,
    };
    for entry in closing {
        entry.handle.close();
    }
}

pub struct AppState {
    /// Holds the init error instead of a connection when the database could not be opened,
    /// so commands can report it to the UI rather than the whole app failing to start.
    pub db: Result<Mutex<Connection>, AppError>,
    pub credentials: Box<dyn CredentialStore>,
    /// Host keys awaiting user confirmation (in memory only).
    pub pending_host_keys: Mutex<PendingKeys>,
    /// Open terminal sessions by id.
    pub terminals: TerminalSessions,
    /// Running monitors by server id.
    pub monitors: MonitorRegistry,
    /// What is firing right now. Never held across an await.
    pub alerts: Mutex<AlertEngine>,
    /// Servers with a Docker listing in flight (at most one per server).
    pub docker_busy: DockerBusy,
}

impl AppState {
    /// Claims the Docker listing slot for a server, or `None` if one is already running.
    pub fn begin_docker(&self, id: &str) -> Option<DockerGuard> {
        claim_docker(&self.docker_busy, id)
    }

    pub fn engine(&self) -> Result<MutexGuard<'_, AlertEngine>, AppError> {
        self.alerts
            .lock()
            .map_err(|_| AppError::internal("Alert state lock was poisoned."))
    }

    pub fn conn(&self) -> Result<MutexGuard<'_, Connection>, AppError> {
        match &self.db {
            Ok(m) => m
                .lock()
                .map_err(|_| AppError::internal("Database lock was poisoned.")),
            Err(e) => Err(e.clone()),
        }
    }

    pub fn terminals(&self) -> Result<MutexGuard<'_, HashMap<String, TerminalEntry>>, AppError> {
        self.terminals
            .lock()
            .map_err(|_| AppError::internal("Terminal state lock was poisoned."))
    }

    pub fn terminal_sessions(&self) -> TerminalSessions {
        self.terminals.clone()
    }

    pub fn monitors(&self) -> Result<MutexGuard<'_, HashMap<String, MonitorEntry>>, AppError> {
        self.monitors
            .lock()
            .map_err(|_| AppError::internal("Monitor state lock was poisoned."))
    }

    pub fn monitor_registry(&self) -> MonitorRegistry {
        self.monitors.clone()
    }

    /// Stops monitoring a server (no-op if it is not monitored).
    pub fn stop_monitor(&self, id: &str, reason: &str) {
        if let Ok(mut map) = self.monitors.lock()
            && let Some(entry) = map.remove(id)
        {
            entry.handle.stop(reason);
        }
    }

    /// Ends every open terminal of a server (no-op if it has none). Used whenever the server is
    /// deleted or its host, credential or trusted key changes, so a session opened under the old
    /// settings cannot outlive them.
    pub fn close_terminals(&self, server_id: &str) {
        close_terminals_in(&self.terminals, server_id);
    }

    /// Revokes everything running against a server: its terminals and its monitor.
    pub fn revoke_server(&self, server_id: &str, reason: &str) {
        self.close_terminals(server_id);
        self.stop_monitor(server_id, reason);
    }

    pub fn pending(&self) -> Result<MutexGuard<'_, PendingKeys>, AppError> {
        self.pending_host_keys
            .lock()
            .map_err(|_| AppError::internal("Host key state lock was poisoned."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_docker_listing_per_server_at_a_time() {
        let busy = DockerBusy::default();
        let first = claim_docker(&busy, "s1");
        assert!(first.is_some());
        assert!(claim_docker(&busy, "s1").is_none(), "same server is busy");
        assert!(
            claim_docker(&busy, "s2").is_some(),
            "other servers are independent"
        );
    }

    #[tokio::test]
    async fn closing_terminals_affects_only_that_server() {
        let sessions = TerminalSessions::default();
        let (a, _ra, closed_a) = TerminalHandle::detached();
        let (b, _rb, closed_b) = TerminalHandle::detached();
        for (sid, server, handle) in [("t1", "s1", a), ("t2", "s2", b)] {
            sessions.lock().unwrap().insert(
                sid.into(),
                TerminalEntry {
                    server_id: server.into(),
                    handle,
                },
            );
        }
        close_terminals_in(&sessions, "s1");
        {
            let map = sessions.lock().unwrap();
            assert!(!map.contains_key("t1") && map.contains_key("t2"));
        }
        let signalled = |n: Arc<tokio::sync::Notify>| async move {
            tokio::time::timeout(std::time::Duration::from_millis(100), n.notified())
                .await
                .is_ok()
        };
        assert!(signalled(closed_a).await, "s1 terminal was told to close");
        assert!(!signalled(closed_b).await, "s2 terminal was left alone");
        close_terminals_in(&sessions, "nobody"); // no-op
    }

    #[test]
    fn the_slot_is_released_when_the_guard_is_dropped() {
        let busy = DockerBusy::default();
        drop(claim_docker(&busy, "s1"));
        assert!(claim_docker(&busy, "s1").is_some());
    }
}
