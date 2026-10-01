#!/usr/bin/env sh
set -eu

destination="${MEERKATEER_PRODUCTION_ENV:-deploy/.env.production}"
version="${1:-}"
case "$version" in
    ''|*[!A-Za-z0-9._-]*|[.-]*|*..*)
    echo 'Usage: scripts/bootstrap-production-env.sh <published-image-tag>' >&2
    exit 2
    ;;
esac
if [ "${#version}" -gt 128 ]; then
    echo 'Published image tag must be at most 128 characters.' >&2
    exit 2
fi
if [ -e "$destination" ]; then
    echo "$destination already exists; leaving it unchanged." >&2
    exit 1
fi
if ! command -v openssl >/dev/null 2>&1; then
    echo 'openssl is required to generate production credentials.' >&2
    exit 1
fi

database_password="$(openssl rand -hex 24)"
app_password="$(openssl rand -hex 24)"
worker_password="$(openssl rand -hex 24)"
metrics_token="$(openssl rand -hex 32)"
bootstrap_token="$(openssl rand -hex 32)"

umask 077
destination_directory="$(dirname "$destination")"
if [ ! -d "$destination_directory" ]; then
    echo "Destination directory does not exist: $destination_directory" >&2
    exit 1
fi
{
    printf 'COMPOSE_PROJECT_NAME=meerkateer-production\n'
    printf 'MEERKATEER_SERVER_IMAGE=ghcr.io/theparitt/meerkateer-server\n'
    printf 'MEERKATEER_WORKER_IMAGE=ghcr.io/theparitt/meerkateer-worker\n'
    printf 'MEERKATEER_IMAGE_TAG=%s\n' "$version"
    printf 'POSTGRES_DB=meerkateer\n'
    printf 'POSTGRES_USER=meerkateer_owner\n'
    printf 'POSTGRES_PASSWORD=%s\n' "$database_password"
    printf 'MEERKATEER_APP_PASSWORD=%s\n' "$app_password"
    printf 'MEERKATEER_WORKER_PASSWORD=%s\n' "$worker_password"
    printf 'MEERKATEER_DATABASE_URL=postgres://meerkateer_app:%s@postgres:5432/meerkateer\n' "$app_password"
    printf 'MEERKATEER_WORKER_DATABASE_URL=postgres://meerkateer_worker:%s@postgres:5432/meerkateer\n' "$worker_password"
    printf 'MEERKATEER_DATABASE_MAX_CONNECTIONS=20\n'
    printf 'MEERKATEER_DEPLOYMENT_MODE=community\n'
    printf 'MEERKATEER_STORAGE_PROFILE=compact\n'
    printf 'MEERKATEER_SERVICE_ENVIRONMENT=production\n'
    printf 'MEERKATEER_METRICS_ENABLED=true\n'
    printf 'MEERKATEER_METRICS_TOKEN=%s\n' "$metrics_token"
    printf 'MEERKATEER_BOOTSTRAP_TOKEN=%s\n' "$bootstrap_token"
    printf 'MEERKATEER_AUTH_RATE_LIMIT_PER_MINUTE=300\n'
    printf 'MEERKATEER_INGEST_RATE_LIMIT_PER_MINUTE=1200\n'
    printf 'MEERKATEER_HEARTBEAT_STALE_AFTER_SECONDS=180\n'
    printf 'MEERKATEER_ALERT_WEBHOOK_URL=\n'
    printf 'MEERKATEER_API_PORT=6510\n'
    printf 'RUST_LOG=info\n'
} > "$destination"
chmod 0600 "$destination"
echo "Created $destination with owner-only permissions."
