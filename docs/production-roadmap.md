# Meerkateer: Roadmap to Production

The shorter [roadmap to 1.0](roadmap-to-1.0.md) is now the canonical version sequence. This
document retains the detailed technical architecture, capacity, enterprise, and commercial
analysis behind those milestones. Where older sections place paid Cloud or Stripe before 1.0,
the canonical plan supersedes them: Community 1.0 comes first, Hosted Beta is parallel, and
billing follows proven free-beta operations.

> Baseline audit: 2026-09-27
> License target: Apache License 2.0
> Product focus: Game Server + SME Server reliability
> Product modes: one-company self-hosted Community + managed multi-tenant Cloud
> Billing direction: free Hosted Beta first; Stripe is deferred until the service is proven
> Current maturity: developer preview; see [delivery status](phase-status.md) and the [commercial release gates](commercial-readiness-plan.md).

The first customer pilot is now specifically [Minecraft Java / Paper](game-server-beta.md).
The broader adapter and SME scope below remains a longer-term architecture target.

## 1. Initial audit state

The initial repository contained the MKS-1 standard and seven JSON Schemas. The
`meerkateer-server` and `meerkateer-web` directories are empty. There is no Git
metadata, application source, package/build manifest, database migration, automated
test, CI/CD pipeline, container image, deployment manifest, or open-source governance
file.

The MKS-1 document is a useful starting contract, but it is not yet safe to freeze as
`Stable`. At minimum, the following contradictions or missing constraints must be
resolved before implementation:

- A check with `ok: false` is documented as requiring a safe `error`, while the
  schemas allow the error to be absent. They also allow `error` when `ok: true`.
- Event-kind prose requires `<domain>_<problem>_<condition>`, but its regex and several
  examples allow two segments.
- The metadata example presents `runtime`, `version`, and `build`, but the schema does
  not require them.
- The deploy example contains `version` and `commit`, but the schema does not require
  either field.
- Payload strings have no practical size limits, timestamp freshness policy, or
  explicit UTC `Z` enforcement.
- Cross-field invariants such as health status versus dependency checks cannot be
  guaranteed by the current schemas and need application-level validators.
- Compatibility checking introduces SSRF and DNS-rebinding risk that is not covered
  by the current standard.

## 2. Production target

The product has two operating modes from the same released public core. The canonical repository
ownership and artifact flow are defined in the
[Community and Cloud boundary](repository-and-cloud-boundary.md):

- **Meerkateer Community:** free self-hosting under Apache-2.0. Billing is disabled and
  Stripe is not required to install or operate the product.
- **Meerkateer Cloud:** a planned managed, multi-tenant service operated by the Meerkateer team.
  It begins as a free named-user beta after the Operations Beta gate. Billing through Stripe is a
  later milestone after tenant isolation, upgrades, backups, monitoring, support, SLOs, and real
  per-tenant cost have been proved.

Apache-2.0 permits commercial hosted services. The cloud terms of service, privacy
policy, support promise, and Meerkateer trademark policy remain separate from the
software license. The community build must not contain a remote license check or lose
core monitoring features when Stripe is absent.

The primary users are game-server owners/operators and SMEs that need dependable
monitoring without employing a dedicated observability team. MKS-1 remains the native
application integration, while a cross-platform Meerkateer Agent and safe network
probes cover servers that cannot be modified.

In either delivery mode, a user can:

1. create a project, environment, and service;
2. issue, rotate, and revoke a service key;
3. ingest heartbeat, operational-event, and deployment-event payloads;
4. register a service URL and run a safe MKS-1 compatibility check;
5. view current service status, recent events, and deployment history;
6. receive a basic webhook notification when a service changes state; and
7. install or upgrade the system using documented container artifacts.

For game servers, the initial product shows host/process health, reachability, query
latency, current/max player count, game/version/map metadata when safely available,
crashes/restarts, and maintenance state. It never stores player names, chat, Steam IDs,
IP addresses, or RCON credentials as telemetry.

For SME servers, the initial product covers Linux and Windows host health, process and
service status, Docker container health, HTTP/HTTPS/TCP/DNS checks, TLS certificate
expiry, and MKS-1 application/dependency state. Database probes only report a bounded
health result and timing; they never collect query text, rows, or customer data.

