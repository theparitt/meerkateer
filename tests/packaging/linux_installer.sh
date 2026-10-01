#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"

docker run --rm --entrypoint /bin/bash \
  --volume "${repo_root}/packaging/systemd:/package:ro" \
  debian:bookworm-slim -euo pipefail -c '
    mkdir -p /fakebin
    ln -s /bin/true /fakebin/systemctl
    export PATH="/fakebin:${PATH}"

    /package/install.sh --binary /bin/true --config /etc/hosts --no-start
    test -x /usr/local/bin/meerkateer-agent
    test "$(stat -c %a /var/lib/meerkateer-agent/agent.json)" = 600
    test "$(stat -c %U /var/lib/meerkateer-agent/agent.json)" = meerkateer-agent
    grep -Fq "User=meerkateer-agent" /etc/systemd/system/meerkateer-agent.service

    if /package/install.sh --binary /bin/true --config /etc/hosts --no-start; then
      echo "installer unexpectedly overwrote an existing credential" >&2
      exit 1
    fi

    /package/uninstall.sh --purge
    test ! -e /usr/local/bin/meerkateer-agent
    test ! -e /var/lib/meerkateer-agent
  '

echo "Linux agent installer test passed."
