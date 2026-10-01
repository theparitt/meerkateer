# Meerkateer host agent CLI

Status: developer preview. The same Rust source builds on Linux and Windows. The agent is
outbound-only: it does not listen on a port and does not expose a remote shell, RCON, file browser,
or machine-control API.

## What it collects

Each run reports a bounded snapshot:

- CPU utilization;
- used and total memory;
- aggregate used and total fixed-filesystem capacity; and
- the running instance count for up to 16 explicitly named processes.

The agent does not collect command lines, environment variables, usernames, file paths, process
IDs, or file contents. Process matching is exact and case-insensitive. On Linux use the executable
name (for example `java`); on Windows it will commonly include `.exe` (for example
`MyGameServer.exe`).

## Build

The repository pins the supported Rust toolchain in `rust-toolchain.toml`.

Linux:

```sh
cargo build --locked --release -p meerkateer-agent
./target/release/meerkateer-agent --help
```

Windows PowerShell:

```powershell
cargo build --locked --release -p meerkateer-agent
.\target\release\meerkateer-agent.exe --help
```

## Inspect a machine without connecting it

`inspect` (alias `status`) needs no config or credential and prints JSON suitable for both a human
and automation:

```sh
meerkateer-agent inspect --watch-process java --watch-process postgres
```

```powershell
.\meerkateer-agent.exe inspect --watch-process MyGameServer.exe --watch-process postgres.exe
```

A missing process is reported as `running: false` and `instances: 0`. An invalid or duplicate
process name fails before any network request.

## Enroll and continuously report

Create a machine enrollment token from the selected workspace in the Console. Keep it out of shell
history and chat.

Linux:

```sh
read -rsp 'Enrollment token: ' MEERKATEER_ENROLLMENT_TOKEN && export MEERKATEER_ENROLLMENT_TOKEN
meerkateer-agent enroll --server https://monitor.example.com --name game-host-01
unset MEERKATEER_ENROLLMENT_TOKEN
meerkateer-agent doctor
meerkateer-agent run --watch-process java --watch-process postgres
```

Windows PowerShell 7:

```powershell
$env:MEERKATEER_ENROLLMENT_TOKEN = Read-Host 'Enrollment token' -MaskInput
.\meerkateer-agent.exe enroll --server https://monitor.example.com --name game-host-01
Remove-Item Env:MEERKATEER_ENROLLMENT_TOKEN
.\meerkateer-agent.exe doctor
.\meerkateer-agent.exe run --watch-process MyGameServer.exe --watch-process postgres.exe
```

Use `run --once` for a scheduler or a one-shot acceptance test. Continuous mode defaults to a
30-second interval; `--interval-seconds` accepts 5 through 3600 seconds. HTTP is accepted only for
loopback development URLs; every non-loopback server must use HTTPS.

The default config is `%APPDATA%\Meerkateer\agent.json` on Windows and
`$XDG_CONFIG_HOME/meerkateer/agent.json` or `~/.config/meerkateer/agent.json` on Linux. Override it
with global `--config PATH`, placed before the subcommand. Unix state is written atomically with
owner-only permissions. Preview background installers now live under `packaging/systemd` and
`packaging/windows`. Linux uses a hardened, unprivileged systemd unit; Windows uses an
ACL-restricted startup task under `LOCAL SERVICE`. Signed packages, native OS secret stores, a
signed Windows Service/MSI, signed updates, and offline spool beyond the current exact pending batch
remain release work.

## Install as a background agent

Enroll a staged config first so the one-time token is never passed to an installer. Then follow the
[Linux systemd instructions](../packaging/systemd/README.md) or
[Windows instructions](../packaging/windows/README.md). Both installers preserve credentials during
a normal uninstall and require an explicit purge to destroy them.

Agents on another machine connect to the API URL over outbound HTTPS; no VPN or inbound firewall
rule is required when that URL is reachable. The Cloud/Console can display their latest telemetry.
It cannot currently restart them: MKA-1 deliberately has no command channel. The constrained future
design and its security gates are documented in [ADR-0008](adr/0008-bounded-remote-recovery.md).

## Acceptance check

1. Run `inspect` and compare memory/filesystem totals with the OS tools.
2. Watch one known-running and one deliberately absent process; verify true and false results.
3. Enroll into the intended workspace and run `doctor`.
4. Run `run --once --watch-process NAME`; verify an accepted sequence is printed.
5. Stop the watched test process, run once again, and verify the stored `process.running` sample is
   zero. Restart it and verify the next fresh sample is non-zero.
6. Stop the API during a send and restart it; the next run must retry the exact persisted batch
   before advancing its sequence.

The Console Machine detail route reads only the latest durable batch and shows CPU, memory, disk,
platform, snapshot freshness/completeness, and watched processes. A stopped process is called out by
name. If the newest batch is old or incomplete, the Console says so instead of filling missing
values from an older batch. Historical host charts, alert-policy integration for process failure,
and retention downsampling remain later work.