**Product boundary for `v1.0`:** Meerkateer is a reliability and alerting control plane,
not a game-hosting provisioner or remote administration panel. Starting/stopping
machines, arbitrary shell execution, file management, game-console/RCON commands, and
raw centralized logs are deliberately excluded from `v1.0`. Those capabilities need a
separate threat model, authorization model, and audited approval flow.

In Meerkateer Cloud, a tenant owner can additionally:

1. start a trial or monthly plan through Stripe Checkout;
2. view plan, quota, renewal, invoice, and payment state;
3. update payment details, change plan, or cancel through Stripe Customer Portal; and
4. retain access according to a documented grace and data-retention policy when a
   payment fails or a subscription ends.

Recommended initial architecture:

- **API and worker:** Rust workspace, Actix-web, Tokio, SQLx.
- **Agent:** Rust, using the same versioned protocol/model crates as the server but no
  server-domain or database dependency.
- **Control database:** PostgreSQL for identity, tenant/configuration, current state,
  audit, billing, and transactional metadata.
- **Telemetry path:** a storage/queue abstraction with two tested deployment profiles:
  a compact PostgreSQL-backed profile for development/small self-hosting, and an HA
  profile using a durable stream plus a columnar/time-series store and object storage.
  Select the concrete HA components through benchmarked ADRs before Phase 3; do not
  pretend the compact profile meets the Cloud capacity envelope.
- **Web:** React + TypeScript + Vite, generated API client from OpenAPI.
- **Jobs:** PostgreSQL-backed durable jobs/outbox initially; add Redis only after
  measured need.
- **Billing:** Stripe-hosted Checkout, Stripe Customer Portal, subscription webhooks,
  and a local entitlement projection. Meerkateer never handles raw card details.
- **Packaging:** OCI images plus Docker Compose for evaluation and Helm/Kubernetes
  manifests for production.
- **Observability:** structured JSON logs, Prometheus metrics, OpenTelemetry traces,
  and MKS-1 endpoints on Meerkateer itself.

Every major choice must be recorded in a short ADR. Phase 1 may replace a recommended
component, but only with an explicit trade-off and migration plan.

### 2.1 Initial commercial model

Start with fixed monthly tiers based on active monitored nodes, external checks, check
frequency, members, and retention period. A node is one enrolled host or one agentless
game/server endpoint according to a documented counting rule. Avoid metered billing for
`v1.0`; it creates invoice, late-event, correction, and customer-dispute complexity
before real usage is known.

Recommended catalogue shape:

| Offer | Billing | Intended use |
| --- | --- | --- |
| Community | None | User operates the Apache-2.0 build on their infrastructure |
| Cloud Trial | No charge for a short fixed period | Validate onboarding and value |
| Cloud Starter | Fixed monthly price | Small operators with bounded nodes/checks/retention |
| Cloud Team | Fixed monthly price | More nodes, members, regional probes, and retention |

Do not set final prices or limits until a staging load test provides storage, egress,
probe, backup, support, and Stripe-fee cost per active node. Do not price by player
count. Store Stripe Product and Price IDs in deployment configuration; do not hard-code
amounts or trust a client-supplied
Price ID. There must be at most one primary subscription per tenant in `v1.0`.

### 2.2 Billing state and ownership

Stripe is the source of truth for payment and subscription state. Meerkateer keeps a
durable local projection so authorization never requires a synchronous Stripe call.
At minimum, persist:

- billing account: `tenant_id`, Stripe customer ID, environment (`sandbox`/`live`);
- subscription: Stripe subscription/price IDs, status, period boundaries, cancellation;
- entitlement/quota snapshot and current usage counters; and
- webhook inbox: unique Stripe event ID, type, API version, received/processed time,
  processing result, and retry count.

Never grant access from a browser success redirect. Provision only after a verified
Stripe event or a server-to-server reconciliation confirms the subscription.

Initial access policy:

- `trialing` and `active`: plan entitlements enabled;
- `past_due`: visible warning and a documented grace period; preserve ingestion during
  the short grace period to avoid monitoring gaps;
- `unpaid`, `canceled`, and `incomplete_expired`: tenant becomes read-only, ingestion is
  suspended, and export/reactivation remains available until the retention deadline;
- cancellation at period end: access continues through the paid period; and
- no destructive data deletion occurs directly inside a webhook handler.

The exact grace and deletion windows are product/legal decisions and must be visible
before checkout.

