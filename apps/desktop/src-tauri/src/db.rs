//! SQLite initialization and schema migrations.
//!
//! The schema version is tracked in `PRAGMA user_version`; applied migrations are also
//! recorded in the `schema_migrations` table for auditing. Each migration runs in its own
//! transaction, so a failing migration leaves the database at the previous version.

use crate::error::AppError;
use rusqlite::Connection;
use std::path::Path;

pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "create_servers",
    sql: "
        CREATE TABLE schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );

        CREATE TABLE servers (
            id             TEXT PRIMARY KEY NOT NULL,
            name           TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
            host           TEXT NOT NULL CHECK (length(host) BETWEEN 1 AND 253),
            port           INTEGER NOT NULL DEFAULT 22 CHECK (port BETWEEN 1 AND 65535),
            username       TEXT NOT NULL CHECK (length(username) BETWEEN 1 AND 32),
            auth_method    TEXT NOT NULL CHECK (auth_method IN ('ssh_key', 'password')),
            credential_ref TEXT,
            description    TEXT CHECK (description IS NULL OR length(description) <= 500),
            created_at     INTEGER NOT NULL,
            updated_at     INTEGER NOT NULL
        );

        CREATE INDEX idx_servers_name ON servers (name COLLATE NOCASE);
    ",
    },
    Migration {
        version: 2,
        name: "create_known_host_keys",
        sql: "
        CREATE TABLE known_host_keys (
            server_id   TEXT PRIMARY KEY NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
            key_type    TEXT NOT NULL CHECK (length(key_type) BETWEEN 1 AND 64),
            fingerprint TEXT NOT NULL CHECK (fingerprint LIKE 'SHA256:%' AND length(fingerprint) <= 128),
            trusted_at  INTEGER NOT NULL
        );
    ",
    },
];

#[cfg(test)]
pub fn latest_version() -> u32 {
    MIGRATIONS.last().map_or(0, |m| m.version)
}

pub fn current_version(conn: &Connection) -> Result<u32, rusqlite::Error> {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
}

/// Applies all pending built-in migrations. Returns the resulting schema version.
pub fn migrate(conn: &mut Connection) -> Result<u32, AppError> {
    migrate_with(conn, MIGRATIONS, crate::servers::repo::now_ms())
}

pub fn migrate_with(
    conn: &mut Connection,
    migrations: &[Migration],
    now: i64,
) -> Result<u32, AppError> {
    let current = current_version(conn)?;
    let latest = migrations.last().map_or(0, |m| m.version);
    if current > latest {
        return Err(AppError::database(format!(
            "The database schema (v{current}) is newer than this app supports (v{latest}). Please update VPSPilot."
        )));
    }

    for m in migrations.iter().filter(|m| m.version > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![m.version, m.name, now],
        )?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
    }
    Ok(current_version(conn)?)
}

fn configure(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(())
}

/// Opens (creating if needed) the database file and brings it up to date.
pub fn open(path: &Path) -> Result<Connection, AppError> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    // WAL improves durability/concurrency for a desktop app; failure to set it is non-fatal.
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    migrate(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    configure(&conn).unwrap();
    migrate(&mut conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn fresh_database_reaches_latest_version() {
        let conn = open_in_memory();
        assert_eq!(current_version(&conn).unwrap(), latest_version());
        let applied: u32 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied as usize, MIGRATIONS.len());
    }

    #[test]
    fn servers_table_has_expected_columns_and_no_secret_column() {
        let conn = open_in_memory();
        let cols = columns(&conn, "servers");
        for c in [
            "id",
            "name",
            "host",
            "port",
            "username",
            "auth_method",
            "credential_ref",
            "description",
            "created_at",
            "updated_at",
        ] {
            assert!(cols.contains(&c.to_string()), "missing column {c}");
        }
        assert!(
            !cols
                .iter()
                .any(|c| c.contains("secret") || c.contains("password") || c.contains("key_data"))
        );
    }

    #[test]
    fn migrating_twice_is_idempotent() {
        let mut conn = open_in_memory();
        let v = migrate(&mut conn).unwrap();
        assert_eq!(v, latest_version());
        let applied: u32 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied as usize, MIGRATIONS.len());
    }

    #[test]
    fn upgrades_stepwise_preserving_data() {
        const V1: Migration = Migration {
            version: 1,
            name: "one",
            sql: "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at INTEGER NOT NULL);
                  CREATE TABLE t (a TEXT); INSERT INTO t VALUES ('keep');",
        };
        const V2: Migration = Migration {
            version: 2,
            name: "two",
            sql: "ALTER TABLE t ADD COLUMN b TEXT;",
        };
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate_with(&mut conn, &[V1], 0).unwrap(), 1);
        assert_eq!(migrate_with(&mut conn, &[V1, V2], 0).unwrap(), 2);
        let a: String = conn.query_row("SELECT a FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(a, "keep");
        assert!(columns(&conn, "t").contains(&"b".to_string()));
    }

    #[test]
    fn failing_migration_rolls_back_and_keeps_version() {
        const V1: Migration = Migration {
            version: 1,
            name: "one",
            sql: "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at INTEGER NOT NULL);
                  CREATE TABLE t (a TEXT);",
        };
        const BAD: Migration = Migration {
            version: 2,
            name: "bad",
            sql: "CREATE TABLE half_done (x TEXT); THIS IS NOT SQL;",
        };
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_with(&mut conn, &[V1], 0).unwrap();
        assert!(migrate_with(&mut conn, &[V1, BAD], 0).is_err());
        assert_eq!(current_version(&conn).unwrap(), 1);
        let exists: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'half_done'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, 0);
    }

    #[test]
    fn v1_database_with_data_upgrades_to_latest_keeping_servers() {
        let mut conn = Connection::open_in_memory().unwrap();
        configure(&conn).unwrap();
        assert_eq!(migrate_with(&mut conn, &MIGRATIONS[..1], 0).unwrap(), 1);
        conn.execute(
            "INSERT INTO servers (id,name,host,port,username,auth_method,created_at,updated_at)
             VALUES ('keep','n','h',22,'u','password',0,0)",
            [],
        )
        .unwrap();

        assert_eq!(migrate(&mut conn).unwrap(), latest_version());
        let name: String = conn
            .query_row("SELECT name FROM servers WHERE id = 'keep'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "n");
        assert_eq!(columns(&conn, "known_host_keys").len(), 4);
    }

    #[test]
    fn refuses_database_from_newer_app() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", latest_version() + 1)
            .unwrap();
        let err = migrate(&mut conn).unwrap_err();
        assert!(err.message.contains("newer"));
    }

    #[test]
    fn check_constraints_reject_invalid_rows() {
        let conn = open_in_memory();
        let insert = |port: i64, auth: &str| {
            conn.execute(
                "INSERT INTO servers (id,name,host,port,username,auth_method,created_at,updated_at)
                 VALUES ('x','n','h',?1,'u',?2,0,0)",
                rusqlite::params![port, auth],
            )
        };
        assert!(insert(0, "password").is_err());
        assert!(insert(70000, "password").is_err());
        assert!(insert(22, "telnet").is_err());
        assert!(insert(22, "password").is_ok());
    }

    #[test]
    fn open_creates_file_and_reopens_without_reapplying() {
        let dir = std::env::temp_dir().join(format!("vpspilot-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        assert_eq!(current_version(&conn).unwrap(), latest_version());
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
