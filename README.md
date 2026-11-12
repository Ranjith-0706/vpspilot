# vpspilot

Open-source VPS monitoring and management desktop app with Docker insights and an integrated SSH terminal.

> **Status:** Early development. Working today: saved server profiles (credentials in the OS
> credential store), SSH connection test with host key verification, an interactive SSH terminal,
> live CPU/memory/disk monitoring, threshold alerts, a read-only Docker container list, and
> container logs. Everything is agentless over SSH and Linux-only for monitoring and Docker. Only
> the sample section on the Dashboard still uses clearly-labeled mock data. See
> [docs/architecture/overview.md](docs/architecture/overview.md) for what is and is not supported.

## Stack

Tauri 2 - React + TypeScript + Vite - Rust (russh, SQLite) - Tailwind CSS - xterm.js. Planned: Go Linux agent.

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
- `agents/linux` - future Go agent
- `docs` - architecture and development docs
- `scripts` - helper scripts

See [docs/architecture/overview.md](docs/architecture/overview.md).

## Contributing and security

- [CONTRIBUTING.md](CONTRIBUTING.md) - setup, checks and the security rules for changes.
- [SECURITY.md](SECURITY.md) - how to report a vulnerability, and what is in scope.
- [CHANGELOG.md](CHANGELOG.md) - what changed.
- Audit reports: [docs/audits](docs/audits).

## License

See [LICENSE](LICENSE).