use crate::credentials::CredentialStore;
use crate::error::AppError;
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};

pub struct AppState {
    /// Holds the init error instead of a connection when the database could not be opened,
    /// so commands can report it to the UI rather than the whole app failing to start.
    pub db: Result<Mutex<Connection>, AppError>,
    pub credentials: Box<dyn CredentialStore>,
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
}
