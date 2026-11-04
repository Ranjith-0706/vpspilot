# vpspilot

Open-source VPS monitoring and management desktop app with Docker insights and an integrated SSH terminal.

> **Status:** Early development. Working today: saved server profiles (credentials in the OS
> credential store), SSH connection test with host key verification, an interactive SSH terminal,
> live CPU/memory/disk monitoring, threshold alerts, and a read-only Docker container list.
> Everything is agentless over SSH and Linux-only for monitoring and Docker. The Logs page and the
> sample section on the Dashboard still use clearly-labeled mock data. See
> [docs/architecture/overview.md](docs/architecture/overview.md) for what is and is not supported.

## Stack

Tauri 2 - React + TypeScript + Vite - Rust (russh, SQLite) - Tailwind CSS - xterm.js. Planned: Go Linux agent.

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

## License

See [LICENSE](LICENSE).