### 2.3 Agent and probe architecture

The system has four explicit planes:

1. **Meerkateer Agent:** a small Rust service on Linux or Windows. It performs local
   host/process/service/container checks and game-protocol adapters, then makes outbound
   TLS connections only.
2. **Regional probe workers:** run agentless HTTP/HTTPS/TCP/DNS/TLS and supported game
   query checks from controlled locations. They use the same bounded check-result model.
3. **Control plane:** tenant/RBAC, enrollment, configuration, ingest, state projection,
   alerting, billing, audit, and APIs.
4. **Web console:** setup, inventory, status, incidents, events, deployments, alert
   policy, billing, and administration.

MKS-1 covers instrumented applications. A new versioned **Meerkateer Agent Protocol
(MKA-1)** must cover agent enrollment, capability negotiation, desired configuration,
telemetry batches, acknowledgements, and credential rotation. The transport should use
ordinary outbound HTTPS so it works behind common SME/game-host firewalls and proxies.

Agent safety requirements:

- one-time, short-lived enrollment tokens; unique agent identity and rotatable scoped
  credential after enrollment;
- server certificate verification and mutual authentication; no shared fleet secret;
- signed/versioned configuration with a last-known-good rollback;
- monotonic sequence and batch IDs, at-least-once delivery, server idempotency, and a
  bounded disk-backed offline spool;
- jitter, exponential backoff, circuit breaking, concurrency/body/time limits, and a
  strict CPU/memory/disk budget;
- no inbound listener and no generic command-execution capability;
- sandboxed/bounded built-in collectors; third-party executable plugins are out of
  scope for `v1.0`;
- secrets are references supplied locally and are never echoed through configuration,
  telemetry, support bundles, or logs; and
- signed release artifacts, staged auto-update, opt-out maintenance window, health
  confirmation, and automatic rollback.

Initial built-in adapters:

- generic host, process, systemd service, Windows Service, and Docker health;
- generic HTTP/HTTPS, TCP connect, DNS resolution, and TLS-expiry checks;
- generic UDP reachability only where a protocol supports a safe bounded response;
- Minecraft Java/Bedrock status and Steam A2S-family query adapters; and
- MKS-1 discovery, health, readiness, metrics, and deployment/event ingestion.

Every adapter must have recorded fixtures, malformed-packet tests, strict packet and
string bounds, deadlines, retry budgets, and a cardinality/privacy review. Game-specific
support is a versioned compatibility matrix, never a claim that every game is supported.

### 2.4 Enterprise stability contract

`Enterprise` describes measurable behavior, not a marketing label. `v1.0 Stable` is
allowed only after all gates below are evidenced:

- Cloud ingestion and alert evaluation SLO of at least 99.9% during the beta window;
- no silent telemetry loss: every accepted batch is durable, acknowledged, processed,
  rejected with a stable reason, or visible in an operator dead-letter workflow;
- agent continues collecting through a 30-minute control-plane outage and drains its
  bounded spool safely after recovery;
- baseline agent budget under steady-state reference load: less than 1% of one CPU core,
  less than 128 MiB RAM, and a configurable disk ceiling;
- status/alert detection p95 within two configured check intervals plus delivery time;
- zero cross-tenant access in automated isolation tests and zero unresolved critical or
  high security findings;
- rolling upgrade and rollback with no accepted-data loss; quarterly restore evidence
  meeting the published RPO/RTO;
- server supports the current and previous two agent minor releases, with an explicit
  end-of-support policy and compatibility tests;
- Linux amd64/arm64 and supported Windows Server versions have install, upgrade,
  uninstall, reboot, proxy, and offline-recovery tests; and
- a published capacity envelope is proven by soak and burst tests. Initial candidate:
  10,000 connected agents, 100,000 configured checks, and a 5,000-sample/second
  15-minute burst per production HA profile; adjust only through a recorded ADR and
  publish the final measured hardware, retention, and downsampling profile.

## 3. Delivery phases

The phase estimates add up to roughly 16 engineering weeks, but that is not a credible
calendar promise for one person when multi-platform testing, external security review,
beta soak, legal work, and production-account activation are included. Plan for:

- **one experienced engineer:** approximately 6-9 calendar months; or
- **a focused 3-4 person team:** approximately 4-5 calendar months, with backend/control
  plane, agent/systems, web/product, and platform/QA ownership.

