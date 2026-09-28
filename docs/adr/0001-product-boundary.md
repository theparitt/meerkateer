# ADR-0001: GameOps reliability is the first product boundary

- Status: accepted
- Date: 2026-09-27
- Owners: maintainers

## Context

Generic SME monitoring is crowded and difficult to explain, while serious game-server
operators have a concrete need to distinguish host, process, network, game-query,
dependency, and deployment failures. The same reliability core can later serve MSPs.

## Decision

The first market and onboarding path is Game Server reliability. The system monitors
and alerts; `v1.0` does not provision game instances or expose RCON, remote shell, file
management, raw chat, or player identity. BusinessOps/MSP is a later solution pack on
the same control plane and agent, not a fork.

## Consequences

Initial adapters prioritize host/process/Docker, Minecraft, Steam A2S, MKS-1, Discord,
and read-only integration with existing control panels. This reduces remote-action risk
and gives the product one testable promise. It does not eliminate the need for generic
contracts or tenant isolation.

## Alternatives considered

- Generic monitoring first: rejected because differentiation and buyer are unclear.
- Full game-hosting panel: rejected because mature panels exist and remote control
  greatly expands security and support scope.
