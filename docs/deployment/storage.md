# PostgreSQL deployment boundary

Meerkateer owns its data model. Agents and monitored workloads never connect to its
database directly; they communicate only with the authenticated Meerkateer API.

## Recommended topology

- **Cloud production:** use a PostgreSQL 18 instance or cluster dedicated to
  Meerkateer. This isolates capacity, failure, maintenance, backup, and recovery.
- **Small self-hosting:** use the PostgreSQL container in the versioned Compose bundle.
- **Shared development or staging server:** a shared PostgreSQL cluster is acceptable,
  but Meerkateer must have a dedicated database and a dedicated login role. Do not
  share its schema owner or credentials with another application.

An existing PostgreSQL server such as `192.168.0.147` is therefore a deployment choice,
not a source-code default. Set `MEERKATEER_DATABASE_URL` through the deployment secret
store. Never commit the address with credentials, place it in an image, or expose port
5432 publicly.

Example administrative setup (replace names and secrets through your secret manager):

```sql
CREATE ROLE meerkateer LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
CREATE DATABASE meerkateer OWNER meerkateer;
REVOKE ALL ON DATABASE meerkateer FROM PUBLIC;
```

Require TLS for traffic crossing hosts, restrict `pg_hba.conf` and the network firewall
to Meerkateer API/worker nodes, and use a separate migration credential if operational
policy permits. Runtime roles must not be superusers. The API and worker use separate
login roles: the API role is tenant-scoped by forced RLS, while the worker can only call
narrow lease/complete/retry/dead-letter functions owned by a `NOLOGIN` executor role.

## Data ownership

The compact profile stores control data, outbox records, and initial telemetry in
PostgreSQL. The HA profile will retain PostgreSQL for control data while moving the
durable stream and high-volume samples behind the storage ports. The exact HA engines
remain an ADR decision backed by load tests; the agent/API contract does not change.

Backups are Meerkateer-specific. A production rollout is not complete until a restore
to a clean PostgreSQL 18 instance is rehearsed and the measured RPO/RTO are recorded.
