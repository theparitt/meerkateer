# ADR-0003: Separate MKS-1 and outbound-only MKA-1

- Status: accepted
- Date: 2026-09-27
- Owners: maintainers

## Context

Instrumented applications and machine agents have different trust, delivery, and
upgrade requirements. Treating them as one interface either weakens the agent protocol
or burdens simple health endpoints.

## Decision

MKS-1 remains the application pull/push interface. MKA-1 is the agent enrollment,
configuration, heartbeat, telemetry, acknowledgement, and rotation protocol. Agents
make outbound HTTPS connections only and have no generic command channel. Both
protocols are schema-first, versioned, and backed by positive/negative fixtures plus
application semantic validation.

## Consequences

The contracts can evolve independently and consumers can state exact conformance.
Protocol duplication is accepted where it makes a security boundary explicit.

## Alternatives considered

- Reuse MKS-1 for agents: rejected because it lacks enrollment, durable sequencing,
  configuration signing, and offline spool acknowledgements.
- Bidirectional remote-control stream: rejected for `v1.0` because it creates a high
  impact administration backdoor.
