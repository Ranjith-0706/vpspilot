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

/// Updates a profile. If the authentication method changes, any stored credential no longer
/// matches it and is removed, so the user must provide a new one.
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
    if method_changed && let Some(reference) = &existing.credential_ref {
        store.delete(reference)?;
    }
    if !repo::update(conn, id, &valid, now)? {
        return Err(AppError::not_found("Server"));
    }
    if method_changed && existing.credential_ref.is_some() {
        repo::set_credential_ref(conn, id, None, now)?;
    }
    require(conn, id)
}

pub fn delete_server(
    conn: &Connection,
    store: &dyn CredentialStore,
    id: &str,
) -> Result<(), AppError> {
    let existing = require(conn, id)?;
    // Remove the secret first: if this fails we abort and nothing is orphaned.
    if let Some(reference) = &existing.credential_ref {
        store.delete(reference)?;
    }
    repo::delete(conn, id)?;
    Ok(())
}

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
    store.set(&reference, secret)?;
    if let Err(e) = repo::set_credential_ref(conn, id, Some(&reference), now) {
        let _ = store.delete(&reference); // don't leave an unreferenced secret behind
        return Err(e);
    }
    require(conn, id)
}

pub fn clear_credential(
    conn: &Connection,
    store: &dyn CredentialStore,
    id: &str,
    now: i64,
) -> Result<ServerProfile, AppError> {
    let existing = require(conn, id)?;
    if let Some(reference) = &existing.credential_ref {
        store.delete(reference)?;
        repo::set_credential_ref(conn, id, None, now)?;
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
}