Run user experience, security, documentation, and test-lab work continuously rather
than postponing them to the final week. Do not shorten the soak/security gates merely
to meet a date.

### Phase 0 — Contract and open-source foundation (3-5 days)

**Goal:** turn the current documents into a trustworthy, testable project foundation.

Work:

- initialize one Git repository and define the monorepo layout;
- change MKS-1 status from `Stable` to `Draft` until conformance tests pass;
- specify MKA-1 as Draft, including enrollment, config, batching, acknowledgement,
  capability, error, and compatibility semantics;
- resolve every contract ambiguity listed in section 1;
- add bounded lengths, slug patterns, UTC timestamp rules, and safe error semantics;
- add positive and negative fixtures for every MKS-1 and MKA-1 schema/protocol message;
- validate all schemas and documentation examples in CI;
- add `LICENSE` with the unmodified Apache-2.0 text and add `NOTICE` where required;
- add `README.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`,
  `GOVERNANCE.md`, `SUPPORT.md`, `CHANGELOG.md`, and issue/PR templates;
- add a separate Cloud terms-of-service, privacy, acceptable-use, cancellation/refund,
  data-processing, retention/deletion, and trademark documentation workstream;
- adopt SPDX identifiers (`Apache-2.0`) and a Developer Certificate of Origin (DCO);
- document trademarks separately: Apache-2.0 licenses code, not project marks; and
- decide the supported Rust, Node.js, PostgreSQL, browser, and platform matrix.

Exit gate:

- schemas compile under JSON Schema 2020-12;
- every example and fixture is checked automatically;
- contract decisions are captured in ADRs;
- repository can accept an external contribution under documented Apache-2.0 terms;
- secret scanning, dependency review, formatting, and lint checks run on every PR.

Release marker: `v0.1.0` (contract preview).

### Phase 1 — Executable skeleton and local developer experience (1 week)

**Goal:** one command starts a minimal end-to-end system.

Work:

- create the Rust workspace (`api`, `worker`, `agent`, protocol, and
  domain/application/infrastructure crates);
- create the TypeScript web application and generated API-client boundary;
- define versioned configuration with startup validation and secret-safe diagnostics;
- add an explicit `community`/`cloud` deployment mode; Community must boot without any
  Stripe configuration, while Cloud must fail closed if billing configuration is
  incomplete;
- add PostgreSQL migrations and a repeatable development seed;
- define control/telemetry storage ports and compact/HA deployment profiles so domain
  logic is not coupled to one high-volume storage engine;
- implement `/live`, `/ready`, `/metrics`, and build/version information for
  Meerkateer itself;
- publish OpenAPI and keep it checked for breaking changes;
- add Dockerfiles, a development Compose stack, `make`/`just` tasks, and `.env.example`;
- add unit, integration, and smoke-test layers to CI;
- generate deterministic builds with locked dependencies;
- create Linux and Windows agent service skeletons with a fake collector and local
  integration harness; and
- add a deterministic fake game/SME server lab for protocol and failure testing.

Exit gate:

- a new contributor can clone, start, test, and stop the stack using README commands;
- clean checkout passes format, lint, unit, integration, schema, and web checks;
- containers run as non-root and expose no default credentials.

Release marker: `v0.2.0` (developer preview).

### Phase 2 — Tenant, identity, and machine trust (1-2 weeks)

**Goal:** establish the security boundary before accepting telemetry.

Work:

- model tenant, user, membership/role, project, environment, and service ownership;
- support bootstrap-admin plus one production authentication path (OIDC recommended);
- enforce authorization centrally with deny-by-default tenant scoping;
- issue service keys once, store only a slow hash, and retain a non-secret prefix for
  lookup and audit display;
- implement key rotation, overlap window, revocation, last-used time, and rate limits;
- implement short-lived enrollment tokens, per-agent identity, certificate/credential
  rotation, revocation, cloning detection, and tenant-bound agent authorization;
- version and sign desired agent configuration and audit every configuration change;
- add an immutable audit log for administrative and credential lifecycle actions;
- add CSRF/session protections for browser auth and strict CORS/security headers; and
- threat-model tenant isolation, key compromise, account takeover, and log leakage.

Exit gate:

