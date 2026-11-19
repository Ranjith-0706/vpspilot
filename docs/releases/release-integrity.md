# Release Integrity: SHA-256 Checksums and Manifest

Checksums let a tester confirm that the installer they downloaded is the file the maintainer published. They do not replace code signing.

## Generate checksums (PowerShell)

Run this on the **final** installers only. If an installer is rebuilt, its checksum changes: discard the old value and hash again. Do not publish checksums for installers that will later be rebuilt.

```powershell
Get-FileHash -Algorithm SHA256 .\VPSPilot_0.2.0_x64-setup.exe
Get-FileHash -Algorithm SHA256 .\VPSPilot_0.2.0_x64_en-US.msi
```

The `Hash` column is the checksum. To write a text file in the common format:

```powershell
Get-ChildItem .\VPSPilot_0.2.0_* | ForEach-Object {
  "{0}  {1}" -f (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower(), $_.Name
} | Set-Content -Encoding ascii SHA256SUMS.txt
```

## Verify (for testers)

```powershell
Get-FileHash -Algorithm SHA256 .\VPSPilot_0.2.0_x64-setup.exe
```

Compare the result with the value on the GitHub Release page, ignoring case. If it differs, do not run the file.

## Release manifest format

Attach `release-manifest.json` to the GitHub Release. Fields marked `TBD` are filled in at release time. Never fill in a checksum from a build that will be redone.

```json
{
  "product": "VPSPilot",
  "version": "0.2.0",
  "channel": "beta",
  "commit": "TBD (full commit SHA the installers were built from)",
  "builtAt": "TBD (UTC, ISO 8601)",
  "signed": false,
  "installers": [
    {
      "filename": "VPSPilot_0.2.0_x64-setup.exe",
      "type": "nsis",
      "role": "primary",
      "version": "0.2.0",
      "architecture": "x64",
      "installScope": "current user",
      "sha256": "TBD",
      "sizeBytes": "TBD"
    },
    {
      "filename": "VPSPilot_0.2.0_x64_en-US.msi",
      "type": "msi",
      "role": "secondary",
      "version": "0.2.0",
      "architecture": "x64",
      "installScope": "all users",
      "sha256": "TBD",
      "sizeBytes": "TBD"
    }
  ]
}
```

Set `signed` to `true` only when both installers carry a valid Authenticode signature, and compute the checksums after signing.
