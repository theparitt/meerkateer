#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"

docker run --rm --entrypoint /bin/bash \
  --volume "${repo_root}/packaging/systemd:/package:ro" \
  debian:bookworm-slim -euo pipefail -c '
    mkdir -p /fakebin
    cat >/fakebin/systemctl <<'"'"'EOF'"'"'
#!/bin/sh
printf "%s\n" "$*" >>/tmp/systemctl-calls
exit 0
EOF
    chmod +x /fakebin/systemctl
    export PATH="/fakebin:${PATH}"

    /package/install.sh --binary /bin/true --config /etc/hosts --no-start
    test -x /usr/local/bin/meerkateer-agent
    test "$(stat -c %a /var/lib/meerkateer-agent/agent.json)" = 600
    test "$(stat -c %U /var/lib/meerkateer-agent/agent.json)" = meerkateer-agent
    grep -Fq "User=meerkateer-agent" /etc/systemd/system/meerkateer-agent.service
    grep -Fq "Restart=always" /etc/systemd/system/meerkateer-agent.service
    grep -Fq "WantedBy=multi-user.target" /etc/systemd/system/meerkateer-agent.service
    grep -Fxq "enable meerkateer-agent.service" /tmp/systemctl-calls
    grep -Fxq "is-enabled --quiet meerkateer-agent.service" /tmp/systemctl-calls
    if grep -Fxq "restart meerkateer-agent.service" /tmp/systemctl-calls; then
      echo "--no-start unexpectedly started the service" >&2
      exit 1
    fi

    if /package/install.sh --binary /bin/true --config /etc/hosts --no-start; then
      echo "installer unexpectedly overwrote an existing credential" >&2
      exit 1
    fi

    : >/tmp/systemctl-calls
    /package/install.sh --binary /bin/true --config /etc/hosts --replace-config
    grep -Fxq "enable meerkateer-agent.service" /tmp/systemctl-calls
    grep -Fxq "is-enabled --quiet meerkateer-agent.service" /tmp/systemctl-calls
    grep -Fxq "restart meerkateer-agent.service" /tmp/systemctl-calls
    grep -Fxq "is-active --quiet meerkateer-agent.service" /tmp/systemctl-calls

    /package/uninstall.sh --purge
    test ! -e /usr/local/bin/meerkateer-agent
    test ! -e /var/lib/meerkateer-agent
  '

echo "Linux agent installer test passed."
