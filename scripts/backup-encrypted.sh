#!/usr/bin/env sh
set -eu

backup_dir="${MEERKATEER_BACKUP_DIR:-backups}"
recipient="${MEERKATEER_BACKUP_RECIPIENT:-}"
env_file="${MEERKATEER_ENV_FILE:-.env}"
compose_file="${MEERKATEER_COMPOSE_FILE:-compose.yaml}"

if [ -z "$recipient" ]; then
    echo 'MEERKATEER_BACKUP_RECIPIENT is required (for example age1...).' >&2
    exit 2
fi
for command_name in age docker sha256sum; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "$command_name is required for an encrypted backup." >&2
        exit 1
    fi
done
if [ ! -f "$env_file" ] || [ ! -f "$compose_file" ]; then
    echo 'The selected environment or Compose file does not exist.' >&2
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

umask 077
mkdir -p "$backup_dir"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
base="meerkateer-${timestamp}"
plain="$(mktemp "$backup_dir/.${base}.plain.XXXXXX")"
partial="$backup_dir/.${base}.dump.age.partial"
encrypted="$backup_dir/${base}.dump.age"
checksum="$encrypted.sha256"
manifest="$backup_dir/${base}.manifest"

cleanup() {
    rm -f -- "$plain" "$partial"
}
trap cleanup EXIT INT TERM HUP

if [ -e "$encrypted" ] || [ -e "$checksum" ] || [ -e "$manifest" ]; then
    echo "Refusing to replace an existing backup named $base." >&2
    exit 1
fi

compose exec -T postgres pg_dump \
    --username "$POSTGRES_USER" \
    --dbname "$POSTGRES_DB" \
    --format=custom \
    --no-owner \
    > "$plain"
compose exec -T postgres pg_restore --list < "$plain" >/dev/null
age --recipient "$recipient" --output "$partial" "$plain"
mv -- "$partial" "$encrypted"

backup_name="$(basename "$encrypted")"
(
    cd "$backup_dir"
    sha256sum "$backup_name" > "${backup_name}.sha256"
)
revision="$(git rev-parse --verify HEAD 2>/dev/null || printf unknown)"
{
    printf 'format=meerkateer-postgresql-custom-age-v1\n'
    printf 'created_at=%s\n' "$timestamp"
    printf 'database=%s\n' "$POSTGRES_DB"
    printf 'schema_through=0018\n'
    printf 'source_revision=%s\n' "$revision"
    printf 'encrypted_file=%s\n' "$backup_name"
} > "$manifest"

echo "Encrypted backup created: $encrypted"
echo "Copy the .dump.age, .sha256, and .manifest files to separate off-host storage."
