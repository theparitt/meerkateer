#!/usr/bin/env bash
set -euo pipefail

readonly service_name="meerkateer-agent"
readonly install_binary="/usr/local/bin/meerkateer-agent"
readonly install_config="/var/lib/meerkateer-agent/agent.json"
readonly install_unit="/etc/systemd/system/meerkateer-agent.service"

usage() {
  cat <<'EOF'
Usage: sudo ./install.sh --binary PATH --config PATH [--replace-config] [--no-start]

Installs an already-enrolled Meerkateer agent as a hardened systemd service.
The enrollment credential is copied with mode 0600 and is never accepted as an argument.
EOF
}

binary_source=""
config_source=""
replace_config=false
start_service=true

while (($#)); do
  case "$1" in
    --binary)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      binary_source="$2"
      shift 2
      ;;
    --config)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      config_source="$2"
      shift 2
      ;;
    --replace-config)
      replace_config=true
      shift
      ;;
    --no-start)
      start_service=false
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ ${EUID} -eq 0 ]] || { echo "install.sh must run as root" >&2; exit 1; }
[[ -n "$binary_source" && -n "$config_source" ]] || { usage >&2; exit 2; }
[[ -f "$binary_source" && ! -L "$binary_source" ]] || { echo "--binary must be a regular, non-symlink file" >&2; exit 1; }
[[ -x "$binary_source" ]] || { echo "--binary must be executable" >&2; exit 1; }
[[ -f "$config_source" && ! -L "$config_source" ]] || { echo "--config must be a regular, non-symlink file" >&2; exit 1; }

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
unit_source="${script_dir}/meerkateer-agent.service"
[[ -f "$unit_source" && ! -L "$unit_source" ]] || { echo "missing service unit beside installer" >&2; exit 1; }

"$binary_source" --config "$config_source" doctor >/dev/null

if [[ -e "$install_config" && "$replace_config" != true ]]; then
  echo "$install_config already exists; use --replace-config only when rotating this machine's identity" >&2
  exit 1
fi

if ! getent group "$service_name" >/dev/null; then
  groupadd --system "$service_name"
fi
if ! id "$service_name" >/dev/null 2>&1; then
  useradd --system --gid "$service_name" --home-dir "/var/lib/${service_name}" --shell /usr/sbin/nologin "$service_name"
fi

install -d -o root -g root -m 0755 /etc/meerkateer
install -d -o "$service_name" -g "$service_name" -m 0700 "/var/lib/${service_name}"

binary_temp="${install_binary}.new"
config_temp="${install_config}.new"
install -o root -g root -m 0755 "$binary_source" "$binary_temp"
install -o "$service_name" -g "$service_name" -m 0600 "$config_source" "$config_temp"
mv -f -- "$binary_temp" "$install_binary"
mv -f -- "$config_temp" "$install_config"
install -o root -g root -m 0644 "$unit_source" "$install_unit"

systemctl daemon-reload
systemctl enable "$service_name.service" >/dev/null
if [[ "$start_service" == true ]]; then
  systemctl restart "$service_name.service"
  systemctl --no-pager --full status "$service_name.service"
fi

echo "Meerkateer agent installed. Configuration: $install_config"
