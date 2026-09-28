# Meerkateer Interface Standard v1 (MKS-1)

- **Standard name:** Meerkateer Interface Standard v1
- **Short name:** MKS-1
- **Interface identifier:** `meerkateer`
- **Interface version:** `1`
- **Status:** Draft
- **Owner:** Meerkateer

MKS-1 is owned by Meerkateer. It is a **generic** standard. It does not name, assume,
or special-case any connected service. Meerkateer treats every connected service as a
generic monitored service, regardless of its domain (auth, board, chat, ticketing,
ecommerce, CMS, or anything else).

> **Reference implementation.** The pull-endpoint shapes in MKS-1 are derived from a
> real, in-production health protocol (Rust + Actix-web): `/health`, `/ready`,
> `/metrics`, and `/server-info`, with a `{ ok, error }` per-dependency check result, a
> `status` of `ok` / `degraded`, a nested `build` block, and Prometheus `*_build_info`,
> `*_uptime_seconds`, `*_dependency_up`, and `*_db_pool_connections` metrics. MKS-1
> standardises that shape and adds Meerkateer discovery + push interfaces on top.

---

## 1. Standard purpose

MKS-1 defines how an external service exposes **health**, **readiness**, **metrics**,
**metadata**, and optional **push events** to Meerkateer.

Every compatible service MUST expose the following pull endpoints:

| Method | Path                            | Purpose                     |
| ------ | ------------------------------- | --------------------------- |
| GET    | `/health`                       | Liveness + dependency state |
| GET    | `/ready`                        | Readiness                   |
| GET    | `/metrics`                      | Prometheus metrics          |
| GET    | `/.well-known/meerkateer.json`  | Machine-readable metadata   |

Every compatible service SHOULD also expose:

| Method | Path           | Purpose                       |
| ------ | -------------- | ----------------------------- |
| GET    | `/server-info` | Human/info build + mode block |

Every compatible service MAY use the following push endpoints, hosted by Meerkateer:

| Method | Path                                            | Purpose         |
| ------ | ----------------------------------------------- | --------------- |
| POST   | `<MEERKATEER_INGEST_URL>/v1/ingest/heartbeat`   | Push heartbeat  |
| POST   | `<MEERKATEER_INGEST_URL>/v1/ingest/event`       | Push event      |
| POST   | `<MEERKATEER_INGEST_URL>/v1/ingest/deploy`      | Push deploy     |

Conformance keywords (MUST, MUST NOT, SHOULD, MAY) follow RFC 2119.

All timestamps in MKS-1 are RFC 3339 strings in UTC (e.g. `2026-06-05T12:00:00Z`).
The wire value MUST end in uppercase `Z`; numeric timezone offsets are not conformant.

Unless a field has a tighter schema constraint, MKS-1 uses these limits:

- `service`: 1-64 characters, `A-Z`, `a-z`, `0-9`, dot, underscore, and hyphen only;
- `project`: 1-64 lowercase characters, `a-z`, `0-9`, underscore, and hyphen only;
- `version`, `commit`, build values, and runtime/mode labels: at most 128 characters;
- `message`: at most 1,024 characters after UTF-8 decoding;
- `kind`: at most 128 characters; and
- endpoint paths: relative absolute-path references beginning with `/`, at most 256
  characters, with no whitespace or embedded authority/credentials.

Implementations MUST reject an oversized request before fully buffering it. The
Meerkateer ingest API limit for each MKS-1 JSON body is 16 KiB unless a future
compatible revision specifies a smaller per-endpoint limit.

---

## 2. Project-prefixed environment standard

Every service implementing MKS-1 MUST configure the interface through
**project-prefixed environment variables**.

**Format:**

```
<PROJECT_PREFIX>_<VARIABLE_NAME>
```

**Rules:**

- `<PROJECT_PREFIX>` MUST be uppercase.
- `<PROJECT_PREFIX>` MUST use `A-Z`, `0-9`, and underscore only, and MUST start with a letter.
- No generic, unprefixed env names are allowed for the MKS-1 interface.
- Do **not** read `SERVICE_NAME`, `SERVICE_PROJECT`, `MEERKATEER_ENABLED`, or
  `METRICS_ENABLED` directly. Use the prefixed forms below.
- The implementation MUST allow the prefix to be configured.
- The implementation MUST NOT hard-code any specific project prefix.

**Required environment variables:**

