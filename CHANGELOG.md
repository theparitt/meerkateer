# Changelog

All notable changes to this project will be documented in this file. The format follows
Keep a Changelog principles, and releases use Semantic Versioning.

## [Unreleased]

### Added

- Public GitHub repository and direct navigation link with GitHub icon.
- Node.js and Go SDKs for MKS-1 heartbeat, event, and deploy delivery, with language
  guides, retry tests, and CI coverage. The website now offers Node.js, Go, Rust,
  Python, and Host agent examples under SDKs.
- Minecraft Java / Paper pilot plan with separate external, host/process, and game-performance
  evidence; a saved game-instance address and bounded, owner-triggered Community status test.
- Minecraft query parser, public-address checks, per-peer test budget, and regression coverage
  for malformed packets, private destinations, migration upgrades, and Console signal display.
- Clickable service status history in the Console: current status summary, heartbeat
  bars, down/recovery report cards, and a UTC observation calendar with reported
  reasons and nearby event context. Unknown days remain visually distinct.
- Friendly system diagram, mascot action illustrations, integration examples, and visible
  feature, update, test, and roadmap sections on the public landing page.
- First-party documentation, support, policy, and SDK guide pages in the web UI.
- Community owner email/password sign-in and setup-key password recovery for existing
  installations.
- Commercial release phases with explicit exit tests; bootstrap now requires an owner
  password, and the setup key can no longer create a session directly.
- Repeatable migration 0007→0009 exercise that preserves a seeded service, plus CI
  coverage for every current migration.
- MKS-1 draft service-interface specification and JSON Schemas.
- Production roadmap for Game/SME reliability, self-hosting, and Meerkateer Cloud.
- Apache-2.0 licensing, DCO, governance, security, support, and contribution policies.
- Draft MKA-1 agent protocol, 13 validated schemas, and 30 positive/negative fixtures.
- Rust API, worker, agent, domain, configuration, protocol, and storage workspace.
- React/TypeScript web shell with types generated from the checked OpenAPI document.
- PostgreSQL 18 migration and deterministic tenant-scoped development seed.
- Non-root, read-only Compose development stack with random local credentials.
- Rust, web, policy, contract, OpenAPI, secret, migration, and smoke-test gates.
- Tenant-aware RBAC and Argon2id one-time service-key primitives with redaction tests.
- Forced PostgreSQL row-level security, separate non-superuser runtime role, identity,
  agent credential/configuration, session, and append-only audit schema.
- One-time bootstrap session with digest-only cookies, double-submit CSRF protection,
  and tenant-scoped project/service inventory endpoints.
- One-time service-key issue, bounded-overlap rotation, immediate revocation, and
  append-only lifecycle audit events.
- Short-lived one-time agent enrollment, installation-digest binding, 90-day scoped
  credentials, five-minute rotation overlap, and immediate fleet credential revocation.
- Durable MKS heartbeat/event/deployment ingestion with service-bound identity,
  timestamp windows, exact-retry idempotency, current-state projection, and outbox.
- Durable MKA telemetry batches with agent-bound authentication, exact replay handling,
  strict sequence ordering, visible gaps, normalized records, and sensitive-shape rejection.
- Lease-based transactional outbox processing with `SKIP LOCKED`, bounded retry backoff,
  dead-letter evidence, and a separate least-privilege worker database login.
- Repeatable API/database/worker integration coverage for authentication tampering,
  malformed and oversized payloads, replay conflicts, retry, and poison messages.
- Memory-bounded per-peer token buckets for authentication and ingestion, applied before
  expensive credential verification and covered by real `429`/`Retry-After` E2E.
- Tenant-scoped service status reads with configurable heartbeat staleness, preserving
  reported state while resolving stale observations to operator-safe `unknown`.
- Authenticated operator dashboard with project/service selection, current-state cards,
  recent heartbeat visualization, and a cause-aware incident/deployment timeline.
- Display-safe service timeline API and MKS secret/player/chat-shape rejection before storage.
- Company → workspace → machine modeling with workspace-scoped enrollment, many-to-many
  machine assignment, tenant-isolated APIs, connection freshness, and operator visualization.
- In-console workspace creation, one-time workspace enrollment tokens, and existing-machine
  assignment/removal with CSRF-protected mutations and memory-only secret presentation.
- Agent CLI enrollment from workspace tokens, HTTPS-only remote transport, redirect refusal,
  atomic owner-only credential storage, credential-safe diagnostics, and durable idempotent
  telemetry retry across process restarts.
- Console enrollment guidance and end-to-end coverage that uses the real agent binary to
  enroll, diagnose, send its first heartbeat, and appear online in the selected workspace.
- First-installation company bootstrap form in the Console.
- Console workflows to create a process/service, issue its one-time SDK key, and issue
  replacement/additional keys for existing processes.
- Dependency-free Python SDK for bounded, idempotent heartbeat, event, and deployment delivery,
  with unit tests, an executable example, and real API integration coverage.
- Typed async Rust SDK with Rustls transport, bounded responses, credential-safe errors,
  exact retry idempotency, executable examples, and real API integration coverage.
- Vite `/v1` proxy routing and web-proxy smoke coverage, fixing Console session 404 responses.
- Mascot-led Console theme using the supplied Meerkateer logo and wordmark, with a playful
  navy, cream, red, and gold design system that remains responsive and operations-focused.

### Changed

- Default local host ports are now `6511` for the web UI and `6510` for the API;
  bootstrap safely migrates exact legacy defaults while preserving custom choices.
- Vite now uses its TypeScript configuration explicitly and writes development cache to
  writable tmpfs, keeping the hardened read-only web container free of cache errors.
