# Changelog

All notable changes. The format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.2.0] - unreleased (beta, not yet published)

### Added

- Server profiles with credentials in the Windows Credential Manager.
- SSH connection test with trust-on-first-use host key verification.
- Interactive SSH terminal (xterm.js).
- Live CPU, memory, disk, load and uptime monitoring, agentless over SSH.
- Threshold alerts with history.
- Read-only Docker container list and container logs.
- Compact, responsive UI shell with light and dark themes.
- GitHub Actions CI (type check, lint, format, tests, build, clippy, `cargo audit`,
  `npm audit --omit=dev`), run on pull requests, on `main` and weekly.
- React error boundary with a recovery screen that shows no error details.
- `CONTRIBUTING.md`, `SECURITY.md` and this changelog.
- Playwright UI tests (`npm run test:e2e`, `apps/desktop/e2e`) run the real frontend in Chromium
  with Tauri's backend mocked, plus a CI job. They cover the connection status, the History tab
  (including switching servers while replies are in flight), the retention setting, and a
  light/dark and wide/narrow layout check. SSH, credentials and SQLite are not exercised.
- `.gitattributes` (`* text=auto eol=lf`) so Windows checkouts keep LF line endings and
  `prettier --check` passes.
- Persistent monitoring history: successful samples are stored in SQLite (at most one a minute
  per server), kept for 7 days by default (1 to 90, configurable in Settings) and removed in
  the background. The Dashboard has a History tab with a server and time-range selector and CPU,
  memory, disk and load charts, clearly labelled as recorded data rather than current status.
  History is deleted with its server.
- Monitor status events carry an optional `cause` for connect failures, so "Authentication
  failed" is shown only when the backend reported it.

- Windows installers: an NSIS setup (primary, installs for the current user) and an MSI
  (secondary, installs for all users), both unsigned. Product description, publisher (`Ranjith`)
  and copyright (`Copyright © 2026 Ranjith`) are set in the bundle metadata.
- Release documentation: Windows beta installation guide (`docs/development`), release checklist,
  draft release notes and a SHA-256 checksum and manifest guide (`docs/releases`), release
  readiness report (`docs/audits`) and a milestone summary (`docs/FINISHED_WORK.md`).

### Changed

- Terminal sessions are registered atomically with the session cap; a session still connecting
  when its server is deleted, its credential or host key changes, or the window reloads is closed
  instead of registered. An idle session whose window is gone is closed.
- Alerts: a gap in monitoring restarts pending "N in a row" counts. A firing alert is shown as
  "status unknown" while the server is unreachable instead of presenting an old reading as
  current, and is not resolved because of the outage.
- Profile and credential changes put the secret back if the database step fails, and the profile,
  its credential reference and its trusted host key now change in one transaction.
- CI actions are pinned to commit SHAs, with `contents: read` permissions and no persisted
  checkout credentials.
- Dashboard and Servers now show the same connection status, derived from live monitoring and
  terminal state: Monitoring active, Connected, Connecting, Reconnecting, Authentication failed,
  Connection error or Not connected. A connection test never counts as a connection and is shown
  separately as "Last test"; test results and terminal state now survive page navigation.
- The credential form and error message explain the 2560-byte limit and which key types fit.

- Installers refuse to install an older version over a newer one (`allowDowngrades` is `false`):
  the MSI errors, and the NSIS installer requires uninstalling first. The application identifier
  `com.vpspilot.desktop` and the data location are unchanged.
- README now describes the 0.2.0 beta and separates verified from unverified behavior.

### Fixed

- The Servers page always showed "Not connected", even while the Dashboard showed live
  monitoring, and its subtitle still said persistent connections were not implemented.

### Security

- Documented and recorded the accepted `RUSTSEC-2023-0071` exception with its reasoning
  (`apps/desktop/src-tauri/.cargo/audit.toml`).
- Recorded the `chrono`, `proc-macro-error` and `glib` audit warnings (RUSTSEC-2026-0543,
  RUSTSEC-2024-0370, RUSTSEC-2024-0429). None is compiled into the Windows build, and none is
  suppressed.

### Known limitations

- Windows only for credential storage; Linux servers only for monitoring and Docker.
- Passphrase-protected keys, ssh-agent and host certificates are not supported.
- No code signing or automatic updates yet; Windows SmartScreen warns about the installers.
- Not yet verified against real Linux servers or from an installed build on a clean Windows PC.
- The application icon is still the stock Tauri icon.
- History is stored once a minute, not per poll, and only while monitoring is running, so it has
  gaps whenever monitoring was off.
- A terminal session ends when you leave the Terminal page, so it is only visible there.
