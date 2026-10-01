# Meerkateer threat model

Status: living document; Phase 2 identity boundary

## Assets and trust boundaries

The primary assets are tenant configuration, service/agent credentials, operational
telemetry, audit evidence, billing state, and availability of monitoring. The major
boundaries are browser-to-control-plane, workload/agent-to-ingest, API/worker-to-
PostgreSQL, probe-to-untrusted destination, and Community-versus-Cloud billing.

Monitored game and SME machines are not trusted to administer a tenant. Agents are not
trusted with another agent's credential or configuration. A tenant administrator is
not trusted to read or mutate another tenant. Stripe/browser redirects are not trusted
as entitlement evidence.

## Phase 2 threats and controls

| Threat | Required control | Current evidence |
| --- | --- | --- |
| Cross-tenant object access | Tenant ID in every owned key; centralized authorization; PostgreSQL RLS fail closed | Domain authorization tests and runtime-role RLS tests |
| API accidentally bypasses RLS | Dedicated `NOSUPERUSER`/`NOBYPASSRLS` runtime role; tenant context set per transaction | Runtime role sees no rows without context |
| Stolen database dump reveals service keys | Key shown once; only Argon2id PHC hash and non-secret prefix stored | Credential issue/verify/redaction tests and schema constraint |
| Credential guessing causes CPU exhaustion | Prefix lookup, per-IP/prefix rate limit, concurrency budget before Argon2 | Prefix exists; rate limiting remains required before endpoint exposure |
| Revoked/copied agent authenticates | Unique agent identity, installation fingerprint digest, expiring/rotatable credentials, cloning quarantine | One-time enrollment, replay/duplicate rejection, rotation, and immediate revocation pass E2E; automated cloning quarantine remains |
| Audit evidence is altered | Append-only table and database trigger; restricted operator access | UPDATE/DELETE rejection tested against PostgreSQL 18 |
| User/session theft | OIDC Authorization Code + PKCE, Secure/HttpOnly/SameSite cookies, short sessions, rotation/revocation | Design requirement; implementation pending |
| Bootstrap takeover | Explicit one-time bootstrap secret, disabled after first owner, no public default | Atomic one-way bootstrap and deterministic replay conflict pass PostgreSQL E2E |
| Secrets leak in logs/errors | `SecretString`, stable public errors, no debug serialization, secret scan | Credential debug-redaction test and repository scan |
| Control plane becomes remote shell | Keep MKA-1 telemetry-only; future actions are signed, short-lived, typed, locally allowlisted, least-privilege, replay-safe, rate-limited, and audited | Remote execution is currently absent/disabled; ADR-0008 defines mandatory gates |

## Database connection rules

Migrations run with an owner role that is never supplied to API or worker containers.
Runtime components use `meerkateer_app`, which is not superuser and cannot create roles
or databases. RLS is enabled and forced on tenant-owned tables. Each database
transaction must set `SET LOCAL meerkateer.tenant_id` before tenant queries; pooled
connections must never use session-global tenant state.

The API must refuse production startup if its database URL is missing. Database URLs
are secret values and are never logged. The database network is private and agents,
browsers, game servers, and probes never connect directly.

## Residual risks before public alpha

The manual Minecraft Java probe is Community-only and rejects non-public resolved
addresses before connecting to a numeric IP. It has bounded DNS/connect/read time,
response size, and a node-local request budget. Cloud probing remains disabled until
separate network egress isolation, DNS rebinding tests, and an operator review exist.

Do not expose the current build publicly. OIDC validation, HTTP rate limits, automated
agent cloning quarantine, desired-configuration signing, and security-header tests
remain release blockers. Tenant transaction scoping, bootstrap shutdown, session CSRF,
service-key rotation, agent enrollment/rotation, and immediate revocation now have
automated or PostgreSQL E2E evidence. Phase 5 separately covers SSRF, DNS rebinding,
parser, and amplification threats. Phase 7 separately covers Stripe signature, replay,
ordering, and entitlement threats.
