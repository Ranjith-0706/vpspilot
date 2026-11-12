# Contributing to VPSPilot

Thanks for helping. VPSPilot handles server credentials and SSH sessions, so changes are reviewed
with security in mind first and features second.

## Getting started

Follow [docs/development/setup.md](docs/development/setup.md) (Windows, Node 20+ (CI uses 22), Rust, MSVC build
tools, WebView2), then from `apps/desktop`:

```bash
npm install
npm run tauri dev
```

## Before you open a pull request

CI runs exactly these; please run them locally first.

```bash
# apps/desktop
npm run typecheck && npm run lint && npm run format:check && npm test && npm run build

# apps/desktop/src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit            # new advisories fail CI
```

`cargo test -- --ignored` additionally runs the test that touches your real Windows Credential
Manager. It is not run in CI.

Keep pull requests small and focused. Add or update tests for behaviour you change; for a bug fix,
add a test that fails without the fix.

## Security rules for changes

These are the project's invariants. A change that breaks one will not be merged.

1. **No generic command execution.** Nothing exposes a shell, a free-form command or a local
   process to the frontend. Remote commands are compile-time constants; the only one that contains
   input (container logs) is built by a validating constructor. A new input-dependent command needs
   the same treatment and tests that try to inject into it.
2. **The frontend never chooses where to connect.** Commands take a saved server's id (or an
   unguessable session id) and nothing else. Host, port, username and secrets are read in Rust.
3. **Host key first.** Nothing is sent to a host whose key is not already trusted. Use the shared
   `connect_authenticated` path; do not add another way to connect.
4. **Secrets stay in the OS credential store.** Never in SQLite, logs, error messages, frontend
   state or test output. No command may return a secret.
5. **Remote output is untrusted.** Cap it, parse it strictly, strip control sequences, never show
   it as HTML and never echo it in an error.
6. **New app commands** must be added in three places: `src-tauri/build.rs`, `generate_handler!`
   in `lib.rs`, and `capabilities/default.json`. Keep Tauri permissions minimal.
7. **Changes that span SQLite and the credential store** follow the pattern in
   `servers/service.rs`: take the secret out first, change SQLite in one transaction, restore the
   secret if that fails. Do not describe it as atomic.
8. **Database changes** are new, numbered migrations in `db.rs`. Never edit an applied one.

## Layout

See [docs/architecture/overview.md](docs/architecture/overview.md). In short: React UI and
services in `apps/desktop/src`, Rust in `apps/desktop/src-tauri/src` (`commands` are thin wrappers
over `*/service.rs`, which use `*/repo.rs` for SQL).

## Reporting security problems

Do not open a public issue. See [SECURITY.md](SECURITY.md).
