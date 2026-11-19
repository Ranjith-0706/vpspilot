# VPSPilot for Windows: Beta Installation Guide

Version 0.2.0 (beta). The installers are **not code-signed** and have **not yet been tested on a clean machine or against real servers**. Use it on servers you can afford to test with.

## System requirements

- Windows 10 or 11, 64-bit.
- Microsoft Edge WebView2 Runtime. Windows 11 and current Windows 10 have it. If it is missing, the NSIS installer downloads it, which needs an internet connection. The MSI does not install it, so install it from Microsoft first if the app does not start.
- SSH access (IP or host name, port, username, and a password or an unencrypted private key) to Linux servers. Monitoring and Docker features need a Linux server. Docker features need an SSH account that is allowed to run `docker`.

## Choose one installer

| File | Installs for | Default location |
| --- | --- | --- |
| `VPSPilot_0.2.0_x64-setup.exe` (NSIS) | The current user, no administrator rights | `%LOCALAPPDATA%\VPSPilot` |
| `VPSPilot_0.2.0_x64_en-US.msi` | All users, asks for administrator approval | `C:\Program Files\VPSPilot` |

Use **one** of them on a PC. Installing both gives two separate copies of the app. They share the same data (see below). Upgrade with the same type of installer you installed with.

## Install

1. Download the installer and run it.
2. **SmartScreen:** because the beta is unsigned, Windows may show "Windows protected your PC" with the publisher listed as unknown. This is expected for an unsigned build. Only continue if you got the file directly from the VPSPilot maintainer, and compare its SHA-256 hash with the one they published (`Get-FileHash <file>` in PowerShell). If you are not sure where the file came from, do not run it. Do not turn off Windows Defender or SmartScreen.
3. Follow the installer and start VPSPilot from the Start menu.

## First run

On first launch VPSPilot creates its database automatically. There is nothing to configure. Go to **Settings** to set how long metric history is kept (default 7 days, range 1 to 90).

## Add a VPS

1. Open **Servers** and add a server: name, IP or host name, port, SSH username.
2. Set the credential: a password, or an unencrypted OpenSSH or PEM private key. Passphrase-protected keys and ssh-agent are not supported.
3. Run the connection test. The first time, VPSPilot shows the server's host key fingerprint. Compare it with the real one on your server before trusting it. If it later changes, VPSPilot refuses to connect until you review it.
4. Use the terminal, Dashboard monitoring, History and alerts for that server.

## How credentials are stored

Passwords and keys are saved in the **Windows Credential Manager**, not in the database. Each secret is limited to 2560 bytes. The database holds only a reference to it. Deleting a server removes its credential, host key and history.

## Where your data lives

- Database (servers, host keys, alerts, history): `%APPDATA%\com.vpspilot.desktop\vpspilot.db`
- Credentials: Windows Credential Manager, entries beginning with `vpspilot:`.

The data folder name is fixed by the app identifier, which is kept unchanged so existing installs keep working.

## Upgrade

Run the newer installer of the same type over the old one. Your data is kept. Back up `vpspilot.db` first if it matters. Installing an older version over a newer one is blocked by the installers (MSI refuses; NSIS asks you to uninstall first). This is configured but not yet tested.

## Uninstall

- Uninstalling removes the program. The NSIS uninstaller has an unchecked "Delete the application data" option; leave it unchecked to keep your servers and history, check it to delete `%APPDATA%\com.vpspilot.desktop`.
- Credentials in the Credential Manager are not removed by the uninstaller. Delete the `vpspilot:` entries in Credential Manager yourself if you want them gone.
- Behavior of the MSI uninstaller for user data has not been tested on a clean machine.

## Known limitations

- Unsigned installers; SmartScreen warning.
- Not yet tested against real servers or from an installed build on a clean PC.
- Windows only. Monitoring and Docker need a Linux server.
- Password or unencrypted key only. No passphrase keys, ssh-agent, jump hosts or host certificates.
- Docker support is read-only (list and logs).
- Monitoring runs only while the app is open.
- Metric history is local to this PC and is not backed up.
