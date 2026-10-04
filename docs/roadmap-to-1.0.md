# Meerkateer roadmap to 1.0

| Item | Current value |
| --- | --- |
| Code version | `0.2.0` |
| Release channel | Developer Preview |
| Current delivery phase | `0.2` Community Alpha |
| Stable target | `1.0.0` Community |
| Last reviewed | 2026-10-03 |

`0.2.0` is the version declared by the Rust workspace and web package. It is the current
code version, not a claim that a signed GitHub release has been published. There is no
`v0.2.0` Git tag yet.

This document is the canonical release sequence. [Delivery status](phase-status.md) records
what has actually passed, the [Community plan](community-first-roadmap.md) expands the current
alpha acceptance cases, and the [technical roadmap](production-roadmap.md) retains detailed
architecture and capacity work.

The [Community and Cloud repository boundary](repository-and-cloud-boundary.md) defines which
repository owns each capability, how Cloud consumes immutable public artifacts, and how the core
and Cloud management databases remain independently owned.

The [UI product plan](ui-product-plan.md) defines how all of those capabilities remain cute and
simple for everyday operators while preserving complete evidence, security, administration,
responsive, and accessibility behavior.

## What 1.0 means

`1.0.0` means the open-source Meerkateer Community product is stable enough for supported
production use within a published capacity and platform envelope. The version is earned by
repeatable evidence, not by completing a feature checklist.

The Community 1.0 scope is:

- one company containing multiple workspaces, machines, people, and monitored services;
- outbound host agents plus Node.js, Go, Rust, Python, and PHP application SDKs;
- bounded host, process, service, container, HTTP, TCP, DNS, TLS, and selected game-server
  checks;
- durable status, staleness, incident, deployment, recovery, alert, and audit evidence;
- safe installation, backup, restore, upgrade, rollback, and removal;
- supported container deployment and signed agent/release artifacts; and
- clear security, privacy, compatibility, retention, and support policies.

Hosted Meerkateer Cloud follows the same core but is a parallel track. A free hosted beta may
open before Community 1.0; Stripe billing is deliberately not a Community 1.0 release gate.
Commerce can be enabled only after the hosted service has proved tenant isolation, operations,
cost, support, and billing lifecycle behavior.

## Product and repository tracks

The release train has one critical path and one parallel service track:

```text
Public Apache-2.0 core
0.1 → 0.2 → 0.3 → 0.4 → 0.5 → 0.6 → 0.7 ─────────→ 0.9 → 1.0
                              └──────────────→ 0.8 Hosted Beta ─→ Cloud GA later

Private Cloud operations
                           provisioner + regions + quotas + SLOs + support
```

- `theparitt/meerkateer` owns all reliability, security, tenant, agent, SDK, Console, storage,
  protocol, migration, and Community deployment behavior.
- `theparitt/meerkateer-cloud` owns only hosted provisioning and operations. It pins signed
  public artifacts by version and digest and never forks or copies core source.
- Community is one company per installation with many workspaces and machines. Cloud is many
  tenants per control plane with the same tenant-aware core and additional isolation evidence.
- Cloud billing is a later service capability. It is not a dependency of Community and is not a
  gate for Community 1.0.

The following stay outside 1.0: arbitrary remote commands, machine provisioning, file
management, RCON/game administration, raw centralized logs, chat/player identity collection,
and automatic remediation. Each would require a separate threat model and approval workflow.

## Release rules

1. Meerkateer uses Semantic Versioning. Breaking changes are possible before 1.0 and must be
   documented with a migration path.
2. A milestone closes only when its exit evidence is recorded in `docs/phase-status.md`.
3. Dates are planning ranges, not release promises. Security, restore, and soak gates are not
   shortened to meet a date.
4. Every supported upgrade starts with a backup and is tested against real prior-version data.
5. Community works without Cloud, Stripe, a remote license server, or an AI key.
6. Hosted features may not weaken tenant, privacy, or credential boundaries in the shared core.

## Milestones

