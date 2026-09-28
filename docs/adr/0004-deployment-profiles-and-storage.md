# ADR-0004: Compact and HA deployment profiles

- Status: accepted
- Date: 2026-09-27
- Owners: maintainers

## Context

A small self-hosted installation values low operational complexity. Meerkateer Cloud
must meet a much larger measured capacity and high-availability envelope. Pretending
one storage topology meets both needs would produce either an unusable Community setup
or an unsafe Cloud setup.

## Decision

Provide two supported profiles behind control, telemetry, queue, and object-storage
ports:

- **Compact:** PostgreSQL-backed control data, jobs, and bounded telemetry for
  development and small self-hosting.
- **HA:** PostgreSQL control data plus a durable stream, columnar/time-series telemetry
  store, and object storage selected by Phase 3 benchmarks.

The concrete HA components remain undecided until representative ingest, query,
retention, recovery, and operator-complexity benchmarks are recorded in a superseding
ADR. The Compact profile never claims the HA capacity envelope.

## Consequences

Domain logic must use explicit ports and conformance tests for both profiles. This adds
implementation work but avoids coupling the product to an unproven high-volume store.

## Alternatives considered

- PostgreSQL for every scale: rejected until it proves the published Cloud envelope.
- Require a distributed stack for all users: rejected because it harms self-hosted
  adoption and recovery.
