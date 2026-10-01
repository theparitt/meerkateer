#!/usr/bin/env python3
"""Isolated Community journey: test channel, outage, recovery, and delivery."""

from __future__ import annotations

import concurrent.futures
import datetime as dt
import http.server
import http.client
import json
import os
import queue
import subprocess
import tempfile
import threading
import time
import uuid
import urllib.error
import urllib.request

from api_e2e import bearer, browser_headers, call, cookies, expect, run_agent, utc


received: queue.Queue[dict[str, object]] = queue.Queue()


class Receiver(http.server.BaseHTTPRequestHandler):
    response_status = 204

    def do_POST(self) -> None:
        assert self.path == "/hook"
        length = int(self.headers["Content-Length"])
        assert length < 4096
        payload = json.loads(self.rfile.read(length))
        received.put({"payload": payload, "event_id": self.headers.get("X-Meerkateer-Event-ID"), "status": self.response_status})
        self.send_response(self.response_status)
        self.end_headers()

    def log_message(self, *_args: object) -> None:
        return


def database_scalar(sql: str) -> str:
    result = subprocess.run(
        [
            "docker", "compose", "-p", os.environ["MEERKATEER_ALERT_TEST_PROJECT"],
            "-f", "compose.yaml", "-f", "tests/integration/compose.alerts.yaml",
            "exec", "-T", "postgres", "psql", "-U", os.environ["POSTGRES_USER"],
            "-d", os.environ["POSTGRES_DB"], "-Atc", sql,
        ],
        check=True, capture_output=True, text=True, timeout=15,
    )
    return result.stdout.strip()


def wait_api() -> None:
    for _ in range(60):
        try:
            if call("GET", "/live").status == 200:
                return
        except OSError:
            pass
        time.sleep(0.2)
    raise AssertionError("API did not start")


def send(secret: str, status: str, timestamp: str, key: str, message: str) -> None:
    expect(call(
        "POST", "/v1/ingest/heartbeat",
        body={
            "interface_version": "1", "service": "fixture-service",
            "project": "fixture-project", "environment": "production",
            "status": status, "message": message, "timestamp": timestamp,
        },
        headers=bearer(secret, key),
    ), 202)


def wait_fake_service(port: int) -> None:
    for _ in range(50):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=0.2):
                return
        except OSError:
            time.sleep(0.1)
    raise AssertionError("fake service did not start")


def fake_mode(port: int, mode: str) -> None:
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/control",
        data=json.dumps({"mode": mode}).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=2) as response:
        assert response.status == 204, (mode, response.status)


def probe_fake_service(port: int) -> tuple[str, str]:
    """Small test probe. This is fixture logic, not a product scheduled probe."""
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=0.15) as response:
            try:
                body = json.loads(response.read(256))
            except json.JSONDecodeError:
                return "down", "probe invalid JSON"
            if isinstance(body, dict) and body.get("status") == "ok":
                return "ok", "fixture process running"
            return "down", "probe invalid health response"
    except urllib.error.HTTPError as error:
        if error.code == 503:
            return "degraded", "probe dependency unavailable (HTTP 503)"
        return "down", f"probe HTTP {error.code}"
    except (TimeoutError, urllib.error.URLError, http.client.RemoteDisconnected, ConnectionError):
        return "down", "probe connection or timeout failure"


def run_worker(*arguments: str) -> None:
    worker_env = {**os.environ, "MEERKATEER_DATABASE_URL": os.environ["MEERKATEER_WORKER_DATABASE_URL"]}
    subprocess.run(
        ["target/debug/meerkateer-worker", "--once", *arguments],
        env=worker_env, check=True, timeout=30,
    )


