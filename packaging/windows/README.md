# Windows Meerkateer Controller

The release workflow produces two Windows downloads from the same Rust source:

- `meerkateer-controller-windows-x86_64.msi` installs the CLI, a friendly first-setup GUI, the
  cross-platform Rust local dashboard, and Start-menu shortcuts.
- `meerkateer-controller-windows-x86_64.zip` is the CLI-only portable build.

After MSI installation, open **Meerkateer Controller Setup**. It enrolls the computer without
putting the one-time token into an MSI property or process argument, lets the operator select CPU,
memory, disk, and exact process signals, and registers an automatically restarted Scheduled Task
under the low-privilege `LOCAL SERVICE` identity. The credential ACL permits only that identity,
`SYSTEM`, and local Administrators.

Enrollment starts the task immediately and verifies that it remains running. The registered task
is enabled with a Windows boot trigger, starts when a missed trigger becomes available, has no
execution time limit, and retries unexpected exits. The optional portable ZIP is deliberately
manual and does not change startup settings.

Setup explicitly shows **Local / self-hosted Community** as the available destination and keeps
**Meerkateer Cloud** disabled until launch. Use **Test connection** before enrollment to check local
config storage/free space, URL policy, DNS, proxy/VPN-sensitive routing, TLS, and the API readiness
endpoint without sending the token, machine credential, or telemetry.

The API host is entered once during first setup and stored in the ACL-protected
`%ProgramData%\Meerkateer\agent.json` alongside the exchanged machine credential. Reopening the UI
shows the enrolled host and current signal choices; it locks host and machine identity while
allowing the signal allowlist to be changed. Purge and enroll again to intentionally move the
computer to another host.

Open **Meerkateer Controller Monitor** from the Start menu for live local signals, diagnostics, and
settings. The shortcut requests elevation so it can read the ACL-protected machine config. The
portable CLI can open the same dashboard with `meerkateer-controller.exe tui`. Closing the TUI does
not stop the headless startup task.

These direct-download developer-preview artifacts are not yet signed. Verify the adjacent SHA-256
file. Microsoft Store signing for MSIX is free, but an always-on packaged service requires a
restricted Store capability that is usually not approved; MSIX remains a future experimental
channel rather than replacing this MSI.

For a development smoke test in PowerShell:

```powershell
$stagedConfig = Join-Path $env:TEMP 'meerkateer-agent.json'
$env:MEERKATEER_ENROLLMENT_TOKEN = Read-Host "Enrollment token" -MaskInput
.\meerkateer-agent.exe --config $stagedConfig enroll `
  --server https://meerkateer.example.com --name game-host-01
Remove-Item Env:MEERKATEER_ENROLLMENT_TOKEN
.\meerkateer-agent.exe --config $stagedConfig doctor
.\meerkateer-agent.exe --config $stagedConfig configure `
  --signals cpu,memory,disk,process,service `
  --watch-process MyGameServer.exe `
  --watch-process postgres.exe `
  --watch-service MyGameServer `
  --watch-service postgresql-x64-18
.\packaging\windows\Install-MeerkateerAgent.ps1 `
  -BinaryPath .\meerkateer-agent.exe `
  -ConfigPath $stagedConfig `
  -ReplaceConfig
Remove-Item -LiteralPath $stagedConfig
```

Run the installer from an elevated PowerShell session. It copies the binary to
`%ProgramFiles%\Meerkateer` as `meerkateer-controller.exe` and the enrolled config to
`%ProgramData%\Meerkateer`. The token itself is
never an installer argument. The task opens outbound HTTPS connections; it exposes no inbound port
and executes no server-supplied command.

The MSI shows the complete Developer Preview notice before installation. First-time Setup also
requires an explicit authorization/risk checkbox immediately before enrollment; installed copies
can reopen the full text from `%ProgramFiles%\Meerkateer\DISCLAIMER.md`. The portable ZIP includes
the same file, and its first-enrollment TUI requires the same explicit consent.

Uninstall preserves the credential for recovery by default. To permanently remove it:

```powershell
.\packaging\windows\Uninstall-MeerkateerAgent.ps1 -Purge
```
