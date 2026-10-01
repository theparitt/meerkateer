# Localhost multi-language SDK guide (for humans)

This guide starts a disposable Meerkateer installation and five real demo servers:
Python, Node.js, Go, PHP, and Rust. It is safe to run beside the normal development
stack because it uses separate Compose projects, a separate PostgreSQL volume, API
port 18110, and web port 18111.

## Quick start

Requirements are Docker Engine 26+, Compose v2, `make`, OpenSSL, Python 3, and `curl`.
You do not need to install Go, PHP, Node.js, or Rust on the host.

```sh
cd /home/theparitt/work/meerkateer
make sdk-lab
```

The first build can take several minutes. When it prints `Multi-language SDK lab is
ready`, open <http://127.0.0.1:18111/login> and sign in with these local-lab-only values:

```text
Email: sdk-lab@example.invalid
Password: local-sdk-lab-password-123
Workspace: Multi-language SDK Lab
```

The Console will contain five processes: `python-demo`, `node-demo`, `go-demo`,
`php-demo`, and `rust-demo`. Their SDK keys are generated into the ignored,
mode-0600 file `dev/.multilang-lab.env`; do not commit or paste that file.

## Trigger visible states

All demos use the same API. Change `19101` to ports 19102 through 19105 to exercise
Node.js, Go, PHP, and Rust respectively.

```sh
curl http://127.0.0.1:19101/health
curl -X POST http://127.0.0.1:19101/scenario/degraded
curl -X POST http://127.0.0.1:19101/scenario/down
curl -X POST http://127.0.0.1:19101/scenario/healthy
```

Select that process in the Console. The status and timeline should show degraded,
offline, and recovery observations together with `demo_degraded`, `demo_failed`, and
`demo_recovered` events. A `down` report simulates an application-detected failure;
stopping a container tests missing-heartbeat detection.

Run the complete automated scenario with:

```sh
make sdk-lab-test
```

That command starts a clean lab, drives all five servers through degraded/down/recovery
concurrently, verifies their API timelines, stops the Node server, waits for the
30-second stale window, verifies `stale=true` and effective state `unknown`, restarts it,
and verifies recovery to `online`. It removes the disposable lab afterward.

## Operate and clean up

```sh
make sdk-lab-logs
make sdk-lab-down
```

`make sdk-lab-down` removes only the two lab Compose projects, their disposable database
volume, and the generated SDK-key file. It does not remove the normal Meerkateer stack or
its `meerkateer_postgres-data` volume.

## Use one SDK in your own local service

First create a process in the Console and copy the environment block shown once. Every
SDK expects the following values:

```sh
export MEERKATEER_URL=http://127.0.0.1:6510
export MEERKATEER_SERVICE_KEY='<one-time mks_sk_ value>'
export MEERKATEER_PROJECT='<workspace slug>'
export MEERKATEER_SERVICE='<process slug>'
export MEERKATEER_ENVIRONMENT=local
```

Run the native examples when the matching runtime is installed:

```sh
# Python 3.10+
PYTHONPATH=./sdk/python/src python3 examples/multi-language/python/server.py

# Node.js 22+
node examples/multi-language/node/server.mjs

# Go 1.22+
cd examples/multi-language/go && go run .

# PHP 8.2+ (start the heartbeat loop and HTTP router in separate terminals)
php examples/multi-language/php/heartbeat.php
php -S 127.0.0.1:19104 examples/multi-language/php/router.php

# Rust 1.93+
cargo run --manifest-path examples/multi-language/rust/Cargo.toml
```

Set a unique `DEMO_PORT` if another local program already uses the default port. For a
non-loopback Meerkateer address, every SDK requires HTTPS and refuses redirects. Never
put SDK keys, player/customer data, request bodies, database rows, or stack traces in
telemetry messages.

## Troubleshooting

- `address already in use`: stop the old lab with `make sdk-lab-down`, or identify the
  process already using ports 18110, 18111, or 19101-19105.
- `telemetry_failed`: inspect `make sdk-lab-logs`; the service key may be revoked or the
  API may not be ready.
- process remains `unknown`: call `/scenario/healthy` and confirm its service slug and
  workspace slug match the key that was issued for it.
- Docker build fails before compiling: confirm Docker can pull the pinned language base
  images and has enough disk space for the Rust build cache.