- cross-tenant property/integration tests prove tenant isolation;
- plaintext service keys never reach storage or logs;
- service-key and agent-credential rotation/immediate revocation work end to end;
- a copied or revoked agent credential cannot enroll a second identity or cross tenant;
- authorization decisions are covered by a role/action matrix.

Release marker: `v0.3.0` (private alpha).

### Phase 3 — Durable telemetry ingestion and status model (1-2 weeks)

**Goal:** reliably accept telemetry and compute current service state.

Work:

- implement the three MKS-1 ingest endpoints plus MKA-1 batch/ack endpoints and
  schema/application validation;
- authenticate first, then bind payload identity to the service-key mapping;
- enforce request/body limits, timeouts, timestamp skew, rate limits, and safe errors;
- add idempotency/event identifiers to prevent retry duplicates;
- store normalized, redacted events and update current status transactionally;
- batch numeric samples into the telemetry store with explicit raw-retention,
  downsampling, rollup, and deletion policies;
- define one stable state machine for `online`, `degraded`, `offline`, `maintenance`,
  and `unknown`, including stale-data and flapping rules;
- aggregate game player counts and timings without persisting player identity;
- use an outbox/durable stream for downstream projection and notification work;
- define retention, partitioning, and deletion behavior;
- reject unsupported interface versions predictably; and
- publish client conformance fixtures and a small reference sender/CLI.

Exit gate:

- the MKS-1 and MKA-1 positive/negative contract suites pass against the running API;
- duplicate delivery is safe and retry behavior is documented;
- acknowledgement occurs only after durable acceptance, and sequence gaps are visible;
- no secrets/PII are emitted by response, storage, metric, trace, or log tests;
- load test meets the agreed baseline without data loss.

Release marker: `v0.4.0` (ingestion alpha).

### Phase 4 — Production-grade cross-platform agent (2 weeks)

**Goal:** collect reliable local telemetry from game and SME servers without creating a
remote-administration backdoor.

Work:

- package the agent as a systemd service, Windows Service/MSI, standalone binary, and
  container where host visibility is not required;
- implement CPU, memory, filesystem, network, process, systemd, Windows Service, and
  Docker collectors with explicit permissions and availability reporting;
- implement enrollment, desired-config polling, capability negotiation, heartbeats,
  monotonic batches, acknowledgements, and credential rotation;
- add a bounded disk-backed spool with checksums, crash recovery, retention, priority,
  and backpressure behavior;
- implement proxy/TLS/custom-CA behavior without insecure global certificate bypasses;
- enforce CPU, RAM, disk, concurrency, payload, cardinality, and collection-time budgets;
- sign artifacts and implement opt-in staged auto-update with maintenance windows,
  health confirmation, rollback, and an offline/manual update path;
- provide a redacted support bundle and local self-diagnostics command; and
- test reboot, sleep/time jump, disk full, corrupted spool, DNS outage, proxy outage,
  revoked credential, server downgrade, and interrupted upgrade.

Exit gate:

- Linux amd64/arm64 and supported Windows Server test matrix passes;
- agent operates within the published resource budget under the reference workload;
- a 30-minute control-plane outage produces no silent loss and drains without a spike;
- no test can trigger arbitrary command execution or leak configured secrets;
- install, upgrade, rollback, uninstall, and credential revocation are reproducible.

Release marker: `v0.5.0` (agent alpha).

### Phase 5 — Game/SME adapters and safe external probing (1-2 weeks)

**Goal:** monitor representative game and SME workloads without turning an agent or
probe worker into an SSRF/amplification proxy.

Work:

- queue compatibility jobs and apply per-host concurrency and retry budgets;
- implement bounded Minecraft Java/Bedrock and Steam A2S query adapters with recorded
  fixtures, malformed-packet/fuzz tests, protocol deadlines, and response-size caps;
- implement host process/service/container and generic HTTP/HTTPS/TCP/DNS/TLS-expiry
  checks; keep database checks opt-in, read-only, and result-only;
- normalize results into stable low-cardinality fields and publish the exact supported
  game/server/version compatibility matrix;
- allow only HTTP/HTTPS according to deployment policy;
- resolve and validate every destination IP, including every redirect and reconnect;
- block loopback, link-local, private, multicast, metadata-service, and Unix-socket
  destinations in Cloud regional probes by default; protect against DNS rebinding;
- allow private destinations only through a tenant-bound local agent or explicit
  self-hosted policy, never by weakening the public Cloud probe boundary;
