# vpspilot

Open-source VPS monitoring and management desktop app with Docker insights and an integrated SSH terminal.

> **Status:** Desktop Foundation milestone. The UI uses clearly-labeled mock data. SSH, terminal,
> Docker and live monitoring are not implemented yet.

## Stack

Tauri 2 - React + TypeScript + Vite - Rust - Tailwind CSS. Planned: SQLite, Rust SSH, xterm.js, Go Linux agent.

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