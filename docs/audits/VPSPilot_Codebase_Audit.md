# VPSPilot — Codebase Architecture, Security and Quality Audit

Branch `feat/ui-redesign` (HEAD `9ef65ca`, plus uncommitted UI-redesign changes). Audit date 2026-11-11.
Method: read the Rust and TypeScript sources, configuration and docs; ran the static checks, test
suites and dependency audits listed in section 11. No application code was changed, nothing was
committed or pushed, and nothing was run against a real server.

Line numbers refer to the working tree at audit time.

---

## 1. Executive summary

VPSPilot is in good shape for an early-stage app. The security-critical design is sound and is
backed by tests: no generic command execution is exposed, every remote command is a constant (the
one parameterised command, container logs, is validated strictly), host keys are verified before
any credential is sent, secrets live only in Windows Credential Manager and never reach the
frontend, and remote output is size-capped and parsed strictly.

**No P0 and no P1 findings were confirmed.** There are 9 P2 and 8 P3 findings. They are mostly
lifecycle races, misleading alert state during outages, release-pipeline gaps (no CI, no signing,
no error boundary) and testing gaps (nothing exercises the Tauri command layer or a real SSH
server).

All local verification passed (fmt, clippy, 177 Rust tests, typecheck, ESLint, Prettier, 158
Vitest tests, production build). `cargo audit` reports one advisory that applies to a shipped
dependency (RSA timing side channel via `russh`), and `npm audit` reports 10 moderate advisories,
all in dev-only tooling.

## 2. Current implementation assessment

| Area | Assessment |
| --- | --- |
| Layering | Clear: `commands/` (thin Tauri wrappers) → `*/service.rs` (use-cases, no Tauri) → `*/repo.rs` (SQL). SSH features share one connect/verify/authenticate path (`ssh/client.rs:125`). |
| Security design | Strong. See section 3. |
| Persistence | SQLite with versioned, transactional migrations, FK cascade, CHECK constraints. |
| Monitoring | Correct parsing and arithmetic; staleness is shown as stale in the UI. |
| Alerts | Pure, well-tested state machine; lifecycle gaps during outages (R-02). |
| Frontend | Clean service/state/hook split with unit-tested pure logic; no error boundary (F-04). |
| Release | Not ready: no CI, no signing, no update strategy (D-03). |

## 3. Critical security findings

**None confirmed (no P0/P1).** Items reviewed and found sound, with evidence:

- **No command injection.** `LogsRequest::new` (`ssh/docker/logs.rs:50-67`) accepts only 12–64
  lowercase hex characters (`logs.rs:44-46`) and a tail of 1–1000, and it is the only way to build
  the command. All other remote commands are constants (`ssh/monitor/parse.rs:14`, docker
  `COMMAND`). No other `exec` call exists.
- **Host key before credentials.** `check_server_key` returns false unless the decision is
  `Trusted` (`ssh/client.rs:99-105`), which aborts the handshake before `authenticate_*`
  (`client.rs:152`). Unknown and changed keys are both refused; a changed key has no override.
- **Trust cannot be spoofed by the UI.** `trust_pending` only accepts the fingerprint the backend
  itself observed (`ssh/service.rs:45-69`).
- **Frontend cannot choose where to connect.** Every command takes a server id only
  (`commands/ssh.rs`, `terminal.rs`, `monitor.rs`, `docker.rs`).
- **Secrets.** Stored only via `keyring` (`credentials.rs:42-65`); the DB stores a reference
  (`db.rs:34`) and a test asserts there is no secret column (`db.rs:176`); `AuthSecret` has a
  redacting `Debug` (`client.rs:26-34`); no command returns a secret; errors are fixed strings.
- **Revocation.** Editing identity fields, changing or clearing the credential, forgetting the
  host key or deleting the server stops the monitor and closes terminals
  (`commands/servers.rs:35-83`, `commands/ssh.rs:56`, `state.rs:144-152`).
- **Tauri.** `core:default` only plus explicit `allow-*` per command (`capabilities/default.json`);
  `build.rs` lists all 26 commands and matches `generate_handler!`; no shell, fs, http or opener
  plugins; CSP is `default-src 'self'` with `style-src 'unsafe-inline'` (needed for xterm/React
  inline styles) and IPC-only `connect-src` (`tauri.conf.json:24`).
- **Local execution.** Nothing in the Rust code spawns a local process.

Residual security risks are in section 5 (F-01, F-05, D-01, F-09) and section 9.

## 4. Architecture findings

| ID | Pri | Title |
| --- | --- | --- |
| A-01 | P2 | Single trusted host key per server can cause false "host key changed" |
| A-02 | P3 | One global `Mutex<Connection>` serialises all DB access |
| A-03 | P3 | Server profile edit orders credential deletion before the DB write |

