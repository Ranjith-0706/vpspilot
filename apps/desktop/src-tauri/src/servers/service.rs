//! Server profile use-cases. Independent of Tauri so it can be unit tested with an
//! in-memory database and an in-memory credential store.

use super::model::{ServerInput, ServerProfile, validate_secret, validate_server_input};
use super::repo;
use crate::credentials::{CredentialStore, reference_for};
use crate::error::AppError;
use rusqlite::Connection;

fn require(conn: &Connection, id: &str) -> Result<ServerProfile, AppError> {
    repo::get(conn, id)?.ok_or_else(|| AppError::not_found("Server"))
}

pub fn list_servers(conn: &Connection) -> Result<Vec<ServerProfile>, AppError> {
    repo::list(conn)
}

pub fn add_server(
    conn: &Connection,
    input: &ServerInput,
    now: i64,
) -> Result<ServerProfile, AppError> {
    let valid = validate_server_input(input)?;
    let id = uuid::Uuid::new_v4().to_string();
    repo::insert(conn, &id, &valid, now)
}

// SQLite and the OS credential store cannot share a transaction, so changes that touch both use
// this order instead of pretending to be atomic:
//
//   1. take the secret out of the store (remembering its value so the step can be undone),
//   2. make every SQLite change in ONE transaction,
//   3. if that fails, put the secret back (compensation).
//
// A failure at step 1 changes nothing. A failure at step 2 is undone by step 3, so the profile
// keeps pointing at a secret that exists. Only if the restore itself also fails (the credential
// store went away between the two steps) is the profile left without a usable credential; that
// state is safe (connections refuse to start without one) and the error tells the user to enter
// it again.

const RESTORE_FAILED: &str = "The change could not be saved and the stored credential could not be restored. Enter the credential for this server again.";

/// A secret that was removed from the store, and what it held.
struct Taken {
    reference: String,
    secret: Option<String>,
}

/// Removes the secret from the store, remembering it so [`Taken::undo`] can put it back.
fn take_secret(store: &dyn CredentialStore, reference: &str) -> Result<Taken, AppError> {
    let secret = store.get(reference)?;
    store.delete(reference)?;
    Ok(Taken {
        reference: reference.to_string(),
        secret,
    })
}

impl Taken {
    /// Compensates for a failed database step: restores the secret and returns the error to
    /// report. A "not found" error means the profile is gone, so nothing is restored for it.
    fn undo(self, store: &dyn CredentialStore, err: AppError) -> AppError {
        if err.code == crate::error::ErrorCode::NotFound {
            return err;
        }
        match self.secret {
            Some(secret) => match store.set(&self.reference, &secret) {
                Ok(()) => err,
                Err(restore) => {
                    eprintln!("credential restore failed after a database error: {restore}");
                    AppError::credential(RESTORE_FAILED)
                }
            },
            None => err,
        }
    }
}

/// Updates a profile. If the authentication method changes, any stored credential no longer
/// matches it and is removed, so the user must provide a new one. The profile row, the trusted
/// host key (dropped when the host or port changes: it is a different machine) and the credential
/// reference change together in one transaction, so a failure cannot leave a profile that points
/// at a deleted credential or keeps trusting the old machine's key.
pub fn update_server(
    conn: &Connection,
    store: &dyn CredentialStore,
    id: &str,
    input: &ServerInput,
    now: i64,
) -> Result<ServerProfile, AppError> {
    let valid = validate_server_input(input)?;
    let existing = require(conn, id)?;

    let method_changed = existing.auth_method != valid.auth_method;
    let machine_changed = existing.host != valid.host || existing.port != valid.port;

    let taken = match (&existing.credential_ref, method_changed) {
        (Some(reference), true) => Some(take_secret(store, reference)?),
        _ => None,
    };

    let saved = (|| -> Result<(), AppError> {
        let tx = conn.unchecked_transaction()?;
        if !repo::update(&tx, id, &valid, now)? {
            return Err(AppError::not_found("Server"));
        }
        if machine_changed {
            crate::ssh::known_hosts::delete(&tx, id)?;
        }
        if taken.is_some() {
            repo::set_credential_ref(&tx, id, None, now)?;
        }
        tx.commit()?;
        Ok(())
    })();
    if let Err(err) = saved {
        return Err(match taken {
            Some(t) => t.undo(store, err),
            None => err,
        });
    }
    require(conn, id)
}

