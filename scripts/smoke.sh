#!/usr/bin/env sh
set -eu

base_url="${MEERKATEER_SMOKE_URL:-http://127.0.0.1:6510}"
web_url="${MEERKATEER_SMOKE_WEB_URL:-http://127.0.0.1:6511}"
live="$(curl --fail --silent --show-error --max-time 5 "$base_url/live")"
health="$(curl --fail --silent --show-error --max-time 5 "$base_url/health")"
openapi="$(curl --fail --silent --show-error --max-time 5 "$base_url/openapi.json")"

python3 - "$live" "$health" "$openapi" "$web_url" <<'PY'
import json
import sys
import time
import urllib.error
import urllib.request

live, health, openapi = (json.loads(value) for value in sys.argv[1:4])
assert live == {"status": "ok"}
assert health["status"] == "ok"
assert health["interface"] == "meerkateer"
assert openapi["openapi"] == "3.1.0"

for attempt in range(20):
    try:
        urllib.request.urlopen(f"{sys.argv[4]}/v1/session", timeout=5)
    except urllib.error.HTTPError as error:
        assert error.code == 401, error.code
        assert json.loads(error.read()) == {"code": "authentication_required"}
        break
    except (urllib.error.URLError, ConnectionError):
        if attempt == 19:
            raise
        time.sleep(0.25)
    else:
        raise AssertionError("unauthenticated web-proxy session request unexpectedly succeeded")

print("Meerkateer smoke test passed.")
PY
