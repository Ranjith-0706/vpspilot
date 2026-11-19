# VPSPilot v0.2.0: Release Readiness

Validated 2026-11-19 on the author's Windows 11 machine. Updated after the installer metadata and downgrade-policy commit (see section 5). Nothing was pushed, tagged, signed or published. No real server was contacted.

## 1. Branch and commit

- Branch `feat/persistent-monitoring-history`. Validation ran on `d50e08e` ("chore: bump VPSPilot version to 0.2.0"), then two local commits were added (installer metadata and downgrade policy; docs). They are not pushed.
- Version 0.2.0 is consistent in `package.json`, `package-lock.json`, `Cargo.toml`, `Cargo.lock` and `tauri.conf.json`, and in the built executable and both installers.
- The branch is 9 commits ahead of `main`. `main` contains only PR #1 (container logs). Everything since (stabilization, CI, history, Playwright tests) is **not merged to `main`**. Local branches `chore/mvp-stabilization` (6 ahead) and `feat/ui-redesign` (1 ahead) also exist, but their commits are already part of this branch's history.
- Pull request status could **not be checked**: the `gh` CLI is not installed and I did not use the network. Whether any PR is open for this branch is unverified.

## 2. Completed functionality (present in code, per history and changelog)

Server profiles with Windows Credential Manager secrets. SSH test with trust-on-first-use host keys. Interactive terminal. Agentless monitoring (CPU, memory, disk, load, uptime) with retry. Threshold alerts with outage handling. Read-only Docker containers and logs. SQLite history with a retention setting (default 7 days). One consistent connection status. Stabilization fixes for sessions, revocation and credentials. CI, Playwright tests, and contributor and security docs.

## 3. Automated verification (run today on this machine)

| Check | Result |
| --- | --- |
| `tsc` (app + e2e), ESLint, Prettier `--check` | Pass |
| Vitest | 204 / 204 pass (12 files) |
| `cargo fmt --check`, `cargo clippy --all-targets -D warnings` | Pass |
| `cargo test` | 232 pass, 0 fail, 1 ignored (the OS credential test, run separately below) |
| `cargo test os_store_round_trip -- --ignored` | Pass against the real Windows Credential Manager (unique throwaway key, deleted afterwards) |
| Playwright (`npm run test:e2e`) | 38 / 38 pass |
| `cargo audit` | Exit 0. One ignored advisory plus 3 warnings, see section 6 |
| `npm audit --omit=dev` | 0 vulnerabilities |

Limits: the Rust tests use in-process SSH test servers and fake credential stores. Playwright runs the frontend with the Tauri backend **mocked**, so it does not exercise SSH, credentials or SQLite. These results cover logic and UI wiring, not real-world behavior.

Coverage by area (from the passing suites, not from real use): migrations and retention cleanup, history storage and recovery after a database failure, atomic profile and credential changes with rollback, wrong-password and unreadable-key handling, host-key gating, terminal registration and revocation, monitor retry limits, alert counting across gaps, Docker permission and unknown errors, oversized logs, per-server history isolation in the UI.

## 4. Real-server test results

**None. All of the following are unverified:** password and key login against real OpenSSH, host-key prompt and mismatch behavior, terminal use and revocation on a live session, monitor output parsing on real distributions, reconnect after a real network drop, 2 to 3 servers monitored at once, Docker list and logs (including a user without Docker permission), history growth and pruning over days, and long-running stability. The existing `docs/testing/mvp-validation-report.md` Appendices A and B still describe the procedure and are marked not run.

## 5. Windows installer results

Built with `npx tauri build`, rebuilt after the metadata and downgrade changes. Both bundles built successfully. The NSIS setup is the primary beta installer and the MSI is secondary:

| File | Size | Signed |
| --- | --- | --- |
| `src-tauri/target/release/bundle/nsis/VPSPilot_0.2.0_x64-setup.exe` (primary) | 2.98 MiB | No |
| `src-tauri/target/release/bundle/msi/VPSPilot_0.2.0_x64_en-US.msi` (secondary) | 4.09 MiB | No |

Executable metadata: ProductName `VPSPilot`, ProductVersion and FileVersion `0.2.0`.

Metadata now set: short and long description ("A lightweight desktop application for monitoring and managing Linux VPS servers through SSH.") in `tauri.conf.json` and `Cargo.toml`. The application identifier `com.vpspilot.desktop`, the product name and the version are unchanged, so the data folder (`%APPDATA%\com.vpspilot.desktop`) and the MSI upgrade code are unchanged.

Downgrade policy: `bundle.windows.allowDowngrades` is `false`. In the generated scripts the MSI refuses to install an older version over a newer one (with an error message) and still allows upgrades and same-version reinstalls. The NSIS installer blocks silent downgrades, and the interactive installer requires uninstalling first. This is read from the generated build scripts only and has not been tested by installing.

Install scope differs: NSIS installs per user (`%LOCALAPPDATA%\VPSPilot`), the MSI per machine (Program Files). Use one of them per PC. The installers are described in `docs/development/windows-beta-install.md`.

