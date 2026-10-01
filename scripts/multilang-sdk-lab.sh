#!/usr/bin/env sh
set -eu

core_project="${MEERKATEER_SDK_LAB_CORE_PROJECT:-meerkateer-sdk-lab-core}"
demo_project="${MEERKATEER_SDK_LAB_DEMO_PROJECT:-meerkateer-sdk-lab-demos}"
api_port="${MEERKATEER_SDK_LAB_API_PORT:-18110}"
web_port="${MEERKATEER_SDK_LAB_WEB_PORT:-18111}"
runtime_env="$(pwd)/dev/.multilang-lab.env"
demo_compose="examples/multi-language/compose.yaml"
lab_url="http://127.0.0.1:${api_port}"

if [ ! -f .env ]; then
    echo 'Run make bootstrap before starting the SDK lab.' >&2
    exit 1
fi

set -a
. ./.env
set +a

core_compose() {
    MEERKATEER_API_PORT="$api_port" \
    MEERKATEER_WEB_PORT="$web_port" \
    MEERKATEER_HEARTBEAT_STALE_AFTER_SECONDS=30 \
        docker compose -p "$core_project" "$@"
}

demo_compose() {
    docker compose -p "$demo_project" -f "$demo_compose" --env-file "$runtime_env" "$@"
}

stop_demos() {
    if [ -f "$runtime_env" ]; then
        demo_compose down --remove-orphans >/dev/null 2>&1 || true
        return
    fi
    MEERKATEER_URL="$lab_url" \
    MEERKATEER_PROJECT=sdk-lab \
    PYTHON_SERVICE_KEY=mks_sk_cleanup \
    NODE_SERVICE_KEY=mks_sk_cleanup \
    GO_SERVICE_KEY=mks_sk_cleanup \
    PHP_SERVICE_KEY=mks_sk_cleanup \
    RUST_SERVICE_KEY=mks_sk_cleanup \
        docker compose -p "$demo_project" -f "$demo_compose" \
        down --remove-orphans >/dev/null 2>&1 || true
}

wait_for_api() {
    attempts=0
    until curl --fail --silent --max-time 2 "$lab_url/ready" >/dev/null; do
        attempts=$((attempts + 1))
        if [ "$attempts" -ge 90 ]; then
            core_compose logs server >&2
            echo 'SDK lab API did not become ready.' >&2
            exit 1
        fi
        sleep 1
    done
}

start_lab() {
    core_compose up -d --build postgres server worker web
    wait_for_api
    MEERKATEER_LAB_URL="$lab_url" \
        python3 tests/integration/multilang_sdk_lab.py prepare --output "$runtime_env"
    demo_compose up -d --build
    MEERKATEER_LAB_URL="$lab_url" python3 tests/integration/multilang_sdk_lab.py wait
    printf '%s\n' \
        "Multi-language SDK lab is ready." \
        "Console: http://127.0.0.1:${web_port}/login" \
        "Email: sdk-lab@example.invalid" \
        "Password: local-sdk-lab-password-123" \
        "Workspace: Multi-language SDK Lab" \
        "Demo ports: Python 19101, Node 19102, Go 19103, PHP 19104, Rust 19105"
}

cleanup() {
    stop_demos
    rm -f "$runtime_env"
    core_compose down --volumes --remove-orphans >/dev/null 2>&1 || true
}

run_test() {
    cleanup
    trap cleanup EXIT INT TERM
    start_lab
    MEERKATEER_LAB_URL="$lab_url" python3 tests/integration/multilang_sdk_lab.py exercise
    demo_compose stop node-demo
    MEERKATEER_LAB_URL="$lab_url" python3 tests/integration/multilang_sdk_lab.py wait-stale node
    demo_compose start node-demo
    MEERKATEER_LAB_URL="$lab_url" python3 tests/integration/multilang_sdk_lab.py recover node
    echo 'Multi-language SDK E2E passed: five SDKs, concurrent transitions, stale detection, and recovery.'
}

case "${1:-up}" in
    up)
        start_lab
        ;;
    test)
        run_test
        ;;
    down)
        cleanup
        echo 'Multi-language SDK lab and its disposable database were removed.'
        ;;
    logs)
        core_compose logs --tail=100 server worker
        if [ -f "$runtime_env" ]; then
            demo_compose logs --tail=100
        fi
        ;;
    *)
        echo 'Usage: scripts/multilang-sdk-lab.sh [up|test|down|logs]' >&2
        exit 2
        ;;
esac