- cap redirects, response bytes, decompression ratio, and total/per-endpoint time;
- validate health, ready, metadata, metrics, content type, and semantic invariants;
- store check evidence without storing credentials or arbitrary response bodies; and
- expose actionable compatibility errors to users.

Exit gate:

- dedicated SSRF/DNS-rebinding test suite passes;
- malicious or oversized endpoints cannot stall workers or exhaust memory;
- compatible and intentionally non-compatible fixture services produce deterministic
  reports;
- malformed or reflection/amplification-prone UDP responses are bounded safely; and
- player names/IDs, chat, database data, and raw response bodies never enter telemetry.

Release marker: `v0.6.0` (workload alpha).

### Phase 6 — Usable Game/SME web product (1-2 weeks)

**Goal:** complete the primary operator journeys.

Work:

- implement sign-in/bootstrap and project/environment/service management;
- implement one-time service-key reveal, rotation, revocation, and copy-safe setup docs;
- show fleet summary, service status, last-seen time, checks, events, and deployments;
- show host resources, process/service/container state, game reachability/latency,
  aggregate players, protocol metadata, agent version/spool health, and maintenance;
- provide guided Game Server and SME Server onboarding with safe presets and an explicit
  permissions/secrets explanation;
- show compatibility reports and remediation guidance;
- add filtering, pagination, empty/error/loading states, and timezone handling;
- meet WCAG 2.1 AA basics: keyboard flow, focus, contrast, labels, and reduced motion;
- add browser tests for every critical journey; and
- ensure the UI does not expose sensitive values through URLs, analytics, or errors.

Exit gate:

- all production-target journeys pass in Playwright against the real API;
- desktop and mobile layouts are usable;
- accessibility audit has no critical findings;
- API errors remain understandable without exposing internal details.

Release marker: `v0.7.0` (public alpha).

### Phase 7 — Hosted SaaS and Stripe Sandbox billing (1-2 weeks)

**Goal:** make monthly Cloud subscriptions complete and testable without moving real
money.

Work:

- create Stripe Sandbox Products and recurring monthly Prices for Starter/Team, plus a
  documented trial policy;
- pin the Stripe API version and maintain a sandbox-to-live configuration map;
- create one Stripe Customer per tenant and prevent duplicate active subscriptions;
- create Checkout Sessions server-side using an allowlisted Price ID and tenant
  reference; use Stripe-hosted Checkout rather than collecting card data directly;
- create short-lived Customer Portal sessions server-side for payment method, invoice,
  plan-change, and cancel-at-period-end flows;
- implement `/v1/webhooks/stripe` using the untouched raw request body and verify the
  `Stripe-Signature` against the correct sandbox endpoint secret;
- persist each verified event before acknowledging it, return `2xx` quickly, and process
  it asynchronously and idempotently by Stripe event ID;
- tolerate duplicate and out-of-order events by retrieving/reconciling the latest
  Customer/Subscription state when needed;
- handle Checkout completion, subscription create/update/delete, trial ending,
  `invoice.paid`, payment failure/action-required, entitlements, refunds, disputes, and
  early fraud warnings that affect access or operator action;
- enforce plan entitlements and quotas at both API and worker boundaries, never only in
  the UI;
- add a periodic reconciliation job so missed webhooks cannot leave permanent drift;
- implement the documented grace, read-only, reactivation, cancellation, export, and
  retention behavior;
- test renewals, upgrades, downgrades, prorations, cancellation, failed payment, 3DS,
  disputes, duplicate delivery, reordered delivery, and secret rotation using Stripe
  test cards, CLI forwarding, and Test Clocks; and
- ensure `sk_test_*`, `whsec_*`, payloads, and customer billing data are never committed
  or logged.

Exit gate:

- the full subscription lifecycle passes in Stripe Sandbox with automated evidence;
- redirects alone can never grant plan access;
- replayed and out-of-order events do not double-provision or corrupt entitlement state;
- Community mode has no runtime or network dependency on Stripe;
- Cloud quota enforcement has backend tests and a customer-visible explanation;
- sandbox and live data/keys cannot be mixed accidentally.

Release marker: `v0.8.0` (billing beta).

### Phase 8 — Enterprise alerts, resilience, and operability (1-2 weeks)

**Goal:** make the system useful and recoverable during failures.