| Version | Phase | Main outcome | Status | Planning range* |
| --- | --- | --- | --- | --- |
| `0.1.0` | Foundation preview | Executable multi-workspace core, durable ingest, SDKs, agent, Console, failure labs, and deployment packaging | Delivered | Implemented |
| `0.2.0` | Community Alpha | One clean install reliably detects, explains, alerts, and recovers from a real fixture failure | Current developer preview | 3–5 weeks |
| `0.3.0` | Monitoring Alpha | Production-shaped agent collectors, scheduled probes, and first supported game/SME adapters | Foundation in progress | 4–6 weeks |
| `0.4.0` | Security Beta | Multi-user access, complete tenant matrix, distributed abuse controls, signed config, and security review | Foundation in progress | 3–5 weeks |
| `0.5.0` | Operations Beta | Tested restore/upgrade, retention, alert operations, observability, and fault tolerance | Planned | 4–6 weeks |
| `0.6.0` | Public Preview | Installable, signed, documented release artifacts and an accessible first-user journey | Planned | 3–4 weeks |
| `0.7.0` | Scale Beta | Published capacity, resource, compatibility, and sustained failure/recovery evidence | Planned | 3–4 weeks |
| `0.8.0` | Hosted Beta | Optional free managed beta with isolated tenants, regional probes, backups, and service SLOs | Parallel; non-blocking | 4–8 weeks |
| `0.9.0` | Release Candidate | Feature freeze, independent review, upgrade/rollback rehearsal, and operator soak | Planned | 3–4 weeks plus soak |
| `1.0.0` | Stable | Supported Community release with no unresolved release blockers | Target | Gate-driven |

\* A single experienced engineer should plan roughly 6–9 calendar months for the Community
critical path. A focused 3–4 person team may reduce elapsed time, but external review and soak
still take real calendar time.

The Community critical path is `0.2 → 0.3 → 0.4 → 0.5 → 0.6 → 0.7 → 0.9 → 1.0`.
Hosted `0.8` begins only after the operations foundation in `0.5` and does not block Community
1.0.

## 0.1 — Foundation preview

Delivered in the current working tree:

- Apache-2.0 project, versioned MKS/MKA contracts, schemas, fixtures, and policy checks;
- company → workspace → machine/service model with PostgreSQL row-level tenant boundaries;
- owner bootstrap/login/recovery, scoped service keys, agent enrollment, rotation, and audit;
- durable heartbeat/event/deployment and agent telemetry ingest with idempotency and outbox;
- visual machine/service state, stale detection, incident timeline, and recovery evidence;
- Node.js, Go, Rust, Python, and PHP SDKs plus a Rust host agent;
- deterministic fleet, SDK-language, control-plane, failure, migration, and alert labs; and
- Cloudflare web packaging, versioned GHCR image commands, and production Compose.

This phase proves a serious foundation. It does not yet prove a supported production release.

Verification and evidence already expected from this phase:

- contract/schema tests reject invalid MKS/MKA messages and preserve idempotency on replay;
- API and SQL integration tests exercise bootstrap, login, tenant context, credential lifecycle,
  heartbeat, event, deployment, timeline, alert retry, dead letter, and migrations;
- a deterministic multi-language lab sends equivalent facts through all five SDKs; and
- browser tests render healthy, degraded, down, unknown, timeline, empty, loading, and API-error
  states without requiring a Cloud account.

Foundation edge cases include duplicate message IDs, events arriving out of order, a revoked key
reused after rotation, a stale heartbeat after a newer recovery, worker restart between claim and
acknowledgement, unavailable PostgreSQL, and malformed SDK payloads. A passing build must reject
or safely retry each case without inventing health or silently losing an accepted fact.

## 0.2 — Community Alpha

Goal: make the narrow failure-to-recovery journey trustworthy on a clean machine.

Required work:

- complete incident open/update/recovery and repeated-failure deduplication rules;
- distinguish failed collection, missing evidence, degraded state, and confirmed outage;
- add bounded scheduled HTTP/TLS checks and baseline host/process collection;
- show alert attempts, retry, dead letter, maintenance, and acknowledgement to operators;
- finish the first-run browser path from company creation to first heartbeat; and
- run the published production packaging on a separate clean host.

Exit gate:

- a fixture is installed, enrolled, stopped, detected, explained, alerted, restarted, and
  recovered without manual database changes;
- stale or missing data never creates a false healthy or false recovery state; and
- the exercise is repeatable from a clean checkout with no Cloud, billing, or AI credential.

Test plan:

- **First use:** install published-shaped Compose on a clean Linux VM, create the company and
  owner in the browser, create two workspaces, enroll two machines, and register multiple services.
- **Detection:** kill a monitored process, stop a fixture HTTP endpoint, delay a heartbeat, and
  expire a TLS fixture; assert the state, reason, evidence time, and next expected check.
- **Incident lifecycle:** send repeated identical failures, contradictory facts, and recovery;
  assert one incident, deduplicated updates, a fresh recovery fact, and an immutable timeline.
