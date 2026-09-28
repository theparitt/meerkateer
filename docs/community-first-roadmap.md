# Meerkateer Community-first delivery plan

Status: CE-0 complete for the isolated Compose pilot, 2026-09-28. CE-1 is in progress.
This plan supersedes the choice of a
game or WooCommerce pilot and pauses Cloud, billing, and hosted signup work. The
generic core, existing experimental game adapter, and documented Cloud boundary
remain in the repository; they are not the release target for this cycle.

Community must run without a Cloud account, activation, expiry, billing, or an AI
key. The first supported workflow is: connect a host and service, observe a
failure, inspect evidence, notify the operator, and observe recovery. The release
claim must follow a real installation and fault exercise, not a successful build.

## CE-0 — source and test audit

The table uses implementation and exercised tests as evidence. “Partial” means
there is runnable code, but the requested operator outcome is not yet verified.

| Capability | Source | Current result | Evidence | Gap |
| --- | --- | --- | --- | --- |
| Owner setup and login | `meerkateer-server/src/lib.rs` (`bootstrap`, `password_login`, `password_setup`), `migrations/0009_community_owner_password.sql`, `meerkateer-web/src/App.tsx` | Partial | `tests/integration/api_e2e.py` rejects second bootstrap, tests password login/recovery and retired setup-key login; `/` routes fresh Community to setup | No clean-machine browser journey recorded |
| Host enrollment and telemetry | `meerkateer-agent/src/main.rs`, `migrations/0006_agent_telemetry.sql`, server `/v1/agent/telemetry` | Partial | Agent CLI enroll/doctor/run and credential lifecycle are exercised in `api_e2e.py`; agent unit tests cover durable batch retry | Agent currently sends `agent.heartbeat` only; no disk, inode, memory-pressure, or process-state collector |
| Service signals | `sdk/{node,go,rust,python,php}`, server `ingest_heartbeat`, `ingest_event`, `ingest_deploy` | Partial | MKS E2E proves auth, idempotency, status transitions and timeline; SDK unit tests pass | No scheduled HTTP/TLS check or last-success worker check |
| Status and staleness | `service_snapshots`, `service_from_row` and `agent_responses` in server, `AvailabilityBoard.tsx` | Partial | E2E proves fresh down then fresh recovery and aged heartbeat becomes `unknown` | Permission failure and collector failure are not first-class status reasons; no automatic incident open/close |
| Incident evidence | `ingest_messages`, `/v1/services/{id}/timeline`, `AvailabilityBoard.tsx` | Partial | E2E checks down/recovery reasons, event/deploy timeline; web tests check detail interactions | Timeline facts are not a deduplicated incident engine; no separate hypotheses/missing-evidence/next-check fields |
| Alert delivery | `outbox`, `outbox_dead_letters`, `meerkateer-worker/src/main.rs`, `/v1/alerts/test` | Partial | Single webhook channel, test button, down/recovery transition enqueue and worker delivery have isolated E2E coverage | Confirmation, maintenance and alert history UI remain |
| Tenant and credential boundary | `migrations/0002_identity_and_tenant_boundary.sql`, `crates/meerkateer-identity`, server auth/CSRF | Partial | API and SQL E2E cover tenant isolation, replay, rotation/revocation, CSRF and rate limit | Release security review, gateway-wide limits and multi-user roles remain |
| Packaging and restore | `compose.yaml`, `scripts/bootstrap.sh`, `scripts/migrate-existing.sh`, `packaging/`, CI | Partial | Fresh and upgrade migration tests, Compose smoke and isolated integration | No verified release artifact install, backup/restore exercise, or uninstall guide |
| Cloud/AI independence | `crates/meerkateer-config`, Community Compose path | Partial | Community E2E runs without Stripe or AI credentials; website now presents Community as current product | Release artifact installation remains unverified |

The old `dbgflow` description in the supplied plan has no verified source mapping in
this repository. It does not count as delivered Meerkateer functionality.

The CE-0 isolated `make integration` run passed on 2026-09-28: bootstrap and
password login, agent enrollment and replay protection, ingest status/recovery,
timeline, migration 0007→0010, worker retry and dead-letter. The separate Compose
volume and containers were removed by the harness. This is test-environment
evidence; it does not verify any real production host.

