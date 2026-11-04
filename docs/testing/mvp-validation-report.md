# VPSPilot MVP Validation Report (Milestone 9)

- Branch audited: `feat/container-logs` (HEAD `31f55ec`), working tree clean at start
- Date: 2026-11-04
- Scope: audit, automated verification, test procedures, and (after review) fixes for B1 and B2. No new features.
- Nothing was committed, merged or pushed.

## 1. Tests completed

| Check | Command | Result |
| --- | --- | --- |
| Rust tests | `cargo test` | **PASS**: 174 passed at audit time; **177 passed, 0 failed, 1 ignored after the B1/B2 fixes** (3 new tests) |
| Windows Credential Manager round trip | `cargo test os_store_round_trip -- --ignored` | **PASS**: set / get / delete / idempotent delete against the real store (uses a unique `vpspilot:test:<uuid>` entry and deletes it) |
| Clippy | `cargo clippy --all-targets -- -D warnings` | **PASS** before and after the fixes (it caught a lock-held-across-await in one of my new tests, now fixed) |
| rustfmt | `cargo fmt --check` | **PASS** (exit 0), also after the fixes |
| Frontend tests | `npm test` (vitest) | **PASS**: 9 files, 158 tests |
| ESLint | `npm run lint` | **PASS** (no output) |
| TypeScript | `npm run typecheck` | **PASS** |
| Prettier | `npm run format:check` | **PASS** |
| Frontend production build | `npm run build` (`tsc && vite build`) | **PASS**. Warning: one 664 kB JS chunk (186 kB gzip) exceeds Vite's 500 kB threshold |
| npm dependency audit | `npm audit --omit=dev` | 0 vulnerabilities |
| Code audit (Phase 1/5) | Manual read of: `ssh/client.rs`, `hostkey.rs`, `service.rs`, `terminal.rs`, `docker/mod.rs`, `docker/logs.rs`, `monitor/mod.rs`, `monitor/parse.rs` (command + head), `credentials.rs`, `db.rs`, `state.rs`, `error.rs`, `lib.rs`, `tauri.conf.json`, `capabilities/default.json`, and `commands/{servers,ssh,terminal,monitor,docker}.rs` | Done, see sections 3-5 |

Notes on what the Rust suite covers (from test names; I did not review every test body): an in-process SSH server is used to exercise host-key refusal before authentication, changed-key rejection, wrong password, no stored credential, PTY/resize/IO, client and server-side close, monitor polling over a single login, no retry after auth failure, backoff retry for unreachable hosts, and unreachable-port handling.

Environment note: `cargo` was not on PATH in Git Bash/PowerShell here. It lives in `~\.cargo\bin`; I added it for the session. Not a code issue, but a setup gotcha for contributors (see `docs/development/setup.md`).

## 2. Tests not completed

| Item | Why |
| --- | --- |
| **Phase 3, all real-VPS testing** | No test server and no approved credentials were provided. I did not connect to any host. A procedure is in Appendix A. |
| **Phase 4, desktop UI verification** | No GUI automation is available in this session and I did not launch the app. Nothing in the UI was exercised. A manual checklist is in Appendix B. |
| `tauri build` (installer / bundle) and `cargo build --release` | Not run. Only the frontend production build was run. |
| `cargo audit` | Not installed; Rust dependency vulnerabilities are **unchecked**. |
| Compatibility on non-Ubuntu distros | Reviewed by reading the commands only (section 6). Not executed on any distro. |
| Slow-network / packet-loss behavior | Timeouts reviewed in code; not tested against a real degraded link. |
| Frontend component/UI tests | The vitest suite covers `services/` state logic only. No test touches React components. |

## 3. Bugs discovered

No crashes or failing tests were found. The issues below come from code review, and none has been reproduced against a running app.

**B1 (Medium, FIXED): Revoking trust or credentials does not end live terminal sessions.**
`delete_server`, `update_server`, `set/clear_server_credential` call `stop_monitor`, but nothing closes open terminals; `forget_host_key` stops neither monitors nor terminals (`commands/servers.rs`, `commands/ssh.rs`). `terminals` is keyed by session id only (`state.rs`), so there is no way to find a server's sessions. Effect: after the user deletes a server, clears its credential or forgets its host key, an already-open shell stays authenticated, and a monitor keeps running after the host key is forgotten (it only re-checks on reconnect). Terminal sessions also have no server association for UI display.

