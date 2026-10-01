# Community and Cloud repository boundary

Status: accepted product architecture for the road to 1.0. Last reviewed 2026-09-29.

Meerkateer is one product with two operating models, not two forks:

- **Meerkateer Community** is the complete Apache-2.0, self-hosted reliability product. One
  installation represents one company and can contain many workspaces, machines, members, and
  monitored services.
- **Meerkateer Cloud** is the managed, multi-tenant operating service. It runs released Community
  core artifacts and adds hosted provisioning, regional operations, account administration,
  quotas, support, and—only after the free beta proves the service—billing.

The boundary exists so that a reliability or security fix is implemented once in the public core
and then consumed by Cloud. Community must never depend on the private repository, a Cloud
account, Stripe, a license server, or a vendor-controlled API.

## Repository model

### Public: `theparitt/meerkateer`

License: Apache-2.0. This repository is the source of truth for the product and protocol:

```text
meerkateer/
├── crates/                    shared domain, configuration, identity, and storage code
├── meerkateer-server/         public API and control plane
├── meerkateer-worker/         durable jobs, status evaluation, and alert delivery
├── meerkateer-agent/          outbound host agent
├── meerkateer-web/            workspace, machine, incident, and setup Console
├── sdk/                       Node.js, Go, Rust, Python, and PHP SDKs
├── migrations/                the only migrations allowed to change core tables
├── schemas/ and openapi/      versioned wire contracts
├── deploy/community/          self-hosted production packaging
├── tests/                     contract, tenant, failure, upgrade, and scale evidence
└── docs/                      operator, security, architecture, and release contracts
```

The public repository owns:

- company, workspace, membership, machine, service, status, incident, and audit models;
- PostgreSQL tenant isolation, RLS policies, and tenant-safe background jobs;
- ingestion, collectors, probes, alerts, exports, retention, backup/restore tooling, and SDKs;
- the shared Console, including ordinary memberships and company switching when implemented;
- Community installation, upgrade, rollback, uninstall, and support-bundle workflows; and
- tests for correctness, isolation, durability, security, accessibility, and capacity.

Reliability and security are Community features. They are not license gates.

### Private: `theparitt/meerkateer-cloud`

This repository contains only code and operations needed to run the hosted service:

```text
meerkateer-cloud/
├── services/
│   ├── provisioner/           create, suspend, export, and delete hosted tenants
│   ├── cloud-identity/        hosted signup and identity-provider integration
│   ├── region-controller/     region placement and probe scheduling
│   ├── quota-controller/      cost and abuse protection
│   └── billing/               Stripe lifecycle; deliberately later than free beta
├── apps/
│   ├── account-portal/        hosted account, plan, usage, and support
│   └── operator-console/      internal support and incident operations
├── infra/                     Terraform, Kubernetes, Cloudflare, and observability
├── environments/              reviewed staging and production declarations
├── tests/                     Cloud E2E, isolation, load, failover, and billing tests
├── ops/                       on-call runbooks, SLOs, and recovery procedures
└── manifests/core-version.yaml
```

Cloud-specific source must not be copied into the public tree and core source must not be copied
into the Cloud tree. The dependency points in one direction:

```text
meerkateer-cloud → released meerkateer images/contracts
meerkateer       ✕ must never import or call private Cloud code
```

An optional public Helm-chart repository may be created later if it develops an independent
release cadence and contributor group. Until then, Community deployment files and every SDK stay
in the public monorepo.

## Runtime modes

The same released server, worker, agent, and web artifacts serve both operating models.
`MEERKATEER_DEPLOYMENT_MODE` selects policy rather than a different fork:

| Concern | `community` | `cloud` |
| --- | --- | --- |
| Company creation | one-time local bootstrap | authenticated Cloud provisioner |
| Company count | exactly one per installation | many tenants per control plane |
| Workspace/machine count | many within the company | many within each tenant |
| Billing dependency | none | disabled in free beta; separate service later |
| Operations | installation owner | Meerkateer on-call team |
| Core API and storage | public release | the same pinned public release |

Cloud mode must fail closed if its service identity, tenant context, or required isolation
configuration is absent. Community mode must start normally when all Cloud and billing variables
are absent.

