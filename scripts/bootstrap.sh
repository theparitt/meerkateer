#!/usr/bin/env sh
set -eu

if ! command -v openssl >/dev/null 2>&1; then
    echo 'openssl is required to generate local development secrets.' >&2
    exit 1
fi

if [ -f .env ]; then
    env_changed=0
    if grep -q '^MEERKATEER_WEB_PORT=5173$' .env \
        || grep -q '^MEERKATEER_WEB_PORT=15173$' .env; then
        sed -i \
            -e 's/^MEERKATEER_WEB_PORT=5173$/MEERKATEER_WEB_PORT=6511/' \
            -e 's/^MEERKATEER_WEB_PORT=15173$/MEERKATEER_WEB_PORT=6511/' \
            .env
        chmod 600 .env
        echo 'Migrated the legacy web host port to 6511 in .env.'
        env_changed=1
    fi

    if grep -q '^MEERKATEER_API_PORT=8080$' .env; then
        sed -i 's/^MEERKATEER_API_PORT=8080$/MEERKATEER_API_PORT=6510/' .env
        chmod 600 .env
        echo 'Migrated the legacy API host port from 8080 to 6510 in .env.'
        env_changed=1
    fi

    if ! grep -q '^MEERKATEER_WORKER_PASSWORD=' .env \
        || ! grep -q '^MEERKATEER_WORKER_DATABASE_URL=' .env; then
        worker_password="$(openssl rand -hex 24)"
        umask 077
        {
            printf 'MEERKATEER_WORKER_PASSWORD=%s\n' "$worker_password"
            printf 'MEERKATEER_WORKER_DATABASE_URL=postgres://meerkateer_worker:%s@postgres:5432/meerkateer\n' "$worker_password"
        } >> .env
        echo 'Added a separate worker database credential to .env.'
        env_changed=1
    fi

    if [ "$env_changed" -eq 0 ]; then
        echo '.env already exists; leaving it unchanged.'
    fi
    exit 0
fi

db_password="$(openssl rand -hex 24)"
metrics_token="$(openssl rand -hex 24)"
app_password="$(openssl rand -hex 24)"
worker_password="$(openssl rand -hex 24)"
bootstrap_token="$(openssl rand -hex 32)"
umask 077
{
    printf 'POSTGRES_DB=meerkateer\n'
    printf 'POSTGRES_USER=meerkateer_owner\n'
    printf 'POSTGRES_PASSWORD=%s\n' "$db_password"
    printf 'MEERKATEER_APP_PASSWORD=%s\n' "$app_password"
    printf 'MEERKATEER_DATABASE_URL=postgres://meerkateer_app:%s@postgres:5432/meerkateer\n' "$app_password"
    printf 'MEERKATEER_WORKER_PASSWORD=%s\n' "$worker_password"
    printf 'MEERKATEER_WORKER_DATABASE_URL=postgres://meerkateer_worker:%s@postgres:5432/meerkateer\n' "$worker_password"
    printf 'MEERKATEER_DATABASE_MAX_CONNECTIONS=10\n'
    printf 'MEERKATEER_AUTH_RATE_LIMIT_PER_MINUTE=300\n'
    printf 'MEERKATEER_INGEST_RATE_LIMIT_PER_MINUTE=1200\n'
    printf 'MEERKATEER_HEARTBEAT_STALE_AFTER_SECONDS=180\n'
    printf 'MEERKATEER_BOOTSTRAP_TOKEN=%s\n' "$bootstrap_token"
    printf 'MEERKATEER_DEPLOYMENT_MODE=community\n'
    printf 'MEERKATEER_STORAGE_PROFILE=compact\n'
    printf 'MEERKATEER_BIND_ADDR=0.0.0.0:8080\n'
    printf 'MEERKATEER_API_PORT=6510\n'
    printf 'MEERKATEER_WEB_PORT=6511\n'
    printf 'MEERKATEER_SERVICE_ENVIRONMENT=development\n'
    printf 'MEERKATEER_METRICS_ENABLED=true\n'
    printf 'MEERKATEER_METRICS_TOKEN=%s\n' "$metrics_token"
    printf 'RUST_LOG=info\n'
} > .env
echo 'Created .env with random local-only credentials.'
