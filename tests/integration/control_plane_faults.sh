#!/usr/bin/env sh
set -eu

project="${MEERKATEER_CONTROL_TEST_PROJECT:-meerkateer-control-test}"
api_port="${MEERKATEER_CONTROL_TEST_API_PORT:-18095}"
web_port="${MEERKATEER_CONTROL_TEST_WEB_PORT:-18096}"

if [ ! -f .env ]; then
    echo 'Run make bootstrap before the control-plane fault test.' >&2
    exit 1
fi
set -a
. ./.env
set +a
MEERKATEER_API_PORT="$api_port"
MEERKATEER_WEB_PORT="$web_port"
MEERKATEER_ALERT_WEBHOOK_URL=
export MEERKATEER_API_PORT MEERKATEER_WEB_PORT MEERKATEER_ALERT_WEBHOOK_URL

cleanup() {
    docker compose -p "$project" down --volumes --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
cleanup

http_code() {
    curl --silent --output /dev/null --max-time 8 --write-out '%{http_code}' "$1" || true
}

wait_code() {
    address="$1"
    expected="$2"
    for _ in $(seq 1 20); do
        actual="$(http_code "$address")"
        case "$expected:$actual" in
            200:200|5xx:5??) return 0 ;;
        esac
        sleep 1
    done
    echo "Expected $expected at $address; last HTTP status was $actual" >&2
    return 1
}

docker compose -p "$project" up -d --build --wait postgres server web worker
wait_code "http://127.0.0.1:${api_port}/ready" 200
wait_code "http://127.0.0.1:${web_port}/v1/instance" 200
echo 'Control plane baseline ready.'

docker compose -p "$project" stop server >/dev/null
wait_code "http://127.0.0.1:${web_port}/v1/instance" 5xx
echo 'Stopped API is visible as a web-proxy failure.'

docker compose -p "$project" up -d --wait server >/dev/null
wait_code "http://127.0.0.1:${api_port}/ready" 200
wait_code "http://127.0.0.1:${web_port}/v1/instance" 200
echo 'API recovered after restart.'

docker compose -p "$project" stop postgres >/dev/null
wait_code "http://127.0.0.1:${api_port}/ready" 5xx
echo 'Database outage makes API readiness fail.'

docker compose -p "$project" up -d --wait postgres >/dev/null
wait_code "http://127.0.0.1:${api_port}/ready" 200
for _ in $(seq 1 20); do
    worker_state="$(docker inspect --format '{{.State.Status}}' "${project}-worker-1" 2>/dev/null || true)"
    if [ "$worker_state" = running ]; then
        break
    fi
    sleep 1
done
test "$worker_state" = running
echo 'Database recovered; API readiness and worker process are running.'
