# Meerkateer Agent Protocol v1 (MKA-1)

- **Standard name:** Meerkateer Agent Protocol v1
- **Short name:** MKA-1
- **Protocol identifier:** `meerkateer-agent`
- **Protocol version:** `1`
- **Status:** Draft
- **Owner:** Meerkateer

MKA-1 defines the security and delivery contract between a Meerkateer Agent and a
Meerkateer control plane. It complements MKS-1: MKS-1 is implemented by applications,
while MKA-1 is implemented by the host agent.

Conformance keywords (MUST, MUST NOT, SHOULD, MAY) follow RFC 2119. All timestamps are
RFC 3339 UTC strings ending in uppercase `Z`.

## 1. Goals and non-goals

MKA-1 provides:

- one-time enrollment and a unique tenant-bound agent identity;
- capability negotiation and desired configuration;
- heartbeat and local spool health;
- bounded, idempotent telemetry batches with explicit acknowledgement; and
- independent credential rotation and revocation.

MKA-1 does not provide arbitrary shell execution, RCON, file management, machine power
control, or an inbound control socket. Adding remote action semantics requires a new
threat model and a compatible protocol extension or new major version.

## 2. Transport and authentication

- Production transport MUST be HTTPS with TLS 1.2 or newer.
- The agent initiates every connection. It MUST NOT listen on a network port.
- Server certificates MUST be verified. A custom CA MAY be configured explicitly; a
  global insecure/skip-verification mode is forbidden in production mode.
- Every request after enrollment uses `Authorization: Bearer <AGENT_CREDENTIAL>`.
- Agent credentials are unique per agent, scoped to agent operations, rotatable,
  revocable, randomly generated, and stored server-side only as a slow hash.
- Enrollment tokens are distinct from agent credentials, single-use, short-lived, and
  resolve tenant/project/environment ownership server-side.
- Neither token type may appear in a URL, JSON body, response after initial enrollment,
  log, metric, trace, support bundle, or audit detail.
- Request bodies are JSON UTF-8. Servers MUST reject a body over the endpoint limit
  before fully buffering it.

The initial credential is returned exactly once by the enrollment response. The agent
MUST store it using operating-system secret storage where available, otherwise in a
root/Administrator-only file. A future additive revision may negotiate mutual TLS.

## 3. Identifiers, limits, and headers

- `agent_id`, `instance_id`, `batch_id`, `record_id`, and `check_id` are UUID strings.
- `instance_id` is generated once by the installation and retained across restarts.
- `agent_id` is assigned by the server and retained across credential rotations.
- Names are 1-64 characters using `A-Z`, `a-z`, `0-9`, dot, underscore, and hyphen.
- A telemetry body is at most 256 KiB and contains at most 1,000 records unless the
  enrollment response advertises a smaller limit.
- An agent configuration contains at most 256 checks.
- Attribute maps contain at most 16 low-cardinality entries. Keys are lowercase
  dot-separated identifiers and values are at most 128 UTF-8 characters.

Every authenticated request SHOULD include a random `X-Request-Id`. Enrollment and
credential rotation require an `Idempotency-Key`. The server MUST echo or generate a
safe request ID in responses without trusting it as authorization input.

## 4. Enrollment

```
POST /v1/agent/enroll
Authorization: Bearer <ONE_TIME_ENROLLMENT_TOKEN>
Idempotency-Key: <INSTANCE_ID>
Content-Type: application/json
```

The request validates against `schemas/mka-1/enroll-request.schema.json`. Ownership is
resolved only from the enrollment token. Display name, platform, and capabilities are
descriptive and never override server-side ownership.

The response validates against `schemas/mka-1/enroll-response.schema.json`. The
`credential` is secret and is shown once. `config_signing_public_key` is a base64
Ed25519 public key used to authenticate desired configuration bytes.

An enrollment retry with the same token and idempotency key MUST return the original
successful result while that result is securely recoverable. The same token with a
different instance ID MUST fail. An expired, revoked, or already-consumed token returns
the same non-enumerating `unauthorized` error.

## 5. Check-in

```
POST /v1/agent/check-in
Authorization: Bearer <AGENT_CREDENTIAL>
Content-Type: application/json
```

The request validates against `schemas/mka-1/check-in.schema.json`. It reports agent
version, supported capabilities, applied configuration revision, collector state, and
bounded spool health. Check-in MUST NOT contain raw collector errors; collector error
values use the MKS-1 safe enum.

