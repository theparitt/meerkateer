# Multi-language SDK demo servers

These five small HTTP services use the real Meerkateer Python, Node.js, Go, PHP,
and Rust SDKs. Every service exposes the same local contract:

| Request | Result |
| --- | --- |
| `GET /health` | Return the demo's current in-memory status. |
| `POST /scenario/healthy` | Report an `ok` heartbeat and recovery event. |
| `POST /scenario/degraded` | Report a `degraded` heartbeat and warning event. |
| `POST /scenario/down` | Report a `down` heartbeat and error event. |

Each service also reports deployment metadata at startup and refreshes its heartbeat
every five seconds. The Docker lab assigns ports 19101 through 19105 in the order shown
in the table below.

| Language | Local port | Source |
| --- | ---: | --- |
| Python | 19101 | `python/server.py` |
| Node.js | 19102 | `node/server.mjs` |
| Go | 19103 | `go/main.go` |
| PHP | 19104 | `php/router.php` + `php/heartbeat.php` |
| Rust | 19105 | `rust/src/main.rs` |

From the repository root, use `make sdk-lab` for an interactive lab or
`make sdk-lab-test` for the automated failure and recovery exercise. Read the
[human guide](../../docs/localhost-sdk-guide.md) or the
[AI runbook](../../docs/ai-localhost-runbook.md) before adapting this code.
