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
MEERKATEER_API_PORT="$api_port" docker compose -p "$project" up -d --build postgres server
MEERKATEER_E2E_URL="http://127.0.0.1:${api_port}" \
MEERKATEER_AGENT_BIN="$(pwd)/target/debug/meerkateer-agent" \
MEERKATEER_RUST_SDK_BIN="$(pwd)/target/debug/examples/send_event" \
python3 tests/integration/api_e2e.py

./tests/integration/upgrade_from_0007.sh

docker compose -p "$project" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    < tests/integration/prepare_outbox_worker.sql

if printf '%s\n' 'SELECT count(*) FROM outbox;' \
    | docker compose -p "$project" exec -T \
        -e "PGPASSWORD=$MEERKATEER_WORKER_PASSWORD" postgres \
        psql -h 127.0.0.1 -U meerkateer_worker -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 \
        >/dev/null 2>&1; then
    echo 'worker login unexpectedly has direct outbox table access' >&2
    exit 1
fi

docker compose -p "$project" run --rm --no-deps worker --once
docker compose -p "$project" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    < tests/integration/assert_e2e.sql

echo 'Meerkateer API, PostgreSQL, worker retry, and dead-letter integration passed.'
