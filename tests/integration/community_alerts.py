#!/usr/bin/env python3
"""Isolated Community journey: test channel, outage, recovery, and delivery."""

from __future__ import annotations

import datetime as dt
import http.server
import json
import os
import queue
import subprocess
import tempfile
import threading
import time
import uuid

from api_e2e import bearer, browser_headers, call, cookies, expect, run_agent, utc


received: queue.Queue[dict[str, object]] = queue.Queue()


class Receiver(http.server.BaseHTTPRequestHandler):
    def do_POST(self) -> None:
        assert self.path == "/hook"
        length = int(self.headers["Content-Length"])
        assert length < 4096
        payload = json.loads(self.rfile.read(length))
        received.put({"payload": payload, "event_id": self.headers.get("X-Meerkateer-Event-ID")})
        self.send_response(204)
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

        worker_env = {**os.environ, "MEERKATEER_DATABASE_URL": os.environ["MEERKATEER_WORKER_DATABASE_URL"]}
        subprocess.run(["target/debug/meerkateer-worker", "--once"], env=worker_env, check=True, timeout=30)
        delivered = [received.get(timeout=5), received.get(timeout=5)]
        contents = [str(item["payload"]["content"]) for item in delivered]
        assert any("[DOWN]" in content and "fixture-project/fixture-service" in content for content in contents), contents
        assert any("[RECOVERED]" in content and recovered_at in content for content in contents), contents
        assert all(item["event_id"] for item in delivered), delivered
        assert database_scalar("SELECT count(*) FROM outbox WHERE topic = 'alert.transition' AND processed_at IS NOT NULL") == "2"
        cookies.clear()
    finally:
        receiver.shutdown()
        receiver.server_close()


if __name__ == "__main__":
    main()