Suitability for future work: multiple concurrent servers (works, hard cap 20 monitors),
historical data (needs a new table plus retention; the pure engine and the monitor forwarder are
good seams), optional Linux agent (the `ssh/monitor` module would need a transport trait, which
does not exist yet; adding one is a small refactor, not a rewrite), other operating systems
(`parse.rs` is Linux-specific by design), centralised monitoring and AI investigation (no
structural blocker; alert events already persist). No rewrite is recommended.

No circular dependencies were found. `state.rs` depends on `ssh` and `alerts` types, and
`alerts::service` depends on `ssh::monitor::parse::Metrics`; this is acceptable coupling.

## 5. Reliability findings

**F-01 (P2) — A terminal opened while its server is being revoked survives the revocation.**
- Where: `commands/terminal.rs:31` (cap check), `:44` (`terminal::start(...).await`, up to 20 s),
  `:49` (insert). `commands/servers.rs:54-59` / `state.rs:144-152` revoke only what is already in
  the map.
- Evidence: the map entry is inserted after the await with no re-check that the server still
  exists, still has the same credential/host key, or that the cap still holds.
- Problem: delete/forget-key/credential-change during the connect window leaves an authenticated
  shell for a server the user just removed or revoked.
- Fix: after the await, re-run `prepare_connection`-style validation (or keep a per-server
  revision counter bumped by `revoke_server` and compare it); also re-check the session cap while
  inserting under one lock. Complexity: S.
- Verify: unit test that revokes between `start` and insert using a gated fake server.

**F-05 (P2, potential) — Idle terminal outlives a reloaded or crashed webview.**
- Where: `commands/terminal.rs:60-65`. A dead frontend is detected only when an output event
  fails to send. An idle shell prints nothing, and keepalives (`client.rs:133`) keep it open.
- Fix: close all terminals on window destroy / webview reload, or detect channel closure
  independently of output. Complexity: S–M. Not reproduced (needs a running app).
- The monitor does not have this problem: it sends on every interval (`commands/monitor.rs:149`).

**R-02 (P2) — Alerts stay "firing" with a stale value while a server is unreachable.**
- Where: `commands/monitor.rs:136-173` closes alerts only on `Failed`/`Stopped`; during `Retrying`
  the engine receives no samples (`alerts/engine.rs`) and the UI keeps the alert
  (`hooks/useMonitoring.tsx:57-62`).
- Also: the consecutive-sample streak is not reset across an outage, so samples separated by a
  gap count as consecutive (`engine.rs` `track.streak`). (P3)
- Problem: an alert can show "firing at 93 %" for a server nobody can reach. This is also a
  distinct item from the planned "server unreachable" alert.
- Fix: mark active alerts as stale on `Retrying` (UI) and reset streaks when a gap occurs.
  Complexity: S. Verify: engine test with a gap; UI test of the reducer.

Other reliability notes (no defect): monitor retry policy is correct (auth, host-key and
credential failures are never retried; network failures back off 5→60 s; deterministic failures
stop after 3 — `ssh/monitor/mod.rs:194-200, 295`). Terminal/monitor/docker tasks all end their SSH
session on every exit path.

## 6. Performance findings

No benchmarks were run, so none of the following are measured.

- Per server: one persistent SSH session, one exec channel per poll, a 1 s `sleep` inside the
  remote command (`monitor/parse.rs:17`), and at most 60 samples in memory on the UI side.
- Expected behaviour by fleet size (reasoning from the code, not measurement): 1 and 5 servers
  are trivial. 10 servers means 10 sessions and 10 DB reads per 5 s window, which is negligible.
  At 20 the hard cap is reached (`commands/monitor.rs:19`); **25 connected servers cannot be
  monitored at once by design** and the 21st `start_monitoring` fails with a clear message.
- **A-02 (P3):** all DB access shares one `std::sync::Mutex<Connection>` (`state.rs:77`). Each
  sample takes it briefly (never across an await). Not a problem at this scale; it would matter if
  historical samples were written on every poll.
- Unbounded buffers: none found. Terminal output is backpressured (channel of 64), remote output
  is capped (64 KiB monitor, 4 MiB docker), alert history is trimmed to 1000
  (`alerts/repo.rs:152`).
- **P-01 (P3):** the production bundle is one 669 kB chunk (build warning). Fine for a desktop
  app; no action needed unless startup time matters.

## 7. Database findings

Migrations (`db.rs:17-86`) are transactional with a version guard against newer databases
(`db.rs:109`), FKs are enabled (`db.rs:129`), and `ON DELETE CASCADE` is used for host keys,
rules and events. Tests cover fresh install, idempotence, step-wise upgrade preserving data,
rollback on failure, and a v1→latest upgrade with data.

