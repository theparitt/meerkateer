#!/usr/bin/env sh
set -eu

project="${MEERKATEER_INTEGRATION_PROJECT:-meerkateer-integration}"
api_port="${MEERKATEER_INTEGRATION_API_PORT:-18088}"

if [ ! -f .env ]; then
    echo 'Run make bootstrap before the integration suite.' >&2
    exit 1
fi

set -a
. ./.env
set +a

# The baseline suite exercises Community without an alert destination.
MEERKATEER_ALERT_WEBHOOK_URL=
export MEERKATEER_ALERT_WEBHOOK_URL

cleanup() {
    docker compose -p "$project" down --volumes --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

cleanup
cargo build --quiet -p meerkateer-agent
cargo build --quiet -p meerkateer-sdk --example send_event
test -x target/debug/meerkateer-agent
test -x target/debug/examples/send_event
fleet_stale_after_seconds="${MEERKATEER_FLEET_STALE_AFTER_SECONDS:-30}"
MEERKATEER_API_PORT="$api_port" \
MEERKATEER_HEARTBEAT_STALE_AFTER_SECONDS="$fleet_stale_after_seconds" \
docker compose -p "$project" up -d --build postgres server
ready_attempt=0
until curl --fail --silent --max-time 2 "http://127.0.0.1:${api_port}/ready" >/dev/null; do
    ready_attempt=$((ready_attempt + 1))
    if [ "$ready_attempt" -ge 30 ]; then
        docker compose -p "$project" logs server >&2
        echo 'integration API did not become ready' >&2
        exit 1
    fi
    sleep 1
done

# The security limiter must be shared by independent application sessions while its pseudonymous
# peer table remains unreadable to the runtime role.
rate_limit_sql="SELECT meerkateer_consume_rate_limit('authentication', decode(repeat('ab', 32), 'hex'), 1);"
distributed_first="$(printf '%s\n' "$rate_limit_sql" | docker compose -p "$project" exec -T \
    -e "PGPASSWORD=$MEERKATEER_APP_PASSWORD" postgres \
    psql -h 127.0.0.1 -U meerkateer_app -d "$POSTGRES_DB" -At -v ON_ERROR_STOP=1)"
distributed_second="$(printf '%s\n' "$rate_limit_sql" | docker compose -p "$project" exec -T \
    -e "PGPASSWORD=$MEERKATEER_APP_PASSWORD" postgres \
    psql -h 127.0.0.1 -U meerkateer_app -d "$POSTGRES_DB" -At -v ON_ERROR_STOP=1)"
test "$distributed_first" = t
test "$distributed_second" = f
if printf '%s\n' 'SELECT peer_digest FROM distributed_rate_limits;' \
    | docker compose -p "$project" exec -T -e "PGPASSWORD=$MEERKATEER_APP_PASSWORD" postgres \
        psql -h 127.0.0.1 -U meerkateer_app -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 \
        >/dev/null 2>&1; then
    echo 'application login unexpectedly has direct distributed limiter table access' >&2
    exit 1
fi
docker compose -p "$project" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c 'TRUNCATE distributed_rate_limits' >/dev/null

MEERKATEER_E2E_URL="http://127.0.0.1:${api_port}" \
MEERKATEER_E2E_PROJECT="$project" \
MEERKATEER_E2E_DB="$POSTGRES_DB" \
MEERKATEER_E2E_DB_OWNER="$POSTGRES_USER" \
MEERKATEER_AGENT_BIN="$(pwd)/target/debug/meerkateer-agent" \
MEERKATEER_RUST_SDK_BIN="$(pwd)/target/debug/examples/send_event" \
python3 tests/integration/api_e2e.py

./tests/integration/upgrade_from_0007.sh

docker compose -p "$project" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    < tests/integration/prepare_outbox_worker.sql

# The server and worker share one Dockerfile but Compose gives each service its
# own image tag. Build the worker explicitly so a previous local test image can
# never hide worker source changes.
docker compose -p "$project" build worker

if printf '%s\n' 'SELECT count(*) FROM outbox;' \
    | docker compose -p "$project" exec -T \
        -e "PGPASSWORD=$MEERKATEER_WORKER_PASSWORD" postgres \
        psql -h 127.0.0.1 -U meerkateer_worker -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 \
        >/dev/null 2>&1; then
    echo 'worker login unexpectedly has direct outbox table access' >&2
    exit 1
fi
if printf '%s\n' 'SELECT count(*) FROM worker_runtime;' \
    | docker compose -p "$project" exec -T \
        -e "PGPASSWORD=$MEERKATEER_WORKER_PASSWORD" postgres \
        psql -h 127.0.0.1 -U meerkateer_worker -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 \
        >/dev/null 2>&1; then
    echo 'worker login unexpectedly has direct worker progress table access' >&2
    exit 1
fi

docker compose -p "$project" run --rm --no-deps worker --once
docker compose -p "$project" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    < tests/integration/assert_e2e.sql

docker compose -p "$project" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    < tests/integration/seed_second_tenant.sql

# Reset the distributed authentication budget exhausted by the abuse-control case. Restarting the
# API alone deliberately does not clear it, so the integration owner resets only this disposable
# test stack before the second-company login journey.
docker compose -p "$project" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c 'TRUNCATE distributed_rate_limits' >/dev/null

# Prove the shared Cloud-ready core cannot cross company boundaries, then exercise a concurrent
# real-agent fleet against the same isolated database.
docker compose -p "$project" restart server >/dev/null
until curl --fail --silent --max-time 2 "http://127.0.0.1:${api_port}/ready" >/dev/null; do
    sleep 1
done
MEERKATEER_E2E_URL="http://127.0.0.1:${api_port}" \
python3 tests/integration/tenant_isolation.py

MEERKATEER_E2E_URL="http://127.0.0.1:${api_port}" \
MEERKATEER_AGENT_BIN="$(pwd)/target/debug/meerkateer-agent" \
MEERKATEER_FLEET_STALE_AFTER_SECONDS="$fleet_stale_after_seconds" \
python3 tests/integration/fleet_e2e.py

docker compose -p "$project" run --rm --no-deps worker --once
docker compose -p "$project" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    < tests/integration/assert_fleet.sql

echo 'Meerkateer API, PostgreSQL, worker, DLQ, and concurrent fleet integration passed.'