- **Alert lifecycle:** make the receiver return `429`, `500`, timeout, and recovery; assert bounded
  retry, cooldown, visible delivery state, dead letter, replay, and no duplicate notification storm.
- **Restart durability:** restart server, worker, agent, and database at each hand-off; assert every
  accepted fact ends processed, retriable, rejected with a reason, or dead-lettered.

Edge cases: system clock skew, a service removed while an alert is queued, workspace reassignment,
an agent enrolling twice, zero services, Unicode names, maximum allowed names, IPv6 targets, proxy
failure, full local spool, and browser refresh during setup. The phase passes only when these cases
have deterministic outcomes documented in the UI/API and the complete journey runs three times
from clean state without manual SQL repair. Store the commands, logs, screenshots, versions, and
test report in the phase evidence record.

## 0.3 — Monitoring Alpha

Goal: cover the first real game-server and SME-server operations safely.

Required work:

- Linux package/service installation and upgrade; Windows service implementation and lab;
- bounded CPU, memory, disk/inode, process, systemd/Windows Service, and Docker collectors;
- disk-backed offline spool, credential rotation automation, and configuration rollback;
- scheduled HTTP, HTTPS, TCP, DNS, and TLS-expiry probes with SSRF/DNS-rebinding controls;
- a versioned Minecraft Java compatibility matrix and one generic SME service journey; and
- fixture fuzzing, malformed-response, timeout, resource-budget, and privacy tests.

Exit gate:

- the agent survives a 30-minute control-plane outage and drains without duplicate facts;
- steady reference load stays below 1% of one CPU core, 128 MiB RAM, and the configured disk
  ceiling; and
- supported adapters pass recorded compatibility and hostile-input fixtures.

Test plan:

- run collectors on supported Linux distributions and Windows versions across boot, service
  restart, user-session absence, permission denial, and package upgrade;
- disconnect the control plane for 30 minutes, fill and rotate the bounded spool, reconnect, and
  verify ordered drain, idempotency, backpressure, and disk ceiling;
- run HTTP/HTTPS/TCP/DNS/TLS probes against success, refusal, NXDOMAIN, redirect loop, slow body,
  oversized body, invalid certificate, IPv4/IPv6, and split-horizon fixtures;
- replay supported Minecraft protocol/version fixtures plus truncated, oversized, malformed,
  compressed, slow, and unexpected responses; and
- benchmark idle and active agent CPU, RSS, file descriptors, disk writes, and network traffic on
  the published reference machine.

Edge cases: PID reuse, process names with spaces, a systemd unit in `activating`, Docker socket
permission denial, disk at 100%, DNS answer changing between validation and connection, redirect to
a private address, certificate renewal during a check, machine sleep/resume, and downgrade to an
older configuration. The phase passes only when unsupported or unsafe targets fail with an
explainable reason, no collector can consume unbounded time/memory/disk, and compatibility results
are published rather than inferred.

## 0.4 — Security Beta

Goal: close the identity and tenant boundary before broader exposure.

Current evidence: the disposable integration stack now creates two independent companies and
tests tenant-scoped lists, direct and nested identifiers, mutations, and an agent credential from
the other company. It then switches back and proves the protected data is unchanged. Current
browser logout requires CSRF, revokes the durable session, expires both cookies, rejects replay of
the old cookie, and records an audit event. Email-free internal invitations and Admin member
management now cover digest-only one-time links, accept/decline/admin cancellation, configurable
expiry, replay, concurrent accept, username login, role changes/removal, self password changes,
non-owner internal reset links, session revocation, and audit. This is useful foundation
evidence, not completion of 0.4: replica-wide abuse controls, signed desired configuration,
platform secret stores, optional production OIDC, and independent review remain open.
Community login now authenticates active owner, admin, operator, and viewer memberships, and the
integration matrix proves representative allow/deny boundaries for every role. Member creation now
uses the same public invitation lifecycle and responsive Admin UI exercised by PostgreSQL E2E.

Required work:

- finish optional production OIDC/SSO and independently review local recovery;
- cross-tenant read/write/guessed-ID tests for every API, worker, export, and background job;
- gateway-wide rate limits and abuse protection across replicas;
- signed desired agent configuration and secure platform credential storage;
- CSRF, session, audit, dependency, container, and secret rotation review; and
- remediation of all critical/high findings from an independent security assessment.

Exit gate: zero known cross-tenant access and no unresolved critical/high security finding.

