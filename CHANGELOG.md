# Changelog

All notable changes. The format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

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

### Changed

- Terminal sessions are registered atomically with the session cap; a session still connecting
  when its server is deleted, its credential or host key changes, or the window reloads is closed
  instead of registered. An idle session whose window is gone is closed.
- Alerts: a gap in monitoring restarts pending "N in a row" counts. A firing alert is shown as
  "status unknown" while the server is unreachable instead of presenting an old reading as
  current, and is not resolved because of the outage.
- Profile and credential changes put the secret back if the database step fails, and the profile,
  its credential reference and its trusted host key now change in one transaction.
- The credential form and error message explain the 2560-byte limit and which key types fit.

### Security

- Documented and recorded the accepted `RUSTSEC-2023-0071` exception with its reasoning
  (`apps/desktop/src-tauri/.cargo/audit.toml`).

### Known limitations

- Windows only for credential storage; Linux servers only for monitoring and Docker.
- Passphrase-protected keys, ssh-agent and host certificates are not supported.
- No code signing or automatic updates yet.