**B2 (Low-Medium, FIXED): Monitor retries deterministic failures forever.**
In `monitor/mod.rs`, any `poll()` error (non-zero exit, unparsable output, "refused to run command") triggers disconnect, backoff (5s up to 60s) and a fresh login, indefinitely. On a non-Linux host or a box lacking `/proc`, this produces a login every 60s in the server's auth log until the user stops it. Not a lockout risk (auth failures stop immediately) but noisy. `Failure::Other(_) => true` also retries generic "Authentication error" cases.

**B3 (Low): `start_monitoring` stops the existing monitor before checking the limit.**
`monitors.remove(&id)` runs before the `MAX_MONITORS` check, so a rejected restart still kills the running monitor. (Only reachable at the 20-monitor cap.)

**B4 (Low): Terminal session cap is racy.**
`open_terminal` checks `len() >= MAX_SESSIONS` and inserts after `await`ing the connection, so concurrent opens can exceed 8. Impact is small (bounded by UI behavior).

**B5 (Low): Host-key type is part of the match.**
`evaluate` requires the same key *type* and fingerprint. The SSH client picks the host-key algorithm from its own preference list, not constrained to the stored type. A server with several host keys is stable today, but a russh upgrade that changes algorithm preference could produce a false "host key changed" for every server. This fails safe (refuses), but it could be mistaken for an attack. Consider restricting the preferred host-key algorithm to the stored type.

## 4. Bugs fixed

Both fixed on request, uncommitted in the working tree:

**B1 (session revocation).** Terminal sessions are now stored with their server id (`state.rs: TerminalEntry`). `AppState::revoke_server` closes a server's terminals and stops its monitor. It is called from `delete_server`, `set_server_credential`, `clear_server_credential` and `forget_host_key` (which previously stopped nothing). `update_server` closes terminals only when host, port, username or auth method changed, so a rename leaves a shell alone. `TerminalHandle::close` now also signals a `Notify`, so a close request is not lost when the 256-item input queue is full. Tests: `closing_terminals_affects_only_that_server`, `close_is_delivered_even_when_the_input_queue_is_full`; the existing close/exit terminal tests still pass.

**B2 (monitor retry).** Poll failures are classified. Network/timing problems (timeout, dropped connection, channel failure) still retry forever with backoff. Failures retrying cannot fix (non-zero exit, unparseable output, refused command, oversized output) are counted across reconnects; the third consecutive one ends monitoring with a `Failed` status and no more logins. A successful sample resets the count. Test: `repeated_unparseable_output_ends_in_failed_instead_of_retrying_forever` (2 retries, then Failed, no remote text echoed). The earlier test for the first retry still passes.

Known limits of these fixes, none tested against a real server:
- If a server is deleted or revoked *while* a terminal is still connecting, that session registers after the revoke and survives until closed. The window is the connect time (up to 20 s).
- `Failure::Other` during login (for example a connection dropped mid-authentication) is still retried indefinitely, as before. Only post-login poll failures are capped.
- Revoking does not stop an in-flight Docker request; those are single, short-lived commands.

## 5. Security findings

Positive (verified in code):