Test plan:

- generate two companies, multiple roles, colliding human-readable names, and objects of every
  tenant-owned type; attempt list, direct-ID, nested-ID, search, export, websocket/stream, and write
  operations from the other tenant;
- exercise RLS through API, worker, migrations, support/export paths, connection-pool reuse, failed
  transactions, and jobs whose original member has been removed;
- test invite replay, expired invite, concurrent acceptance, role downgrade, last-owner removal,
  session revocation, password reset, CSRF, cookie flags, and optional OIDC account linking;
- rotate database, signing, service, agent, webhook, and OIDC credentials while traffic continues;
  assert old credentials stop within the documented grace window; and
- run SAST, dependency/license scan, container scan, secret scan, fuzz targets, and an independent
  assessment against the release candidate.

Edge cases: guessed UUIDs, mixed-tenant batch input, stale cache keys, reused pooled connections,
tenant deletion racing a worker, hostile filenames in exports, spreadsheet formula injection,
Unicode/confusable identity values, rate-limit bypass across replicas, and audit-log injection. Any
cross-tenant observation or mutation is a release blocker. Critical/high findings require a fix and
regression test; accepted medium findings require an owner and expiry.

## 0.5 — Operations Beta

Goal: make data and alerts recoverable under operational failure.

Required work:

- versioned migration runner with supported upgrade and rollback procedures;
- encrypted off-host backup, scheduled restore test, and published measured RPO/RTO;
- retention and per-workspace quota enforcement;
- harden the delivered alert-history and maintenance foundations with replay, cooldown,
  escalation, acknowledgement, and an external watchdog;
- Prometheus metrics, trace correlation, queue/worker progress, and actionable runbooks; and
- database, worker, network, disk-pressure, receiver-outage, and control-plane fault drills.

Exit gate: a production-shaped backup restores on another machine, an upgrade preserves data,
and fault drills expose every accepted fact as processed, retriable, rejected, or dead-lettered.

Test plan:

- restore an encrypted off-host backup into a clean isolated host on a schedule, compare row counts,
  critical hashes, tenant boundaries, credentials, incidents, and attachments, then run application
  smoke tests against the restored system;
- upgrade from every supported prior version using real fixtures, interrupt migration at safe and
  unsafe points, retry it, and exercise the documented application rollback window;
- inject PostgreSQL restart/failover, worker crash, queue backlog, network partition, DNS failure,
  disk pressure, clock drift, receiver outage, and exhausted connection pool;
- verify retention and deletion around exact cutoff times while incidents, exports, and legal
  retention rules reference old data; and
- alert on the monitoring system itself through an independent watchdog and prove the runbook can
  distinguish source outage, ingestion outage, evaluation lag, and notification outage.

Edge cases: partial backup upload, wrong encryption key, corrupt archive, insufficient restore disk,
schema newer than binary, rollback after an irreversible migration, duplicate worker execution,
poison jobs, webhook recovery during retry, and daylight-saving/time-zone boundaries. Publish the
measured RPO/RTO and test topology. A backup that has not been restored does not count as evidence.

## 0.6 — Public Preview

Goal: let an external operator install and understand Meerkateer without repository knowledge.

Required work:

- immutable OCI images, checksums, SBOM, signatures, and build provenance;
- supported Linux amd64/arm64 agent packages and the declared Windows package;
- install, upgrade, rollback, uninstall, proxy, firewall, backup, and recovery guides;
- guided Console onboarding, complete empty/error/recovery states, and useful diagnostics;
- WCAG 2.2 AA keyboard, screen-reader, contrast, and responsive review; and
- release notes containing compatibility, security, migration, and known-limit sections.

Exit gate: a new operator completes install through first recovered incident on a clean machine
using only published artifacts and documentation.

Test plan:

- ask an operator with no repository knowledge to install from signed artifacts, create a company,
  add a workspace and machine, trigger and understand an incident, upgrade, restore, and uninstall
  using only published documentation;
- verify image/package signatures, checksums, SBOM completeness, provenance, reproducibility inputs,
  least-privilege containers, read-only filesystems where possible, and unsupported architecture
  errors;
- run the Console by keyboard and screen reader at 200% zoom and narrow/mobile widths; cover every
  loading, empty, partial, permission, offline, error, retry, and recovery state; and
- install behind TLS reverse proxies, custom CA, outbound proxy, IPv6, restrictive firewall, and
  non-default ports without exposing PostgreSQL or privileged agent interfaces.