def exercise_fake_system(secret: str, project_id: str, service_id: str) -> None:
    port = int(os.environ.get("MEERKATEER_FAKE_SERVICE_PORT", "18092"))
    command = [os.environ.get("PYTHON", "python3"), "tests/integration/fake_service.py", "--port", str(port)]
    fake: subprocess.Popen[bytes] | None = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    case_number = 0
    base = dt.datetime.now(dt.timezone.utc).replace(microsecond=0) + dt.timedelta(seconds=2)

    def observe(expected: str, label: str) -> tuple[str, str, str]:
        nonlocal case_number
        status, message = probe_fake_service(port)
        assert status == expected, (label, status, message)
        observed = (base + dt.timedelta(seconds=case_number)).isoformat().replace("+00:00", "Z")
        case_number += 1
        send(secret, status, observed, str(uuid.uuid4()), message)
        services = expect(call("GET", f"/v1/projects/{project_id}/services"), 200).body["items"]
        current = next(item["status"] for item in services if item["id"] == service_id)
        mapped = {"ok": "online", "down": "offline", "degraded": "degraded"}[status]
        assert current["state"] == mapped and current["observed_at"] == observed, (label, current)
        return status, message, observed

    try:
        wait_fake_service(port)
        for invalid_body, expected_code in [
            (b'{"mode":"missing"}', 400),
            (b'{"mode":[]}', 400),
            (b"x" * 129, 413),
        ]:
            request = urllib.request.Request(
                f"http://127.0.0.1:{port}/control", data=invalid_body, method="POST",
            )
            try:
                urllib.request.urlopen(request, timeout=2)
            except urllib.error.HTTPError as error:
                assert error.code == expected_code, (invalid_body, error.code)
            else:
                raise AssertionError("fake service accepted invalid fault control")
        for mode, expected in [
            ("healthy", "ok"),
            ("http_500", "down"),
            ("http_500", "down"),
            ("healthy", "ok"),
            ("dependency_down", "degraded"),
            ("healthy", "ok"),
            ("invalid_shape", "down"),
            ("malformed_json", "down"),
            ("slow", "down"),
            ("disconnect", "down"),
        ]:
            fake_mode(port, mode)
            observe(expected, mode)

        assert fake is not None
        fake.terminate()
        fake.wait(timeout=5)
        fake = None
        observe("down", "process stopped")
        fake = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        wait_fake_service(port)
        observe("ok", "process restarted")

        fake_mode(port, "flaky")
        observe("ok", "flaky first request")
        observe("down", "flaky second request")
        fake_mode(port, "healthy")
        observe("ok", "flaky recovered")

        pending = database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NULL")
        assert pending == "6", f"expected six real fixture transitions, got {pending}"
        run_worker()
        deliveries = [received.get(timeout=5) for _ in range(6)]
        contents = [str(item["payload"]["content"]) for item in deliveries]
        assert sum("[DOWN]" in item for item in contents) == 3, contents
        assert sum("[RECOVERED]" in item for item in contents) == 3, contents
        assert all(item["status"] == 204 for item in deliveries), deliveries
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NULL") == "0"

        # A silent service becomes unknown, while its last reported state remains online.
        changed = database_scalar(
            f"UPDATE service_snapshots SET observed_at = now() - interval '10 minutes' "
            f"WHERE service_id = '{service_id}'"
        )
        assert changed == "UPDATE 1", changed
        services = expect(call("GET", f"/v1/projects/{project_id}/services"), 200).body["items"]
        stale = next(item["status"] for item in services if item["id"] == service_id)
        assert stale["state"] == "unknown" and stale["stale"] is True and stale["reported_state"] == "online", stale
        observe("ok", "fresh after silence")
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NULL") == "0"

        # A real receiver failure must retry with the same event ID and then dead-letter.
        Receiver.response_status = 503
        expect(call("POST", "/v1/alerts/test", headers=browser_headers()), 502, "alert_delivery_failed")
        rejected_test = received.get(timeout=5)
        assert rejected_test["status"] == 503 and "[TEST]" in str(rejected_test["payload"])
        fake_mode(port, "http_500")
        observe("down", "receiver unavailable")
        run_worker("--max-attempts", "2")
        first_try = received.get(timeout=5)
        assert first_try["status"] == 503 and first_try["event_id"]
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NULL AND attempts = 1") == "1"
        assert database_scalar("UPDATE outbox SET available_at = now() WHERE topic = 'alert.transition' AND processed_at IS NULL") == "UPDATE 1"
        run_worker("--max-attempts", "2")
        second_try = received.get(timeout=5)
        assert second_try["event_id"] == first_try["event_id"] and second_try["status"] == 503
        assert database_scalar("SELECT count(*) FROM outbox_dead_letters WHERE topic = 'alert.transition'") == "1"
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND dead_lettered_at IS NOT NULL") == "1"

        Receiver.response_status = 204
        history = expect(
            call("GET", f"/v1/alerts/deliveries?project_id={project_id}&limit=200"), 200
        ).body["items"]
        dead_letter = next(item for item in history if item["status"] == "dead_lettered")
        expect(
            call("POST", f"/v1/alerts/deliveries/{dead_letter['id']}/replay"),
            403,
            "csrf_failed",
        )
        expect(
            call(
                "POST",
                f"/v1/alerts/deliveries/{dead_letter['id']}/replay",
                headers=browser_headers(),
            ),
            204,
        )
        replay_history = expect(
            call("GET", f"/v1/alerts/deliveries?project_id={project_id}&limit=200"), 200
        ).body["items"]
        replay = next(item for item in replay_history if item["replay_of"] == dead_letter["id"])
        assert replay["status"] == "queued" and replay["transition"] == "down", replay
        expect(
            call(
                "POST",
                f"/v1/alerts/deliveries/{dead_letter['id']}/replay",
                headers=browser_headers(),
            ),
            409,
            "alert_not_replayable",
        )
        run_worker()
        replayed = received.get(timeout=5)
        assert replayed["status"] == 204 and "[DOWN]" in str(replayed["payload"]["content"]), replayed

        fake_mode(port, "healthy")
        observe("ok", "receiver restored")
        run_worker()
        recovered = received.get(timeout=5)
        assert recovered["status"] == 204 and "[RECOVERED]" in str(recovered["payload"]["content"]), recovered
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NULL") == "0"

        # Eight simultaneous reports of the same outage are distinct ingest facts,
        # but must serialize to a single state transition and one notification.
        concurrent_at = utc(dt.timedelta(seconds=60))
        concurrent_body = {
            "interface_version": "1", "service": "fixture-service",
            "project": "fixture-project", "environment": "production",
            "status": "down", "message": "simultaneous fixture outage",
            "timestamp": concurrent_at,
        }

        def concurrent_report(_number: int) -> int:
            request = urllib.request.Request(
                f"{os.environ['MEERKATEER_E2E_URL']}/v1/ingest/heartbeat",
                data=json.dumps(concurrent_body).encode(),
                headers={**bearer(secret, str(uuid.uuid4())), "Content-Type": "application/json"},
                method="POST",
            )
            with urllib.request.urlopen(request, timeout=15) as response:
                return response.status

        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            statuses = list(pool.map(concurrent_report, range(8)))
        assert statuses == [202] * 8, statuses
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NULL") == "1"
        services = expect(call("GET", f"/v1/projects/{project_id}/services"), 200).body["items"]
        current = next(item["status"] for item in services if item["id"] == service_id)
        assert current["state"] == "offline" and current["observed_at"] == concurrent_at, current
        later_at = (dt.datetime.fromisoformat(concurrent_at.replace("Z", "+00:00")) + dt.timedelta(seconds=1)).isoformat().replace("+00:00", "Z")
        send(secret, "ok", later_at, str(uuid.uuid4()), "simultaneous outage recovered")
        run_worker()
        pair = [received.get(timeout=5), received.get(timeout=5)]
        assert sum("[DOWN]" in str(item["payload"]["content"]) for item in pair) == 1, pair
        assert sum("[RECOVERED]" in str(item["payload"]["content"]) for item in pair) == 1, pair
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NULL") == "0"

        # A bounded cooldown suppresses only repeat-down notification noise. It does
        # not suppress the incident, state transition, or subsequent recovery alert.
        expect(call(
            "PUT", "/v1/alerts/policy",
            body={
                "enabled": True, "notify_down": True, "notify_recovered": True,
                "cooldown_seconds": 86401,
            }, headers=browser_headers(),
        ), 400, "invalid_request")
        expect(call(
            "PUT", "/v1/alerts/policy",
            body={
                "enabled": True, "notify_down": True, "notify_recovered": True,
                "cooldown_seconds": 300,
            }, headers=browser_headers(),
        ), 200)
        cooldown_down = (
            dt.datetime.fromisoformat(later_at.replace("Z", "+00:00")) + dt.timedelta(seconds=1)
        ).isoformat().replace("+00:00", "Z")
        send(secret, "down", cooldown_down, str(uuid.uuid4()), "repeat outage in cooldown")
        current = expect(call("GET", f"/v1/projects/{project_id}/services"), 200).body["items"][0]["status"]
        assert current["state"] == "offline", current
        cooldown_history = expect(
            call("GET", f"/v1/alerts/deliveries?project_id={project_id}&limit=200"), 200
        ).body["items"]
        suppressed = cooldown_history[0]
        assert suppressed["status"] == "suppressed" and "cooldown" in suppressed["suppression_reason"].lower(), suppressed
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NULL") == "0"
        cooldown_recovered = (
            dt.datetime.fromisoformat(cooldown_down.replace("Z", "+00:00")) + dt.timedelta(seconds=1)
        ).isoformat().replace("+00:00", "Z")
        send(secret, "ok", cooldown_recovered, str(uuid.uuid4()), "repeat outage recovered")
        run_worker()
        recovery_after_cooldown = received.get(timeout=5)
        assert "[RECOVERED]" in str(recovery_after_cooldown["payload"]["content"]), recovery_after_cooldown
        print("Failure lab: HTTP errors, dependency failure, malformed data, timeout, disconnect, crash, flapping, stale data, webhook retry/dead letter, and concurrent outage reports passed.")
    finally:
        Receiver.response_status = 204
        if fake is not None:
            fake.terminate()
            fake.wait(timeout=5)


