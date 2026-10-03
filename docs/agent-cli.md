# Meerkateer Controller installers and CLI

Status: developer preview. The same Rust source builds the Controller on Linux and Windows. It is
outbound-only: it does not listen on a port and does not expose a remote shell, RCON, file browser,
or machine-control API.

## Download and install

The landing page links to a rolling, unsigned `controller-preview` release with three artifacts:

- **Windows MSI:** `meerkateer-controller-windows-x86_64.msi` installs the Controller, a Start-menu
  setup UI, a Start-menu local monitor TUI, the background-task helper, and the CLI.
- **Ubuntu DEB:** `meerkateer-controller_0.2.0_amd64.deb` installs the CLI and a hardened systemd
  unit. Run `sudo meerkateer-controller setup` for its Rust terminal UI.
- **Windows portable CLI:** `meerkateer-controller-windows-x86_64.zip` contains only the native
  executable and its instructions. It does not modify the machine.

Every package has an adjacent `.sha256` file. Preview artifacts are not code-signed yet, so verify
that checksum before installation. Microsoft Store signing is free for Store-distributed MSIX,
but the Controller's always-on background component needs the restricted `packagedServices`
capability, which Store policy says is usually not approved. An MSIX channel is therefore an
experiment, not a replacement for the MSI/ZIP path today.

Every package also contains the shared
[Developer Preview notice](../packaging/CONTROLLER-DISCLAIMER.md). The MSI displays it before
installation. Windows Setup and the cross-platform first-enrollment TUI require explicit consent
before exchanging a token, writing a machine credential, or starting the background service. The
Debian post-install script prints the notice location rather than blocking unattended `apt`
operations; its first-enrollment TUI remains the consent gate.

## What it collects

The setup UI lets the operator enable or disable each bounded signal:

- CPU utilization;
- used and total memory;
- aggregate used and total fixed-filesystem capacity, including inode pressure on Linux; and
- the running instance count for up to 16 explicitly named processes.

An agent heartbeat is always sent so disconnects can be detected. The Controller does not collect
command lines, environment variables, usernames, file paths, process IDs, or file contents.
Process matching is exact and case-insensitive. On Linux use the executable
name (for example `java`); on Windows it will commonly include `.exe` (for example
`MyGameServer.exe`).

## Windows setup UI

Install the MSI, then open **Meerkateer Controller Setup** from the Start menu. The UI asks for the
destination, HTTPS API URL, computer name, one-time token from Console → Connect, and which CPU,
memory, disk, or exact process signals may be sent. Local / self-hosted Community is available;
Meerkateer Cloud is visible but disabled until the hosted service opens. **Test connection** checks
the local state directory/free space, URL, DNS, network route, proxy/VPN-sensitive failures, TLS,
and the public `/ready` endpoint before enrollment. The token is used only through the child process environment,
cleared immediately, and never becomes an MSI property, process argument, task argument, or log
entry. Reopen the setup UI later to change signals without a new token. Open **Meerkateer
Controller Monitor** from the Start menu for the live local Rust dashboard; it requests elevation
because the packaged machine credential is readable only by Administrators, `LOCAL SERVICE`, and
`SYSTEM`.

Successful first enrollment registers an enabled Windows boot task under `LOCAL SERVICE`, starts
it immediately, and verifies that it remains running. The task is configured to start when a
missed boot trigger becomes available and to recover from repeated process exits. Reopening Setup
updates the same startup task rather than creating duplicates.

## Ubuntu setup UI

```sh
sudo apt install ./meerkateer-controller_0.2.0_amd64.deb
sudo meerkateer-controller setup
```

The Rust terminal UI asks for the same destination, API host, machine name, and masked one-time
token. Run **Test connection** before **Connect safely**; successful enrollment enables the hardened
`meerkateer-controller.service`. Run `sudo meerkateer-controller tui` later to open the same live
local dashboard. A normal removal preserves the machine credential; an explicit package purge
removes it.

Successful first enrollment enables the unit in `multi-user.target`, starts it immediately, and
verifies both `systemctl is-enabled` and `systemctl is-active`. The service uses `Restart=always`,
so an unexpected clean exit or failure is restarted after a short delay. A deliberate
`systemctl stop` still keeps it stopped until the next manual start or reboot.

