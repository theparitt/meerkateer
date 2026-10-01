# AI runbook: localhost installation and SDK lab

Use this runbook when an AI coding agent must install or test Meerkateer locally. The normal
Community installation and the disposable multi-language SDK lab are separate environments.

## Normal Community installation

### Safety contract

1. Work from the repository root and preserve unrelated working-tree changes.
2. Treat `.env` as secret. Never print, summarize, commit, or place its values in command
   arguments or chat messages.
3. Preserve the normal Compose volumes. Never run `docker compose down --volumes`, delete a named
   volume, or reinitialize PostgreSQL unless the human explicitly requests destructive reset after
   seeing the exact data-loss scope.
4. Keep the default services on loopback ports 6510 and 6511. Do not expose them publicly.
5. Community installation does not require Stripe, a Cloud account, or a license key.

### Deterministic install and verification

```sh
make bootstrap
make migrate
docker compose up -d --build --wait
make smoke
```

Then verify read-only state:

```sh
docker compose ps
curl --fail http://127.0.0.1:6510/ready
curl --fail http://127.0.0.1:6511/
```

Success requires every expected container to be running or healthy, migrations to complete, and
the smoke test plus both HTTP checks to pass. If a check fails, inspect only the relevant service
with `docker compose logs --tail=200 SERVICE`, fix the smallest in-scope cause, and repeat all
verification. Redact any secret that appears unexpectedly.

The human completes access setup at <http://127.0.0.1:6511/login>. **Sign in** is the normal path,
**First setup** creates the company and first owner once, and **Recover** sets or replaces the owner
password. The human must take the setup key directly from their private `.env`; the agent must not
request or echo it.

## Disposable multi-language SDK lab

Use this runbook when an AI coding agent must start, validate, diagnose, or extend the
Meerkateer multi-language SDK lab without harming a user's normal installation.

## Scope and invariants

1. Work from the repository root.
2. Treat `.env` and `dev/.multilang-lab.env` as secrets. Never print, summarize, commit,
   or place their contents in command arguments.
3. Use only Compose projects `meerkateer-sdk-lab-core` and
   `meerkateer-sdk-lab-demos` for this lab.
4. Never run `docker compose down --volumes` without the explicit lab project name.
5. Do not alter the user's normal ports 6510/6511 or its PostgreSQL volume.
6. HTTP is acceptable only because every SDK connects to loopback. Require HTTPS for any
   remote/non-loopback endpoint.

## Deterministic workflow

```sh
make sdk-lab-test
```

Success is the final line:

```text
Multi-language SDK E2E passed: five SDKs, concurrent transitions, stale detection, and recovery.
```

The command must prove all of these assertions, not merely return exit code zero:

- five demo HTTP endpoints become ready on ports 19101-19105;
- Python, Node.js, Go, PHP, and Rust SDKs each deliver authenticated telemetry;
- each service resolves `degraded`, then `offline`, then `online`;
- each timeline contains `demo degraded`, `demo failed`, and `demo recovered` events;
- stopping `node-demo` causes `stale=true` and effective state `unknown` after the
  configured 30-second freshness window;
- restarting Node and sending a healthy observation resolves it to `online`;
- the test trap removes the disposable Compose projects and credential file.

## Interactive diagnosis

Start without automatic cleanup:

```sh
make sdk-lab
make sdk-lab-logs
```

Read-only checks:

```sh
curl --fail http://127.0.0.1:18110/ready
curl --fail http://127.0.0.1:18111/
for port in 19101 19102 19103 19104 19105; do
  curl --fail "http://127.0.0.1:${port}/health"
done
docker compose -p meerkateer-sdk-lab-core ps
docker compose -p meerkateer-sdk-lab-demos -f examples/multi-language/compose.yaml \
  --env-file dev/.multilang-lab.env ps
```

If one demo fails, inspect only that container's logs. Error output must not contain a
value beginning with `mks_sk_`. Fix source or Docker configuration, rebuild that service,
then rerun the complete test so status and failure-detection assertions still hold.

## Extension contract

When adding a language, implement `GET /health` plus the three scenario endpoints, send
a startup deployment, send a heartbeat at least every five seconds in the lab, add the
language to `examples/multi-language/compose.yaml` and `LANGUAGES` in
`tests/integration/multilang_sdk_lab.py`, and add it to both documentation tables. Bind
only to loopback in host-network mode. Keep payloads bounded and operational; never send
user, player, credential, database, or request-body data.

Clean up an interactive run with:

```sh
make sdk-lab-down
```
