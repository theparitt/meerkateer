# ADR-0008: Remote recovery must be typed, locally allowlisted, and outbound-only

- Status: proposed
- Date: 2026-10-01
- Owners: maintainers

## Context

Operators want to restart a failed game or SME service from the Meerkateer Console. Turning the host
agent into a general remote shell would make compromise of a browser session or control plane a
direct compromise of every enrolled machine. MKA-1 is intentionally telemetry-only and agents open
no inbound port.

## Proposed decision

MKA-1 remains telemetry-only. A future, version-negotiated MKA-2 extension may offer one initial
action: `service.restart`. It must preserve outbound-only networking and all of these constraints:

- a local administrator maps an opaque target ID to one exact systemd unit or Windows service;
  the server cannot supply a command, executable, path, arguments, or service name;
- the agent runs without administrator/root privileges and receives narrowly scoped permission to
  restart only each allowlisted target;
- an action envelope binds action ID, tenant, workspace, agent, target, action type, issue time,
  expiry, nonce, and policy version; it is signed by a pinned control-plane key;
- the agent rejects invalid signatures, mismatched identity, replay, expired actions, unknown
  targets, unsupported versions, overlapping execution, and local cooldown violations;
- every request and transition (`queued`, `delivered`, `accepted`, `succeeded`, `failed`, `expired`,
  or `rejected`) is durable, idempotent, tenant-scoped, and append-only audited;
- only an authorized owner/admin can request an action, with recent reauthentication, CSRF defense,
  explicit confirmation, rate limits, and no bulk-selection default;
- one bounded action runs at a time with a deadline and cooldown. Automatic retry or restart loops
  are disabled by default; local policy can always deny remote recovery; and
- shell execution, scripts, arbitrary arguments, file transfer, interactive sessions, host reboot,
  account management, and security-control changes are not part of this protocol.

The Console may show host telemetry now, but it must not display an enabled restart control until
the server, agent, and platform privilege boundary all satisfy the gates below.

## Release gates

1. Multi-user OIDC/RBAC and recent-auth evidence are complete.
2. Signed desired configuration and action-key rotation are complete.
3. Linux systemd and Windows service permissions are proven to cover only the configured target.
4. Cross-tenant, replay, clock-skew, duplicate-delivery, stale-action, offline-agent, timeout,
   cooldown, privilege-denial, service-already-stopped, and crash-during-restart tests pass.
5. The UI shows exact actor, target, expiry, current state, result, and audit link without optimistic
   success; accessibility and destructive-action confirmation tests pass.
6. An independent security assessment has no unresolved critical/high finding in this boundary.

## Consequences

Remote restart is deliberately unavailable in the current preview. Linux and Windows agents can be
installed and monitored across a network without granting the control plane execution authority.
Recovery arrives only after its narrower privilege and audit model is demonstrably safer than a
general remote-management agent.

## Alternatives considered

- Inbound SSH/WinRM/RCON from the control plane: rejected because it requires reachable management
  ports and transfers machine-level credentials into the control plane.
- Server-supplied command strings: rejected because validation eventually becomes a remote shell.
- Running the agent permanently as root/SYSTEM: rejected because a collector compromise would gain
  full host privileges.