- **A-01 (P2): one host key per server.** `known_host_keys.server_id` is the primary key
  (`db.rs:48`) and `evaluate` compares both key type and fingerprint (`ssh/hostkey.rs:40`).
  If a server offers several host key types and negotiation ever selects a different one
  (server reconfigured, or a russh upgrade changes preference order), a legitimate server is
  reported as "host key changed" and needs a manual forget-and-retrust. Safe failure, poor
  experience. Fix: store a set of keys per server, or pin the preferred host-key algorithm to the
  stored type. Complexity: M. Verify: fake server offering two key types.
- **A-03 (P3): non-atomic profile edit.** `servers/service.rs:41-53` deletes the credential, then
  updates the row, then clears `credential_ref`. If the DB step fails the secret is gone but the
  reference remains; the effect is a "no credential stored" message, not a leak. Fix: update the
  row first or wrap the DB steps in a transaction.
- Not a defect: the database path is the Tauri app-data dir (`lib.rs:25-35`); a failed open is
  reported to the UI instead of crashing. WAL failure is deliberately non-fatal (`db.rs:138`).
  No `busy_timeout` is needed with a single in-process connection. Corruption handling is "report
  the error"; there is no backup or recovery path (acceptable for MVP).
- Orphans: server deletion removes keys, rules and events by cascade and the secret explicitly.

## 8. Monitoring accuracy (reviewed against `ssh/monitor/parse.rs`)

Confirmed correct: CPU from two `/proc/stat` samples one second apart with idle+iowait as idle
and backwards counters rejected (`:109-137`); memory uses `MemAvailable` with a pre-3.14 fallback
(`:157-163`); disk uses `df -P -k /` parsed from the right and `used/(used+avail)` to match `df`
(`:176-197`); zero denominators are guarded; uptime and load reject NaN/negative; core count is
bounded; sample time comes from the local clock. Standard `/proc` and POSIX `df` make this work
on Ubuntu, Debian and most distributions, but compatibility with real servers (BusyBox, Alpine,
containers, unusual `df` output) was **not** tested.

Staleness: `applyMonitorEvent` keeps the last sample on a status change and
`LiveServerCard` dims it and labels it "Last seen" when the state is not `online`
(`components/monitoring/LiveServerCard.tsx:40,82,124`); summaries average only online servers
(`monitorState.ts` `summarizeLive`). **Stale metrics are not presented as live.**

Observation (P3, not a bug): CPU percent is a single one-second window, so it is spiky; with a
`consecutive` value of 1 an alert can fire on a one-second burst.

## 9. Dependency and release findings

- **D-01 (P2): RUSTSEC-2023-0071** (Marvin attack, `rsa` 0.10.0-rc.18, severity 5.9 medium),
  pulled in by `russh` with the `rsa` feature (`Cargo.toml:27`). It is a timing side channel in
  RSA private-key operations. Here RSA is used only for client-side signing during public-key
  authentication, so practical exposure is low. Options: drop the `rsa` feature and document
  "ed25519/ECDSA keys only", or track `russh`/`rsa` upgrades. Verify: `cargo audit`.
- Unmaintained/unsound transitive Rust crates (`proc-macro-error`, `glib` 0.18.5) arrive via
  Tauri's Linux GTK stack; they are not compiled into the Windows build as far as can be told
  from the lockfile alone (not verified with `cargo tree`). P3, tracked upstream.
- **D-02 (P3): `npm audit`** reports 10 moderate advisories, all in the lint toolchain
  (`minimatch` DoS via `eslint`/`typescript-eslint`). Dev-only; nothing ships in the app. The
  suggested `--force` fix downgrades ESLint, so it was not applied.
- **D-03 (P2): no release pipeline.** There is no `.github/` directory, no CI running the checks
  above, no code-signing configuration (`tauri.conf.json` has no `bundle.windows` signing
  settings), no update strategy (no updater plugin), and `bundle.targets` is `"all"`. An unsigned
  installer triggers SmartScreen warnings. Lockfiles for npm and Cargo exist and are committed.
  The installer was not built in this audit.

## 10. Frontend findings

- **F-04 (P2): no error boundary.** `main.tsx:9-17` renders the tree with no boundary (confirmed:
  no `ErrorBoundary`/`componentDidCatch` anywhere). An unexpected render error blanks the whole
  window. Fix: one boundary at the layout level with a "reload" action. Complexity: S.
- Event cleanup is done correctly in the terminal view (observers disconnected, disposal guarded
  against StrictMode double-mount, `TerminalView.tsx:74-129`). Under StrictMode in development
  two logins happen for one terminal; production is unaffected.
- Keystrokes typed before the session id is known are dropped (`TerminalView.tsx:97-99`); minor.
- Loading and error states exist for connect, monitor and terminal. Accessibility: 69 `aria-`/
  `role` usages across components; no automated accessibility checks were run.
- No component or integration tests exist for the frontend (see section 11).

## 11. Verification results