The server returns `204` when configuration is current or `200` with the desired
configuration when it has changed. A server MAY return `429` with `Retry-After`. Agents
MUST add jitter and honor backoff without stopping local collection.

## 6. Desired configuration

Configuration validates against `schemas/mka-1/config.schema.json`. It contains no
credential values. `secret_ref` is only a local alias resolved by the agent; the server
does not receive the referenced secret.

The response includes:

```
ETag: "<revision>"
Content-Digest: sha-256=:<base64-digest>:
Meerkateer-Config-Key-Id: <key-id>
Meerkateer-Config-Signature: <base64-ed25519-signature>
```

The signature is calculated over the exact response-body bytes. An agent MUST verify
the digest, key ID, signature, matching `agent_id`, increasing revision, issue/expiry
time, and schema before applying a configuration. Failure keeps the last-known-good
configuration and emits a safe local diagnostic. Configuration rollback requires an
explicit server rollback marker and remains fully audited.

Targets received from Meerkateer Cloud remain subject to local allow/deny policy. A
configuration cannot disable TLS verification or enable a generic command.

## 7. Telemetry batches

```
POST /v1/agent/telemetry
Authorization: Bearer <AGENT_CREDENTIAL>
Idempotency-Key: <BATCH_ID>
Content-Type: application/json
```

The body validates against `schemas/mka-1/telemetry.schema.json`. Sequence numbers are
strictly increasing for an agent and never reused. `first_sequence` and `last_sequence`
match the first and last record. A retry uses the same batch ID, record IDs, sequences,
and body.

Samples are numeric, low-cardinality measurements. Events describe a bounded state
change. Allowed examples include:

```
host.cpu.utilization          host.memory.used_bytes
filesystem.used_ratio        process.up
game.query.latency_ms         game.players.current
game.players.max              agent.spool.queued_records
```

Telemetry MUST NOT contain player names or identifiers, chat, IP addresses, request
bodies, database queries/rows, file contents, credentials, or unbounded dynamic labels.
Raw collection errors map to `unavailable`, `timeout`, `misconfigured`, `disabled`, or
`unknown` before leaving the host.

## 8. Acknowledgement and retry

Responses validate against `schemas/mka-1/telemetry-ack.schema.json`.

- `accepted`: every record through `accepted_through_sequence` is durable.
- `partial`: records through the acknowledgement are durable and listed later records
  are rejected with stable reasons.
- `rejected`: no record is accepted.

The server MUST NOT acknowledge beyond durably accepted data. The agent removes spool
records only after acknowledgement. Transport failure, `408`, `429`, and `5xx` are
retryable with exponential backoff and jitter. Schema, unsupported version, revoked
credential, and permanently oversized record errors are not retried forever; they are
visible in local diagnostics and the server operator workflow where authenticated.

Servers deduplicate by `(agent_id, record_id)` and `(agent_id, batch_id)`. Replaying an
accepted batch returns the original acknowledgement without duplicating state or alerts.

## 9. Credential rotation and revocation

An authenticated agent rotates its credential before expiry using an idempotent
rotation request. The server returns a new credential once and permits a short overlap
window. Successful use of the new credential SHOULD end the overlap early. Revocation
is immediate and is recorded in the administrative audit log.

A lost credential cannot be recovered; the operator revokes the agent and re-enrolls or
uses an audited recovery flow. Cloned credentials and impossible concurrent identity
changes trigger a security signal but MUST NOT leak tenant information to the caller.

## 10. Compatibility

- `protocol_version` is the string `"1"`.
- Breaking changes create MKA-2.
- Additive fields may be added to MKA-1; consumers ignore unknown response fields.
- Servers reject unsupported major versions with a stable `unsupported_version` error.
- A Stable server supports the current and previous two Stable agent minor versions.
- Capability negotiation, not version string comparison alone, controls optional
  collectors and features.

## 11. Normative schemas

- `schemas/mka-1/enroll-request.schema.json`
- `schemas/mka-1/enroll-response.schema.json`
- `schemas/mka-1/check-in.schema.json`
- `schemas/mka-1/config.schema.json`
- `schemas/mka-1/telemetry.schema.json`
- `schemas/mka-1/telemetry-ack.schema.json`

MKA-1 remains Draft until the agent and server pass the conformance, offline recovery,
credential rotation, malformed-input, compatibility, and upgrade/rollback test suites.
