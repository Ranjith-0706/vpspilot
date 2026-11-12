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
- Tauri capabilities stay minimal: `core:default` plus one explicit `allow-*` permission per app command (`capabilities/default.json`). No shell, filesystem, HTTP or opener plugins are enabled. Adding a command means adding it to `build.rs`, `generate_handler!` and the capability file.
- No credentials in source. Server secrets live only in the OS credential store (Windows Credential Manager); SQLite stores a reference. No command can read a secret back.
- Nothing runs `sudo` or destructive commands without explicit user approval.

## UI shell

The window is a compact desktop layout: a sidebar of pages (Dashboard, Servers, Docker, Terminal,
Logs, Alerts, Settings, see `src/routes.ts`), a status footer, and a light/dark theme
(`hooks/useTheme.tsx`). Monitoring state lives in a provider above the router so it survives
navigation. The whole tree sits inside an `ErrorBoundary`, so an unexpected render error shows a
recovery screen instead of a blank window.

## Local storage (Milestone 2)

- SQLite file `vpspilot.db` in the Tauri app data directory (`%APPDATA%\com.vpspilot.desktop`).
- Schema version is tracked in `PRAGMA user_version` plus a `schema_migrations` table. Migrations
  live in `src-tauri/src/db.rs`, each runs in a transaction, and a database newer than the app is refused.
- Rust layout: `servers/model.rs` (types + validation) -> `servers/repo.rs` (parameterized SQL) ->
  `servers/service.rs` (use-cases) -> `commands/servers.rs` (thin Tauri wrappers). `credentials.rs`
  holds the `CredentialStore` trait and the Windows Credential Manager implementation.
- **Profile and credential changes** span SQLite and the OS credential store, which cannot share
  a transaction. The order is: take the secret out of the store (remembering it), make all SQLite
  changes in one transaction, and if that fails put the secret back. If the restore also fails the
  error says to enter the credential again; the profile then simply has no usable credential.
- The React tree is wrapped in an `ErrorBoundary` (`components/ErrorBoundary.tsx`) that shows a
  recovery screen with a reload button and never displays error details.
- **Credential limits.** A secret is at most 2560 bytes (the Windows Credential Manager blob
  limit), enforced in both the frontend and Rust. Measured unencrypted private keys: ed25519 about
  0.4 KB, ECDSA P-256 about 0.5 KB, RSA-2048 about 1.7-1.8 KB (all fit), RSA-3072 about
  2.5-2.6 KB (OpenSSH format is too large, PEM only just fits) and RSA-4096 about 3.2-3.4 KB (does
  not fit). Passphrase-protected keys are not supported. The form and the error message both
  say so.
- Validation runs in both the frontend (`services/serverValidation.ts`) and Rust; Rust is authoritative.
- Demo data lives in `src/demo/` and is only used by the Dashboard, in its labeled demo section.
  Every page is otherwise real: Alerts (Milestone 6), Docker (7) and Logs (8).
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
- **Lifecycle rules** (`ssh/terminal_registry.rs`): the session cap is reserved before connecting
  and committed after, under one lock, so concurrent opens cannot exceed it. Each server has a
  revision that deleting it, changing its identity or credential, or forgetting its host key
  bumps; a terminal that was still connecting when that happened is closed instead of registered.
  A reload or destroyed window closes every terminal (`on_page_load`, `on_window_event` in
  `lib.rs`), and an idle session sends an empty-output probe every 30 s so a vanished frontend
  is noticed and the shell is closed.
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
  provider above the router so it survives page navigation. The live view keeps the last 60
  samples per server in memory; a thinned copy is also stored (see Monitoring history).
- **Demo data** stays on the Dashboard in its own clearly labelled, hideable section.
- **Not supported yet:** non-Linux servers, per-process stats, network/disk I/O, Docker metrics.
  (Threshold alerts arrived in Milestone 6, stored history in Milestone 9.)

## Monitoring history (Milestone 9)

Successful samples are stored in SQLite so charts survive a restart. No new connection is made:
the monitor's existing forwarder task stores what it already receives.

