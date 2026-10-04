# Changelog

All notable changes to this project will be documented in this file. The format follows
Keep a Changelog principles, and releases use Semantic Versioning.

## [Unreleased]

Current developer-preview code version: `0.3.0-alpha.2` (Monitoring Alpha prerelease). This is
not a stable release or a claim that the final `v0.3.0` release gates have passed.

### Added

- Early Security/Operations/Public Preview foundations pulled forward without closing their
  release gates: PostgreSQL now enforces replica-wide fixed-window abuse budgets through a bounded
  security-definer function and stores only installation-keyed peer digests; application logins
  cannot read the limiter table directly. New `age`-encrypted backup and guarded restore commands
  validate PostgreSQL archives, checksum encrypted output, record source/schema metadata, reapply
  migrations, and require post-restore API readiness. A tag-gated multi-architecture OCI workflow
  adds BuildKit SBOM/provenance and GitHub/Sigstore build attestations, and Controller DEB/MSI/ZIP
  artifacts receive the same repository/workflow provenance evidence. These are foundations—not a
  completed independent security review, off-host restore drill, platform signature, or clean
  operator gate.
- Service-level manual and scheduled HTTP, HTTPS, TCP, DNS, and TLS-certificate diagnostics in the Console and
  public API. Checks are CSRF- and tenant-protected, Community-only, rate-limited, bounded to a
  250–10,000 ms timeout, and fail closed when any DNS answer is private, local, reserved, or mixed
  trust. Connections pin the validated numeric answers, HTTP redirects are disabled, TLS chains
  are verified, and only bounded operational metadata is returned. Scheduled monitors use
  database leases, configurable failure/recovery consensus, bounded history, and the existing
  incident/timeline/alert projection. A service cannot combine a scheduled monitor with an active
  SDK credential, avoiding contradictory health sources.
- Complete email-free local credential recovery for Community members. The Account screen changes
  a signed-in member's password and revokes every other session. Owner/admin users can issue a
  digest-only, expiring, one-time reset link for a non-owner member; replacing a link cancels the
  older one, completion revokes old sessions, and concurrent/replayed/expired attempts fail closed.
  Pending invitations now have configurable expiry and audited cancellation from the Admin UI.
- Email-free internal member invitations for Community. Owner/admin users create a one-time,
  24-hour local invite link from the Admin Console; recipients see the company, username, and role
  before choosing Accept or Decline and create their own local password only when accepting. Invite
  tokens stay out of request paths, are stored only as SHA-256 digests, expire, cannot be replayed,
  and are consumed transactionally so two simultaneous accepts produce one member. The same UI
  lists members, changes admin/operator/viewer roles, removes non-owner members, and records the
  invitation lifecycle in the immutable audit stream. PostgreSQL E2E covers invalid inputs,
  expiry, decline, replay, concurrent acceptance, username login, permissions, and secret absence
  from list/audit responses.
- Security Beta foundations: the Docker integration suite now provisions a disposable second
  company and proves tenant isolation across collection, direct-ID, nested-ID, write, and
  machine-credential paths before rechecking the untouched rival data. Browser sign-out now
  revokes the server-side PostgreSQL session, requires CSRF proof, expires both cookies, records
  an immutable `session.logout` audit event, and is available from the responsive Console header.
  Owner password setup/recovery also revokes every older owner session before issuing the fresh
  session, so a captured cookie cannot survive credential recovery.
- Community password authentication now resolves any active tenant member instead of hard-coding
  the owner. Disposable PostgreSQL E2E sessions prove viewer read-only access, operator service
  writes without credential issuance, and admin workspace/administration access through the
  central deny-by-default role matrix.
- Tenant-scoped member administration API for listing members, changing admin/operator/viewer
  roles, and removing non-owner members. Mutations require CSRF, protect the acting user and owner,
  revoke the target's active sessions immediately, and append an audit event.
- Bounded OS-service monitoring for exact systemd `.service` units and Windows Service names.
  Queries run concurrently with a three-second ceiling, never invoke a shell, distinguish active,
  transitional, failed, missing, permission, manager, and timeout states, and expose the result in
  the local TUI, machine telemetry API, and responsive Console without treating unknown as down.
