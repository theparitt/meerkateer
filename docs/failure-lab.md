# Disposable failure lab

Run the Community failure lab on a machine with Docker Compose, Rust, Python 3,
and the project's local `.env`:

```sh
make failure-lab
```

The command creates a separate Compose project named `meerkateer-alerts-test`.
It uses an isolated PostgreSQL volume and loopback ports 18543 (database),
18090 (API), 18091 (alert receiver), and 18092 (fake service). The script
removes its containers and volume on exit. It does not stop your normal stack,
touch a real monitored service, or change the existing `.env` credentials.
Use `MEERKATEER_ALERT_TEST_*` and `MEERKATEER_FAKE_SERVICE_PORT` to change
ports/project name if needed.

The fake service is a separate Python HTTP process. Its `/health` endpoint
can be healthy, return HTTP 500 or 503, answer slowly, send invalid JSON or
shape, disconnect, or alternate success and failure. The harness also kills
and restarts that process. A small test probe requests `/health` and submits
the observed result through the real MKS heartbeat API. **That probe belongs
to the lab; a production scheduled HTTP probe has not shipped yet.**

You can inspect the fake service by itself in another terminal:

```sh
python3 tests/integration/fake_service.py --port 18092
curl http://127.0.0.1:18092/health
curl -X POST -H 'Content-Type: application/json' \
  -d '{"mode":"http_500"}' http://127.0.0.1:18092/control
curl -i http://127.0.0.1:18092/health
```

Supported modes are `healthy`, `http_500`, `dependency_down`, `slow`,
`malformed_json`, `invalid_shape`, `disconnect`, and `flaky`. The full lab
starts and controls its own instance, so stop this manual example before
running `make failure-lab` on the same port.

| Fault or edge case | Asserted result |
| --- | --- |
| Fresh install and owner | Bootstrap, session and test alert work |
| Enroll test host | Agent enrolls and sends real telemetry |
| Healthy fake service | Service shows online |
| Invalid fault-control request | Fake service returns 400 or 413 and keeps running |
| HTTP 500 | Service shows offline; one down alert is queued |
| Repeated HTTP 500 | No extra down alert |
| HTTP 503 dependency failure | Service shows degraded; no false recovery alert when it clears |
| Invalid JSON or response shape | Probe records an explicit failure |
| Slow response | Probe times out and records failure |
| Abrupt disconnect | Probe records connection failure |
| Process killed | Connection refusal is observed, then restart restores online |
| Flapping service | Each genuine down/recovery transition alerts once |
| Eight simultaneous outage reports | All facts are accepted; one down alert is queued |
| Duplicate ingest key | Exact retry is accepted as a duplicate |
| Older observation | Current state and alert count remain unchanged |
| Silent service | Status becomes unknown/stale, preserving reported state |
| Receiver returns HTTP 503 | Test alert returns failure; worker retries with the same event ID |
| Receiver remains unavailable | Alert enters the dead-letter queue after the configured attempts |
| Receiver recovers | A later recovery alert is delivered |

`make integration` also runs the existing identity, migration, agent, SDK,
worker and abuse-control suite before this lab. The lab is a test of the
current Community alpha. It does not claim production readiness or replace
restore, security, external watchdog, or real-host tests.
