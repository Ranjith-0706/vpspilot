# Architecture overview

VPSPilot is a Tauri 2 desktop app.

| Layer | Location | Responsibility |
| --- | --- | --- |
| UI | `apps/desktop/src/{components,layouts,pages}` | Presentation only |
| Application logic | `src/hooks`, `src/services` | State, data access, error handling |
| Native | `apps/desktop/src-tauri` | Rust commands exposed to the UI |

## Security rules

- The frontend may only call narrowly-scoped, typed Rust commands. Each command is listed in `src-tauri/build.rs` and granted by an explicit `allow-*` permission in `capabilities/default.json`.
- No generic shell/command execution is exposed to the frontend.
- Tauri capabilities stay minimal (`core:default` only); add permissions per feature.
- No credentials in source. Server secrets live only in the OS credential store (Windows Credential Manager); SQLite stores a reference. No command can read a secret back.
- Nothing runs `sudo` or destructive commands without explicit user approval.

## Local storage (Milestone 2)

- SQLite file `vpspilot.db` in the Tauri app data directory (`%APPDATA%\com.vpspilot.desktop`).
- Schema version is tracked in `PRAGMA user_version` plus a `schema_migrations` table. Migrations
  live in `src-tauri/src/db.rs`, each runs in a transaction, and a database newer than the app is refused.
- Rust layout: `servers/model.rs` (types + validation) -> `servers/repo.rs` (parameterized SQL) ->
  `servers/service.rs` (use-cases) -> `commands/servers.rs` (thin Tauri wrappers). `credentials.rs`
  holds the `CredentialStore` trait and the Windows Credential Manager implementation.
- Validation runs in both the frontend (`services/serverValidation.ts`) and Rust; Rust is authoritative.
- Demo data lives in `src/demo/` and is only used by the Dashboard, Docker, Logs and Alerts pages.
  The Servers page shows real saved profiles only.

### servers table

| Column | Type | Notes |
| --- | --- | --- |
| id | TEXT PK | UUID v4 |
| name | TEXT | 1-64 chars |
| host | TEXT | hostname or IP, 1-253 chars |
| port | INTEGER | 1-65535, default 22 |
| username | TEXT | 1-32 chars |
| auth_method | TEXT | `ssh_key` or `password` |
| credential_ref | TEXT NULL | reference to the OS credential entry, never a secret |
| description | TEXT NULL | max 500 chars |
| created_at / updated_at | INTEGER | Unix epoch milliseconds |

## SSH connection test (Milestone 3)

A saved server can be tested from its details panel. The test connects, verifies the host key,
authenticates, and disconnects. It never opens a channel, shell or exec request, so it cannot run
anything on the server.

- **Library:** `russh` (pure Rust, `ring` crypto backend; no OpenSSL or CMake needed on Windows).
- **Host keys (trust on first use):** the first test shows the server's SHA-256 fingerprint and
  stops *before* sending any credential. The user compares it with the real server's fingerprint and
  explicitly trusts it. The trusted key is stored in `known_host_keys` (one per server).
- **Changed key:** a mismatch is a hard failure, again before any credential is sent. There is no
  one-click override; the user must deliberately "Forget host key" and re-verify.
- **Trust is bound to what the backend saw:** `trust_host_key` only accepts the fingerprint of the
  key presented on the last test (kept in memory), so the UI cannot trust an arbitrary value.
- **Host or port edits** delete the trusted key (it is a different machine).
- **Commands take a server id only.** The backend reads host, port, username and secret itself; the
  frontend can never supply connection parameters. The secret never reaches the frontend.
- **Timeouts:** 15 seconds overall.
- **Not supported yet:** passphrase-protected private keys, host certificates, ssh-agent, jump hosts.
- **Test results** are shown in the UI for the current session only and are not persisted.

### known_host_keys table (migration 2)

| Column | Type | Notes |
| --- | --- | --- |
| server_id | TEXT PK | FK to `servers(id)`, `ON DELETE CASCADE` |
| key_type | TEXT | e.g. `ssh-ed25519` |
| fingerprint | TEXT | must start with `SHA256:` |
| trusted_at | INTEGER | Unix epoch milliseconds |
## Milestone status

- Milestone 1: UI shell, routing, theming, demo data.
- Milestone 2: local server profiles, SQLite, credential store.
- Milestone 3: SSH connection test with host key verification (no terminal or commands).
- Future: terminal, Docker, live monitoring, Go agent.