```
# Service identity
<PROJECT_PREFIX>_SERVICE_NAME=<service-name>
<PROJECT_PREFIX>_SERVICE_PROJECT=<project-slug>
<PROJECT_PREFIX>_SERVICE_ENVIRONMENT=<environment>

# Build identity (compile-time; surfaced in /health, /server-info, metadata)
<PROJECT_PREFIX>_GIT_SHA=<git-commit>
<PROJECT_PREFIX>_GIT_BRANCH=<git-branch>
<PROJECT_PREFIX>_BUILD_TIME_UTC=<build-time>

# Meerkateer interface
<PROJECT_PREFIX>_MEERKATEER_INTERFACE_VERSION=1
<PROJECT_PREFIX>_MEERKATEER_ENABLED=false
<PROJECT_PREFIX>_MEERKATEER_INGEST_URL=
<PROJECT_PREFIX>_MEERKATEER_SERVICE_KEY=
<PROJECT_PREFIX>_MEERKATEER_TIMEOUT_MS=3000
<PROJECT_PREFIX>_MEERKATEER_HEARTBEAT_INTERVAL_SECONDS=60

# Metrics
<PROJECT_PREFIX>_METRICS_ENABLED=true
<PROJECT_PREFIX>_METRICS_TOKEN=
<PROJECT_PREFIX>_METRICS_ALLOWLIST=
```

**Environment value meanings:**

- `SERVICE_NAME` — identifies the running service.
- `SERVICE_PROJECT` — identifies the owning project/application (the project slug).
- `SERVICE_ENVIRONMENT` — one of: `development`, `staging`, `production`, `test`, `local`.
- `GIT_SHA` — the source-control commit, surfaced as `build.git_sha`. Defaults to
  `"unknown"` when not set at build time.
- `GIT_BRANCH` — the build branch, surfaced as `build.git_branch`. Defaults to
  `"unknown"`.
- `BUILD_TIME_UTC` — the build timestamp (RFC 3339), surfaced as `build.built_at_utc`.
  Defaults to `"unknown"`.

The released **version** is read from the build manifest (e.g. Cargo's
`CARGO_PKG_VERSION`), not from an env var, matching the reference implementation.

`<PROJECT_PREFIX>_MEERKATEER_SERVICE_KEY` is a secret. It MUST NOT appear in any
pull-endpoint response, in `/.well-known/meerkateer.json`, in metrics, or in logs.

---

## 3. The dependency check result

Both `/health` and `/ready` report per-dependency results using a single, shared
shape — the **check result** — taken directly from the reference implementation:

```json
{ "ok": true }
```

or, when the dependency is down:

```json
{ "ok": false, "error": "unavailable" }
```

**Check result rules:**

- `ok` (boolean) is **required**.
- `error` (string) is present **only when `ok` is false**, and MUST be omitted when
  `ok` is true (the reference implementation serializes it as
  `skip_serializing_if = "Option::is_none"`).
- `error` MUST be one of the safe enum values below. The implementation MUST map any
  raw dependency error (sqlx, redis, HTTP client, etc.) to one of these — it MUST NOT
  pass `err.to_string()` straight through, because raw errors can leak connection
  strings, credentials, or internal topology.

**Allowed safe error values:** `unavailable`, `timeout`, `misconfigured`,
`disabled`, `unknown`.

> **Note for the reference implementation.** The current Rust server emits the raw
> `error.to_string()` in this field. To be MKS-1 conformant it must map those raw
> strings to the safe enum above (e.g. a connection failure → `"unavailable"`, a
> timeout → `"timeout"`). The wire shape `{ ok, error? }` is unchanged.

Common dependency names: `database`, `cache`, `redis`, `object_storage`, `queue`,
`worker`, `mail`, `websocket`, `external_api`, `config`, `filesystem`, `search`,
`scheduler`.

---

## 4. `GET /health`

**Purpose:** Liveness **and** current dependency state in a single response. Proves
the process is alive and reports whether each backing dependency is reachable.

**Response `200` when healthy / `503` when degraded** — same body shape, the HTTP
status and the `status` field carry the verdict:

```json
{
  "status": "ok",
  "service": "<service-name>",
  "project": "<project-slug>",
  "environment": "<environment>",
  "interface": "meerkateer",
  "interface_version": "1",
  "version": "<version>",
  "build": {
    "built_at_utc": "<build-time>",
    "git_sha": "<commit>",
    "git_branch": "<branch>"
  },
  "checks": {
    "database": { "ok": true },
    "redis": { "ok": true }
  },
  "timestamp": "<rfc3339-timestamp>"
}
```

