from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from meerkateer_sdk import Meerkateer  # noqa: E402


class Response:
    status = 202
    headers = {"Content-Length": "75"}

    def __init__(self) -> None:
        self.body = json.dumps(
            {
                "status": "accepted",
                "idempotency_key": "00000000-0000-4000-8000-000000000001",
            }
        ).encode()

    def read(self, amount: int) -> bytes:
        return self.body[:amount]

    def __enter__(self) -> "Response":
        return self

    def __exit__(self, *_: object) -> None:
        return None


class Opener:
    def __init__(self) -> None:
        self.requests = []

    def open(self, request, timeout: float) -> Response:
        self.requests.append((request, timeout))
        return Response()


class ClientTests(unittest.TestCase):
    def client(self, opener: Opener) -> Meerkateer:
        return Meerkateer(
            url="http://127.0.0.1:6510",
            service_key="mks_sk_test_credential",
            project="arena",
            service="game-server-01",
            _opener=opener,
        )

    def test_heartbeat_uses_bound_identity_and_idempotency(self) -> None:
        opener = Opener()
        result = self.client(opener).heartbeat(
            "degraded",
            message="tick loop delayed",
            idempotency_key="00000000-0000-4000-8000-000000000001",
        )
        request, timeout = opener.requests[0]
        payload = json.loads(request.data)
        self.assertEqual(result["status"], "accepted")
        self.assertEqual(timeout, 5.0)
        self.assertEqual(request.full_url, "http://127.0.0.1:6510/v1/ingest/heartbeat")
        self.assertEqual(request.get_header("Idempotency-key"), "00000000-0000-4000-8000-000000000001")
        self.assertEqual(payload["project"], "arena")
        self.assertEqual(payload["service"], "game-server-01")
        self.assertEqual(payload["status"], "degraded")

    def test_requires_https_away_from_loopback(self) -> None:
        with self.assertRaisesRegex(ValueError, "HTTPS is required"):
            Meerkateer(
                url="http://ops.example.com",
                service_key="mks_sk_test_credential",
                project="arena",
                service="game-server-01",
            )

    def test_validates_public_enums_before_network(self) -> None:
        opener = Opener()
        client = self.client(opener)
        with self.assertRaises(ValueError):
            client.heartbeat("maybe")
        with self.assertRaises(ValueError):
            client.event("game_server_failed", level="debug")
        with self.assertRaises(ValueError):
            client.deploy("1.2.3", "abcdef", status="unknown")
        self.assertEqual(opener.requests, [])


if __name__ == "__main__":
    unittest.main()