| Check | Result |
| --- | --- |
| `cargo fmt --check` | **Passed** |
| `cargo clippy --all-targets` | **Passed**, 0 warnings |
| `cargo test` | **Passed**, 177 passed, 1 ignored (real Credential Manager test) |
| `npm run typecheck` | **Passed** |
| `npm run lint` (ESLint) | **Passed** |
| `npm run format:check` | **Passed** |
| `npm test` (Vitest) | **Passed**, 158 tests in 9 files |
| `npm run build` | **Passed** (chunk-size warning only) |
| `cargo audit` | **Failed**: 1 vulnerability (RUSTSEC-2023-0071), 2 warnings |
| `npm audit` | **Failed**: 10 moderate, dev-only |
| Real-server SSH, Docker and monitoring | **Not performed** (rules forbid it) |
| Ignored Credential Manager test | **Not performed** (touches the user's real store) |
| `tauri build` installer, signing | **Not performed** |
| Benchmarks, memory/leak profiling | **Not performed** |

## 12. Testing gaps

Strengths: an in-process fake SSH server covers host-key trust, wrong password, no retry on auth
failure, retry/backoff, and that only the expected commands are requested; the docker and logs
parsers have injection and sanitisation tests; DB migrations are tested; frontend state reducers
are unit-tested.

- **T-01 (P2): no tests of the Tauri command layer** (`commands/*.rs`), which is where the
  revocation (F-01), forwarder lifecycle (R-02) and cap logic live. Nothing tests two features
  racing.
- **T-02 (P2): nothing validates behaviour against a real OpenSSH server**, including key types,
  login shells, Alpine/BusyBox, and `docker` group behaviour. The fake server proves protocol
  logic only.
- **T-03 (P3):** the real Credential Manager test is `#[ignore]`d and not part of any automated
  run; no frontend component tests; no security regression test for the capability list matching
  `generate_handler!` (it is kept in sync by comment only, `build.rs:1-2`).

## 13. Documentation findings

- **Doc-01 (P3):** `docs/architecture/overview.md` says "Tauri capabilities stay minimal
  (`core:default` only); add permissions per feature", but the capability file grants
  `core:default` plus 26 `allow-*` permissions. The Milestone 5 "Not supported yet" list still
  says "alert thresholds", which Milestone 6 added. The redesign on this branch is not described.
- **Doc-02 (P3):** missing `CONTRIBUTING.md`, `SECURITY.md` (vulnerability reporting), a
  changelog, and a statement of supported key types (unencrypted keys only) and the 2560-byte
  secret limit in user-facing docs. `docs/development/setup.md` is otherwise accurate.

**F-09 (P2, usability) — RSA-4096 private keys do not fit the credential store.**
`servers/model.rs:9` caps secrets at 2560 bytes (the Windows Credential Manager blob limit) and
rejects larger ones with a clear message (`model.rs:185-191`). An unencrypted RSA-4096 key in
PEM form is typically about 3.2 kB, so many users' keys cannot be saved and passphrase-protected
keys are unsupported (`client.rs:161-163`). This is a documented limitation, not a bug, but it
affects who can use the app. Fix: document it prominently and/or store large secrets encrypted
with a key held in the credential store. Complexity: M.

**P3 note — secrets in memory.** The password or key is held as a plain `String` for the life of
a monitor and cloned per connection (`client.rs:155-156`). It is never logged. Zeroizing is
optional hardening.

## 14. Future scalability risks

1. Single host key per server (A-01) will cause friction as soon as users reconnect to rebuilt
   servers or many key types.
2. The monitor is tied to SSH; a Linux agent needs a transport abstraction first.
3. Alerts and samples are in-memory; historical data and restart-surviving alert state need a
   schema plus retention policy and a write path that does not hold the global DB mutex.
4. The 20-monitor cap and per-poll 1 s remote sleep set the practical fleet ceiling.
5. Windows-only credential storage (`keyring` feature `windows-native`, `Cargo.toml:25`) blocks
   other desktop platforms until another backend is enabled.

## 15. Prioritized remediation plan

See `VPSPilot_Fix_Priority.md`. Order: F-01 → R-02 → F-04 → D-03 (CI first, then signing) → D-01
→ T-01/T-02 → A-01 → F-05 → F-09, then the P3 items.

## 16. Final MVP readiness assessment

| Area | Verdict |
| --- | --- |
| Security | **Ready for a private/beta MVP.** No exploitable defect found; residual items are low-likelihood. |
| Architecture | **Ready.** Layering is clean and extension points exist. |
| Testing | **Partially ready.** Strong unit coverage, but no command-layer, race or real-server tests. |
| Release | **Not ready for public distribution:** no CI, signing or update path. |

**Verdict: suitable for an internal or beta MVP now; fix the release blockers (D-03, F-04, F-01,
R-02, D-01) and run a real-server compatibility pass before a public release.**
