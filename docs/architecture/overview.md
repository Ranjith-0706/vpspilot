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
## SSH terminal (Milestone 4)

The Terminal page opens an interactive shell (with a PTY) on one of the user's *saved servers*,
rendered with xterm.js. Nothing runs on the local machine.

- **Same trust path as the test:** `connect_authenticated` in `ssh/client.rs` is shared. The
  terminal additionally refuses to start unless the host key is already trusted and a credential is
  stored, so no credential is ever sent to an unverified host.
- **Commands** (`open_terminal`, `terminal_write`, `terminal_resize`, `close_terminal`) take a
  server id or an unguessable session id. The frontend never supplies host, port, user or secret.
- **Streaming:** output flows Rust -> frontend through a Tauri `Channel` as base64 bytes (so UTF-8
  split across packets stays intact). Backpressure: the session waits when the UI is slow.
- **Input:** typed text is sent as-is; pastes are split into chunks (backend cap 64 KiB per write).
- **Limits:** at most 8 concurrent sessions; terminal size 1-1000 per axis; keepalive every 30 s;
  a closed window or dropped channel ends the remote shell.
- **Not supported yet:** passphrase-protected keys, multiple tabs, session logging, SFTP,
  scrollback persistence, reconnect-on-network-change.
## Live monitoring (Milestone 5)

The Dashboard shows real CPU, memory, disk, load and uptime for saved servers. Monitoring is
**agentless**: nothing is installed on the server.

- **One fixed command.** The only thing ever run is the constant `COMMAND` in
  `ssh/monitor/parse.rs`: read-only (`/proc/stat`, `/proc/meminfo`, `df -P -k /`, `/proc/uptime`,
  `/proc/loadavg`, `getconf`), no privileges, no `sudo`, wrapped in `sh -c` so it works with any
  login shell. No part of it comes from the frontend or from stored data. A test asserts the script
  contains no quotes, `$`, backticks, redirections or destructive commands, and the test SSH server
  records that nothing else is ever requested.
- **One persistent session per server** (not one login per poll), so the server's auth log shows a
  single login. Default poll interval is 5 s (allowed 2-300 s); CPU is computed from two samples
  one second apart.
- **Same trust rules as the terminal:** it refuses to start unless the host key is trusted and a
  credential is stored, and shares the connect/verify/authenticate code.
- **No lockout risk.** Authentication, credential and host key failures stop the monitor
  (`failed`) and are never retried. Only network problems (unreachable, timeout, connection dropped)
  are retried, with exponential backoff from 5 s up to 60 s.
- **Untrusted output.** Remote output is size-capped (64 KiB), parsed strictly and numerically, and
  never echoed in error messages.
- **Lifecycle.** Commands: `start_monitoring` (id + interval; restarting replaces the old monitor)
  and `stop_monitoring`. Editing a server, changing its credential, or deleting it stops its
  monitor. At most 20 monitors. Events stream over a Tauri `Channel`; the frontend keeps state in a
  provider above the router so it survives page navigation. Samples are kept in memory only (last 60
  per server) and are not stored in SQLite.
- **Demo data** stays on the Dashboard in its own clearly labelled, hideable section.
- **Not supported yet:** non-Linux servers, per-process stats, network/disk I/O, alert thresholds,
  persisted history, Docker metrics.
## Milestone status

- Milestone 1: UI shell, routing, theming, demo data.
- Milestone 2: local server profiles, SQLite, credential store.
- Milestone 3: SSH connection test with host key verification (no terminal or commands).
- Milestone 4: interactive SSH terminal (xterm.js).
- Milestone 5: live server monitoring over SSH.
- Future: Docker, alerts, persisted metrics history, Go agent, passphrase-protected keys.