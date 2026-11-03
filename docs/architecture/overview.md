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

## Milestone status

- Milestone 1: UI shell, routing, theming, demo data.
- Milestone 2: local server profiles, SQLite, credential store. No SSH connections yet.
- Future: SSH, terminal, Docker, live monitoring, Go agent.