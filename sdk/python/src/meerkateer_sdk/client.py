"""Bounded, idempotent MKS-1 client using only the Python standard library."""

from __future__ import annotations

import json
import ipaddress
import os
import socket
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from datetime import datetime, timezone
from typing import Any


_MAX_RESPONSE_BYTES = 64 * 1024
_ENVIRONMENTS = {"development", "staging", "production", "test", "local"}


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class MeerkateerError(RuntimeError):
    """A safe SDK failure that never contains the service credential."""

    def __init__(self, message: str, *, status: int | None = None, code: str | None = None):
        super().__init__(message)
        self.status = status
        self.code = code


class Meerkateer:
    """Send service heartbeat, event, and deployment facts to Meerkateer."""

    def __init__(
        self,
        *,
        url: str,
        service_key: str,
        project: str,
        service: str,
        environment: str = "production",
        timeout: float = 5.0,
        max_retries: int = 2,
        _opener: Any | None = None,
    ) -> None:
        self.url = _validate_url(url)
        if not service_key.startswith("mks_sk_"):
            raise ValueError("service_key is not a Meerkateer service credential")
        if environment not in _ENVIRONMENTS:
            raise ValueError("environment is unsupported")
        if not project or not service:
            raise ValueError("project and service are required")
        if not 0.1 <= timeout <= 60:
            raise ValueError("timeout must be between 0.1 and 60 seconds")
        if not 0 <= max_retries <= 5:
            raise ValueError("max_retries must be between 0 and 5")
        self._service_key = service_key
        self.project = project
        self.service = service
        self.environment = environment
        self.timeout = timeout
        self.max_retries = max_retries
        self._opener = _opener or urllib.request.build_opener(_NoRedirect())

    @classmethod
    def from_env(cls) -> "Meerkateer":
        """Build a client from the environment variables shown by the Console."""
        required = {
            "url": "MEERKATEER_URL",
            "service_key": "MEERKATEER_SERVICE_KEY",
            "project": "MEERKATEER_PROJECT",
            "service": "MEERKATEER_SERVICE",
        }
        values: dict[str, str] = {}
        for argument, variable in required.items():
            value = os.environ.get(variable)
            if not value:
                raise MeerkateerError(f"{variable} is required")
            values[argument] = value
        return cls(
            **values,
            environment=os.environ.get("MEERKATEER_ENVIRONMENT", "production"),
        )

    def heartbeat(
        self,
        status: str = "ok",
        *,
        message: str | None = None,
        idempotency_key: str | None = None,
    ) -> dict[str, Any]:
        """Report current process health: ok, degraded, or down."""
        if status not in {"ok", "degraded", "down"}:
            raise ValueError("heartbeat status must be ok, degraded, or down")
        payload = self._base_payload()
        payload.update({"status": status, "timestamp": _now()})
        if message is not None:
            payload["message"] = message
        return self._send("/v1/ingest/heartbeat", payload, idempotency_key)

    def event(
        self,
        kind: str,
        *,
        level: str = "info",
        message: str | None = None,
        count: int = 1,
        idempotency_key: str | None = None,
    ) -> dict[str, Any]:
        """Report a bounded operational event without player or secret data."""
        if level not in {"info", "warning", "error", "critical"}:
            raise ValueError("event level is unsupported")
        if not 1 <= count <= 1_000_000:
            raise ValueError("event count must be between 1 and 1000000")
        payload = self._base_payload()
        payload.update({"level": level, "kind": kind, "count": count, "timestamp": _now()})
        if message is not None:
            payload["message"] = message
        return self._send("/v1/ingest/event", payload, idempotency_key)

    def deploy(
        self,
        version: str,
        commit: str,
        *,
        status: str = "finished",
        idempotency_key: str | None = None,
    ) -> dict[str, Any]:
        """Report a deployment transition: started, finished, or failed."""
        if status not in {"started", "finished", "failed"}:
            raise ValueError("deployment status is unsupported")
        payload = self._base_payload()
        payload.update(
            {
                "version": version,
                "commit": commit,
                "status": status,
                "timestamp": _now(),
            }
        )
        return self._send("/v1/ingest/deploy", payload, idempotency_key)

    def _base_payload(self) -> dict[str, Any]:
        return {
            "interface_version": "1",
            "service": self.service,
            "project": self.project,
            "environment": self.environment,
        }

    def _send(
        self,
        path: str,
        payload: dict[str, Any],
        idempotency_key: str | None,
    ) -> dict[str, Any]:
        key = idempotency_key or str(uuid.uuid4())
        try:
            uuid.UUID(key)
        except ValueError as error:
            raise ValueError("idempotency_key must be a UUID") from error
        data = json.dumps(payload, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
        endpoint = urllib.parse.urljoin(self.url, path.lstrip("/"))
        request = urllib.request.Request(
            endpoint,
            data=data,
            method="POST",
            headers={
                "Accept": "application/json",
                "Authorization": f"Bearer {self._service_key}",
                "Content-Type": "application/json",
                "Idempotency-Key": key,
                "User-Agent": "meerkateer-python-sdk/0.2.0",
            },
        )
        last_error: BaseException | None = None
        for attempt in range(self.max_retries + 1):
            try:
                with self._opener.open(request, timeout=self.timeout) as response:
                    body = _read_bounded(response)
                    if response.status not in {200, 202}:
                        raise _response_error(response.status, body)
                    parsed = json.loads(body)
                    if not isinstance(parsed, dict):
                        raise MeerkateerError("Meerkateer returned an invalid acknowledgement")
                    return parsed
            except urllib.error.HTTPError as error:
                body = _read_bounded(error)
                failure = _response_error(error.code, body)
                if error.code != 429 and error.code < 500:
                    raise failure from None
                last_error = failure
            except (urllib.error.URLError, TimeoutError, socket.timeout) as error:
                last_error = error
            if attempt < self.max_retries:
                time.sleep(min(0.25 * (2**attempt), 1.0))
        raise MeerkateerError("telemetry delivery failed after bounded retries") from last_error


def _validate_url(value: str) -> str:
    parsed = urllib.parse.urlsplit(value)
    if parsed.scheme not in {"http", "https"} or not parsed.hostname:
        raise ValueError("url must be an absolute HTTP(S) URL")
    if parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise ValueError("url must not contain credentials, query, or fragment")
    if parsed.path not in {"", "/"}:
        raise ValueError("url must not contain a path")
    loopback = parsed.hostname == "localhost"
    try:
        loopback = loopback or ipaddress.ip_address(parsed.hostname).is_loopback
    except ValueError:
        pass
    if parsed.scheme != "https" and not loopback:
        raise ValueError("HTTPS is required unless url is loopback")
    return urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, "/", "", ""))


def _read_bounded(response: Any) -> bytes:
    length = response.headers.get("Content-Length")
    if length is not None and int(length) > _MAX_RESPONSE_BYTES:
        raise MeerkateerError("Meerkateer response exceeded the size limit")
    body = response.read(_MAX_RESPONSE_BYTES + 1)
    if len(body) > _MAX_RESPONSE_BYTES:
        raise MeerkateerError("Meerkateer response exceeded the size limit")
    return body


def _response_error(status: int, body: bytes) -> MeerkateerError:
    code = "unexpected_response"
    try:
        parsed = json.loads(body)
        if isinstance(parsed, dict) and isinstance(parsed.get("code"), str):
            code = parsed["code"]
    except (UnicodeDecodeError, json.JSONDecodeError):
        pass
    return MeerkateerError(
        f"Meerkateer rejected telemetry with HTTP {status} ({code})",
        status=status,
        code=code,
    )


def _now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