Findings:
- **Publisher and copyright are now set** (owner-approved): publisher `Ranjith`, copyright `Copyright © 2026 Ranjith`, via `bundle.publisher` and `bundle.copyright`. Because the files are unsigned, Windows still reports an unknown publisher. The executable's file description still reads "VPSPilot".
- **The application icon is still the stock Tauri logo** (`src-tauri/icons/128x128.png`), pending an approved source image. The configuration is ready: `npx tauri icon <source.png>` regenerates the set.
- Old 0.1.0 installers still sit in the same `bundle` folder; delete or ignore them when distributing.
- The installers are unsigned, so Windows SmartScreen will warn. Signing was not attempted, as instructed.
- **Not performed:** installing, first-run database creation from an installed build, upgrade from 0.1.0, and uninstall. I did not install on this machine, which holds the author's real data. Code review only: the database file is `vpspilot.db` in the Tauri app data directory and is created and migrated on startup (migration tests pass). Whether the uninstaller keeps user data (including the credential entries) is unverified. Run the checklist in section 9 on a clean VM.

## 6. Security assessment

- **RUSTSEC-2023-0071** (`rsa` Marvin attack): accepted and documented in `src-tauri/.cargo/audit.toml`. `russh` 0.65.x pins a pre-release `rsa`, so it cannot be upgraded here. VPSPilot only signs on the user's own machine for public-key login. Exposure is reduced, not zero. The entry has re-check conditions.
- `cargo audit` also printed 3 allowed warnings that are not in the policy file: `chrono` unmaintained (RUSTSEC-2026-0543), `proc-macro-error` unmaintained (RUSTSEC-2024-0370), `glib` unsound (RUSTSEC-2024-0429, a Linux GTK dependency of Tauri, not used on Windows). They do not fail the audit. Worth recording in the audit policy so a later change is noticed.
- Tauri capability `default` lists only the app's own commands plus `core:default`. The earlier audit's S2 (narrow `core:default`) is still open and low risk. No shell, filesystem or HTTP plugin permissions are granted. CSP is restrictive (`default-src 'self'`, IPC only), though `style-src` allows `'unsafe-inline'`.
- Secrets live in the Windows Credential Manager. Profiles hold only a reference. Errors are written not to echo remote output or keys (covered by tests). SQLite is not encrypted, but it holds no secrets (profiles, host keys, metrics, alerts).
- **GitHub Actions has not been verified to pass for `d50e08e`.** CI exists (`.github/workflows/ci.yml`) and I ran its steps locally, but I did not read its results. Treat CI as unconfirmed.

## 7. Remaining blockers

For **public release**:
1. No real-VPS testing has been done (section 4).
2. No installed-build testing: first run, upgrade, uninstall, clean machine (section 5).
3. Placeholder application icon (waiting for an approved image).
4. Installers are unsigned.
5. Work is not merged to `main` and CI status is unconfirmed.

For **internal beta**: none of these block a small group of informed testers, provided they know the installers are unsigned and unverified against real servers.

No genuine release-blocking code defect was found during validation, so no code was changed.

## 8. Recommended fixes

1. When an approved icon arrives: add a real icon set (`npx tauri icon <source.png>`) and rebuild. (Description, publisher, copyright and the downgrade policy are done.)
2. Run `docs/testing/mvp-validation-report.md` Appendices A and B on a throwaway VPS (password and key, host key change, Docker with and without permission, outage and reconnect, 2 to 3 servers).
3. Test the installers on a clean Windows VM (section 9).
4. Open a PR to `main`, confirm CI is green, then merge.
5. Obtain a code signing certificate before any public download.
6. Optional: narrow `core:default`; add the 3 audit warnings to the policy file with reasons; drop `'unsafe-inline'` from `style-src` if feasible.

## 9. Release checklist

- [x] Version 0.2.0 consistent everywhere
- [x] Type check, lint, format, unit and Playwright tests pass locally
- [x] Clippy and rustfmt clean
- [x] `cargo audit` and `npm audit` pass under the documented policy
- [x] NSIS and MSI build
- [ ] CI green on GitHub for this commit
- [ ] PR opened, reviewed and merged to `main`
- [ ] Real-VPS test procedure completed and recorded
- [ ] Clean-VM install: app starts, `vpspilot.db` is created, add a server, credential saved
- [ ] Upgrade 0.1.0 to 0.2.0 on a VM keeps servers, host keys and history
- [ ] Uninstall removes the app and its handling of user data matches what is documented
- [x] Product description set; downgrades blocked (`allowDowngrades: false`), build-script check only
- [ ] Downgrade block verified by installing 0.2.0 then an older build on a VM
- [x] Publisher and copyright set (Ranjith)
- [ ] Real icon set; rebuilt (waiting for an approved image)
- [ ] Installers signed (needs approval and a certificate)
- [ ] Tag `v0.2.0` and publish (needs approval)

## 10. Final verdict

**Internal Beta Ready.**

Code quality and automated checks are strong, and the installers build. It is not Public Release Ready because the product has never been tested against a real server or from an installed build, the icon is a placeholder, and the installers are unsigned.