Work:

- implement state transitions with hysteresis to avoid alert flapping;
- ship actionable presets for server offline, process/game crash loop, high CPU/memory,
  low disk, stale agent, TLS expiry, dependency failure, and missing backup heartbeat;
- ship email, Discord, and signed generic-webhook notifications with retries, backoff,
  rate limits, and dead-letter handling;
- add admin health, queue depth, lag, ingest rate/error, job failure, and DB-pool metrics;
- add trace/request correlation using non-sensitive identifiers;
- define SLOs and alert rules for ingestion availability and processing delay;
- document backup, restore, key rotation, incident response, and data retention;
- automate PostgreSQL backups and regularly test restore to a clean instance;
- test graceful shutdown, rolling upgrade, DB outage, full disk, and slow dependency;
- test alert deduplication, grouping, acknowledgement, maintenance suppression,
  escalation, and recovery notification;
- create operator runbooks and a status/diagnostics page; and
- add Cloud abuse controls, tenant quotas, storage/cost dashboards, support tooling,
  public status communication, and billing-webhook/reconciliation alerts.

Exit gate:

- restore drill meets initial `RPO <= 15 minutes` and `RTO <= 60 minutes` targets;
- a 24-hour fault-injection/soak run has no silent loss or stuck work;
- alerts identify user impact and link to a tested runbook;
- deploy and rollback preserve accepted data; and
- the enterprise stability contract in section 2.4 is measured and all deviations are
  release blockers or explicitly revised through an ADR before public commitment.

Release marker: `v0.9.0` (enterprise beta).

### Phase 9 — Supply chain, deployment, and release candidate (1 week)

**Goal:** produce auditable, supportable release artifacts.

Work:

- build minimal non-root multi-architecture images with read-only filesystem support;
- add Compose deployment for small installations and versioned Helm manifests;
- add migration preflight, backward-compatible rolling upgrade, and rollback policy;
- generate SBOMs, vulnerability reports, checksums, signed images, and provenance;
- pin CI actions and enforce protected-branch/review/release rules;
- run dependency/license policy checks and document bundled third-party notices;
- commission a security review of auth, tenant isolation, ingestion, and SSRF controls;
- include agent enrollment/update, game-protocol parsers, local secret handling, and
  probe-network boundaries in the security review;
- publish installation, hardening, upgrade, backup, and uninstall documentation;
- create a deterministic release workflow with SemVer and release notes; and
- provision a reproducible Cloud environment with managed secrets, TLS, DNS, encrypted
  storage, backups, restricted administration, audit logging, and separate staging and
  production Stripe credentials/webhook endpoints.

Exit gate:

- no unresolved critical/high vulnerability or threat-model finding;
- fresh install, upgrade from previous release, rollback, backup, and restore pass in CI
  or a reproducible staging run;
- release artifacts are signed and traceable to a protected source revision;
- Apache-2.0 source and binary distribution checks pass; and
- Cloud terms, privacy policy, billing support process, tax/invoice decision, and data
  deletion process have an accountable owner and launch approval.

Release marker: `v1.0.0-rc.1`.

### Phase 10 — Staging, beta, and general availability (2-3 weeks)

**Goal:** validate real operations, then release a supportable `1.0`.

Work:

- run production-like staging with TLS, real DNS, backups, monitoring, and alerts;
- onboard 3-5 representative game/SME operators and gather usability/compatibility
  feedback;
- include at least two real game-server protocols, Linux and Windows SME hosts, Docker,
  an instrumented MKS-1 application, a proxy-restricted network, and a slow/unreliable
  network in the beta cohort;
- execute capacity, abuse, failover, upgrade, rollback, and disaster-recovery drills;
- run at least a 7-day beta soak at expected peak load plus safety margin;
- run a complete Sandbox billing-cycle simulation, then perform a tightly controlled
  live-mode purchase, renewal/cancel verification, refund, and reconciliation test;
- confirm production webhook signing secret, Price IDs, return URLs, branding, support
  contact, receipts/invoices, tax behavior, and customer-portal configuration;
- freeze the MKS-1 and MKA-1 contracts only after conformance evidence exists;
- publish support/security response expectations and the compatibility matrix; and
- tag, sign, publish, and verify `v1.0.0` from a clean environment.

Exit gate:

