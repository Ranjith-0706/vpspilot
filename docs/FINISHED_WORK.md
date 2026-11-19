# VPSPilot: Finished Work

Status as of 2026-11-19, version 0.2.0, branch `feat/persistent-monitoring-history`.

## Core features

| Area | What is done |
| --- | --- |
| Desktop foundation | Tauri desktop app with local server profile storage; credentials kept in the Windows Credential Manager |
| SSH connection test | Host key verification with trust-on-first-use |
| Interactive terminal | SSH terminal (xterm.js) with atomic session registration, session cap and revocation handling |
| Live monitoring | Agentless CPU, memory, disk, load and uptime over SSH, with retry |
| Alerts | Threshold alerts with history; "N in a row" counts restart after a monitoring gap; firing alerts show "status unknown" while the server is unreachable |
| Docker | Read-only container list and container logs |
| Persistent history | Samples stored in SQLite (at most one a minute per server), 7-day default retention (1 to 90, configurable), background cleanup, History tab with CPU/memory/disk/load charts, deleted with the server |
| Connection status | Dashboard and Servers show one consistent status (Monitoring active, Connected, Connecting, Reconnecting, Authentication failed, Connection error, Not connected); "Last test" shown separately |
| UI | Compact, responsive shell with light and dark themes; React error boundary with a recovery screen |

## Quality and tooling

- GitHub Actions CI: type check, lint, format, tests, build, clippy, `cargo audit`, `npm audit --omit=dev`. It runs on pull requests, on `main` and weekly. Actions are pinned to commit SHAs.
- Playwright UI tests (`npm run test:e2e`) run against a mocked Tauri backend. They cover connection status, the History tab, the retention setting and a light/dark and wide/narrow layout check. SSH, credentials and SQLite are not exercised.
- `.gitattributes` enforces LF line endings.
- MVP validation report in `docs/audits`.

## Security and docs

- Profile, credential and host key changes happen in one transaction, with rollback of the secret if the database step fails.
- The accepted `RUSTSEC-2023-0071` exception is documented in `apps/desktop/src-tauri/.cargo/audit.toml`.
- `README.md`, `CONTRIBUTING.md`, `SECURITY.md` and `CHANGELOG.md` are in place.

## Milestones (from git history)

1. Desktop foundation and profile storage
2. SSH connection test, interactive terminal
3. Live monitoring and threshold alerts
4. Docker containers and logs (PR #1)
5. Stability fixes: sessions, revocation, alerts, credentials
6. CI, dependency audit policy and contributor docs
7. Consistent connection status
8. Persistent monitoring history
9. Playwright UI tests
10. Version 0.2.0