Edge cases: stale documentation cache, mismatched server/agent version, failed package post-install,
existing ports, read-only host directory, low disk, proxy-auth failure, missing CA bundle, and
uninstall with retained data. The phase passes when at least three fresh operators complete the
journey without maintainer intervention and every failure leads to a safe, actionable diagnostic.

## 0.7 — Scale Beta

Goal: publish an honest operating envelope.

Required work:

- sustained load, burst, queue backlog, connection churn, and high-cardinality tests;
- measured detection/alert latency and storage cost by retention profile;
- current and previous two supported agent-minor compatibility tests;
- rolling server/worker upgrade while agents continue buffering; and
- 14-day representative soak with forced database, worker, network, and webhook failures.

Exit gate: publish hardware, topology, agent/check count, throughput, latency, retention, and
failure limits. Do not advertise capacity that has not passed this test.

Test plan:

- define small, medium, and large reference topologies, then run sustained load, 10× burst, reconnect
  storm, high-cardinality labels, slow tenants, large tenants, and queue catch-up;
- measure ingest acceptance latency, evaluation lag, incident detection, alert delivery, database
  growth, backup duration, restore duration, and cost per retention profile at p50/p95/p99;
- mix current server/worker with the current and previous two supported agent minor versions and
  protocol fixtures, including rolling upgrade and rollback; and
- run at least 14 continuous days with scheduled database, worker, network, webhook, disk-pressure,
  and agent outage injection while checking for leaks and silent loss.

Edge cases: one noisy workspace, one tenant approaching every quota simultaneously, millions of
short incidents, reconnect thundering herd, sequence gaps, hot indexes, autovacuum lag, long export,
backup overlapping retention, and regional latency. The phase passes only with a published hardware
and topology envelope, explicit rejection/backpressure behavior beyond it, no silent loss, and no
unbounded resource growth during soak.

## 0.8 — Hosted Beta (parallel, free first)

Goal: learn whether operators value a managed version before adding billing complexity.

Required work:

- managed tenant creation, invitation, deletion/export, quota, and support workflows;
- isolated production accounts, regional probes, backups, restore drills, and on-call runbooks;
- published beta SLOs and privacy/retention commitments; and
- per-tenant cost measurement for storage, egress, probes, backup, and support.

Exit gate: named beta users complete onboarding and sustained monitoring without weakening the
Community boundary. Stripe remains disabled. A later commerce milestone must separately prove
Checkout, webhook, entitlement, renewal, grace, cancellation, refund, and reconciliation.

Repository and architecture gate:

- Cloud provisioning, regional placement, provider infrastructure, account portal, support tools,
  quotas, and SLO automation live in the private `meerkateer-cloud` repository;
- the hosted data plane runs pinned, signed `meerkateer` images by version and digest;
- public core migrations exclusively own core tables while the Cloud management database owns
  account/provisioning metadata; and
- Cloud never patches or forks core behavior. A required core capability is added and tested in the
  public repository first.

Test plan:

- provision many tenants concurrently, invite members, switch companies, suspend/reactivate,
  export, delete, and expire backups while continuously probing for cross-tenant leakage;
- attempt another tenant's IDs through API, worker, caches, metrics, object storage, exports,
  regional probes, and internal support tools;
- lose a region, database node, worker group, secret provider, DNS provider, and notification
  receiver; execute on-call, failover, restore, and customer-communication runbooks;
- validate per-tenant quotas and fairness under abusive probe rates, oversized payloads, cardinality,
  egress, storage, and invite attempts; and
- measure real per-tenant storage, compute, egress, backup, probe, and support cost without charging
  beta users.

Edge cases: duplicate signup, identity email change, account belonging to several companies,
provisioning timeout after partial creation, tenant deletion during an incident, region move with
agents offline, support impersonation, export larger than browser limits, and deletion subject to
backup retention. Pass requires named beta users, no isolation failure, published beta SLO/privacy/
retention terms, successful restore and region drills, and sustainable measured cost. Stripe stays
off throughout this phase.

## 0.9 — Release Candidate

Goal: freeze scope and prove the complete supported journey.

Required work and exit gate:

- all 1.0 acceptance scenarios pass from immutable signed artifacts;
- supported prior versions upgrade and roll back according to the published procedure;
- backup/restore and incident-response drills meet published targets;
- independent security and privacy findings are closed or explicitly accepted with owners;
- 3–5 representative operators complete at least a 14-day soak; and
- documentation, support matrix, known limitations, changelog, and risk register are current.

