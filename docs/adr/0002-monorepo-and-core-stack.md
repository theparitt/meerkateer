# ADR-0002: Monorepo and core implementation stack

- Status: accepted
- Date: 2026-09-27
- Owners: maintainers

## Context

The agent and control plane share protocol types and safety rules. The web console needs
a stable generated API boundary. The project must support reproducible self-hosting and
a managed Cloud deployment.

## Decision

Use one repository containing a Rust workspace for API, worker, agent, protocol, domain,
and infrastructure crates; a React/TypeScript web application; JSON Schema contracts;
and deployment assets. Use Actix-web/Tokio for HTTP and async work, SQLx with PostgreSQL
for control data, and OpenAPI-generated web clients.

Dependencies are locked. Domain crates do not import web frameworks, Stripe, or concrete
telemetry stores. The agent shares protocol/model crates but never server-domain or
database crates.

## Consequences

One change can test protocol producer and consumer together. The repository has a
larger CI matrix and must prevent accidental feature coupling through crate/package
boundaries.

## Alternatives considered

- Multiple repositories: deferred until independent release cadence creates proven
  value.
- Node.js control plane: viable, but Rust reduces runtime diversity with the agent and
  supports bounded parsers and low-overhead workers.