- Monitoring Alpha resilience foundation: the Controller now keeps an ordered, disk-backed,
  four-MiB/128-batch offline spool, migrates legacy one-batch state, drains accepted batches in
  order after an outage, rotates expiring machine credentials automatically (or explicitly with
  `rotate-credential`), and can atomically restore the previous signal allowlist with
  `rollback-config`. Linux disk collection now includes bounded inode capacity, and Console
  Machine detail visualizes inode pressure without inventing values on unsupported platforms.
- Enforced automatic Controller startup after enrollment: Windows now verifies an enabled boot
  task, starts it immediately, and uses extended restart recovery; Linux verifies its systemd unit
  is enabled and active and uses `Restart=always`. CI locks both startup contracts, while the
  portable Windows ZIP remains explicitly manual and non-mutating.
- A shared Controller Developer Preview disclaimer shipped inside MSI, portable ZIP, and DEB
  packages. Windows MSI displays the full notice, Windows Setup and the Rust TUI require explicit
  consent before first enrollment, and Debian installation prints the notice location without
  breaking non-interactive package management.
- Copy-ready Ubuntu/Debian terminal installation commands on the landing page that download the
  Controller package and adjacent checksum, verify it with `sha256sum`, and install the verified
  `.deb` with `apt`.
- Explicit Local / self-hosted Controller destination with disabled Cloud placeholder, plus shared
  pre-enrollment connection diagnostics for config storage/free space, URL/DNS, proxy/VPN-sensitive
  routing, TLS, and API readiness across the Rust CLI/TUI and Windows Setup. Runtime failures now
  persist a redacted code, summary, and next action while retaining the durable telemetry batch.
- Cross-platform Rust Controller TUI for masked first enrollment, live local host/process signals,
  an atomic signal allowlist, safe readiness diagnostics, and session-only events while keeping the
  Windows Scheduled Task and Linux systemd daemon headless.
- Installable Meerkateer Controller preview: a WiX-based Windows MSI with Start-menu signal setup
  UI, an Ubuntu 22.04+ DEB with guided headless setup and hardened systemd service, and a portable
  Windows CLI ZIP. A pinned GitHub Actions workflow publishes all three with SHA-256 files to a
  rolling preview release; the landing page exposes direct platform downloads. Controller config
  now persists an explicit CPU/memory/disk/process allowlist while keeping the heartbeat mandatory
  and preserving older agent configurations safely.
- Simple geometric Kalahari scenery behind the public landing hero, using flat acacia trees,
  polygon mountains, and layered sand dunes kept deliberately faded for readable content on
  desktop and mobile.
- Dedicated responsive `/roadmap` page with a playful introduction, a single-column milestone
  timeline, expandable feature/test/edge-case evidence, and clear pass gates; the full roadmap no
  longer makes the product landing page unnecessarily long.
- Friendly GitHub README masthead using the Meerkateer mascot and wordmark, current product and
  Community sign-in screenshots, plain-language product benefits, and a human-first quick start
  with the longer AI installation prompt kept available in a collapsible section.
- Six transparent mini Meerkateer friends give Machines, Services, Timeline, Workspaces, States,
  and Access their own color and role on the landing page, with responsive placement and
  reduced-motion-safe hover behavior.
- Cross-platform Rust host-agent collector and `inspect`/`status` CLI for Linux and Windows,
  reporting bounded CPU, memory, deduplicated fixed-filesystem capacity, and up to 16 exact-name
  process instance counts without collecting command lines, environment variables, usernames,
  paths, PIDs, or file contents. Continuous and one-shot runs send the same metrics through durable
  exact-batch retry, with native Windows/Linux CI coverage and an operator guide.
- Tenant-scoped latest-machine telemetry API and responsive Machine detail visualization for CPU,
  memory, disk, OS/architecture, collection completeness/freshness, and watched process state. The
  read model uses only one durable batch, never fills gaps with older evidence, and calls out stale,
  missing, and stopped-process states explicitly.
- Preview background-agent installers for hardened, unprivileged Linux systemd and ACL-restricted
  Windows `LOCAL SERVICE` startup tasks, including safe uninstall/purge behavior and CI syntax/
  disposable-install checks. ADR-0008 defines a future signed, typed, locally allowlisted restart
  boundary while keeping the current agent outbound-only and command-free.
- Cute authenticated Console shell with honest deep-linked Overview, Machines, Services,
  incident-evidence, and Connect views; responsive desktop/mobile navigation; workspace context;
  role-aware controls; API retry; and a real-data first-signal guide from company to workspace to
  machine/application to fresh evidence. Machine and service lists now include responsive,
  accessible search and state/environment filters with explicit no-result states. Tenant-safe
  machine, service, and evidence detail URLs survive reload/sign-in and never substitute an
  unknown requested object with a different workspace object. Dedicated Incidents, Alerts,
  Maintenance, and Admin views expose real operational records instead of placeholder controls.
