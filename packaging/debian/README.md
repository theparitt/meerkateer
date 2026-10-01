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

The setup command provides a terminal UI suitable for headless Ubuntu servers. It keeps the
one-time enrollment token out of command history, then asks which bounded CPU, memory, disk, and
exact process signals may be sent. The installed service runs as the unprivileged
`meerkateer-controller` account with systemd hardening and outbound network access only.

`apt remove` keeps `/var/lib/meerkateer-controller/agent.json` for recovery. `apt purge` removes the
credential and local sequence state permanently.
