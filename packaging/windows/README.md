# Windows agent service preview

The Rust workspace produces a Windows-compatible `meerkateer-agent.exe` and validates
the `enroll`, `doctor`, and `run --once` lifecycle. The signed MSI and Windows Service installer are a
Phase 4 release gate; do not treat this directory as a production installer yet.

For a development smoke test in PowerShell:

```powershell
$env:MEERKATEER_ENROLLMENT_TOKEN = Read-Host "Enrollment token"
.\meerkateer-agent.exe enroll --server https://meerkateer.example.com --name game-host-01
Remove-Item Env:MEERKATEER_ENROLLMENT_TOKEN
.\meerkateer-agent.exe doctor
.\meerkateer-agent.exe run --once
```

The production service will run under a dedicated low-privilege virtual service
account and protect its unique credential using Windows DPAPI. The current preview keeps
the atomic config under the enrolling user's `%APPDATA%\Meerkateer` directory. It does not expose an
inbound port or execute server-supplied commands.
