# VPSPilot

A lightweight desktop application for monitoring and managing Linux VPS servers through SSH.

**Version 0.2.0 (beta).** VPSPilot is a standalone Windows desktop application. It runs entirely
on your PC: no AWS account, cloud backend or hosted service is needed, and nothing has to be
installed on your servers (no Linux agent). It connects to your servers over SSH.

To install the beta, read the [Windows beta installation guide](docs/development/windows-beta-install.md).
The installers are unsigned, so Windows SmartScreen will warn.

## Features

- Saved server profiles, with passwords and keys kept in the Windows Credential Manager.
- SSH connection test with host key verification (trust on first use).
- Interactive SSH terminal.
- Live CPU, memory, disk, load and uptime monitoring (agentless, over SSH).
- Persistent metric history in a local SQLite database (7 days by default, 1 to 90 configurable).
- Threshold alerts with history.
- Read-only Docker container list and container logs, for SSH accounts allowed to run `docker`.
- Light and dark themes.

Monitoring and Docker features need a Linux server. See
[docs/architecture/overview.md](docs/architecture/overview.md) for what is and is not supported.

## What has and has not been verified

| Verified (automated)                                                                                   | Not yet verified                                                                                                    |
| ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| Rust, frontend and Playwright test suites, type check, lint, formatting and dependency audit all pass. | Behavior against **real Linux servers** (login, terminal, monitoring, Docker, reconnect, several servers at once).  |
| The Windows Credential Manager round trip works on a real machine.                                     | **Installed builds**: installing, first run, upgrading, uninstalling and the downgrade block on a clean Windows PC. |
| Both Windows installers (NSIS and MSI) build.                                                          | GitHub Actions results for the release commit.                                                                      |

The automated tests use in-process test SSH servers, and the UI tests run with the Tauri backend
mocked. They do not prove that VPSPilot works against a real server. Use the beta on servers you
can afford to test with. The remaining steps are tracked in
[docs/releases/v0.2.0-beta-checklist.md](docs/releases/v0.2.0-beta-checklist.md).

## Stack

Tauri 2 - React + TypeScript + Vite - Rust (russh, SQLite) - Tailwind CSS - xterm.js.

## Supported credentials

- **Password**, or an **unencrypted SSH private key** (OpenSSH or PEM format).
- Passphrase-protected keys, ssh-agent, host certificates and jump hosts are not supported yet.
- Secrets are stored in the Windows Credential Manager, which limits each to **2560 bytes**.
  ed25519, ECDSA and RSA-2048 keys fit; RSA-3072 and RSA-4096 keys usually do not. Generate an
  ed25519 key with `ssh-keygen -t ed25519` if yours is too large.

## Quick start

See [docs/development/setup.md](docs/development/setup.md) for prerequisites.

```bash
cd apps/desktop
npm install
npm run tauri dev
```

## Layout

- `apps/desktop` - the desktop app (`src` = React UI, `src-tauri` = Rust)
- `agents/linux` - reserved for a possible future agent; not used or required
- `docs` - architecture and development docs
- `scripts` - helper scripts

See [docs/architecture/overview.md](docs/architecture/overview.md).

## Contributing and security

- [CONTRIBUTING.md](CONTRIBUTING.md) - setup, checks and the security rules for changes.
- [SECURITY.md](SECURITY.md) - how to report a vulnerability, and what is in scope.
- [CHANGELOG.md](CHANGELOG.md) - what changed.
- Release preparation: [docs/releases](docs/releases).
- Audit reports: [docs/audits](docs/audits).

## License

See [LICENSE](LICENSE).
