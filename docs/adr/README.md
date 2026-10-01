# Architecture Decision Records

Architecture Decision Records (ADRs) capture decisions that change a public contract,
security boundary, operational promise, persistent model, or costly dependency.

Statuses are `proposed`, `accepted`, `superseded`, or `rejected`. Superseding an ADR
creates a new record and links both records; accepted history is not rewritten.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-product-boundary.md) | GameOps reliability is the first product boundary | Accepted |
| [0002](0002-monorepo-and-core-stack.md) | Monorepo and core implementation stack | Accepted |
| [0003](0003-service-and-agent-protocols.md) | Separate MKS-1 and outbound-only MKA-1 | Accepted |
| [0004](0004-deployment-profiles-and-storage.md) | Compact and HA deployment profiles | Accepted |
| [0005](0005-tenant-and-machine-trust.md) | Tenant authorization and machine credentials | Accepted |
| [0006](0006-community-and-cloud-billing-boundary.md) | Community/Cloud and Stripe boundary | Accepted |
| [0007](0007-public-core-and-private-cloud-operations.md) | Public core and private Cloud operations repositories | Accepted |
| [0008](0008-bounded-remote-recovery.md) | Bound remote recovery to typed, locally allowlisted actions | Proposed |

Use [0000-template.md](0000-template.md) for new records.