**Rules:**

- `status` MUST be `"ok"` when **every** check is `ok`, otherwise `"degraded"`.
- Return HTTP `200` when `status == "ok"`, HTTP `503` when `status == "degraded"`.
  Any single failed dependency makes the whole service degraded — MKS-1 does **not**
  define critical/non-critical tiers (matching the reference implementation).
- Each dependency check MUST have a timeout.
- `build.built_at_utc`, `build.git_sha`, `build.git_branch` default to `"unknown"`
  when not provided at build time.
- MUST NOT expose secrets. The `checks` errors are limited to the §3 safe enum.

---

## 5. `GET /ready`

**Purpose:** Readiness probe for orchestrator gates (Kubernetes `readinessProbe`,
load-balancer health gates). Reports whether the service can serve traffic **right
now**. Returns a minimal body — no `build` block.

**Response `200` when ready:**

```json
{
  "ready": true,
  "service": "<service-name>",
  "project": "<project-slug>",
  "environment": "<environment>",
  "interface": "meerkateer",
  "interface_version": "1",
  "checks": {
    "database": { "ok": true },
    "redis": { "ok": true }
  },
  "timestamp": "<rfc3339-timestamp>"
}
```

**Response `503` when not ready:**

```json
{
  "ready": false,
  "service": "<service-name>",
  "project": "<project-slug>",
  "environment": "<environment>",
  "interface": "meerkateer",
  "interface_version": "1",
  "checks": {
    "database": { "ok": false, "error": "unavailable" },
    "redis": { "ok": true }
  },
  "timestamp": "<rfc3339-timestamp>"
}
```

**Rules:**

- `ready` is `true` only when **every** check is `ok`; otherwise `false`.
- Return HTTP `200` when `ready == true`, HTTP `503` when `ready == false`.
- Same check-result shape and same safe-error enum as `/health` (see §3).
- Every dependency check MUST have a timeout.

---

## 6. `GET /metrics`

**Purpose:** Prometheus metrics endpoint. Output MUST be Prometheus text exposition
format, served as `text/plain; version=0.0.4; charset=utf-8` — matching the reference
implementation.

The reference implementation is intentionally **dependency-light**: rather than a
global recorder + middleware, it samples cheap runtime facts at scrape time (uptime,
DB-pool gauges, dependency reachability, build info). MKS-1 codifies that minimal,
namespaced metric set as the required base; richer request/worker metrics are optional.

**Rules:**

- Output MUST be Prometheus text format (`0.0.4`).
- MUST respect `<PROJECT_PREFIX>_METRICS_ENABLED`. If disabled, return `404` or `403`
  **consistently** (pick one and document it).
- MUST support `<PROJECT_PREFIX>_METRICS_TOKEN`. If set, require
  `Authorization: Bearer <token>`.
- MUST NOT expose secrets or PII.
- MUST avoid high-cardinality labels.
- MUST use route templates, not raw paths.
- All metric names MUST be namespaced with the **project slug**. Replace each hyphen in
  the slug with an underscore before using it as a Prometheus metric namespace:
  `<normalized_project_slug>_<metric_name>`. This normalization is only for metric
  names; the `project` field retains the registered slug.