- First-class tenant incidents correlate one outage into a durable open/resolved record, retain the
  reported cause, reject stale transitions, and avoid duplicate incidents during repeated reports.
  Operators can acknowledge without inventing recovery, assign/unassign themselves, add immutable
  Unicode notes, and inspect the audited activity stream from the responsive Console.
- Durable alert-delivery outcomes stay synchronized with worker retry and dead-letter processing;
  the Console shows queued, delivered, retrying, dead-lettered, suppressed, disabled, and
  unconfigured outcomes together with audited down/recovery policy controls. A bounded repeat-down
  cooldown suppresses flapping noise without hiding incidents, and operators can replay a
  dead-letter as a new audited delivery while preserving the original terminal record.
- Audited per-service maintenance windows suppress notifications without hiding health or incident
  evidence, and owner/admin users can inspect tenant counts plus append-only audit activity.
- Durable installation-level worker progress records the latest bounded cycle counts through a
  least-privilege database function. The Admin view now distinguishes a healthy worker, a worker
  stalled for more than 30 seconds, and a worker that has never checked in, alongside the oldest
  pending alert. Fault coverage proves healthy → stalled → recovered without changing service
  health evidence.
- Shared administration contracts remain Cloud-ready while hosted provisioning, regional
  operations, quotas, support, and billing stay isolated in the private Cloud repository.
- Canonical cute, simple, and functionally complete UI product plan covering information
  architecture, every operator/admin surface, progressive disclosure, delivery phases, edge cases,
  responsive/accessibility requirements, and measurable screen acceptance gates.
- Distinct Meerkateer Cloud identity and responsive sky-themed `/cloud` page with an explicit
  Cloud logo lockup, friendly cloud illustration, hosted-beta status, and accurate free-beta gates.
- Canonical Community/Cloud repository boundary: one public Apache-2.0 product core and a separate
  private hosted-operations repository that consumes signed core releases without forking them.
- Expanded 0.1–1.0 roadmap and landing-page phase board with features, verification strategy,
  adversarial edge cases, measurable pass gates, and Community/Cloud critical-path ownership.
- Public GitHub repository and direct navigation link with GitHub icon.
- Node.js, Go, and PHP SDKs for MKS-1 heartbeat, event, and deploy delivery, with
  language guides, retry tests, and CI coverage. The website now offers Node.js,
  Go, Rust, Python, PHP, and Host agent examples under SDKs.
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
- Public product landing page separated from Community setup, login, and authenticated
  operations routes, with honest Community/Cloud positioning, product preview, docs, and help.
- Deterministic concurrent fleet E2E using ten real agents to prove online, stale failure
  detection, selective refresh, random recovery, durable retry, PostgreSQL evidence, and
  credential-safe telemetry.
- Disposable multi-language localhost lab with real Python, Node.js, Go, PHP, and Rust
  HTTP services, a shared failure/recovery contract, automatic workspace provisioning,
  timeline assertions, stopped-process detection, human guidance, and an AI runbook.
- Cloudflare Workers Static Assets deployment with a same-origin, fail-closed API gateway,
  optional Cloudflare Access service-token injection, Wrangler dry runs, and encrypted API
  origin configuration.
- Reproducible GHCR server/worker image publishing, generated production credentials, and a
  hardened production Compose topology for a loopback API and dedicated PostgreSQL 18 volume.
- Operator runbook for Cloudflare Tunnel, immutable releases, GHCR authentication, backups,
  health verification, upgrades, and rollback on the self-managed production host.
- Visible `v0.1.0` Developer Preview and current Community Alpha phase on the landing page and
  README, plus a canonical milestone-and-evidence roadmap from the current code to Community
  `1.0.0` Stable.

### Changed

- Combined the MSI and portable CLI downloads into one Windows Controller card on the landing
  page, leaving a simple two-card Windows-versus-Ubuntu comparison.
- Default local host ports are now `6511` for the web UI and `6510` for the API;
  bootstrap safely migrates exact legacy defaults while preserving custom choices.
- Vite now uses its TypeScript configuration explicitly and writes development cache to
  writable tmpfs, keeping the hardened read-only web container free of cache errors.