- **Table** `metric_samples` (migration 4): server id, timestamp (ms), CPU, memory and disk
  percentages, the three load averages, uptime, core count and memory/disk totals. No host names,
  commands, remote output or credentials. Indexed on `(server_id, at)` and on `at`.
  `app_settings` holds the retention period.
- **What is stored.** Only `sample` events that pass validation (finite, in range, complete).
  Statuses, failures and stops are never stored as readings. At most one sample per server per
  minute is kept, however fast the monitor polls, so a server adds about 1,440 rows a day.
- **Never in the way.** The write runs after the UI has been sent the sample, on a blocking
  thread. If it fails, monitoring continues, the failure is logged once per run of failures, and
  the next sample tries again.
- **Retention.** 7 days by default, configurable from 1 to 90 days in Settings. Expired rows are
  deleted at startup, hourly, and right after the setting changes, in batches of 5,000 so the
  database is never held for long.
- **Deletion.** A server's history is deleted with the server (`ON DELETE CASCADE`). Stopping
  monitoring simply stops the writes; stored history stays until it expires.
- **Queries.** `get_metric_history(server_id, range_secs, max_points)` returns at most 300
  points, averaging samples into time buckets, plus the sample count and the first and last sample
  times. Missing data is returned as missing (no zero-filled points); the charts draw a gap.
- **Historical is not live.** The Dashboard has Live and History tabs. History is labelled as
  recorded data and never shows a connection status.
## Alerts (Milestone 6)

Threshold rules over the live monitoring samples. Nothing new runs on a server: alerts are
computed locally from the samples Milestone 5 already collects.

- **Rules** (`alert_rules`, migration 3): a metric (`cpu`, `memory` or `disk`), a threshold of
  1-100 %, and a number of checks in a row (1-60). A rule applies to one server or to all of
  them, and can be disabled. At most 100 rules.
- **Engine** (`alerts/engine.rs`): a pure state machine with no I/O or clock.
  - An alert fires after the value is at or above the threshold for the required number of
    consecutive samples.
  - It resolves only when the value drops clearly below the threshold (5 points, capped at half
    the threshold), so a value hovering at the limit does not flap.
  - It resolves with no value when its rule is deleted, disabled or changed to another metric, or
    when monitoring of the server ends.
- **History** (`alert_events`, migration 3): every fire and resolve, newest first, trimmed to the
  latest 1000. `metric` and `threshold` are a snapshot, and `rule_id` is deliberately not a
  foreign key, so history stays readable after a rule is edited or deleted. Deleting a server
  deletes its rules and history.
- **Where it runs:** the monitor's forwarder task evaluates each sample, writes the events, and
  streams them to the UI on the same channel (`{"type":"alert", ...}`). The currently-firing set
  lives in memory only and ends with monitoring; it is not restored after a restart.
- **Gaps in monitoring:** when a server is connecting or reconnecting no samples arrive, so the
  forwarder tells the engine (`AlertEngine::gap`): pending "N in a row" streaks start again, but
  an alert that is already firing keeps firing (an unreachable server has not recovered) and
  nothing is written to history. The first sample after the gap decides whether it resolves. The
  UI shows such an alert as "status unknown" until fresh samples arrive (`isAlertStale`).
- **Failure isolation:** an alert evaluation error is logged and never interrupts monitoring.
- **Locking:** the alert engine and the database are never held across an await. Code that holds
  the database may take the monitor registry afterwards, never the reverse, so the forwarder
  releases the registry before touching the database.
- **Not supported yet:** desktop or email notifications, "server unreachable" alerts, per-process
  or network metrics, severity levels, snoozing, and alert state that survives a restart.

## Docker containers (Milestone 7)

A read-only list of a server's containers, loaded on demand from the Docker page.

- **One fixed command** (`ssh::docker::parse::COMMAND`): `docker ps -a --format "{{json .}}"`
  wrapped in `sh -c`, preceded by a `command -v docker` check. It takes no container name or
  other argument, so nothing from the UI or the database can change it or be injected into it.
  A test checks it has no quote, `$` or backtick in the script and none of `sudo`, `rm`, `kill`,
  `stop`, `start`, `exec`, `run`, `pull` or `prune`, and that only `docker ps` appears.
