# Supported platform policy

Status: Phase 1 developer preview

## Build and development baseline

| Component | Baseline |
| --- | --- |
| Rust | 1.93.1, pinned by `rust-toolchain.toml` |
| Node.js | 22 LTS, minimum 22.22.2 |
| npm | 10.9.8 package-manager metadata |
| PostgreSQL | 18 for the compact profile |
| Container runtime | Docker Engine 26+ with Compose v2 |
| Web browsers | Current and previous major Chrome, Firefox, Safari, and Edge |

## Target `v1.0` runtime matrix

- Meerkateer server: OCI/Linux amd64 and arm64.
- Agent: Linux amd64/arm64 with systemd; Windows Server 2022 and 2025 amd64.
- Compact self-hosted profile: PostgreSQL 18 and the versioned Compose bundle.
- HA Cloud profile: Kubernetes plus managed PostgreSQL; stream, telemetry, and object
  storage versions will be frozen by a benchmarked ADR before Phase 3.

Only rows exercised by installation, upgrade, rollback, reboot, proxy, and offline
recovery tests become supported for `v1.0`. Other environments may work but are not a
support commitment.
