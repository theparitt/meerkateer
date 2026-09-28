# ADR-0006: Community/Cloud and Stripe boundary

- Status: accepted
- Date: 2026-09-27
- Owners: maintainers

## Context

The project is Apache-2.0 and must remain useful when self-hosted, while the managed
service needs subscription billing and quotas.

## Decision

One open-source codebase supports explicit `community` and `cloud` deployment modes.
Community starts without Stripe and has no remote license check. Cloud uses Stripe
hosted Checkout, Customer Portal, signed webhooks, and a durable local subscription/
entitlement projection. Billing never authorizes from a browser redirect, and Stripe
event delivery is treated as duplicated and unordered.

Initial plans use fixed monthly node/check/retention quotas rather than metered billing.
Meerkateer names, Cloud contracts, and service SLAs are separate from the software
license.

## Consequences

Every core feature requires a no-Stripe Community path. Cloud needs webhook inbox,
reconciliation, grace/read-only behavior, and Sandbox-to-Live isolation tests.

## Alternatives considered

- Open-core feature removal: rejected for the initial product.
- Usage billing at launch: deferred until real costs and customer expectations are
  measured.
