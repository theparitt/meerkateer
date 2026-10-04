#!/usr/bin/env sh
set -eu

encrypted="${1:-}"
identity="${MEERKATEER_BACKUP_IDENTITY:-}"
env_file="${MEERKATEER_ENV_FILE:-.env}"
compose_file="${MEERKATEER_COMPOSE_FILE:-compose.yaml}"

if [ -z "$encrypted" ] || [ ! -f "$encrypted" ]; then
    echo 'Usage: MEERKATEER_BACKUP_IDENTITY=/safe/key.txt MEERKATEER_RESTORE_CONFIRM=replace-current-database scripts/restore-encrypted.sh <backup.dump.age>' >&2
    exit 2
fi
if [ "${MEERKATEER_RESTORE_CONFIRM:-}" != 'replace-current-database' ]; then
    echo 'Restore replaces the current database. Set MEERKATEER_RESTORE_CONFIRM=replace-current-database after taking a safety backup.' >&2
    exit 2
fi
if [ -z "$identity" ] || [ ! -f "$identity" ]; then
    echo 'MEERKATEER_BACKUP_IDENTITY must name a readable age identity file.' >&2
    exit 2
fi
for command_name in age curl docker sha256sum; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "$command_name is required for restore." >&2
        exit 1
    fi
done
if [ ! -f "$env_file" ] || [ ! -f "$compose_file" ]; then
    echo 'The selected environment or Compose file does not exist.' >&2
    exit 1
fi
if [ ! -f "$encrypted.sha256" ]; then
    echo "Missing checksum: $encrypted.sha256" >&2
    exit 1
fi

set -a
# shellcheck disable=SC1090
. "$env_file"
set +a
: "${POSTGRES_USER:?POSTGRES_USER is missing from the environment file}"
: "${POSTGRES_DB:?POSTGRES_DB is missing from the environment file}"

compose() {
    docker compose --env-file "$env_file" -f "$compose_file" "$@"
}

backup_dir="$(dirname "$encrypted")"
backup_name="$(basename "$encrypted")"
(
    cd "$backup_dir"
    sha256sum --check "${backup_name}.sha256"
)

umask 077
plain="$(mktemp "${TMPDIR:-/tmp}/meerkateer-restore.XXXXXX")"
cleanup() {
    rm -f -- "$plain"
}
trap cleanup EXIT INT TERM HUP

age --decrypt --identity "$identity" --output "$plain" "$encrypted"
compose up -d --wait postgres
compose exec -T postgres pg_restore --list < "$plain" >/dev/null
compose stop server worker >/dev/null 2>&1 || true
compose exec -T postgres pg_restore \
    --username "$POSTGRES_USER" \
    --dbname "$POSTGRES_DB" \
    --clean \
    --if-exists \
    --no-owner \
    --exit-on-error \
    < "$plain"

MEERKATEER_ENV_FILE="$env_file" MEERKATEER_COMPOSE_FILE="$compose_file" \
    ./scripts/migrate-existing.sh
compose up -d --wait server worker

api_port="${MEERKATEER_API_PORT:-6510}"
if ! curl --fail --silent --max-time 5 "http://127.0.0.1:${api_port}/ready" >/dev/null; then
    echo 'Database restored, but the API readiness check failed. Inspect Compose logs before accepting the restore.' >&2
    exit 1
fi
echo "Restore completed and API readiness passed from $encrypted."
