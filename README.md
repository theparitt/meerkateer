# Meerkateer

Meerkateer is an open-source reliability platform for game-server operators. The first
pilot focuses on Minecraft Java / Paper, beginning with a manual external status test
and the existing agent, service contracts, and incident timeline. Scheduled probes,
Paper metrics, Discord alerts, and player status pages are tracked in the
[game-server beta plan](docs/game-server-beta.md).

> **Status: developer preview.** Phases 0 and 1 are complete. Phase 2 now has the
> tenant/RLS boundary, bootstrap session, inventory, service-key lifecycle, and agent
> enrollment/rotation/revocation. Phase 3 has durable MKS/MKA ingestion foundations,
> but production OIDC, alert delivery, billing, and the production security gates are
> not complete. Do not expose this build to untrusted networks.

## Product modes

- **Meerkateer Community** is self-hosted under Apache-2.0 and has no billing or remote
  license dependency.
- **Meerkateer Cloud** is the planned managed service with monthly Stripe billing for
  hosting, upgrades, backups, monitoring, and support.

Both modes use the same open-source core. The initial product monitors reliability; it
does not provide arbitrary remote commands, RCON administration, game provisioning, or
raw player/chat collection.

## Company and workspace model

- A **tenant** is one company and is the security, membership, and future billing boundary.
- A **project** is shown to operators as a workspace: one job or managed environment inside
  that company.
- An **agent** is one enrolled machine. A workspace can contain many machines, and a shared
  machine can be assigned to more than one workspace without duplicating its identity.
- A **service** is a monitored game server, SME application, or dependency inside a workspace.

Workspace-scoped enrollment tokens attach a newly enrolled machine automatically. Existing
company machines can also be assigned to or removed from a workspace without revoking them.

## Run locally

Requirements: Docker Engine 26+ with Compose v2, `make`, and OpenSSL.

```sh
make dev
```

The first run creates a mode-`0600` `.env` with random local credentials, builds
non-root containers, initializes PostgreSQL 18, and starts:

- web console: <http://127.0.0.1:6511>
- API: <http://127.0.0.1:6510>
- OpenAPI: <http://127.0.0.1:6510/openapi.json>

In a second terminal, run `make smoke`. Stop the stack with `make down`. To use an
existing PostgreSQL 18 instance, keep a dedicated database and role and set
`MEERKATEER_DATABASE_URL`; see [the storage deployment boundary](docs/deployment/storage.md).

Open the web console and choose **Self-host Community**. On a new database, create the company
and owner with the `MEERKATEER_BOOTSTRAP_TOKEN` value from the private `.env` file and an owner
password of at least 12 characters. Later visits use the owner's email and password at `/login`.
For installations created before passwords were added, use `/recover` once with the setup key
to set the existing owner's password. The setup key remains a recovery credential and must stay
private. Community sign-in is rate-limited, audited, and unavailable in Cloud mode.
Before starting an existing installation with this version, back up its database and run
`make migrate` to apply the workspace-assignment, password, and Minecraft-instance
migrations without changing
existing companies or sessions.

## Add a Minecraft Java / Paper instance

In the Console, create a workspace, open **Manage workspaces, machines, and processes**,
choose **Minecraft Java / Paper**, and enter the game's public DNS name or IP address
and Java port (usually 25565). Each game instance is its own service, independent of
the machine hosting it. Select the instance and click **Test game status now**. The
manual test reports version, server-reported player counts, and response time from the
Community control plane. It does not prove player login or gameplay, does not save an
uptime record, and does not send alerts. Private or reserved destinations are blocked;
Cloud probing stays disabled until separate network egress controls are in place.

For development without containers:

```sh
cargo run --locked -p meerkateer-server
cd meerkateer-web && npm ci --ignore-scripts && npm run dev
```

## Enroll a machine

In the Console, select a workspace, open **Manage workspaces and machines**, and issue a
ten-minute enrollment token. On the machine being monitored, place the token in the
environment without putting it in a command-line argument, then enroll and start the agent:

```sh
read -rsp 'Enrollment token: ' MEERKATEER_ENROLLMENT_TOKEN && export MEERKATEER_ENROLLMENT_TOKEN
cargo run --locked -p meerkateer-agent -- enroll --server http://127.0.0.1:6510 --name game-host-01
cargo run --locked -p meerkateer-agent -- doctor
cargo run --locked -p meerkateer-agent -- run
```

