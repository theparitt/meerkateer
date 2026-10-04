# Encrypted backup and restore

Meerkateer backups are PostgreSQL custom-format dumps encrypted locally with
[age](https://age-encryption.org/). A backup is not accepted as recovery evidence until it has
been restored into an isolated installation and the API journey has passed.

## Create an identity once

Keep the identity outside the Meerkateer host and repository. Store an additional protected copy
with the person responsible for recovery.

```sh
age-keygen -o /safe/off-host/meerkateer-backup-key.txt
age-keygen -y /safe/off-host/meerkateer-backup-key.txt
```

The second command prints the public `age1...` recipient. It is safe to place that public value in
a backup job; never copy the private identity into `.env`.

## Back up

```sh
export MEERKATEER_BACKUP_RECIPIENT='age1...'
export MEERKATEER_BACKUP_DIR='/srv/meerkateer-backups'
make backup
```

The command validates the dump before encryption and writes an encrypted dump, SHA-256 file, and
non-secret manifest with owner-only permissions. Copy all three files to off-host object storage or
another protected machine. Configure retention and immutability in that storage provider; the
local script deliberately does not delete old backups.

For the production Compose file, additionally set:

```sh
export MEERKATEER_ENV_FILE='deploy/.env.production'
export MEERKATEER_COMPOSE_FILE='deploy/compose.production.yaml'
```

## Restore rehearsal

Restore is destructive to its selected database. Rehearse first on an isolated host using a copy
of the production environment with different ports and volumes.

```sh
export MEERKATEER_BACKUP_IDENTITY='/safe/off-host/meerkateer-backup-key.txt'
export MEERKATEER_RESTORE_CONFIRM='replace-current-database'
make restore BACKUP='/srv/meerkateer-backups/meerkateer-YYYYMMDDTHHMMSSZ.dump.age'
```

Restore verifies the checksum, decrypts into an owner-only temporary file, validates the archive,
stops API/worker writers, replaces database objects, applies any forward migrations, restarts the
services, and requires `/ready` to pass. Record the start/end time, source revision, target host,
row-count comparison, application smoke result, and operator in the recovery log.

## Failure handling

- Missing/wrong identity, corrupt ciphertext, checksum mismatch, or invalid archive stops before
  changing the database.
- Failure after writers stop requires inspecting `docker compose logs`; do not repeatedly retry an
  unknown partial restore against the only production database.
- Keep the pre-restore safety backup until application and tenant-isolation checks pass.
- Losing the age identity makes the encrypted backup unrecoverable. Meerkateer cannot recover it.