/// Deletes a profile and its credential. If the database delete fails the credential is
/// restored, so the profile is never left pointing at a deleted secret.
pub fn delete_server(
    conn: &Connection,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<(), AppError> {
    let existing = require(conn, id)?;
    // Remove the secret first: if this fails we abort and nothing is orphaned.
    let taken = match &existing.credential_ref {
        Some(reference) => Some(take_secret(store, reference)?),
        None => None,
    };
    if let Err(err) = repo::delete(conn, id) {
        return Err(match taken {
            Some(t) => t.undo(store, err),
            None => err,
        });
    }
    Ok(())
}

/// Stores or replaces a credential. If recording it in the database fails, the previous secret
/// (if there was one) is put back, or the new one removed, so the store and the profile agree.
pub fn set_credential(
    conn: &Connection,
    store: &dyn CredentialStore,
    id: &str,
    secret: &str,
    now: i64,
) -> Result<ServerProfile, AppError> {
    validate_secret(secret)?;
    require(conn, id)?;
    let reference = reference_for(id);
    let previous = store.get(&reference)?;
    store.set(&reference, secret)?;
    if let Err(e) = repo::set_credential_ref(conn, id, Some(&reference), now) {
        let restored = match &previous {
            Some(old) => store.set(&reference, old),
            None => store.delete(&reference), // don't leave an unreferenced secret behind
        };
        if let Err(restore) = restored {
            eprintln!("credential rollback failed after a database error: {restore}");
        }
        return Err(e);
    }
    require(conn, id)
}

/// Removes a credential. If clearing the reference fails, the secret is restored.
pub fn clear_credential(
    conn: &Connection,
    store: &dyn CredentialStore,
    id: &str,
    now: i64,
) -> Result<ServerProfile, AppError> {
    let existing = require(conn, id)?;
    if let Some(reference) = &existing.credential_ref {
        let taken = take_secret(store, reference)?;
        if let Err(err) = repo::set_credential_ref(conn, id, None, now) {
            return Err(taken.undo(store, err));
        }
    }
    require(conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::testing::MemoryStore;
    use crate::db::open_in_memory;
    use crate::error::ErrorCode;
    use crate::servers::model::AuthMethod;

    fn input(name: &str) -> ServerInput {
        ServerInput {
            name: name.into(),
            host: "web-01.example".into(),
            port: 22,
            username: "deploy".into(),
            auth_method: AuthMethod::Password,
            description: Some("demo".into()),
        }
    }

    #[test]
    fn add_and_list() {
        let conn = open_in_memory();
        assert!(list_servers(&conn).unwrap().is_empty());
        let a = add_server(&conn, &input("beta"), 1000).unwrap();
        add_server(&conn, &input("Alpha"), 2000).unwrap();
        assert_eq!(a.created_at, 1000);
        assert_eq!(a.updated_at, 1000);
        assert_eq!(a.credential_ref, None);
        assert_eq!(a.id.len(), 36); // UUID
        let names: Vec<_> = list_servers(&conn)
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, ["Alpha", "beta"]); // case-insensitive order
    }

    #[test]
    fn add_rejects_invalid_input_without_writing() {
        let conn = open_in_memory();
        let mut bad = input("x");
        bad.port = 0;
        let err = add_server(&conn, &bad, 0).unwrap_err();
        assert_eq!(err.code, ErrorCode::Validation);
        assert_eq!(err.field.as_deref(), Some("port"));
        assert!(list_servers(&conn).unwrap().is_empty());
    }

    #[test]
    fn update_changes_fields_and_timestamp() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 1000).unwrap();
        let mut changed = input("two");
        changed.port = 2222;
        changed.description = None;
        let u = update_server(&conn, &store, &s.id, &changed, 5000).unwrap();
        assert_eq!(
            (u.name.as_str(), u.port, u.description.clone()),
            ("two", 2222, None)
        );
        assert_eq!(u.created_at, 1000);
        assert_eq!(u.updated_at, 5000);
    }

    #[test]
    fn update_and_delete_unknown_id_are_not_found() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        assert_eq!(
            update_server(&conn, &store, "nope", &input("x"), 0)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        assert_eq!(
            delete_server(&conn, &store, "nope").unwrap_err().code,
            ErrorCode::NotFound
        );
    }

    #[test]
    fn delete_removes_profile_and_credential() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        assert_eq!(store.items.lock().unwrap().len(), 1);
        delete_server(&conn, &store, &s.id).unwrap();
        assert!(list_servers(&conn).unwrap().is_empty());
        assert!(store.items.lock().unwrap().is_empty());
    }

    #[test]
    fn delete_aborts_if_credential_removal_fails() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        *store.fail.lock().unwrap() = true;
        let err = delete_server(&conn, &store, &s.id).unwrap_err();
        assert_eq!(err.code, ErrorCode::Credential);
        assert_eq!(list_servers(&conn).unwrap().len(), 1);
    }

    #[test]
    fn credential_set_and_clear() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        let with = set_credential(&conn, &store, &s.id, "pw", 10).unwrap();
        assert_eq!(
            with.credential_ref.as_deref(),
            Some(reference_for(&s.id).as_str())
        );
        assert_eq!(
            store.get(&reference_for(&s.id)).unwrap().as_deref(),
            Some("pw")
        );
        let cleared = clear_credential(&conn, &store, &s.id, 20).unwrap();
        assert_eq!(cleared.credential_ref, None);
        assert!(store.items.lock().unwrap().is_empty());
    }

    #[test]
    fn secret_is_never_written_to_sqlite() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "SUPER-SECRET-VALUE", 1).unwrap();
        let mut stmt = conn.prepare("SELECT * FROM servers").unwrap();
        let n = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..n {
                let v: rusqlite::types::Value = row.get(i).unwrap();
                if let rusqlite::types::Value::Text(t) = v {
                    assert!(!t.contains("SUPER-SECRET-VALUE"));
                }
            }
        }
    }

    #[test]
    fn invalid_or_failed_credential_changes_nothing() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        assert_eq!(
            set_credential(&conn, &store, &s.id, "  ", 1)
                .unwrap_err()
                .code,
            ErrorCode::Validation
        );
        assert_eq!(
            set_credential(&conn, &store, "nope", "pw", 1)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        *store.fail.lock().unwrap() = true;
        assert_eq!(
            set_credential(&conn, &store, &s.id, "pw", 1)
                .unwrap_err()
                .code,
            ErrorCode::Credential
        );
        assert_eq!(list_servers(&conn).unwrap()[0].credential_ref, None);
    }

    #[test]
    fn changing_auth_method_clears_stored_credential() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        let mut changed = input("one");
        changed.auth_method = AuthMethod::SshKey;
        let u = update_server(&conn, &store, &s.id, &changed, 2).unwrap();
        assert_eq!(u.credential_ref, None);
        assert!(store.items.lock().unwrap().is_empty());
    }

    #[test]
    fn sql_metacharacters_are_stored_literally() {
        let conn = open_in_memory();
        let nasty = "x'); DROP TABLE servers;--";
        let s = add_server(&conn, &input(nasty), 0).unwrap();
        assert_eq!(s.name, nasty);
        assert_eq!(list_servers(&conn).unwrap().len(), 1);
    }

    // ----- SQLite / credential store consistency (A-03) -----

    /// Makes every matching SQL statement fail, to simulate a database error.
    fn break_db(conn: &Connection, event: &str, table: &str) {
        conn.execute_batch(&format!(
            "CREATE TRIGGER injected_failure BEFORE {event} ON {table}
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;"
        ))
        .unwrap();
    }

    fn stored_secret(store: &MemoryStore, id: &str) -> Option<String> {
        store.get(&reference_for(id)).unwrap()
    }

    fn trust_key(conn: &Connection, id: &str) {
        let key = crate::ssh::hostkey::ObservedKey {
            key_type: "ssh-ed25519".into(),
            fingerprint: "SHA256:a".into(),
        };
        crate::ssh::known_hosts::upsert(conn, id, &key, 1).unwrap();
    }

    #[test]
    fn replacing_a_credential_restores_the_old_one_if_the_database_fails() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "old-pw", 1).unwrap();

        break_db(&conn, "UPDATE", "servers");
        let err = set_credential(&conn, &store, &s.id, "new-pw", 2).unwrap_err();
        assert_eq!(err.code, ErrorCode::Database);
        assert_eq!(stored_secret(&store, &s.id).as_deref(), Some("old-pw"));
        assert!(list_servers(&conn).unwrap()[0].credential_ref.is_some());
    }

    #[test]
    fn a_first_credential_is_removed_again_if_the_database_fails() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        break_db(&conn, "UPDATE", "servers");
        assert!(set_credential(&conn, &store, &s.id, "pw", 1).is_err());
        assert_eq!(stored_secret(&store, &s.id), None, "no unreferenced secret");
        assert_eq!(list_servers(&conn).unwrap()[0].credential_ref, None);
    }

    #[test]
    fn auth_method_change_keeps_the_credential_if_the_database_fails() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();

        break_db(&conn, "UPDATE", "servers");
        let mut changed = input("one");
        changed.auth_method = AuthMethod::SshKey;
        assert!(update_server(&conn, &store, &s.id, &changed, 2).is_err());

        let after = list_servers(&conn).unwrap().remove(0);
        assert_eq!(after.auth_method, AuthMethod::Password, "profile unchanged");
        assert!(after.credential_ref.is_some());
        assert_eq!(stored_secret(&store, &s.id).as_deref(), Some("pw"));
    }

    #[test]
    fn a_failure_part_way_through_an_update_rolls_every_change_back() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        trust_key(&conn, &s.id);

        // The row update succeeds, then dropping the old host's key fails.
        break_db(&conn, "DELETE", "known_host_keys");
        let mut moved = input("renamed");
        moved.host = "other.example".into();
        moved.auth_method = AuthMethod::SshKey;
        assert!(update_server(&conn, &store, &s.id, &moved, 2).is_err());

        let after = list_servers(&conn).unwrap().remove(0);
        assert_eq!(after.name, "one", "the row update was rolled back too");
        assert_eq!(after.host, "web-01.example");
        assert_eq!(after.auth_method, AuthMethod::Password);
        assert!(after.credential_ref.is_some());
        assert_eq!(stored_secret(&store, &s.id).as_deref(), Some("pw"));
        assert!(
            crate::ssh::known_hosts::get(&conn, &s.id)
                .unwrap()
                .is_some(),
            "the key is only ever dropped together with the host change"
        );
    }

    #[test]
    fn a_failed_secret_removal_changes_nothing() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        *store.fail_delete.lock().unwrap() = true;

        let mut changed = input("one");
        changed.auth_method = AuthMethod::SshKey;
        assert_eq!(
            update_server(&conn, &store, &s.id, &changed, 2)
                .unwrap_err()
                .code,
            ErrorCode::Credential
        );
        assert_eq!(
            clear_credential(&conn, &store, &s.id, 3).unwrap_err().code,
            ErrorCode::Credential
        );
        let after = list_servers(&conn).unwrap().remove(0);
        assert_eq!(after.auth_method, AuthMethod::Password);
        assert!(after.credential_ref.is_some());
        assert_eq!(stored_secret(&store, &s.id).as_deref(), Some("pw"));
    }

    #[test]
    fn clearing_restores_the_secret_if_the_database_fails() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        break_db(&conn, "UPDATE", "servers");
        assert!(clear_credential(&conn, &store, &s.id, 2).is_err());
        assert!(list_servers(&conn).unwrap()[0].credential_ref.is_some());
        assert_eq!(stored_secret(&store, &s.id).as_deref(), Some("pw"));
    }

    #[test]
    fn deleting_restores_the_secret_if_the_database_fails() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        break_db(&conn, "DELETE", "servers");
        assert!(delete_server(&conn, &store, &s.id).is_err());
        assert_eq!(
            list_servers(&conn).unwrap().len(),
            1,
            "profile still exists"
        );
        assert_eq!(stored_secret(&store, &s.id).as_deref(), Some("pw"));
    }

    #[test]
    fn if_the_restore_also_fails_the_error_says_to_re_enter_the_credential() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        break_db(&conn, "UPDATE", "servers");
        *store.fail_set.lock().unwrap() = true; // the store goes away before the restore

        let err = clear_credential(&conn, &store, &s.id, 2).unwrap_err();
        assert_eq!(err.code, ErrorCode::Credential);
        assert!(
            err.message.contains("Enter the credential"),
            "{}",
            err.message
        );
        // Nothing stale is left behind a reference: connections see "no credential".
        assert_eq!(stored_secret(&store, &s.id), None);
    }

    #[test]
    fn a_deleted_server_does_not_get_its_secret_restored() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        // The profile vanishes after it was loaded: the update then affects no row.
        conn.execute_batch(
            "CREATE TRIGGER vanish BEFORE UPDATE ON servers
             BEGIN DELETE FROM servers WHERE id = OLD.id; SELECT RAISE(IGNORE); END;",
        )
        .unwrap();
        let mut changed = input("one");
        changed.auth_method = AuthMethod::SshKey;
        assert_eq!(
            update_server(&conn, &store, &s.id, &changed, 2)
                .unwrap_err()
                .code,
            ErrorCode::NotFound
        );
        assert_eq!(stored_secret(&store, &s.id), None, "no orphan secret");
    }

    #[test]
    fn a_plain_edit_never_touches_the_credential_store() {
        let conn = open_in_memory();
        let store = MemoryStore::default();
        let s = add_server(&conn, &input("one"), 0).unwrap();
        set_credential(&conn, &store, &s.id, "pw", 1).unwrap();
        *store.fail.lock().unwrap() = true; // any store call would now fail
        let renamed = update_server(&conn, &store, &s.id, &input("renamed"), 2).unwrap();
        assert_eq!(renamed.name, "renamed");
        assert!(renamed.credential_ref.is_some());
    }
}