The agent requires HTTPS for non-loopback servers, follows no redirects, and stores its
credential plus durable sequence/retry state in an atomically replaced config file (mode `0600`
on Unix).
Use `--config PATH` before the subcommand when running multiple local agent identities.

## Add a process or application

Inside a workspace, open **Manage workspaces, machines, and processes**, create the process,
and copy the SDK key shown once. Choose Node.js, Go, Rust, Python, or PHP and use the
environment block shown by the Console. For example, install the Python SDK:

```sh
python3 -m pip install ./sdk/python
python3 examples/python/basic_monitor.py
```

Application code can report health and operational facts directly:

```python
from meerkateer_sdk import Meerkateer

client = Meerkateer.from_env()
client.heartbeat("ok", message="game loop healthy")
client.event("matchmaker_unavailable", level="error", message="dependency unavailable")
client.deploy("1.2.3", "abcdef123456", status="finished")
```

Rust services can use the typed async SDK from this workspace:

```toml
[dependencies]
meerkateer-sdk = { path = "../meerkateer/sdk/rust" }
```

```rust
use meerkateer_sdk::{HeartbeatStatus, Meerkateer};

let client = Meerkateer::from_env()?;
client.heartbeat(HeartbeatStatus::Ok, None).await?;
```

See the [Node.js SDK](sdk/node/README.md), [Go SDK](sdk/go/README.md),
[Rust SDK](sdk/rust/README.md), [Python SDK](sdk/python/README.md), and
[PHP SDK](sdk/php/README.md) guides.
A computer uses the outbound host agent; a process/service uses an SDK key. Both appear
under the selected workspace.

## Repository status

The repository now contains:

- draft, machine-tested MKS-1 and MKA-1 contracts with positive/negative fixtures;
- a Rust workspace for API, worker, agent, domain, protocol, configuration, and storage
  boundaries;
- a React/TypeScript operator-console skeleton with a checked OpenAPI boundary;
- PostgreSQL 18 migration and deterministic development seed;
- one-time bootstrap, company-scoped workspace/service inventory, many-to-many workspace
  machine assignment, CSRF-protected mutations, and auditable credential lifecycle APIs;
- authenticated, idempotent MKS heartbeat/event/deploy and MKA telemetry ingestion
  with atomic outbox writes, sequence-gap evidence, and privacy guards;
- a working outbound agent enrollment/doctor/run lifecycle with secure local credential
  persistence and exact-batch retry across restarts;
- Node.js, Go, Python, PHP, and typed async Rust SDKs with bounded transport, retry
  idempotency, and heartbeat/event/deployment helpers; Python and Rust also have real
  API E2E coverage;
- service inventory reads that expose reported and effective state and fail stale
  heartbeat observations safely to `unknown`;
- an authenticated operator dashboard with workspace switching, machine connection state,
  workspace/machine management, one-time enrollment tokens, service status, visual heartbeat
  history, outage/recovery reasons, events, and deployments;
- lease-based multi-worker outbox delivery with bounded exponential retry, poison-message
  DLQ, and a database login that cannot read tenant tables directly;
- non-root development containers and a Compose topology with generated credentials;
- policy, contract, unit, migration, web, lint, and secret-scanning CI gates; and
- the phased production roadmap and architecture decisions.

Follow [the milestone status](docs/phase-status.md),
[commercial release phases](docs/commercial-readiness-plan.md), and
[technical roadmap](docs/production-roadmap.md) for remaining scope and release gates.

## Quality commands

```sh
make test
make lint
make integration
```

## Security and privacy principles

- deny-by-default tenant boundaries;
- outbound-only agent communication;
- no shared fleet credential or plaintext stored service key;
- bounded parsing, collection, queues, labels, and payloads;
- bounded node-local authentication/ingestion rates; production proxies must also enforce
  distributed limits because the application intentionally ignores spoofable forwarding headers;
- no player names, player identifiers, chat, IP addresses, database rows, or query text
  in telemetry; and
- no entitlement decision based only on a browser redirect.

See [SECURITY.md](SECURITY.md) before reporting a vulnerability.

## Contributing

Contributions are welcome after reading [CONTRIBUTING.md](CONTRIBUTING.md),
[GOVERNANCE.md](GOVERNANCE.md), and the [Code of Conduct](CODE_OF_CONDUCT.md).
Contributions require a Developer Certificate of Origin sign-off.

## License

Copyright 2026 The Meerkateer Authors.

Licensed under the [Apache License, Version 2.0](LICENSE). The license covers the
software and documentation, not the Meerkateer names or logos; see
[TRADEMARKS.md](TRADEMARKS.md).
