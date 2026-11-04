//! Use-cases around connection tests and host key trust. Independent of Tauri and the network,
//! so they can be unit tested with an in-memory database and credential store.

use super::client::{AuthSecret, DEFAULT_TIMEOUT, TestParams};
use super::hostkey::{HostKeyRecord, ObservedKey};
use super::known_hosts;
use crate::credentials::CredentialStore;
use crate::error::AppError;
use crate::servers::model::AuthMethod;
use crate::servers::repo;
use rusqlite::Connection;
use std::collections::HashMap;

/// Pending host keys, keyed by server id: the key a server presented on its last test while
/// not yet trusted. Only a fingerprint matching this entry can be trusted.
pub type PendingKeys = HashMap<String, ObservedKey>;

/// Loads everything a connection (test or terminal) needs from storage. The secret is read here, inside Rust.
pub fn prepare_connection(
    conn: &Connection,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<TestParams, AppError> {
    let server = repo::get(conn, id)?.ok_or_else(|| AppError::not_found("Server"))?;
    let secret = match &server.credential_ref {
        Some(reference) => store.get(reference)?,
        None => None,
    };
    let auth = match (secret, server.auth_method) {
        (None, _) => AuthSecret::None,
        (Some(s), AuthMethod::Password) => AuthSecret::Password(s),
        (Some(s), AuthMethod::SshKey) => AuthSecret::PrivateKey(s),
    };
    Ok(TestParams {
        host: server.host,
        port: server.port,
        username: server.username,
        auth,
        stored: known_hosts::get(conn, id)?,
        timeout: DEFAULT_TIMEOUT,
    })
}

/// Trusts the pending key for a server, but only if the caller's fingerprint matches it exactly.
pub fn trust_pending(
    conn: &Connection,
    pending: &mut PendingKeys,
    server_id: &str,
    fingerprint: &str,
    now: i64,
) -> Result<HostKeyRecord, AppError> {
    repo::get(conn, server_id)?.ok_or_else(|| AppError::not_found("Server"))?;
    let observed = pending.get(server_id).ok_or_else(|| {
        AppError::validation(
            "fingerprint",
            "No host key is awaiting confirmation. Run a connection test first.",
        )
    })?;
    if observed.fingerprint != fingerprint {
        return Err(AppError::validation(
            "fingerprint",
            "The fingerprint does not match the key the server presented. Run the test again.",
        ));
    }
    known_hosts::upsert(conn, server_id, observed, now)?;
    pending.remove(server_id);
    known_hosts::get(conn, server_id)?
        .ok_or_else(|| AppError::internal("Trusted host key could not be read back."))
}

pub fn forget(
    conn: &Connection,
    pending: &mut PendingKeys,
    server_id: &str,
) -> Result<(), AppError> {
    repo::get(conn, server_id)?.ok_or_else(|| AppError::not_found("Server"))?;
    known_hosts::delete(conn, server_id)?;
    pending.remove(server_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::testing::MemoryStore;
    use crate::db::open_in_memory;
    use crate::error::ErrorCode;
    use crate::servers::model::ServerInput;
    use crate::servers::service as servers;

    fn input(auth: AuthMethod) -> ServerInput {
        ServerInput {
            name: "web".into(),
            host: "web.example".into(),
            port: 2222,
            username: "deploy".into(),
            auth_method: auth,
            description: None,
        }
    }

    fn key(fp: &str) -> ObservedKey {
        ObservedKey {
            key_type: "ssh-ed25519".into(),
            fingerprint: fp.into(),
        }
    }

    #[test]
    fn prepare_reads_secret_and_maps_auth_method() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = servers::add_server(&conn, &input(AuthMethod::Password), 0).unwrap();
        let p = prepare_connection(&conn, &store, &s.id).unwrap();
        assert!(matches!(p.auth, AuthSecret::None));
        assert_eq!(
            (p.host.as_str(), p.port, p.username.as_str()),
            ("web.example", 2222, "deploy")
        );

        servers::set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        assert!(
            matches!(prepare_connection(&conn, &store, &s.id).unwrap().auth, AuthSecret::Password(ref x) if x == "pw")
        );

        let k = servers::add_server(&conn, &input(AuthMethod::SshKey), 0).unwrap();
        servers::set_credential(&conn, &store, &k.id, "KEY", 1).unwrap();
        assert!(
            matches!(prepare_connection(&conn, &store, &k.id).unwrap().auth, AuthSecret::PrivateKey(ref x) if x == "KEY")
        );
    }

    #[test]
    fn prepare_unknown_server_is_not_found_and_debug_hides_secret() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        assert_eq!(
            prepare_connection(&conn, &store, "nope").unwrap_err().code,
            ErrorCode::NotFound
        );
        let s = AuthSecret::Password("hunter2".into());
        assert!(!format!("{s:?}").contains("hunter2"));
    }

    #[test]
    fn trust_requires_matching_pending_fingerprint() {
        let conn = open_in_memory();
        let s = servers::add_server(&conn, &input(AuthMethod::Password), 0).unwrap();
        let mut pending = PendingKeys::new();

        // nothing pending
        let e = trust_pending(&conn, &mut pending, &s.id, "SHA256:a", 5).unwrap_err();
        assert_eq!(e.code, ErrorCode::Validation);

        pending.insert(s.id.clone(), key("SHA256:a"));
        // wrong fingerprint
        assert!(trust_pending(&conn, &mut pending, &s.id, "SHA256:evil", 5).is_err());
        assert!(known_hosts::get(&conn, &s.id).unwrap().is_none());
        assert!(pending.contains_key(&s.id));

        // correct
        let rec = trust_pending(&conn, &mut pending, &s.id, "SHA256:a", 5).unwrap();
        assert_eq!((rec.fingerprint.as_str(), rec.trusted_at), ("SHA256:a", 5));
        assert!(pending.is_empty());
    }

    #[test]
    fn forget_removes_key_and_pending() {
        let conn = open_in_memory();
        let s = servers::add_server(&conn, &input(AuthMethod::Password), 0).unwrap();
        known_hosts::upsert(&conn, &s.id, &key("SHA256:a"), 1).unwrap();
        let mut pending = PendingKeys::new();
        pending.insert(s.id.clone(), key("SHA256:b"));
        forget(&conn, &mut pending, &s.id).unwrap();
        assert!(known_hosts::get(&conn, &s.id).unwrap().is_none());
        assert!(pending.is_empty());
        assert_eq!(
            forget(&conn, &mut pending, "nope").unwrap_err().code,
            ErrorCode::NotFound
        );
    }

    #[test]
    fn upsert_replaces_and_deleting_server_cascades() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = servers::add_server(&conn, &input(AuthMethod::Password), 0).unwrap();
        known_hosts::upsert(&conn, &s.id, &key("SHA256:a"), 1).unwrap();
        known_hosts::upsert(&conn, &s.id, &key("SHA256:b"), 2).unwrap();
        assert_eq!(known_hosts::list(&conn).unwrap().len(), 1);
        assert_eq!(
            known_hosts::get(&conn, &s.id).unwrap().unwrap().fingerprint,
            "SHA256:b"
        );
        servers::delete_server(&conn, &store, &s.id).unwrap();
        assert!(known_hosts::list(&conn).unwrap().is_empty());
    }

    #[test]
    fn changing_host_or_port_clears_trusted_key_but_other_edits_keep_it() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = servers::add_server(&conn, &input(AuthMethod::Password), 0).unwrap();
        known_hosts::upsert(&conn, &s.id, &key("SHA256:a"), 1).unwrap();

        let mut rename = input(AuthMethod::Password);
        rename.name = "renamed".into();
        servers::update_server(&conn, &store, &s.id, &rename, 2).unwrap();
        assert!(known_hosts::get(&conn, &s.id).unwrap().is_some());

        let mut new_port = rename.clone();
        new_port.port = 22;
        servers::update_server(&conn, &store, &s.id, &new_port, 3).unwrap();
        assert!(known_hosts::get(&conn, &s.id).unwrap().is_none());

        known_hosts::upsert(&conn, &s.id, &key("SHA256:a"), 4).unwrap();
        let mut new_host = new_port.clone();
        new_host.host = "other.example".into();
        servers::update_server(&conn, &store, &s.id, &new_host, 5).unwrap();
        assert!(known_hosts::get(&conn, &s.id).unwrap().is_none());
    }

    #[test]
    fn schema_rejects_malformed_fingerprints() {
        let conn = open_in_memory();
        let s = servers::add_server(&conn, &input(AuthMethod::Password), 0).unwrap();
        assert!(known_hosts::upsert(&conn, &s.id, &key("MD5:aa"), 1).is_err());
        assert!(known_hosts::upsert(&conn, "no-such-server", &key("SHA256:a"), 1).is_err());
    }
}
