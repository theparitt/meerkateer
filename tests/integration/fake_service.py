#!/usr/bin/env python3
"""Loopback-only disposable service for the Community failure lab.

POST /control with {"mode": "..."} to change its behavior. The service is
deliberately separate from the test runner so SIGTERM/connection refusal are real.
"""

from __future__ import annotations

import argparse
import http.server
import json
import threading
import time


MODES = {
    "healthy", "http_500", "dependency_down", "slow", "malformed_json",
    "invalid_shape", "disconnect", "flaky",
}


class State:
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.mode = "healthy"
        self.requests = 0


state = State()


class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self) -> None:
        if self.path != "/control":
            self.send_error(404)
            return
        try:
            size = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self.send_error(400)
            return
        if size <= 0:
            self.send_error(400)
            return
        if size > 128:
            self.send_error(413)
            return
        try:
            mode = json.loads(self.rfile.read(size))["mode"]
        except (ValueError, KeyError, TypeError):
            self.send_error(400)
            return
        if not isinstance(mode, str) or mode not in MODES:
            self.send_error(400)
            return
        with state.lock:
            state.mode = mode
            state.requests = 0
        self.send_response(204)
        self.end_headers()

    def do_GET(self) -> None:
        if self.path != "/health":
            self.send_error(404)
            return
        with state.lock:
            mode = state.mode
            state.requests += 1
            request_number = state.requests
        if mode == "disconnect":
            self.close_connection = True
            return
        if mode == "slow":
            time.sleep(0.5)
        status = 200
        body: bytes
        if mode == "http_500" or (mode == "flaky" and request_number % 2 == 0):
            status, body = 500, b'{"status":"down","reason":"fixture_error"}'
        elif mode == "dependency_down":
            status, body = 503, b'{"status":"degraded","reason":"database_unavailable"}'
        elif mode == "malformed_json":
            body = b'{"status":'
        elif mode == "invalid_shape":
            body = b'{"unexpected":"value"}'
        else:
            body = b'{"status":"ok"}'
        try:
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *_args: object) -> None:
        return


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    args = parser.parse_args()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    server.serve_forever(poll_interval=0.1)


if __name__ == "__main__":
    main()
