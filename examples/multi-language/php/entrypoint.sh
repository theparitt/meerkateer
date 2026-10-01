#!/bin/sh
set -eu

php /app/examples/multi-language/php/heartbeat.php &
heartbeat_pid=$!
trap 'kill "$heartbeat_pid" 2>/dev/null || true' EXIT INT TERM
exec php -S "127.0.0.1:${DEMO_PORT:-19104}" /app/examples/multi-language/php/router.php
