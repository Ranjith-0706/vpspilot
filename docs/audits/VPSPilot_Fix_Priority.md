# VPSPilot — Fix Priority

Companion to `VPSPilot_Codebase_Audit.md`. Only verified findings appear here. Complexity: S
(under a day), M (a few days), L (a week or more).

No P0 or P1 findings were confirmed.

## Fix batch 1 status (terminal lifecycle)

F-01 and the session-cap race are fixed and covered by regression tests. F-05 is fixed for
reload and window destroy; detection of a *crashed* webview relies on a 30 s delivery probe whose
real-webview behaviour could not be verified here. See the Batch 1 report.

## Fix batch 3 status (dependencies, CI, docs)

D-01 is mitigated by an explicit, reasoned exception (no fixed upgrade exists yet); D-03 is
partly done (CI added; signing and updates are not); F-09, Doc-01 and Doc-02 are done. See the
Batch 3 report.

## Fix batch 2 status (reliability)

R-02, F-04 and A-03 are fixed and covered by regression tests; see the Batch 2 report for the
remaining risks.

## Fix immediately

Nothing is exploitable or causes data loss today, so nothing is "stop and fix now". The closest
items, to do first in the next working session:

| ID | Pri | Title | Where | Complexity |
| --- | --- | --- | --- | --- |
| F-01 | P2 | A terminal opened while its server is being revoked survives the revocation | `commands/terminal.rs:31,44,49` | S |
| F-04 | P2 | No React error boundary; a render error blanks the window | `src/main.tsx:9-17` | S |

## Fix before MVP release

| ID | Pri | Title | Where | Complexity |
| --- | --- | --- | --- | --- |
| R-02 | P2 | Alerts remain "firing" with stale values during an outage; streak not reset across gaps | `commands/monitor.rs:136-173`, `alerts/engine.rs`, `hooks/useMonitoring.tsx:57-62` | S |
| D-03 | P2 | No CI, no code signing, no update strategy | repo root (no `.github/`), `tauri.conf.json` | M |
| D-01 | P2 | RUSTSEC-2023-0071 (`rsa` via `russh`); drop the `rsa` feature or upgrade | `Cargo.toml:27` | S–M |
| T-01 | P2 | No tests for the Tauri command layer or feature races | `commands/*.rs` | M |
| T-02 | P2 | No run against a real OpenSSH server (key types, Alpine/BusyBox, docker group) | whole SSH stack | M |
| F-05 | P2 (potential) | Idle terminal outlives a reloaded or crashed webview | `commands/terminal.rs:60-65` | S–M |
| F-09 | P2 | RSA-4096 keys exceed the 2560-byte store limit; document it and the unencrypted-key requirement | `servers/model.rs:9`, `ssh/client.rs:161` | S (docs) / M (fix) |

## Fix after MVP release

| ID | Pri | Title | Where | Complexity |
| --- | --- | --- | --- | --- |
| A-01 | P2 | One trusted host key per server can produce a false "host key changed" | `db.rs:48`, `ssh/hostkey.rs:40` | M |
| A-02 | P3 | Single global DB mutex; revisit before writing history on every poll | `state.rs:77` | M |
| A-03 | P3 | Profile edit deletes the credential before the DB update | `servers/service.rs:41-53` | S |
| T-03 | P3 | Credential Manager test not automated; no frontend component tests; capability list vs handler list kept in sync by comment only | `credentials.rs:113`, `build.rs:1` | S–M |
| Doc-01 | P3 | Docs drift (`core:default` only, "alert thresholds" unsupported, redesign) | `docs/architecture/overview.md` | S |
| Doc-02 | P3 | Add `CONTRIBUTING.md`, `SECURITY.md`, changelog; state supported key types | repo root | S |

## Optional improvements

| ID | Pri | Title | Where | Complexity |
| --- | --- | --- | --- | --- |
| — | P3 | Session cap check is not atomic with insert (can exceed 8 under a race) | `commands/terminal.rs:31,49` | S (fix together with F-01) |
| D-02 | P3 | 10 moderate `npm audit` advisories, dev-only (`minimatch` via ESLint); update when upstream fixes | `package.json` | S |
| P-01 | P3 | Single 669 kB JS chunk | `vite.config.ts` | S |
| — | P3 | Zeroize secrets held in memory and avoid per-connection clones | `ssh/client.rs:155-156` | S |
| — | P3 | CPU is a one-second window, so alerts with `consecutive = 1` can fire on bursts | `ssh/monitor/parse.rs:17` | S |
| — | P3 | Keystrokes typed before the session id is known are dropped | `TerminalView.tsx:97-99` | S |
| — | P3 | Transitive unmaintained/unsound Linux-only crates (`proc-macro-error`, `glib`); confirm with `cargo tree` that they are not in the Windows build | `Cargo.lock` | S |

## Not findings (planned features, not defects)

Alert acknowledgement, severity, snoozing, "server unreachable" alerts, and alert state surviving
a restart (documented as unsupported); persisted metrics history; container actions; passphrase
-protected keys; more than 20 monitored servers.
