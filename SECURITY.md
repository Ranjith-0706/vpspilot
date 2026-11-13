# Security policy

## Supported versions

VPSPilot is pre-release (0.2.0). Only the latest commit on `main` is supported.

## Reporting a vulnerability

Please report privately, not in a public issue or pull request.

Use GitHub's **private vulnerability reporting** for this repository (the **Security** tab,
**Report a vulnerability**). Include what you found, how to reproduce it, and the impact. Do not
include real server addresses or credentials.

If that option is not available, open a public issue that says only that you have a security
report and asking for a private way to send it. Do not put details in the issue.

Maintainers: enable private vulnerability reporting in the repository settings and add a contact
here once one exists; until then this file only points at the GitHub feature.

## What is in scope

- A credential (password or private key) reaching SQLite, logs, error messages or the frontend.
- Connecting to, or sending a credential to, a host whose key was not verified.
- Any way for the frontend or remote output to run an unintended command, locally or remotely.
- A terminal, monitor or Docker request outliving a revoked server or a closed window.
- Tauri capability, CSP or IPC weaknesses.

## Known and accepted

- `RUSTSEC-2023-0071` (timing side channel in the `rsa` crate, pulled in by `russh`). It cannot be
  upgraded from this repository yet; the reasoning and the condition for removing the exception
  are in `apps/desktop/src-tauri/.cargo/audit.toml`.
- Docker group membership is root-equivalent on a server. VPSPilot never uses `sudo`; granting
  Docker access is the server owner's decision.
- Passphrase-protected keys, ssh-agent and host certificates are not supported.

Audit reports are kept in [docs/audits](docs/audits).
