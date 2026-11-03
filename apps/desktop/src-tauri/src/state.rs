use crate::credentials::CredentialStore;
use crate::error::AppError;
use crate::ssh::service::PendingKeys;
use crate::ssh::terminal::TerminalHandle;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

pub type TerminalSessions = Arc<Mutex<HashMap<String, TerminalHandle>>>;

pub struct AppState {
    /// Holds the init error instead of a connection when the database could not be opened,
    /// so commands can report it to the UI rather than the whole app failing to start.
    pub db: Result<Mutex<Connection>, AppError>,
    pub credentials: Box<dyn CredentialStore>,
    /// Host keys awaiting user confirmation (in memory only).
    pub pending_host_keys: Mutex<PendingKeys>,
    /// Open terminal sessions by id.
    pub terminals: TerminalSessions,
}

impl AppState {
    pub fn conn(&self) -> Result<MutexGuard<'_, Connection>, AppError> {
        match &self.db {
            Ok(m) => m
                .lock()
                .map_err(|_| AppError::internal("Database lock was poisoned.")),
            Err(e) => Err(e.clone()),
        }
    }

    pub fn terminals(&self) -> Result<MutexGuard<'_, HashMap<String, TerminalHandle>>, AppError> {
        self.terminals
            .lock()
            .map_err(|_| AppError::internal("Terminal state lock was poisoned."))
    }

    pub fn terminal_sessions(&self) -> TerminalSessions {
        self.terminals.clone()
    }

    pub fn pending(&self) -> Result<MutexGuard<'_, PendingKeys>, AppError> {
        self.pending_host_keys
            .lock()
            .map_err(|_| AppError::internal("Host key state lock was poisoned."))
    }
}