### Test environment inventory

The selected CE-1 test environment is an **isolated Compose project on this machine**,
not a production server. `scripts/integration.sh` creates its own PostgreSQL volume,
API port, and containers, then removes them. The first service is a controllable
fixture process; its heartbeat and down/recovery reports are test facts. Production
hosts, systemd units, databases, jobs and notification destinations remain unknown
until the owner supplies an inventory. No existing production service will be
stopped to satisfy these gates.

## Phases and exit gates

| Phase | Required outcome | Exit evidence |
| --- | --- | --- |
| CE-0 — Audit | Source-backed capability map, test inventory, one safe pilot target | This document plus a current isolated integration run |
| CE-1 — Internal alpha | One host and service can be enrolled, checked, shown as down/recovered, and notify one channel | Fresh Compose journey with a stopped/restarted fixture, alert test, no Cloud/AI key |
| CE-2 — Evidence-led incidents | Findings, missing data, suggested next checks and recovery are separate from acknowledgement | Replay fixtures prove deduplication and no false recovery from missing data |
| CE-3 — Trustworthy alerts | Confirmation, cooldown, maintenance, retry/dead letter and an external watchdog | Fault tests for receiver outage, monitoring outage and expired maintenance |
| CE-4 — Operator context | One real API/worker/database workflow mapped to project/environment/dependencies | Source comparison shows less time gathering evidence, with verified metrics |
| CE-5 — Release safety | Bounded resources, isolation, secret handling, restore and upgrade | Security review and restore on a separate machine |
| CE-6 — Public preview | Downloadable Linux x86-64 artifact and one documented install path | Clean-machine install from artifact, checksum, setup through recovery |
| CE-7 — Stable | Continuous supported use and feedback from 3–5 operators | Approximately 14-day soak, upgrade/restore, alert quality and resolved blockers |

## Twenty acceptance scenarios

These are release gates, not claims that each case already passes.

| ID | Scenario | Expected result |
| --- | --- | --- |
| CF-01 | Fresh Community install | Owner is created once without Cloud or AI credentials |
| CF-02 | Second bootstrap attempt | Existing tenant cannot be claimed again |
| CF-03 | Owner signs out and back in | Password login works; setup key does not create a session |
| CF-04 | Host enrollment replay | Token reuse is rejected; normal enrollment succeeds |
| CF-05 | Host telemetry stops | Host becomes stale/unknown, not falsely offline or healthy |
| CF-06 | Collector lacks permission | UI says collection failed, not zero resource usage |
| CF-07 | Disk/inode/memory sample | Values match a bounded source observation |
| CF-08 | Service fixture stops | One outage is visible with source and timestamp |
| CF-09 | Same failure repeats | One incident continues; no new incident each poll |
| CF-10 | Service fixture recovers | Fresh evidence closes the outage; old/missing data does not |
| CF-11 | Operator acknowledges | Observed service state remains unchanged |
| CF-12 | Operator requests test alert | Selected channel receives a clearly marked test |
| CF-13 | Outage and recovery alert | Both messages carry service, state and observed time |
| CF-14 | Alert destination fails | Delivery is retried, then visible as failed if exhausted |
| CF-15 | Monitor itself stops | Independent watchdog detects the monitoring outage |
| CF-16 | HTTP/TLS check fails | Report distinguishes timeout, certificate failure and HTTP error |
| CF-17 | Worker stops progressing | Last success and queue evidence reveal stalled work |
| CF-18 | Cross-tenant request | Other tenant data and channel configuration are inaccessible |
| CF-19 | Backup and restore | Fresh installation recovers database, config and required keys |
| CF-20 | Release install/upgrade | Verified artifact installs and upgrades without data loss |

CE-1 work should use CF-01–05, CF-08, CF-10, CF-12–13 as its first narrow
journey. Later phases own the remaining tests. No case is marked passed based on
the roadmap alone; results belong in `docs/phase-status.md`.

The isolated `community_alerts.sh` exercise currently proves CF-01, CF-04,
CF-08, CF-10, CF-12, and CF-13 with a local fixture, together with alert
deduplication for an exact retry and a late observation. The broader
`integration.sh` covers CF-02, CF-03, and CF-05. These are test-environment
results; CF-06–07 and CF-09 need richer collectors and an incident model.
