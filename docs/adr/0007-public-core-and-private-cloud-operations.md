# ADR-0007: Public core and private Cloud operations repositories

- Status: accepted
- Date: 2026-09-29
- Owners: maintainers

## Context

Meerkateer Community must be a complete Apache-2.0 reliability product that a company can operate
without a vendor account. Meerkateer Cloud must operate many isolated tenants and needs a different
deployment cadence, access boundary, provider infrastructure, on-call tooling, and eventually a
commercial account lifecycle.

Maintaining Community and Cloud as source forks would duplicate migrations and security fixes,
allow APIs and status behavior to drift, and make it difficult to prove that Cloud runs the same
core released to the community. Keeping provider credentials and internal operator tooling in the
public product repository would also mix two different security and release boundaries.

## Decision

Keep the complete product core in public `theparitt/meerkateer`. It owns the server, worker, agent,
web Console, SDKs, public contracts, core migrations, tenant/RLS enforcement, Community packaging,
and all reliability and security behavior.

Create private `theparitt/meerkateer-cloud` only when Hosted Beta implementation begins. It owns
hosted provisioning, provider infrastructure, regional placement, provider-wide quotas and abuse
controls, Cloud account and internal operator portals, SLO/on-call automation, and later billing.

Cloud consumes immutable, signed public artifacts pinned by version and digest. The public
repository never imports private code. The private repository never copies or patches core source
or owns migrations over core tables. A Cloud requirement that changes core behavior is implemented
and tested publicly first.

Community defaults to one company per installation with many workspaces and machines. Cloud uses
the same tenant-aware core for many companies per managed control plane. Core data and Cloud
management data have separate ownership, migration histories, roles, and preferably databases.

Hosted Beta is free and may begin after the `0.5 Operations Beta` gate. Stripe and commerce are
deferred until tenant isolation, operations, restore, SLO, support, and real per-tenant cost are
proved. ADR-0006 still governs billing verification and idempotency when commerce is implemented;
this ADR changes where that implementation lives and when it enters the release sequence.

The detailed ownership, data, CI, release, and feature-placement rules are maintained in
[Community and Cloud repository boundary](../repository-and-cloud-boundary.md).

## Consequences

- Reliability and security fixes are made once and can be independently verified in public.
- Community remains useful and operable if Meerkateer Cloud or its company ceases to exist.
- Cloud deployments record both a Cloud release ID and exact public core digests.
- Cloud CI must exercise tenant isolation, core-version compatibility, upgrade, rollback,
  backup/restore, regional failure, and abuse controls before promotion.
- Changes spanning both repositories require a public core release before Cloud can consume them.
- Cloud operational code has a smaller contributor audience and needs explicit internal review,
  audit, secret-management, and disaster-recovery practices.

## Alternatives considered

- **Separate Community and Cloud forks:** rejected because fixes, schemas, APIs, and behavior would
  drift and tenant security would be harder to reason about.
- **One public repository for product and all provider operations:** viable for a fully open
  infrastructure business, but rejected initially because provider secrets, internal tooling, and
  deployment cadence have a different access boundary. The decision can be revisited without
  changing the one-way artifact dependency.
- **Private open-core modules inside the public build:** rejected for the initial product because
  reliability and tenant security must remain inspectable and useful to self-hosters.
- **A repository per SDK or service:** deferred until a component has a genuinely independent
  release cadence and maintainer community.