def main() -> None:
    port = int(os.environ.get("MEERKATEER_ALERT_TEST_HOOK_PORT", "18091"))
    receiver = http.server.ThreadingHTTPServer(("127.0.0.1", port), Receiver)
    threading.Thread(target=receiver.serve_forever, daemon=True).start()
    try:
        wait_api()
        expect(call("POST", "/v1/alerts/test"), 401, "authentication_required")
        expect(call("POST", "/v1/bootstrap", body={
            "tenant_slug": "alert-test", "tenant_name": "Alert Test",
            "owner_email": "owner@example.test", "owner_name": "Owner",
            "owner_password": "test-password-12345",
        }, headers={"Authorization": f"Bearer {os.environ['MEERKATEER_BOOTSTRAP_TOKEN']}"}), 201)
        expect(call("POST", "/v1/alerts/test"), 403, "csrf_failed")
        expect(call("POST", "/v1/alerts/test", headers=browser_headers()), 204)
        test = received.get(timeout=5)
        assert "[TEST]" in str(test["payload"]), test

        project = expect(call("POST", "/v1/projects", body={
            "slug": "fixture-project", "display_name": "Fixture Project",
        }, headers=browser_headers()), 201).body
        enrollment = expect(call(
            "POST", f"/v1/projects/{project['id']}/enrollment-tokens",
            body={"expires_in_seconds": 600}, headers=browser_headers(),
        ), 201).body
        with tempfile.TemporaryDirectory(prefix="meerkateer-alert-host-") as directory:
            config = f"{directory}/agent.json"
            enrolled = run_agent([
                "--config", config, "enroll", "--server", os.environ["MEERKATEER_E2E_URL"],
                "--name", "Compose fixture host",
            ], token=enrollment["secret"])
            assert enrolled["project_id"] == project["id"], enrolled
            host_sample = run_agent(["--config", config, "run", "--once"])
            assert host_sample["status"] == "accepted", host_sample
        service = expect(call("POST", f"/v1/projects/{project['id']}/services", body={
            "slug": "fixture-service", "environment": "production",
        }, headers=browser_headers()), 201).body
        secret = expect(call("POST", f"/v1/services/{service['id']}/credentials", headers=browser_headers()), 201).body["secret"]

        up_at = utc(dt.timedelta(seconds=-3))
        down_at = utc(dt.timedelta(seconds=-2))
        recovered_at = utc(dt.timedelta(seconds=-1))
        send(secret, "ok", up_at, str(uuid.uuid4()), "fixture process running")
        down_key = str(uuid.uuid4())
        send(secret, "down", down_at, down_key, "fixture process stopped")
        expect(call("POST", "/v1/ingest/heartbeat", body={
            "interface_version": "1", "service": "fixture-service",
            "project": "fixture-project", "environment": "production",
            "status": "down", "message": "fixture process stopped", "timestamp": down_at,
        }, headers=bearer(secret, down_key)), 200)
        send(secret, "ok", recovered_at, str(uuid.uuid4()), "fixture process restarted")
        send(secret, "down", up_at, str(uuid.uuid4()), "late old sample")

        state = expect(call("GET", f"/v1/projects/{project['id']}/services"), 200).body["items"][0]["status"]
        assert state["state"] == "online" and state["observed_at"] == recovered_at, state
        timeline = expect(call("GET", f"/v1/services/{service['id']}/timeline"), 200).body["items"]
        assert any(item.get("message") == "fixture process stopped" for item in timeline), timeline
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition'") == "2"

        run_worker()
        delivered = [received.get(timeout=5), received.get(timeout=5)]
        contents = [str(item["payload"]["content"]) for item in delivered]
        assert any("[DOWN]" in content and "fixture-project/fixture-service" in content for content in contents), contents
        assert any("[RECOVERED]" in content and recovered_at in content for content in contents), contents
        assert all(item["event_id"] for item in delivered), delivered
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NOT NULL") == "2"
        worker_summary = expect(call("GET", "/v1/admin/summary"), 200).body
        assert worker_summary["worker_status"] == "healthy", worker_summary
        assert worker_summary["worker_last_cycle_claimed"] >= 2, worker_summary
        assert database_scalar(
            "UPDATE worker_runtime SET last_cycle_at = now() - interval '31 seconds'"
        ).startswith("UPDATE ")
        stalled_summary = expect(call("GET", "/v1/admin/summary"), 200).body
        assert stalled_summary["worker_status"] == "stalled", stalled_summary
        run_worker()
        recovered_worker_summary = expect(call("GET", "/v1/admin/summary"), 200).body
        assert recovered_worker_summary["worker_status"] == "healthy", recovered_worker_summary
        exercise_fake_system(secret, project["id"], service["id"])
        cookies.clear()
    finally:
        receiver.shutdown()
        receiver.server_close()


if __name__ == "__main__":
    main()