Only release-blocking fixes enter after `0.9.0`. Every fix reruns the affected soak or drill.

Release-candidate test plan:

- build once from the tag and promote the same digests through clean install, upgrade, rollback,
  backup/restore, failure/recovery, security, accessibility, compatibility, and capacity suites;
- run destructive disaster recovery from operator documentation with a person who did not author
  the runbook;
- repeat the complete acceptance matrix on supported platforms and reverse-proxy/network profiles;
- have 3–5 representative game/SME operators run real workloads for at least 14 days; and
- triage every failure with severity, owner, reproduction, affected versions, fix, regression test,
  and soak-reset decision.

Edge cases concentrate on interaction effects: upgrade during an open incident, restore while agents
buffer, credential rotation during rolling deployment, retention during export, notification outage
during control-plane restart, and rollback with newer agents still online. The candidate fails on
any unresolved severity-one/two defect, critical/high security issue, silent data loss, tenant leak,
unrecoverable migration, or unsupported documentation gap.

## 1.0 — Stable Community

Release `1.0.0` only when:

- no critical or high security issue is open;
- no accepted telemetry can disappear silently;
- status and alert detection p95 is within two configured check intervals plus delivery time;
- every supported platform passes install, restart, upgrade, rollback, and uninstall tests;
- restore evidence meets published RPO/RTO;
- the capacity envelope and compatibility window are public;
- the 14-day release-candidate soak has no unresolved severity-one or severity-two defect; and
- maintainers have named owners for security response, releases, migrations, and documentation.

Final acceptance also requires:

- an automated release workflow that produces identical version metadata across binaries, images,
  packages, Console, OpenAPI, changelog, and documentation;
- published support, compatibility, deprecation, vulnerability-response, backup, capacity, and data
  retention contracts;
- evidence links for every gate below, reviewed and signed off by named engineering and operations
  owners; and
- a rollback/communication decision tree for a defect discovered immediately after release.

Post-release edge cases—revoked signing key, bad mirror/cache, critical dependency disclosure,
corrupt release artifact, schema regression, or incompatible agent rollout—must have rehearsed
response procedures. `1.0.0` is complete only when the release can be installed, operated,
diagnosed, upgraded, restored, and removed without private maintainer knowledge.

After 1.0, use `1.x` minor releases for backward-compatible capabilities and patch releases for
fixes. Breaking protocol, storage, or API changes require a documented deprecation window and
the next major version.

### Post-1.0 option: bounded remote recovery

Stable monitoring does not require remote machine authority, so `1.0.0` remains outbound telemetry
only. A later `1.x` release may add an owner/admin Console action to restart one locally allowlisted
service. It is not a shell, host reboot, file manager, or general remote administration feature.

Implementation order is protocol/threat-model review, signed action envelope and key rotation,
durable tenant-scoped queue/result/audit APIs, least-privilege Linux and Windows helpers, then the
Console confirmation/result UI. Cross-tenant access, replay, duplicate delivery, expiration,
offline agent, clock skew, denied privilege, cooldown, crash, and service-already-stopped cases must
pass before the control is enabled. [ADR-0008](adr/0008-bounded-remote-recovery.md) is the normative
proposal and keeps the feature disabled until all gates pass.

## Evidence board

Every milestone should link evidence for these journeys:

| Gate | Evidence required |
| --- | --- |
| Fresh install | Published artifact → company → workspace → first machine/service |
| Tenant boundary | Complete cross-tenant API, SQL, worker, export, and guessed-ID matrix |
| Detection | Process, HTTP/TLS, host, stale telemetry, and game-adapter failure reasons |
| Recovery | Fresh evidence closes an incident; missing/old evidence cannot |
| Durability | Crash/retry/replay proves accepted facts are never silently lost |
| Alerts | Deduplication, cooldown, maintenance, receiver outage, replay, and watchdog |
| Agent resilience | Offline spool, reboot, proxy, rotation, resource budget, and rollback |
| Data operations | Migration, encrypted backup, isolated restore, and rollback |
| Supply chain | Locked dependencies, scan, SBOM, signatures, provenance, checksums |
| Usability | First-user journey, accessibility, mobile, diagnostics, and redacted support bundle |
| Capacity | Published reference hardware, sustained/burst load, latency, and retention |
| Soak | 3–5 operators, at least 14 days, fault injection, and closed blocker register |

Passing unit tests is necessary but does not close these gates by itself.
