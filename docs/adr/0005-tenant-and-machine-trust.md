# ADR-0005: Tenant authorization and machine credentials

- Status: accepted
- Date: 2026-09-27
- Owners: maintainers

## Context

Telemetry payload identity is attacker-controlled. Shared API keys create excessive
blast radius and cannot support safe revocation or audit.

## Decision

Every human action is authorized against tenant membership and role with deny-by-default
queries. Service keys and agent credentials resolve ownership server-side, are displayed
once, stored only as slow hashes, have non-secret lookup prefixes, and support rotation,
overlap, expiry, and immediate revocation. Enrollment uses single-use short-lived
tokens and creates a unique agent identity. Payload tenant/project fields never grant
ownership.

## Consequences

Cross-tenant property tests and credential lifecycle tests are release gates. Operators
must recover by rotation/re-enrollment rather than reading a stored secret.

## Alternatives considered

- Trust payload tenant IDs: rejected as direct cross-tenant exposure.
- One permanent project token: rejected because compromise affects every machine and
  rotation is disruptive.