## Database ownership

Community uses its own PostgreSQL database or cluster. Sharing a database server is possible for
an experienced operator, but Meerkateer must use a dedicated database, role, credentials, backup,
and resource limits. It must not share tables or a schema with another application.

Cloud separates two data planes:

1. The **core data plane** stores tenant-scoped Meerkateer data. Public migrations exclusively own
   these tables and RLS policies.
2. The **Cloud management plane** stores hosted accounts, provisioning state, regions, quotas,
   support state, and later Stripe identifiers. It uses a separate database or, at minimum, a
   separate schema, role, migration history, and backup policy.

The Cloud provisioner calls a versioned, service-authenticated internal API. It does not insert
directly into core tables. If Cloud needs a new core field or operation, that contract is added to
the public repository and tested there. Cloud migrations never patch public core tables.

RLS is one layer, not the whole boundary. Every tenant-owned relationship uses tenant-aware
foreign keys; transactions set and verify tenant context; queues, caches, object paths, exports,
rate limits, metrics, and support tools carry tenant identity; tests attempt guessed IDs and
cross-tenant reads and writes through every path.

## Release and dependency flow

1. A change enters the public repository and passes contract, integration, tenant, failure,
   migration, and relevant performance tests.
2. A versioned tag, such as `v0.3.0`, produces immutable OCI images, packages, checksums, SBOMs,
   provenance, and signatures.
3. Cloud opens a dependency update that pins both version and digest. It never deploys `latest`, a
   mutable tag, an untagged commit, or a developer working tree.
4. Cloud staging runs cross-tenant, upgrade, rollback, load, backup/restore, and regional-failure
   suites against that exact digest.
5. A canary receives the version before progressive production rollout. Rollback retains database
   compatibility for the published window.
6. The Cloud release records its own deployment identifier, for example `cloud-2026.10.1`, and the
   exact core version and digests it contains.

Security fixes land in public core first whenever disclosure safety permits. Cloud then consumes
the patched artifact. A compatibility matrix records which Cloud deployment supports which core,
agent, protocol, and database schema versions.

## Branch and change policy

- `main` remains releasable; feature branches are short-lived and reviewed.
- Database and wire-contract changes require backward-compatibility tests and a migration note.
- A Cloud change that needs a core capability begins with a public core proposal or ADR.
- Core release candidates are tested by Cloud, but Cloud availability never blocks Community 1.0.
- No production secret, customer data, private hostname, access token, or Stripe credential is
  committed to either repository. CI obtains short-lived credentials from the selected secret
  manager.
- Repository access is not a tenant security boundary. The deployed identities, network policy,
  database roles, encryption, audit trail, and tested authorization rules are the boundary.

## Feature-placement rule

Put a capability in public core when any of these are true:

- it affects whether monitoring evidence is correct, durable, explainable, or secure;
- a self-hosted company can reasonably use it;
- it changes a public protocol, agent, SDK, database table, status, incident, or alert behavior;
- it enforces tenant isolation or least privilege; or
- keeping it private would require a Cloud fork of core behavior.

Put it in the private Cloud repository only when it operates the hosted business itself: global
signup, region placement, hosted fleet administration, provider-wide abuse/cost controls,
internal support tooling, Cloud SLO automation, or billing and tax workflows.

## Boundary acceptance tests

This architecture is considered enforced only when CI proves all of the following:

- Community starts and completes its supported journey with Cloud, billing, and external license
  endpoints unavailable.
- Cloud deploys only pinned, signed public artifacts and rejects an unknown or incompatible core
  schema/version.
- A tenant cannot read, modify, export, alert on, cache, or infer another tenant's objects through
  API, SQL, worker jobs, object storage, metrics, probes, or support tools.
- Core migrations can run without the Cloud management database, and Cloud migrations cannot
  modify core-owned objects.
- A public core security patch can be promoted through Cloud staging and rollback without a
  private source patch.
- Deleting a hosted tenant follows a tested export, retention, erasure, audit, and backup-expiry
  contract without affecting any other tenant.

Any test failure blocks the affected release. A waiver requires a written risk owner, expiry, and
customer-impact statement; tenant isolation and critical/high security findings cannot be waived
for general availability.