- **No sudo, ever.** The account must already be allowed to use Docker (usually the `docker`
  group). If it is not, the page says so and how to fix it on the server. Note that membership in
  the `docker` group is itself root-equivalent on that server; granting it is the server owner's
  decision, not VPSPilot's.
- **One login per load**: it signs in, runs the command and disconnects. There is no background
  polling, and no retry on any failure, so it cannot trigger lockouts such as fail2ban. At most one
  load per server runs at a time (a second request is refused while one is in flight).
- **Same trust rules** as the terminal and monitor: it refuses, without any network access, unless
  the host key is already trusted and a credential is stored, and a changed host key is a hard
  failure.
- **Remote output is untrusted:** capped at 4 MiB (and 8 KiB of error text), at most 500
  containers, parsed strictly as one JSON object per line. Control characters are stripped and
  every field is length-limited. Parse errors and Docker errors never show remote text: only
  messages written by VPSPilot (not installed, permission denied, daemon not running, or a generic
  exit status) are shown.
- **Outcomes** are distinct from connection problems: `not_installed`, `permission_denied`,
  `daemon_unavailable` and `error` come back as a normal result; an untrusted host key, login
  failure or unreachable server is an error, like every other SSH feature.
- **Not stored:** nothing is persisted. The list is kept in memory for the session.
- **Not supported yet:** starting, stopping or restarting containers, per-container CPU and
  memory, images, volumes and networks, Docker Compose projects, Podman, and servers that need
  `sudo docker`.

## Container logs (Milestone 8)

The last lines of one container's output, loaded on demand from the Logs page (also reachable
from a "View logs" link on each Docker row). It uses the same trust, login and failure rules as
the Docker list: one login per load, no retry, no sudo, and one Docker request per server at a
time (the list and logs share that slot).

- **The only command built from input.** Everything else VPSPilot runs is a constant. This one is
  `docker logs --timestamps --tail <N> <id> 2>&1`, wrapped in `sh -c` after the same
  `command -v docker` check, and it contains exactly two request-controlled values, both
  validated before anything else happens (`LogsRequest::new`, the only way to build the command):
  - the container id must be 12 to 64 lowercase hexadecimal characters, so it cannot start with
    `-`, or contain a space, quote, `;`, `$` or any other shell character (Unicode look-alikes
    included);
  - the line count must be a number from 1 to 1000.
  Tests assert the exact command string, that it has only the wrapper's two quotes, and that a
  long list of injection attempts is rejected. The request is rejected before the database or the
  network is touched.
- **Log text is untrusted program output.** Terminal escape sequences (colors, cursor movement,
  window titles, hyperlinks) are removed, other control characters are dropped, tabs become
  spaces, lines are cut at 2000 characters, and at most the newest 1000 lines are kept. Output
  over 4 MiB is an error. Text is shown as plain text, never as HTML. Errors never echo it.
- **Because stderr is merged into the output** (containers write errors there), the output of a
  failed command is Docker's error text. It is used only to recognize known problems (container
  not found, permission denied, daemon not running, Docker not installed) and is never shown.
- **Level is a guess.** Docker records no log level, so the UI guesses one from the text (words
  such as `error`, `warn`, `level=error`) and labels it as a guess. It will sometimes be wrong.
- **Not stored:** logs are kept in memory for the session only.
- **Not supported yet:** following logs live, downloading or copying, host (non-container) logs
  such as `journalctl`, time-range queries, and stdout/stderr separation.

## Milestone status

- Milestone 1: UI shell, routing, theming, demo data.
- Milestone 2: local server profiles, SQLite, credential store.
- Milestone 3: SSH connection test with host key verification (no terminal or commands).
- Milestone 4: interactive SSH terminal (xterm.js).
- Milestone 5: live server monitoring over SSH.
- Milestone 6: threshold alerts (rules, history, Alerts page).
- Milestone 7: read-only Docker container list.
- Milestone 8: container logs.
- Milestone 9: persistent monitoring history (SQLite, retention, History tab).
- Future: container actions, host logs, live log following, notifications, Go agent,
  passphrase-protected keys.