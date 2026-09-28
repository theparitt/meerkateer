#!/usr/bin/env python3
"""Deterministic MKS-1 fixture service for local compatibility and failure tests."""

import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    mode = "healthy"

    def do_GET(self) -> None:  # noqa: N802 - stdlib callback name
        if self.path == "/live":
            self.respond(200, {"status": "ok"})
            return
        if self.path in {"/health", "/ready"}:
            healthy = self.mode == "healthy"
            body = {
                "status": "ok" if healthy else "degraded",
                "service": "fixture-server",
                "project": "game-lab",
                "environment": "test",
                "interface": "meerkateer",
                "interface_version": "1",
                "version": "0.0.0-fixture",
                "build": {
                    "git_sha": "0000000",
                    "git_branch": "fixture",
                    "build_time": "2026-01-01T00:00:00Z",
                },
                "checks": (
                    {"fixture": {"ok": True}}
                    if healthy
                    else {"fixture": {"ok": False, "error": "fixture_failure"}}
                ),
                "timestamp": "2026-01-01T00:00:00Z",
            }
            if self.path == "/ready":
                body["ready"] = healthy
                body.pop("status")
                body.pop("version")
                body.pop("build")
            self.respond(200 if healthy else 503, body)
            return
        self.respond(404, {"error": "not_found"})

    def log_message(self, format: str, *args: object) -> None:
        return

    def respond(self, status: int, body: dict[str, object]) -> None:
        encoded = json.dumps(body, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bind", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=18080)
    parser.add_argument("--mode", choices=("healthy", "degraded"), default="healthy")
    arguments = parser.parse_args()
    Handler.mode = arguments.mode
    server = ThreadingHTTPServer((arguments.bind, arguments.port), Handler)
    print(f"fixture MKS-1 server listening on {arguments.bind}:{arguments.port}")
    server.serve_forever()


if __name__ == "__main__":
    main()
