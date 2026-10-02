# Ubuntu Meerkateer Controller package

Build an amd64 package from the pinned Rust workspace:

```sh
cargo build --locked --release -p meerkateer-agent
packaging/debian/build-deb.sh \
  --binary target/release/meerkateer-agent \
  --version 0.1.0 \
  --architecture amd64 \
  --output dist/meerkateer-controller_0.1.0_amd64.deb
```

Install and configure it:

```sh
sudo apt install ./dist/meerkateer-controller_0.1.0_amd64.deb
sudo meerkateer-controller setup
```

The setup command opens the cross-platform Rust terminal UI suitable for headless Ubuntu servers.
It selects Local / self-hosted Community (Cloud stays disabled until launch), keeps the one-time
enrollment token out of command history, and can test config storage/free space, URL, DNS,
proxy/VPN-sensitive routing, TLS, and API readiness before enrollment. It then shows live CPU,
memory, disk, exact process signals, diagnostics, and the signal allowlist. Reopen it later with:

```sh
sudo meerkateer-controller tui
```

For non-interactive diagnostics:

```sh
sudo meerkateer-controller test-connection --strict \
  --server https://meerkateer-api.example.com
```

The installed service remains headless and runs as the unprivileged
`meerkateer-controller` account with systemd hardening and outbound network access only.

The API host is entered once during first setup and stored with the exchanged machine credential in
`/var/lib/meerkateer-controller/agent.json`. Later setup runs show that enrollment and update only
the signal allowlist. Purge and enroll again to intentionally move the machine to another host.

`apt remove` keeps `/var/lib/meerkateer-controller/agent.json` for recovery. `apt purge` removes the
credential and local sequence state permanently.
