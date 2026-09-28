#!/bin/sh
set -eu
php -l Meerkateer.php
php test.php
php -S 127.0.0.1:18081 test_server.php >/tmp/meerkateer-php-test-server.log 2>&1 &
server_pid=$!
trap 'kill "$server_pid" 2>/dev/null || true' EXIT
sleep 1
php http_test.php
