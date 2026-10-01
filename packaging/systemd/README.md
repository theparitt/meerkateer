# Linux systemd installation

Enroll into a temporary owner-only config first, then install it. Never put the enrollment token on
the command line because shell history and process listings can expose it.

```sh
cargo build --locked --release -p meerkateer-agent
umask 077
read -rsp 'Enrollment token: ' MEERKATEER_ENROLLMENT_TOKEN && export MEERKATEER_ENROLLMENT_TOKEN
./target/release/meerkateer-agent --config ./agent.json enroll \
  --server https://monitor.example.com --name game-host-01
unset MEERKATEER_ENROLLMENT_TOKEN
sudo ./packaging/systemd/install.sh \
  --binary ./target/release/meerkateer-agent --config ./agent.json
rm -f ./agent.json
```

To watch processes, create `/etc/meerkateer/agent.env` as root:

```text
MEERKATEER_AGENT_ARGS=--watch-process java --watch-process postgres
```

Then run `sudo systemctl restart meerkateer-agent`. Only root can alter this local allowlist. The
agent opens outbound HTTPS connections and no inbound listener.

The installer refuses to overwrite an existing machine credential unless `--replace-config` is
explicitly supplied. `uninstall.sh` preserves the credential by default; `--purge` permanently
deletes it.
