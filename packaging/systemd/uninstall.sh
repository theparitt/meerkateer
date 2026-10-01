#!/usr/bin/env bash
set -euo pipefail

purge=false
if [[ ${1:-} == "--purge" ]]; then
  purge=true
  shift
fi
if (($#)); then
  echo "Usage: sudo ./uninstall.sh [--purge]" >&2
  exit 2
fi
[[ ${EUID} -eq 0 ]] || { echo "uninstall.sh must run as root" >&2; exit 1; }

systemctl disable --now meerkateer-agent.service >/dev/null 2>&1 || true
rm -f -- /etc/systemd/system/meerkateer-agent.service /usr/local/bin/meerkateer-agent
systemctl daemon-reload

if [[ "$purge" == true ]]; then
  rm -f -- /etc/meerkateer/agent.env
  rm -rf -- /var/lib/meerkateer-agent
  userdel meerkateer-agent >/dev/null 2>&1 || true
  groupdel meerkateer-agent >/dev/null 2>&1 || true
  echo "Meerkateer agent and its credential/state were permanently removed."
else
  echo "Meerkateer agent removed; /var/lib/meerkateer-agent was preserved for recovery."
fi
