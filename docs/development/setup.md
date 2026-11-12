# Development setup (Windows)

## Prerequisites

1. **Node.js** 20+ and npm (https://nodejs.org)
2. **Rust** via rustup (https://rustup.rs) - provides `rustc` and `cargo`
3. **Microsoft C++ Build Tools** - install "Desktop development with C++" (MSVC + Windows SDK)
   from https://visualstudio.microsoft.com/visual-cpp-build-tools/
4. **WebView2 runtime** - preinstalled on Windows 11

Full guide: https://tauri.app/start/prerequisites/

## Commands (run from `apps/desktop`)

| Command | Purpose |
| --- | --- |
| `npm install` | Install dependencies |
| `npm run tauri dev` | Launch the desktop app with hot reload |
| `npm run dev` | Frontend only, in a browser (Tauri calls fall back) |
| `npm run lint` | ESLint |
| `npm run typecheck` | TypeScript strict check |
| `npm test` | Frontend tests (Vitest) |
| `cargo test` (in `src-tauri`) | Rust tests; add `-- --ignored` for the real Credential Manager test |
| `cargo clippy --all-targets` (in `src-tauri`) | Rust lints |
| `npm run format` / `format:check` | Prettier |
| `npm run build` | Typecheck + production frontend build |
| `npm run tauri build` | Package the desktop installer |

## Continuous integration

`.github/workflows/ci.yml` runs on every pull request and push to `main`: type check, ESLint,
Prettier, frontend tests and build, then `cargo fmt --check`, `cargo clippy -- -D warnings` and
`cargo test` on Windows, plus a dependency audit (`cargo audit` and `npm audit --omit=dev`) that
also runs weekly. Run the same commands locally before opening a pull request. The one accepted
Rust advisory, and why, is recorded in `apps/desktop/src-tauri/.cargo/audit.toml`.