**Required base metrics** (names shown with `<project_slug>` namespace; these are the
reference implementation's emitted set):

```
# Build info — constant 1, values carried in labels.
<project_slug>_build_info{version="...",git_sha="...",git_branch="..."} 1

# Seconds since the process started.
<project_slug>_uptime_seconds <float>

# Dependency reachability (1 = up), one series per dependency.
<project_slug>_dependency_up{dependency="database"} 1
<project_slug>_dependency_up{dependency="redis"} 1

# Connection-pool gauges (when the service has a DB pool).
<project_slug>_db_pool_connections{state="total"} <n>
<project_slug>_db_pool_connections{state="idle"} <n>
<project_slug>_db_pool_connections{state="in_use"} <n>
```

Each metric MUST be preceded by its `# HELP` and `# TYPE` lines.

**Good labels:** `version`, `git_sha`, `git_branch`, `dependency`, `state`, `service`,
`project`, `environment`, `method`, `route`, `status_class`, `provider`, `flow`,
`operation`.

**Bad labels (forbidden):** `user_id`, `email`, `phone`, `tenant_name`, `tenant_slug`,
`request_id`, `session_id`, `token`, `full_url`, `raw_path`, `ip_address`, `object_id`,
`document_id`, `dynamic_id`.

**Optional additional metrics** (namespaced the same way):

```
<project_slug>_http_requests_total
<project_slug>_http_request_duration_seconds
<project_slug>_http_requests_in_flight
<project_slug>_http_errors_total
<project_slug>_worker_jobs_total
<project_slug>_worker_job_errors_total
<project_slug>_queue_depth
<project_slug>_queue_lag_seconds
<project_slug>_external_api_requests_total
<project_slug>_external_api_errors_total
```

These MUST NOT include PII labels, dynamic IDs, or secret values.

---

## 7. `GET /server-info`

**Purpose:** Human-facing / info build + mode block, taken from the reference
implementation. SHOULD be exposed. Unlike `/.well-known/meerkateer.json` it is not the
machine-discovery document — it carries the running **mode** and build metadata.

**Response `200`:**

```json
{
  "name": "<service-name>",
  "version": "<version>",
  "mode": "<mode>",
  "build": {
    "built_at_utc": "<build-time>",
    "git_sha": "<commit>",
    "git_branch": "<branch>"
  }
}
```

`mode` is a free-form runtime mode label (e.g. `development`, `production`). It MUST
NOT contain secrets. This endpoint MUST NOT expose secrets, credentials, or PII.

---

## 8. `GET /.well-known/meerkateer.json`

**Purpose:** Machine-readable service metadata for Meerkateer discovery.

**Response `200`:**

```json
{
  "interface": "meerkateer",
  "interface_version": "1",
  "service": "<service-name>",
  "project": "<project-slug>",
  "environment": "<environment>",
  "runtime": "<runtime>",
  "version": "<version>",
  "build": {
    "built_at_utc": "<build-time>",
    "git_sha": "<commit>",
    "git_branch": "<branch>"
  },
  "endpoints": {
    "health": "/health",
    "ready": "/ready",
    "metrics": "/metrics",
    "server_info": "/server-info"
  },
  "capabilities": {
    "health": true,
    "readiness": true,
    "prometheus_metrics": true,
    "server_info": true,
    "push_heartbeat": true,
    "push_event": true,
    "push_deploy": true
  }
}
```

**Rules:** Safe to expose publicly. No secrets, no internal URLs with credentials,
no private keys, no service keys, no personal data.

---

## 9. Push: heartbeat

**Purpose:** Allow a service to push heartbeat events to Meerkateer.

**Target:** `POST <MEERKATEER_INGEST_URL>/v1/ingest/heartbeat`

**Headers:**

```
Authorization: Bearer <SERVICE_KEY>
Content-Type: application/json
```

**Payload:**

```json
{
  "interface_version": "1",
  "service": "<service-name>",
  "project": "<project-slug>",
  "environment": "<environment>",
  "status": "ok",
  "message": "heartbeat",
  "timestamp": "<rfc3339-timestamp>"
}
```

**Rules:** Heartbeat push is optional. Failure to send a heartbeat MUST NOT break the
main app. Use a timeout. Do not panic. Do not send secrets or PII.

---

## 10. Push: event

**Purpose:** Allow a service to send operational events to Meerkateer.

**Target:** `POST <MEERKATEER_INGEST_URL>/v1/ingest/event`

**Headers:**

```
Authorization: Bearer <SERVICE_KEY>
Content-Type: application/json
```

**Payload:**

```json
{
  "interface_version": "1",
  "service": "<service-name>",
  "project": "<project-slug>",
  "environment": "<environment>",
  "level": "error",
  "kind": "<stable-event-kind>",
  "message": "<safe-message>",
  "count": 1,
  "timestamp": "<rfc3339-timestamp>"
}
```

**Allowed event levels:** `info`, `warning`, `error`, `critical`.

**Event kind naming format:** at least two lowercase segments separated by underscore,
for example `<domain>_<condition>` or `<domain>_<problem>_<condition>`.

**Common event kind examples:**

```
database_unavailable        cache_unavailable
object_storage_unavailable  queue_backlog_high
worker_job_failed           external_api_unavailable
webhook_delivery_failed     email_delivery_failed
config_misconfigured        filesystem_unavailable
```

**Rules:** Event kind MUST be stable. Event message MUST be safe. Do not send raw
request bodies, tokens, cookies, authorization headers, or personal data. Failure to
send an event MUST NOT break the main app.

---

## 11. Push: deploy event

**Purpose:** Allow a service to notify Meerkateer about deployments.

**Target:** `POST <MEERKATEER_INGEST_URL>/v1/ingest/deploy`

**Headers:**

```
Authorization: Bearer <SERVICE_KEY>
Content-Type: application/json
```

**Payload:**

```json
{
  "interface_version": "1",
  "service": "<service-name>",
  "project": "<project-slug>",
  "environment": "<environment>",
  "version": "<version>",
  "commit": "<commit>",
  "status": "started",
  "timestamp": "<rfc3339-timestamp>"
}
```

**Allowed deploy statuses:** `started`, `finished`, `failed`.

**Rules:** `version` and `commit` are required. Deploy events are optional. Failure to
send a deploy event MUST NOT fail the deployment. Do not send secrets or private
repository tokens.

---

## 12. Security standard

The following MUST NEVER appear in any health, ready, metrics, metadata, log, or push
event:

```
passwords                     password hashes
access tokens                 refresh tokens
session tokens                cookies
authorization headers         one-time tokens
API keys                      service keys
private keys                  database URLs with credentials
cache URLs with credentials   object storage secret keys
OAuth client secrets          authorization codes
personal names                emails
phone numbers                 raw request bodies
```

**Required behavior:**

- Add redaction helpers where needed.
- All error messages returned from MKS-1 endpoints MUST be safe.
- Logs MUST NOT contain secret values.
- Metrics labels MUST NOT contain personal data or dynamic IDs.

---

## 13. Meerkateer server requirements

Inside Meerkateer, the official MKS-1 documentation and schemas live at:

- `docs/standards/mks-1.md` (this document)
- `schemas/mks-1/health.schema.json`
- `schemas/mks-1/ready.schema.json`
- `schemas/mks-1/metadata.schema.json`
- `schemas/mks-1/server-info.schema.json`
- `schemas/mks-1/heartbeat.schema.json`
- `schemas/mks-1/event.schema.json`
- `schemas/mks-1/deploy.schema.json`

When the Meerkateer server is scaffolded, it MUST add validation helpers that:

- validate MKS-1 metadata payloads;
- validate heartbeat payloads;
- validate event payloads;
- validate deploy payloads;
- reject unsupported `interface_version`;
- reject missing `service` / `project` / `environment`;
- reject unsafe event `level`;
- reject unsafe deploy `status`.

> **Implementation status:** The server-side validators and ingest API below are
> specified by MKS-1 but are **not yet implemented** in this repository (no server
> project exists at the time of writing). The schemas in `schemas/mks-1/` are the
> normative contract those validators must satisfy.

---

## 14. Meerkateer ingest API standard

Meerkateer exposes:

```
POST /v1/ingest/heartbeat
POST /v1/ingest/event
POST /v1/ingest/deploy
```

**Authentication:** service-key bearer token.

```
Authorization: Bearer <SERVICE_KEY>
```

The service key MUST map server-side to: `tenant_id`, `project_id`,
`environment_id`, `service_id`.

**Rules:**

- Do NOT trust `tenant_id` from the payload.
- Do NOT trust `project_id` from the payload.
- Resolve ownership from the **service key**, not the client payload.
- Validate the payload's `service` / `project` / `environment` against the registered
  service identity.
- Reject invalid `interface_version`.
- Store safe payload only.
- Redact unsafe values before logging.

**Responses:**

Success:

```json
{ "status": "accepted", "interface": "meerkateer", "interface_version": "1" }
```

Auth failure:

```json
{ "status": "error", "error": "unauthorized" }
```

Validation failure:

```json
{ "status": "error", "error": "invalid_payload" }
```

---

## 15. Compatibility checker

Meerkateer provides a compatibility checker that calls a service and verifies:

```
GET /health
GET /ready
GET /.well-known/meerkateer.json
GET /metrics   (optional)
```

The checker verifies:

- HTTP status;
- JSON shape;
- `interface == "meerkateer"`;
- `interface_version == "1"`;
- `service` name exists;
- `project` slug exists;
- `environment` exists;
- endpoints are declared;
- required capabilities exist;
- no obvious secret-looking values appear in any response.

**Compatibility result:**

```json
{
  "compatible": true,
  "interface": "meerkateer",
  "interface_version": "1",
  "checks": {
    "health": "pass",
    "ready": "pass",
    "metadata": "pass",
    "metrics": "pass"
  },
  "warnings": []
}
```

---

## 16. Documentation examples

All examples in this document use placeholder names only. No real connected-project
name appears anywhere.

**Allowed placeholders:**

```
<PROJECT_PREFIX>        <PROJECT_SLUG>          <SERVICE_NAME>
<SERVICE_ENVIRONMENT>   <SERVICE_VERSION>       <SERVICE_COMMIT>
<GIT_BRANCH>            <BUILD_TIME>            <MODE>
<RUNTIME>               <MEERKATEER_INGEST_URL> <SERVICE_KEY>
```

---

## 17. Tests

When the Meerkateer server is scaffolded, the following tests MUST be added.

**Schema / document tests:**

- `docs/standards/mks-1.md` exists.
- Schema files exist under `schemas/mks-1/`.
- Examples contain no specific connected-project names.
- Examples use placeholders.

**Payload validation tests:**

- valid heartbeat is accepted;
- heartbeat without `interface_version` is rejected;
- heartbeat with unsupported `interface_version` is rejected;
- valid event is accepted;
- event with invalid `level` is rejected;
- event with missing `kind` is rejected;
- valid deploy event is accepted;
- deploy event with invalid `status` is rejected;
- metadata with `interface == "meerkateer"` and `interface_version == "1"` is accepted;
- metadata with unsupported `interface_version` is rejected;
- a healthy `/health` response has `status == "ok"` and every check `ok == true`;
- a `/health` response with any failed check has `status == "degraded"` and is served
  with HTTP `503`;
- a check result with `ok == false` carries an `error` from the safe enum only;
- a check result with a raw (non-enum) `error` string is rejected;
- a `server-info` response validates against `server-info.schema.json`.

**Security tests:**

- payload containing obvious secret fields is rejected or redacted;
- service key is never logged;
- authorization header is never logged;
- raw token values are not stored in event message;
- unsafe personal fields are rejected or redacted.

**Compatibility checker tests:**

- compatible service returns pass;
- missing health endpoint returns fail;
- missing metadata endpoint returns fail;
- wrong `interface_version` returns fail;
- metadata containing secret-looking fields returns warning or fail.

---

## 18. Verification commands

Once a `cargo` project and `.env` exist, conformance is verified with:

```bash
set -a && source .env && set +a && cargo fmt --check
set -a && source .env && set +a && cargo clippy --all-targets --all-features -- -D warnings
set -a && source .env && set +a && cargo test -q
```

Targeted tests:

```bash
set -a && source .env && set +a && cargo test mks
set -a && source .env && set +a && cargo test interface
set -a && source .env && set +a && cargo test ingest
set -a && source .env && set +a && cargo test compatibility
set -a && source .env && set +a && cargo test schema
```

Static checks:

```bash
rg "docs/standards/mks-1.md" .
rg "SERVICE_KEY|Authorization|Bearer|access_token|refresh_token|password|secret|private_key" src docs schemas
rg "tenant_id|project_id|environment_id|service_id" src
```

Review all matches: service keys and authorization headers MUST NOT be logged,
secrets MUST NOT appear in examples, and service ownership MUST come from the service
key mapping — never from trusted client payload.

---

## 19. Versioning

- `interface_version` is the string `"1"` for MKS-1.
- A breaking change to any schema produces MKS-2 with `interface_version == "2"`.
- Additive, backward-compatible fields MAY be added to MKS-1 without a version bump;
  consumers MUST ignore unknown fields they do not recognise.
- Meerkateer MUST reject any payload whose `interface_version` it does not support.

---

## 20. Conformance summary

A service is **MKS-1 conformant** when:

- it exposes `/health`, `/ready`, `/metrics`, and `/.well-known/meerkateer.json`
  (and SHOULD expose `/server-info`);
- those responses validate against the schemas in `schemas/mks-1/`;
- `/health` reports `status: "ok" | "degraded"` with per-dependency `{ ok, error? }`
  results, returning `503` when degraded, with `error` drawn only from the §3 safe enum;
- `/ready` reports `ready: bool` with the same check-result shape, returning `503` when
  not ready;
- `/metrics` emits Prometheus `0.0.4` text with `<project_slug>`-namespaced
  `*_build_info`, `*_uptime_seconds`, `*_dependency_up`, and (if it has a DB pool)
  `*_db_pool_connections`;
- it uses `<PROJECT_PREFIX>_`-prefixed environment variables for all interface config;
- it never exposes any secret or PII listed in §12;
- (if it pushes) its heartbeat/event/deploy payloads validate against the schemas and
  authenticate with a service-key bearer token.
