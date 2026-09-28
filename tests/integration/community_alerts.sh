#!/usr/bin/env sh
set -eu

project="${MEERKATEER_ALERT_TEST_PROJECT:-meerkateer-alerts-test}"
db_port="${MEERKATEER_ALERT_TEST_DB_PORT:-18543}"
api_port="${MEERKATEER_ALERT_TEST_API_PORT:-18090}"
hook_port="${MEERKATEER_ALERT_TEST_HOOK_PORT:-18091}"
compose_files='-f compose.yaml -f tests/integration/compose.alerts.yaml'

if [ ! -f .env ]; then
    echo 'Run make bootstrap before the alert integration suite.' >&2
    exit 1
fi
set -a
. ./.env
set +a

cleanup() {
    if [ -n "${server_pid:-}" ]; then
        kill "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    fi
    docker compose -p "$project" $compose_files down --volumes --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
cleanup

cargo build --quiet -p meerkateer-server -p meerkateer-worker -p meerkateer-agent
docker compose -p "$project" $compose_files up -d --wait postgres

MEERKATEER_DATABASE_URL="$(printf '%s' "$MEERKATEER_DATABASE_URL" | sed "s/@postgres:5432/@127.0.0.1:${db_port}/")"
MEERKATEER_WORKER_DATABASE_URL="$(printf '%s' "$MEERKATEER_WORKER_DATABASE_URL" | sed "s/@postgres:5432/@127.0.0.1:${db_port}/")"
MEERKATEER_BIND_ADDR="127.0.0.1:${api_port}"
MEERKATEER_ALERT_WEBHOOK_URL="http://127.0.0.1:${hook_port}/hook"
export MEERKATEER_DATABASE_URL MEERKATEER_WORKER_DATABASE_URL MEERKATEER_BIND_ADDR MEERKATEER_ALERT_WEBHOOK_URL
MEERKATEER_E2E_URL="http://127.0.0.1:${api_port}"
MEERKATEER_ALERT_TEST_PROJECT="$project"
MEERKATEER_AGENT_BIN="$(pwd)/target/debug/meerkateer-agent"
export MEERKATEER_E2E_URL MEERKATEER_ALERT_TEST_PROJECT MEERKATEER_AGENT_BIN

target/debug/meerkateer-server &
server_pid=$!
python3 tests/integration/community_alerts.py
echo 'Community alert test, outage/recovery delivery, and deduplication passed.'
