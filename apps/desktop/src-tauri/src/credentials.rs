//! Credential store abstraction.
//!
//! Secrets (passwords, private keys) live only in the operating system's credential store
//! (Windows Credential Manager). SQLite holds just a *reference* string. The frontend can
//! write or clear a secret but can never read one back: there is intentionally no command
//! that returns secret material.

use crate::error::AppError;

const SERVICE: &str = "VPSPilot";

pub trait CredentialStore: Send + Sync {
    /// Stores (or replaces) the secret under `reference`.
    fn set(&self, reference: &str, secret: &str) -> Result<(), AppError>;
    /// Deletes the secret. Succeeds if nothing was stored.
    fn delete(&self, reference: &str) -> Result<(), AppError>;
    /// Reads a secret. Reserved for the future SSH layer in Rust; never exposed as a command.
    #[allow(dead_code)]
    fn get(&self, reference: &str) -> Result<Option<String>, AppError>;
}

/// Reference string stored in the database for a server's credential.
pub fn reference_for(server_id: &str) -> String {
    format!("vpspilot:server:{server_id}")
}

pub struct OsCredentialStore;

fn map_err(err: keyring::Error) -> AppError {
    // Do not include anything beyond the error kind; keyring errors never contain secrets,
    // but the UI only needs a generic message.
    eprintln!("credential store error: {err}");
    AppError::credential("The OS credential store reported an error.")
}

impl OsCredentialStore {
    fn entry(reference: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(SERVICE, reference).map_err(map_err)
    }
}

impl CredentialStore for OsCredentialStore {
    fn set(&self, reference: &str, secret: &str) -> Result<(), AppError> {
        Self::entry(reference)?
            .set_secret(secret.as_bytes())
            .map_err(map_err)
    }

    fn delete(&self, reference: &str) -> Result<(), AppError> {
        match Self::entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }

    fn get(&self, reference: &str) -> Result<Option<String>, AppError> {
        match Self::entry(reference)?.get_secret() {
            Ok(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| AppError::credential("Stored credential is not valid UTF-8.")),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory store for tests. Can be told to fail.
    #[derive(Default)]
    pub struct MemoryStore {
        pub items: Mutex<HashMap<String, String>>,
        pub fail: Mutex<bool>,
        /// Fail only `set` / only `delete`, to test compensation between SQLite and the store.
        pub fail_set: Mutex<bool>,
        pub fail_delete: Mutex<bool>,
    }

    impl CredentialStore for MemoryStore {
        fn set(&self, reference: &str, secret: &str) -> Result<(), AppError> {
            if *self.fail.lock().unwrap() || *self.fail_set.lock().unwrap() {
                return Err(AppError::credential("simulated failure"));
            }
            self.items
                .lock()
                .unwrap()
                .insert(reference.into(), secret.into());
            Ok(())
        }
        fn delete(&self, reference: &str) -> Result<(), AppError> {
            if *self.fail.lock().unwrap() || *self.fail_delete.lock().unwrap() {
                return Err(AppError::credential("simulated failure"));
            }
            self.items.lock().unwrap().remove(reference);
            Ok(())
        }
        fn get(&self, reference: &str) -> Result<Option<String>, AppError> {
            Ok(self.items.lock().unwrap().get(reference).cloned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_contains_no_secret_and_is_stable() {
        assert_eq!(reference_for("abc"), "vpspilot:server:abc");
    }

    /// Round-trips through the real OS credential store. Ignored by default because it touches
    /// the user's Windows Credential Manager; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn os_store_round_trip() {
        let store = OsCredentialStore;
        let r = format!("vpspilot:test:{}", uuid::Uuid::new_v4());
        assert_eq!(store.get(&r).unwrap(), None);
        store.set(&r, "s3cret").unwrap();
        assert_eq!(store.get(&r).unwrap().as_deref(), Some("s3cret"));
        store.delete(&r).unwrap();
        assert_eq!(store.get(&r).unwrap(), None);
        store.delete(&r).unwrap(); // idempotent
    }
}