- **No credentials in SQLite**: schema has only `credential_ref`; a test asserts no secret-like column. Secrets are stored in Windows Credential Manager (`keyring`), and no command can read one back.
- **Host-key verification is strict**: `check_server_key` returns `false` unless the key exactly matches a stored, user-trusted key, which aborts the handshake before any credential is sent. Unknown and changed keys are both refused. A single code path (`connect_authenticated`) serves test, terminal, monitor and Docker. Terminal, monitor and Docker additionally refuse to start with no stored key. Host certificates are rejected.
- **TOFU trust flow**: `trust_host_key` accepts only a fingerprint that equals the key the backend itself observed on the last test; changing host or port clears the trusted key (tested).
- **No frontend-supplied connection details**: every SSH command takes a saved server id only.
- **Command injection**: the only request-controlled values are Docker container id (validated as 12-64 lowercase hex, so it cannot start with `-` or contain shell metacharacters) and `tail` (1-1000). The test suite asserts exactly two quote characters and no `$ ` `;|<"\` in the controlled part. Monitor and Docker-list commands are compile-time constants. Remote output is size-capped, escape/control characters are stripped from logs, and Docker error text is never shown.
- **No Docker modifications**: only `docker ps -a` and `docker logs`. No `sudo` anywhere.
- **Capabilities**: `capabilities/default.json` grants the main window exactly the 26 registered app commands (matches `generate_handler!`). No shell/fs/http plugins. CSP is `default-src 'self'` with `connect-src` limited to IPC.
- **No `dangerouslySetInnerHTML`/`innerHTML` in the frontend.** `localStorage` holds only the theme and a demo-hide flag.
- **Error messages** are fixed user-safe strings; `AuthSecret` has a redacting `Debug`.
- **Logging**: only 5 `eprintln!` calls, none of which prints secrets. They print DB errors, keyring error kinds and alert errors. The DB-error and keyring messages come from library error text and I did not check them for sensitive paths.
- Migrations are transactional, versioned, refuse a newer-schema DB, and a rollback test exists.
- `npm audit`: 0 known vulnerabilities in production dependencies.

Concerns:

- **S1 (Medium, FIXED)**: B1 above. Revocation now terminates live terminals and monitors (see section 4).
- **S2 (Low)**: `core:default` is broader than needed. Narrowing to only the core permissions the app uses (events/channels) would be least-privilege-tighter. Not verified which sub-permissions are required.
- **S3 (Low)**: Unencrypted private keys only. Passphrase-protected keys are rejected with a clear message (documented limitation, which also means users must store unencrypted key material in Credential Manager).
- **S4 (Info)**: SQLite file is unencrypted. It contains hostnames, usernames, ports and fingerprints (no secrets). Acceptable for MVP; mention it in the privacy docs.
- **S5 (Info)**: `cargo audit` not run; Rust supply-chain status unknown.
- **S6 (Info)**: Terminal output is rendered by xterm.js, so a malicious remote can send terminal escape sequences. That is inherent to a terminal. Log lines are sanitized separately.

## 6. Compatibility limitations

Reviewed by reading only; nothing was executed on a distro.

- **Monitoring**: Linux only. Uses `/proc/stat`, `/proc/meminfo`, `/proc/uptime`, `/proc/loadavg`, `df -P -k /`, `getconf _NPROCESSORS_ONLN`, wrapped in `sh -c`. Should work on Ubuntu/Debian/RHEL-family/Alpine (BusyBox provides `head`, `cut`, `grep -E`, `df -P`; `getconf` may be absent on minimal Alpine, which would fail the poll; **untested**). Not supported: FreeBSD/macOS/any host without `/proc`. Old kernels without `MemAvailable` (<3.14) depend on parser fallback behavior I did not verify. Containers/LXC may report host-wide `/proc` values.
- Disk metric covers only `/`, not other mounts.
- **Docker**: requires the `docker` CLI on the SSH account's PATH and docker-group access (no sudo). Rootless/Podman-only setups are unsupported. The container id regex requires full lowercase hex ids from `docker ps`.
- **SSH**: password and unencrypted key auth only. No passphrase keys, agent, keyboard-interactive or host certificates. The shell allocation sends `xterm-256color`.
- **Platform**: credential storage targets Windows Credential Manager (via `keyring`); macOS/Linux builds are untested.
- Frontend bundle is one 664 kB chunk (fine for a desktop app).

## 7. Release blockers

I found no confirmed crash or data-loss blocker. Treat the following as blockers **until resolved or explicitly accepted**:

1. **Real-server testing has not happened.** Passing in-process tests do not prove behavior against OpenSSH on Ubuntu, real Docker output, or Credential Manager under the installed build.
2. **UI has not been exercised at all** (Phase 4).
3. B1/S1 is fixed and unit-tested, but the fix has not been exercised against a real server or the UI (Appendix A step 5 and B should include: delete a server and forget a host key while a terminal is open).
4. Installer (`tauri build`) never built or smoke-tested.

## 8. Recommended fixes (in priority order)

1. ~~B1~~ done (section 4). Optional follow-up: close the connect-time race by re-checking the server after `open_terminal` connects.
2. ~~B2~~ done (section 4). Optional follow-up: cap retries of `Failure::Other` during login.
3. B5: pin the preferred host-key algorithm to the stored key's type, with a test.
4. B3/B4: reorder the monitor limit check; reserve the terminal slot before the `await`.
5. S2: narrow `core:default`.
6. Install and run `cargo audit` in CI; add the Cargo toolchain path note to the setup doc.
7. Add a few component-level UI tests, and code-split the bundle if startup matters.

## 9. MVP release readiness

**Not ready to release yet; conditionally ready for a closed alpha.** The code is carefully built and every automated check passes (177 Rust, after the fixes, and 158 frontend tests, clippy, rustfmt, ESLint, tsc, Prettier, production build). The security design (strict host-key gating, id-only commands, validated Docker input, OS credential store, narrow capabilities) holds up in review. But the parts that matter most for first-user trust (a real VPS and the real UI) are untested. B1 and B2 are fixed in the working tree but uncommitted. Complete Appendices A and B on a throwaway VPS, consider B5, run `cargo audit`, then re-assess.

---

## Appendix A: Safe real-VPS test procedure (not yet run)

Prerequisites: a **disposable Ubuntu 22.04/24.04 VPS** you own, with nothing important on it; a new non-root user (`vpspilot-test`); do **not** use production servers or credentials. Get explicit approval before the first connection.

1. **Prep server**: create the user, add to `docker` group, install Docker, start 2-3 throwaway containers (e.g. `docker run -d --name t1 nginx`, one that logs steadily, one that exits). Generate a dedicated ed25519 key without passphrase (`ssh-keygen -t ed25519 -N ""`), install the public key. Keep a second SSH session open as a safety net.
2. **Key auth**: add server (key), store key, run connection test. Expect "host key unknown" with fingerprint. Compare to `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` on the server, then trust and re-test, expecting success with latency.
3. **Password auth**: add a second profile for the same host (password). Wrong password first (expect auth failed, no retry storm; check `/var/log/auth.log`), then correct.
4. **Changed host key**: on the server, regenerate host keys (`sudo rm /etc/ssh/ssh_host_* && sudo dpkg-reconfigure openssh-server && sudo systemctl restart ssh`), or point a second profile at a different host with the same name. Expect "host key changed", with **no auth attempt in auth.log**. Then Forget key, retest, trust the new one.
5. **Terminal**: with a terminal open, delete a second profile for the same server, forget its host key, and clear its credential in turn; each must end the shell (and forget-key must stop monitoring). Rename a server and confirm its shell survives. Then open, run `whoami`, `top`, `vim`, resize the window, paste a large block, `exit` (expect Closed). Open 9 terminals (expect the cap message). Close the app mid-session and confirm the remote `sshd` session disappears (`who`).
6. **Monitoring**: start against a non-Linux or `/proc`-less host if available and confirm it ends in Failed after 3 attempts. Start at 2s and 5s; verify CPU against `top`/`mpstat` under `stress-ng` load, RAM vs `free -k`, disk vs `df -P /`, load vs `uptime`, uptime. Block port 22 (`ufw deny 22` from a second session, careful!) or reboot and watch Retrying then recovery. Confirm a single login in auth.log per monitoring session.
7. **Docker list**: matches `docker ps -a`; stop/remove a container and refresh; remove the user from the docker group to see the permission message; `systemctl stop docker` to see daemon-unavailable.
8. **Logs**: tail 1 / 200 / 1000; container with colored output and ANSI/OSC sequences (`echo -e '\e]0;x\a\e[31mred'`), a very long line, binary output, a removed container (expect "no longer exists").
9. **Alerts**: add CPU>50 for 3 samples; `stress-ng --cpu 0`; expect firing, then resolve after stopping; disk and memory rules; restart the app and confirm history persists; stop monitoring while firing and confirm it resolves.
10. **Cleanup**: delete profiles, confirm no `VPSPilot` entries remain in Windows Credential Manager (`cmdkey /list`), destroy the VPS and the test key.

## Appendix B: Manual UI checklist (not yet run)

- **Servers**: add (valid / each invalid field: empty, port 0/65536, over-long), edit (rename keeps host key; host/port change clears it with a warning), delete (confirm dialog; credential removed from Credential Manager; cascades keys/alerts), credential set/replace/clear (field never echoes secret), connection-test panel for every outcome.
- **Terminal**: navigate away and back (session cleanup/reuse), close, remote exit, disconnect network, theme switch, copy/paste.
- **Dashboard / monitoring**: start/stop, interval change, status badges, sparkline, demo-data hide toggle persistence, restart after editing the server.
- **Alerts**: create/edit/disable/delete rules, global vs per-server, history clear, active-alert indicator.
- **Docker / Logs**: empty state, not-installed, permission denied, busy double-click, line-count selector, truncated banner, no HTML injection from container names/log text.
- **Settings**: theme (light/dark/system), app info, host-key list and forget.
- **General**: window min size 960x600, keyboard focus in dialogs, startup with a corrupt or newer-schema database (error shown, app still opens), first launch with empty DB.