## Local Rust dashboard

The background service is deliberately headless. The on-demand dashboard runs in a normal terminal
on both Windows and Linux:

```sh
meerkateer-controller tui
```

The packaged Ubuntu command is `sudo meerkateer-controller tui`; the MSI provides a Start-menu
shortcut that opens the protected machine config with elevation. The portable Windows ZIP can run
`meerkateer-controller.exe tui` against its normal per-user config.

The dashboard provides:

- a live local CPU, memory, aggregate disk/inode, and exact-process snapshot refreshed every five seconds;
- the enrolled API host, short machine/workspace identifiers, local sequence, last delivery
  success/error, retry state, and background-service registration state;
- an explicit signal allowlist and exact process-name editor, saved atomically without exposing the
  credential;
- local credential, config-directory/free-space, URL-policy, collector, and durable-retry
  diagnostics;
- a connection test for DNS, routing, timeout, proxy/VPN effects, TLS, and public `/ready` that
  sends no telemetry and no machine credential; and
- a bounded in-memory event list for the current TUI session only.

Use `Tab` or `1`–`4` to move between pages, `R` to refresh, `Space` to toggle a signal, `E` to edit
process names, `S` to save, `T` to run connection diagnostics, and `Q` to close. Closing the TUI does not
stop the daemon. The Web Console remains the authoritative fleet view; the TUI intentionally shows
only one computer and has no remote shell or command channel.

## Where the API host is configured

Set the API host once during enrollment, in the MSI setup UI or the Ubuntu terminal UI. The
Controller stores the normalized host, machine identity, selected signals, and the exchanged
machine credential in its protected config; the one-time enrollment token is never stored. Normal
service starts read that config, so unattended restarts do not need flags or environment variables.

Reopening setup shows the enrolled host and current signal selection. Host and machine identity are
read-only after enrollment because moving a machine to another control plane requires a new scoped
credential. Re-enroll (or explicitly purge first) to change hosts; use setup at any time to change
only the signal allowlist. This avoids an accidental typo silently sending telemetry to a different
server.

## Test the route and understand failures

Run the same redacted checks used by both setup UIs:

```sh
meerkateer-controller test-connection \
  --server https://meerkateer-api.example.com
```

Add `--strict` in automation when a failed required check should return a non-zero exit code. The
JSON report identifies `config_storage`, `disk_space`, `server_url`, `dns`, proxy environment, and
`api_ready` separately. It never prints proxy values, credentials, or the enrollment token.

During normal service operation, `agent.status.json` beside the protected config records only a
redacted error code, summary, hint, and timestamps. It distinguishes storage full/permission,
invalid config, DNS, connect/timeout, proxy, TLS, credential rejection/expiry, rate limiting, and
API/upstream outage. Network software cannot reliably prove that a VPN is the root cause, so VPN is
reported as an actionable routing/DNS/proxy possibility rather than a false certainty. After a
successful retry, the current error is cleared while `last_success_at` is updated.

The machine credential is never rendered by the TUI. The enrollment token is held in zeroizing
memory only for the enrollment request and is not written to config, command arguments, environment
variables, MSI properties, or session events.

## Build from source

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
meerkateer-controller inspect --watch-process java --watch-process postgres
```

```powershell
.\meerkateer-controller.exe inspect --watch-process MyGameServer.exe --watch-process postgres.exe
```

A missing process is reported as `running: false` and `instances: 0`. An invalid or duplicate
process name fails before any network request.

## Enroll and continuously report

Create a machine enrollment token from the selected workspace in the Console. Keep it out of shell
history and chat.

Linux:

```sh
read -rsp 'Enrollment token: ' MEERKATEER_ENROLLMENT_TOKEN && export MEERKATEER_ENROLLMENT_TOKEN
meerkateer-controller enroll --server https://monitor.example.com --name game-host-01
unset MEERKATEER_ENROLLMENT_TOKEN
meerkateer-controller configure --signals cpu,memory,disk,process \
  --watch-process java --watch-process postgres
