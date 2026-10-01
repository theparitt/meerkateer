#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 --binary PATH --version X.Y.Z --architecture amd64 --output PATH" >&2
}

binary=""
version=""
architecture=""
output=""
while (($#)); do
  case "$1" in
    --binary) binary="$2"; shift 2 ;;
    --version) version="$2"; shift 2 ;;
    --architecture) architecture="$2"; shift 2 ;;
    --output) output="$2"; shift 2 ;;
    *) usage; exit 2 ;;
  esac
done

[[ -x "$binary" && -f "$binary" ]] || { echo "--binary must be an executable file" >&2; exit 2; }
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "--version must be X.Y.Z" >&2; exit 2; }
[[ "$architecture" =~ ^(amd64|arm64)$ ]] || { echo "--architecture must be amd64 or arm64" >&2; exit 2; }
[[ -n "$output" ]] || { usage; exit 2; }

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
package_root="$(mktemp -d -t meerkateer-deb.XXXXXXXX)"
trap 'rm -rf -- "$package_root"' EXIT

install -d -m 0755 \
  "$package_root/DEBIAN" \
  "$package_root/usr/bin" \
  "$package_root/usr/lib/meerkateer" \
  "$package_root/lib/systemd/system"

cat >"$package_root/DEBIAN/control" <<EOF
Package: meerkateer-controller
Version: ${version}
Section: admin
Priority: optional
Architecture: ${architecture}
Depends: adduser, ca-certificates, systemd
Maintainer: Meerkateer Community <theparitt@users.noreply.github.com>
Homepage: https://github.com/theparitt/meerkateer
Description: outbound-only server reliability controller
 Connects an Ubuntu host to Meerkateer, sends only selected bounded host
 signals, and runs as a hardened systemd service.
EOF

install -m 0755 "$binary" "$package_root/usr/lib/meerkateer/meerkateer-controller"
install -m 0755 "$script_directory/meerkateer-controller" "$package_root/usr/bin/meerkateer-controller"
ln -s meerkateer-controller "$package_root/usr/bin/meerkateer-agent"
install -m 0644 "$script_directory/meerkateer-controller.service" \
  "$package_root/lib/systemd/system/meerkateer-controller.service"
install -m 0755 "$script_directory/postinst" "$package_root/DEBIAN/postinst"
install -m 0755 "$script_directory/prerm" "$package_root/DEBIAN/prerm"
install -m 0755 "$script_directory/postrm" "$package_root/DEBIAN/postrm"

mkdir -p -- "$(dirname -- "$output")"
dpkg-deb --root-owner-group --build "$package_root" "$output"
echo "Built $output"
