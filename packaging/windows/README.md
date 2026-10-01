# Windows background-agent installer

The Rust workspace produces a native Windows `meerkateer-agent.exe`. The included installer runs it
at startup as a Windows Scheduled Task under the low-privilege `LOCAL SERVICE` identity, locks the
credential ACL to that identity, `SYSTEM`, and local Administrators, and configures automatic
restart. It is usable for managed preview deployments; a
signed MSI, signed executable, DPAPI-backed secret, and native Windows Service remain 1.0 gates.

For a development smoke test in PowerShell:

```powershell
$stagedConfig = Join-Path $env:TEMP 'meerkateer-agent.json'
$env:MEERKATEER_ENROLLMENT_TOKEN = Read-Host "Enrollment token" -MaskInput
.\meerkateer-agent.exe --config $stagedConfig enroll `
  --server https://meerkateer.example.com --name game-host-01
Remove-Item Env:MEERKATEER_ENROLLMENT_TOKEN
.\meerkateer-agent.exe --config $stagedConfig doctor
.\packaging\windows\Install-MeerkateerAgent.ps1 `
  -BinaryPath .\meerkateer-agent.exe `
  -ConfigPath $stagedConfig `
  -WatchProcess MyGameServer.exe,postgres.exe
Remove-Item -LiteralPath $stagedConfig
```

Run the installer from an elevated PowerShell session. It copies the binary to
`%ProgramFiles%\Meerkateer` and the enrolled config to `%ProgramData%\Meerkateer`. The token itself is
never an installer argument. The task opens outbound HTTPS connections; it exposes no inbound port
and executes no server-supplied command.

Uninstall preserves the credential for recovery by default. To permanently remove it:

```powershell
.\packaging\windows\Uninstall-MeerkateerAgent.ps1 -Purge
```
