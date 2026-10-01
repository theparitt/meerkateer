#!/usr/bin/env python3
"""Small HTTP service that demonstrates the Meerkateer Python SDK."""

from __future__ import annotations

import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from meerkateer_sdk import Meerkateer


LANGUAGE = "python"
PORT = int(os.environ.get("DEMO_PORT", "19101"))
INTERVAL = float(os.environ.get("DEMO_HEARTBEAT_SECONDS", "5"))
client = Meerkateer.from_env()
state_lock = threading.Lock()
state = "ok"


def signal(next_state: str) -> None:
    global state
    messages = {
        "ok": "demo service recovered",
        "degraded": "demo dependency is slow",
        "down": "demo process is unavailable",
    }
    with state_lock:
        previous = state
        state = next_state
    client.heartbeat(next_state, message=messages[next_state])
    if previous != next_state:
        level = {"ok": "info", "degraded": "warning", "down": "error"}[next_state]
        kind = {"ok": "demo_recovered", "degraded": "demo_degraded", "down": "demo_failed"}[next_state]
        client.event(kind, level=level, message=messages[next_state])


def heartbeat_loop() -> None:
    while True:
        with state_lock:
            current = state
        try:
            client.heartbeat(current, message=f"{LANGUAGE} demo heartbeat")
        except Exception as error:  # SDK errors are already credential-safe.
            print(f"heartbeat delivery failed: {error}", flush=True)
        time.sleep(INTERVAL)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format_string: str, *args: object) -> None:
        print(f"{LANGUAGE}: {format_string % args}", flush=True)

    def reply(self, status_code: int, payload: dict[str, str]) -> None:
        body = json.dumps(payload, separators=(",", ":")).encode()
        self.send_response(status_code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
        if self.path != "/health":
            self.reply(404, {"error": "not_found"})
            return
        with state_lock:
            current = state
        self.reply(200, {"language": LANGUAGE, "status": current})

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
        requested = self.path.removeprefix("/scenario/")
        mapping = {"healthy": "ok", "degraded": "degraded", "down": "down"}
        if requested not in mapping or self.path == requested:
            self.reply(404, {"error": "not_found"})
            return
        try:
            signal(mapping[requested])
        except Exception as error:
            self.reply(502, {"error": "telemetry_failed", "detail": str(error)})
            return
        self.reply(200, {"language": LANGUAGE, "status": mapping[requested]})


if __name__ == "__main__":
    client.deploy("demo-1", "local", status="finished")
    signal("ok")
    threading.Thread(target=heartbeat_loop, daemon=True).start()
    print(f"{LANGUAGE} demo listening on http://127.0.0.1:{PORT}", flush=True)
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
