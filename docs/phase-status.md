# Delivery status

Last verified: 2026-09-28

This file reports evidence, not a production-readiness claim. A phase is complete only
when every exit gate in the [production roadmap](production-roadmap.md) has evidence.
The [commercial readiness plan](commercial-readiness-plan.md) has separate C0–C8 release
gates. C0 is in progress; C1–C8 are not complete.
The first pilot follows [Minecraft Java / Paper G0–G5](game-server-beta.md). G0 is in
progress; automatic probes, Paper TPS/MSPT, Discord alerts, and player status pages
have not shipped.

| Phase | State | Evidence / blocker |
| --- | --- | --- |
| 0 — Contract and OSS foundation | Complete | 13 schemas, 30 fixtures, policy/license/secret checks, six accepted ADRs, Apache-2.0 governance |
| 1 — Executable skeleton | Complete | Generated OpenAPI types, PostgreSQL 18 migration, storage ports, non-root/read-only Compose, API/worker/agent/web skeletons and full-stack smoke evidence |
| 2 — Tenant, identity, machine trust | In progress | Bootstrap/session/CSRF, audited Community owner re-login, inventory, service/agent credential lifecycle, immutable audit, forced RLS, separate runtime roles, and bounded node-local auth/ingest limits pass real PostgreSQL E2E; multi-user OIDC, gateway-wide limits, cloning quarantine, and signed desired config remain |
| 3 — Durable ingestion/status | In progress | Authenticated MKS/MKA paths durably commit idempotent facts plus outbox; lease-based worker retry/DLQ, gap/replay/privacy, and service status/staleness read model pass E2E. Timeline APIs, retention, partial acknowledgement, state-machine completion, and load evidence remain |
| 4 — Production agent | In progress | CLI enrollment, HTTPS policy, atomic Unix-owner-only credential/sequence state, doctor, real heartbeat delivery, and exact durable batch retry pass unit/E2E; production OS installers/secret stores, richer bounded collectors, signed updates, rotation automation, and soak remain |
| 5 — Game/SME adapters/probes | Not started | Protocol labs and SSRF boundary still required |
| 6 — Usable web product | In progress | Authenticated workspace dashboard signs Community owners in, switches projects, creates process/service SDK credentials, manages workspace/machine membership and enrollment tokens, and renders machine freshness, service state, heartbeat history, and incident/deployment timeline; company switching, richer configuration, accessibility review, and visual regression remain |
| 7 — Stripe Sandbox SaaS | Not started | Cloud mode fails closed; no billing calls or entitlements exist yet |
| 8 — Resilience/alerts | Not started | Requires durable ingest and notification outbox |
| 9 — Supply chain/release candidate | Not started | Signing, SBOM, Helm, security review outstanding |
| 10 — Beta/GA | Not started | Requires representative users, soak, restore and billing lifecycle evidence |

## Latest local evidence

- Minecraft G0: a game instance can be saved as a separate service with its own
  address/port. The Community Console can run a manual Minecraft Java status test;
  it keeps external response, host/process, and Paper performance signals distinct.
  Private/reserved probe destinations are rejected. Cloud probe access is disabled.
  The parser and a local protocol fixture pass unit tests, and isolated API/PostgreSQL
  E2E covers saved game configuration, CSRF, unsafe destinations, and missing services.
  G0 remains open until protocol compatibility and cross-tenant probe exercises pass.
- Commercial C0: new bootstrap requires an owner password, and the old setup-key
  session endpoint returns 404 even with a valid key. Owner password login and
  setup-key recovery pass real API/PostgreSQL E2E.
- The isolated 0007→0010 migration exercise preserved a seeded service, proved
  workspace/password schema objects exist, and passed twice. Fresh Compose install,
  API, worker, and dead-letter E2E passed. Remote CI has not yet run on this change.
- Console status history now exposes clickable down reports, recovery observations,
  heartbeat bars, and UTC day details from the 100 most recent service timeline facts.
  It does not calculate uptime from missing or sparse data.

- Repository policy, MKS/MKA contracts, OpenAPI and secret-shape scan pass.
- Rust workspace: format, compile, 47 unit/integration tests, and Clippy with warnings
  denied pass.
- Web: Biome check, 27 Vitest tests, TypeScript build and Vite production build pass.
- Python SDK: three unit tests plus real API E2E for authenticated event delivery pass.
- Rust SDK: three unit tests plus real API E2E for typed, authenticated event delivery pass.
- Node.js SDK: retry/idempotency and destination policy tests pass locally; CI is configured.
- Go SDK: retry/idempotency and destination policy tests pass in a Go 1.24 container; CI is configured.
- PostgreSQL migration and deterministic seed were applied from an empty container;
  the tenant-scoped fixture service count was verified.
- A real API process passed the `/live`, `/health`, and `/openapi.json` smoke harness.
- The complete Compose stack passed on PostgreSQL 18.1 using alternate host ports; API
  and worker ran as UID 10001, web as UID 1000, and API plus web proxy smoke tests
  passed. The isolated test stack and volume were removed afterward.
- A fresh owner can create a project and service, issue/rotate/revoke a service key,
  and every credential write is audited while only Argon2id hashes reach storage.
- Agent enrollment tokens are tenant-scoped, short-lived, one-time credentials. The real
  agent CLI enrolls into its selected workspace, persists owner-only state, reports doctor
  health, sends telemetry, and advances its durable sequence in Compose E2E. The same suite
  proves token replay rejection, duplicate-install rejection, credential rotation overlap,
  and immediate revocation of every agent credential.
- MKS E2E proved authenticate-before-parse, identity binding, exact retries, conflict
  rejection, credential revocation, heartbeat projection, and atomic outbox creation.
- MKA E2E proved durable normalized batches, exact replay, changed-payload conflict,
  sequence gap evidence, contiguous follow-up, and secret-shaped telemetry rejection.
- A clean PostgreSQL 18.1/Compose run processed thirteen supported outbox records, scheduled
  one poison record with bounded backoff, dead-lettered one at attempt five, and proved
  the worker login has no direct access to the outbox table.
- Running API E2E exhausts the node-local authentication budget and verifies a bounded
  `429` response with `Retry-After`; forwarded client-IP headers are not trusted by the app.
- Service inventory E2E proves an aged but valid heartbeat resolves to `unknown` with
  `stale=true`, then a fresh heartbeat restores `online` without losing reported-state evidence.
- Incident timeline E2E exposes bounded, display-safe heartbeat/event/deploy facts and
  proves an offline reason plus subsequent recovery are visible to the operator UI.

CI repeats these checks on a clean checkout. Production claims remain blocked until the
later phase gates are complete.