meerkateer-controller doctor
meerkateer-controller tui
meerkateer-controller run
```

Windows PowerShell 7:

```powershell
$env:MEERKATEER_ENROLLMENT_TOKEN = Read-Host 'Enrollment token' -MaskInput
.\meerkateer-controller.exe enroll --server https://monitor.example.com --name game-host-01
Remove-Item Env:MEERKATEER_ENROLLMENT_TOKEN
.\meerkateer-controller.exe configure --signals cpu,memory,disk,process `
  --watch-process MyGameServer.exe --watch-process postgres.exe
.\meerkateer-controller.exe doctor
.\meerkateer-controller.exe tui
.\meerkateer-controller.exe run
```

Use `run --once` for a scheduler or a one-shot acceptance test. Continuous mode defaults to a
30-second interval; `--interval-seconds` accepts 5 through 3600 seconds. HTTP is accepted only for
loopback development URLs; every non-loopback server must use HTTPS.

The default config is `%APPDATA%\Meerkateer\agent.json` on Windows and
`$XDG_CONFIG_HOME/meerkateer/agent.json` or `~/.config/meerkateer/agent.json` on Linux. Override it
with global `--config PATH`, placed before the subcommand. Unix state is written atomically with
owner-only permissions. Packaged configuration is stored under `%ProgramData%\Meerkateer` on
Windows and `/var/lib/meerkateer-controller` on Ubuntu. Linux uses a hardened, unprivileged systemd
unit; Windows uses an ACL-restricted startup task under `LOCAL SERVICE`. The Controller persists an
ordered outage spool capped at 128 batches and four MiB, saves every acknowledgement atomically,
and drains at most 16 batches per cycle so reconnect work stays bounded. Package signing, native OS
secret stores, and signed updates remain release work.

Rotate a machine credential immediately without re-enrolling or printing the new secret:

```sh
meerkateer-controller rotate-credential
```

Continuous operation also rotates automatically during the final seven days of the current
credential. A failed early rotation leaves the still-valid credential in place and retries later.
To undo the last saved signal-allowlist change without altering identity, sequence, credential, or
queued telemetry, run `meerkateer-controller rollback-config`.

The daemon writes non-secret delivery health to `agent.status.json` beside the protected config.
This separate atomic file lets the TUI explain the last attempt, last success, and current bounded
error without racing signal settings or rendering the machine credential.

## Install as a background Controller

Enroll a staged config first so the one-time token is never passed to an installer. Then follow the
[Linux systemd instructions](../packaging/systemd/README.md) or
[Windows instructions](../packaging/windows/README.md). Both installers preserve credentials during
a normal uninstall and require an explicit purge to destroy them.

Package source lives in [`packaging/debian`](../packaging/debian) and
[`packaging/windows/Package.wxs`](../packaging/windows/Package.wxs). GitHub Actions builds the DEB,
MSI, Windows CLI ZIP, and adjacent SHA-256 files from the same pinned Rust source.

Agents on another machine connect to the API URL over outbound HTTPS; no VPN or inbound firewall
rule is required when that URL is reachable. The Cloud/Console can display their latest telemetry.
It cannot currently restart them: MKA-1 deliberately has no command channel. The constrained future
design and its security gates are documented in [ADR-0008](adr/0008-bounded-remote-recovery.md).

## Acceptance check

1. Run `inspect` and compare memory/filesystem totals with the OS tools.
2. Watch one known-running and one deliberately absent process; verify true and false results.
3. Enroll into the intended workspace and run `doctor`.
4. Run `configure --signals cpu,memory,disk,process --watch-process NAME`, followed by `run --once`;
   verify an accepted sequence is printed.
5. Stop the watched test process, run once again, and verify the stored `process.running` sample is
   zero. Restart it and verify the next fresh sample is non-zero.
6. Stop the API while continuous mode is collecting, then restart it; queued batches must drain in
   order without exceeding the documented spool ceiling or advancing past an unacknowledged batch.
7. Run `rotate-credential`, confirm the command output contains no secret, and verify the next send
   succeeds while the previous credential is no longer accepted after its grace window.

The Console Machine detail route reads only the latest durable batch and shows CPU, memory, disk,
platform, snapshot freshness/completeness, and watched processes. A stopped process is called out by
name. If the newest batch is old or incomplete, the Console says so instead of filling missing
values from an older batch. Historical host charts, alert-policy integration for process failure,
and retention downsampling remain later work.