- agreed SLO is met throughout the beta window;
- the published agent/probe compatibility and capacity envelopes match measured beta
  evidence;
- zero open release-blocking defects and zero unexplained data loss;
- an on-call operator can diagnose and recover the documented failure scenarios;
- installation and upgrade are reproduced by someone other than the implementer;
- `v1.0.0` source, images, SBOM, signatures, docs, and changelog are public; and
- a paid Cloud tenant can subscribe, operate, update payment details, cancel, export,
  and reactivate without manual database changes.

Release marker: `v1.0.0` (general availability).

## 4. Production definition of done

Meerkateer is production-ready only when all of the following are true:

- **Correctness:** contracts, cross-field invariants, migrations, retries, and tenant
  isolation have automated tests.
- **Security:** threat model is reviewed; secrets are hashed/redacted; SSRF controls are
  tested; no unresolved critical/high findings remain.
- **Reliability:** idempotent ingestion, durable jobs/outbox, graceful shutdown, backup,
  restore, upgrade, and rollback are demonstrated.
- **Agent:** Linux/Windows lifecycle, offline spool, resource budget, credential
  rotation/revocation, update/rollback, and two-minor-version compatibility are proven.
- **Workloads:** each advertised game/SME adapter has fixtures, malformed-input tests,
  deadlines, size/cardinality limits, and a published support matrix.
- **Performance:** explicit test hardware, dataset, request mix, and targets are recorded;
  initial suggestion is ingest p95 below 250 ms at the agreed beta load.
- **Operations:** dashboards, actionable alerts, runbooks, retention, and capacity limits
  exist, and Meerkateer monitors its own MKS-1 endpoints.
- **Delivery:** reproducible signed artifacts, SBOM, provenance, versioned migrations,
  and supported deployment instructions are published.
- **Billing:** Stripe events are verified, durable, idempotent, order-independent, and
  reconciled; Sandbox lifecycle tests and a controlled live smoke test pass.
- **Cloud operations:** tenant quotas, abuse prevention, support, cost monitoring,
  incident communication, privacy, retention, export, and deletion are operational.
- **High availability:** stateless API/probe/worker replicas, leader-safe scheduled jobs,
  PostgreSQL recovery/HA guidance, zero-downtime migration rules, and failure drills are
  documented and demonstrated for the published Cloud topology.
- **Open source:** Apache-2.0 licensing, DCO, governance, security reporting, release
  process, contribution workflow, and third-party notices are complete.
- **User value:** a new operator can install the system and complete every target journey
  from the public documentation without private knowledge.

## 5. Scope deliberately deferred beyond v1.0

Keep the first release narrow. Defer native mobile applications, annual contracts,
metered/usage billing, coupons and reseller billing, arbitrary plugin execution,
multi-region active-active operation, dozens of notification providers, full
incident-management workflows, long-term metrics storage, raw log aggregation, RCON,
remote shell, machine power control, and game-server provisioning until production
usage and a separate security design demonstrate a need.

## 6. Original Phase 0 implementation sequence

The original foundation sequence was:

1. initialize Git and correct repository ownership;
2. add Apache-2.0 and governance files;
3. mark MKS-1 as Draft and specify the MKA-1 agent protocol as Draft;
4. resolve and test the schema/protocol contradictions;
5. add MKS-1/MKA-1 fixtures and a contract-test runner;
6. create CI for schema/docs/license/secret checks;
7. record architecture, auth, tenancy, agent trust/update, telemetry, jobs,
   deployment-mode, Cloud, and Stripe billing ADRs;
8. define the initial plan-entitlement matrix without fixing final prices prematurely;
9. scaffold the Rust workspace and web application only after the contract gate passes.

The [delivery status](phase-status.md) records the completed foundation work and remaining
gates. Use the [commercial readiness plan](commercial-readiness-plan.md) for the current
implementation order and test cases.

## 7. Official Stripe implementation references

- [Checkout quickstarts](https://docs.stripe.com/payments/checkout/quickstarts)
- [Customer management and portal](https://docs.stripe.com/customer-management)
- [Subscription webhook lifecycle](https://docs.stripe.com/billing/subscriptions/webhooks)
- [Webhook security and delivery behavior](https://docs.stripe.com/webhooks)
- [Sandbox and test payment data](https://docs.stripe.com/testing)
- [Test Clocks](https://docs.stripe.com/billing/testing/test-clocks)
