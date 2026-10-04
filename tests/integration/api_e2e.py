#!/usr/bin/env python3
"""Real API/PostgreSQL regression suite for identity, credentials, and ingestion."""

from __future__ import annotations

import datetime as dt
import hashlib
import http.cookiejar
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
import uuid
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from typing import Any


BASE_URL = os.environ.get("MEERKATEER_E2E_URL", "http://127.0.0.1:6510")
BOOTSTRAP_TOKEN = os.environ.get("MEERKATEER_BOOTSTRAP_TOKEN")
AGENT_BIN = os.environ.get("MEERKATEER_AGENT_BIN")
RUST_SDK_BIN = os.environ.get("MEERKATEER_RUST_SDK_BIN")
E2E_PROJECT = os.environ.get("MEERKATEER_E2E_PROJECT")
E2E_DB = os.environ.get("MEERKATEER_E2E_DB")
E2E_DB_OWNER = os.environ.get("MEERKATEER_E2E_DB_OWNER")


@dataclass
class Response:
    status: int
    body: Any
    headers: Any


cookies = http.cookiejar.CookieJar()
client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookies))


def call(
    method: str,
    path: str,
    *,
    body: Any | None = None,
    headers: dict[str, str] | None = None,
    raw: bytes | None = None,
) -> Response:
    request_headers = dict(headers or {})
    data = raw
    if body is not None:
        data = json.dumps(body, separators=(",", ":")).encode()
        request_headers.setdefault("Content-Type", "application/json")
    request = urllib.request.Request(
        f"{BASE_URL}{path}", data=data, headers=request_headers, method=method
    )
    try:
        response = client.open(request, timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    raw_body = response.read()
    parsed: Any = None
    if raw_body:
        try:
            parsed = json.loads(raw_body)
        except json.JSONDecodeError:
            parsed = raw_body
    return Response(response.status, parsed, response.headers)


def expect(response: Response, status: int, code: str | None = None) -> Response:
    assert response.status == status, (response.status, response.body)
    if code is not None:
        assert response.body == {"code": code}, response.body
    return response


def isolated_call(method: str, path: str, body: Any) -> Response:
    return isolated_request(method, path, body=body)


def isolated_request(
    method: str,
    path: str,
    *,
    body: Any | None = None,
    headers: dict[str, str] | None = None,
) -> Response:
    request_headers = dict(headers or {})
    payload = None
    if body is not None:
        payload = json.dumps(body, separators=(",", ":")).encode()
        request_headers.setdefault("Content-Type", "application/json")
    request = urllib.request.Request(
        f"{BASE_URL}{path}",
        data=payload,
        headers={"Accept": "application/json", **request_headers},
        method=method,
    )
    isolated_client = urllib.request.build_opener()
    try:
        response = isolated_client.open(request, timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    raw_body = response.read()
    parsed = json.loads(raw_body) if raw_body else None
    return Response(response.status, parsed, response.headers)


def force_token_expired(secret: str, table: str = "member_invitations") -> None:
    assert E2E_PROJECT and E2E_DB and E2E_DB_OWNER, "integration database context is required"
    assert table in {"member_invitations", "member_password_resets"}
    digest = hashlib.sha256(secret.encode()).hexdigest()
    statement = (
        f"UPDATE {table} SET created_at = now() - interval '10 minutes', "
        "expires_at = now() - interval '1 second' "
        f"WHERE token_digest = decode('{digest}', 'hex');"
    )
    subprocess.run(
        [
            "docker",
            "compose",
            "-p",
            E2E_PROJECT,
            "exec",
            "-T",
            "postgres",
            "psql",
            "-U",
            E2E_DB_OWNER,
            "-d",
            E2E_DB,
            "-v",
            "ON_ERROR_STOP=1",
        ],
        input=statement,
        check=True,
        capture_output=True,
        text=True,
        timeout=20,
    )


def force_invitation_expired(secret: str) -> None:
    force_token_expired(secret)


def utc(offset: dt.timedelta = dt.timedelta()) -> str:
    value = dt.datetime.now(dt.timezone.utc) + offset
    return value.replace(microsecond=0).isoformat().replace("+00:00", "Z")


def csrf() -> str:
    for cookie in cookies:
        if cookie.name == "meerkateer_csrf":
            return cookie.value
    raise AssertionError("CSRF cookie was not issued")


def browser_headers() -> dict[str, str]:
    return {"X-Meerkateer-CSRF": csrf()}


def bearer(secret: str, idempotency_key: str | None = None) -> dict[str, str]:
    result = {"Authorization": f"Bearer {secret}"}
    if idempotency_key is not None:
        result["Idempotency-Key"] = idempotency_key
    return result


def run_agent(arguments: list[str], *, token: str | None = None) -> dict[str, Any]:
    assert AGENT_BIN, "MEERKATEER_AGENT_BIN is required"
    environment = dict(os.environ)
    if token is not None:
        environment["MEERKATEER_ENROLLMENT_TOKEN"] = token
    completed = subprocess.run(
        [AGENT_BIN, *arguments],
        check=True,
        capture_output=True,
        text=True,
        env=environment,
        timeout=20,
    )
    output = completed.stdout + completed.stderr
    assert "mka_agent_" not in output and "mka_enroll_" not in output, output
    try:
        parsed = json.loads(completed.stdout)
        if isinstance(parsed, dict):
            return parsed
    except json.JSONDecodeError:
        pass
    for line in reversed(completed.stdout.splitlines()):
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict):
            return parsed
    raise AssertionError(("agent emitted no JSON result", completed.stdout, completed.stderr))


def bootstrap_and_inventory() -> tuple[str, str, str, str]:
    assert BOOTSTRAP_TOKEN, "MEERKATEER_BOOTSTRAP_TOKEN is required"
    initial_instance = expect(call("GET", "/v1/instance"), 200).body
    assert initial_instance == {"deployment_mode": "community", "setup_required": True}
    bootstrap = {
        "tenant_slug": "e2e-operator",
        "tenant_name": "E2E Operator",
        "owner_email": "e2e-owner@example.com",
        "owner_name": "E2E Owner",
        "owner_password": "test-only-owner-password-123",
    }
    auth = {"Authorization": f"Bearer {BOOTSTRAP_TOKEN}"}
    expect(call("POST", "/v1/bootstrap", body={key: value for key, value in bootstrap.items() if key != "owner_password"}, headers=auth), 400)
    expect(call("POST", "/v1/bootstrap", body={**bootstrap, "owner_password": "too-short"}, headers=auth), 400, "invalid_request")
    expect(
        call(
            "POST",
            "/v1/bootstrap",
            body=bootstrap,
            headers={"Authorization": f"Bearer {BOOTSTRAP_TOKEN}"},
        ),
        201,
    )
    expect(call("GET", "/v1/session"), 200)
    ready_instance = expect(call("GET", "/v1/instance"), 200).body
    assert ready_instance == {"deployment_mode": "community", "setup_required": False}
    expect(
        call(
            "POST",
            "/v1/bootstrap",
            body=bootstrap,
            headers={"Authorization": f"Bearer {BOOTSTRAP_TOKEN}"},
        ),
        409,
        "bootstrap_already_completed",
    )
    cookies.clear()
    expect(call("GET", "/v1/session"), 401, "authentication_required")
    expect(
        call("POST", "/v1/session/password-login", body={"email": bootstrap["owner_email"], "password": "wrong-password-value"}),
        401,
        "invalid_credentials",
    )
    expect(
        call("POST", "/v1/session/password-login", body={"email": bootstrap["owner_email"], "password": bootstrap["owner_password"]}),
        200,
    )
    expect(call("GET", "/v1/session"), 200)
    expect(call("POST", "/v1/alerts/test"), 403, "csrf_failed")
    expect(call("POST", "/v1/alerts/test", headers=browser_headers()), 409, "alert_webhook_not_configured")
    previous_session = {cookie.name: cookie.value for cookie in cookies}
    cookies.clear()
    expect(
        call("POST", "/v1/session/password-setup", body={"password": "new-test-only-password-456"}, headers={"Authorization": "Bearer incorrect-admin-token"}),
        401,
        "invalid_admin_token",
    )
    expect(
        call("POST", "/v1/session/password-setup", body={"password": "new-test-only-password-456"}, headers={"Authorization": f"Bearer {BOOTSTRAP_TOKEN}"}),
        200,
    )
    expect(
        call(
            "GET",
            "/v1/session",
            headers={
                "Cookie": f"meerkateer_session={previous_session['meerkateer_session']}"
            },
        ),
        401,
        "authentication_required",
    )
    expect(call("GET", "/v1/session"), 200)
    cookies.clear()
    expect(
        call("POST", "/v1/session/password-login", body={"email": bootstrap["owner_email"], "password": bootstrap["owner_password"]}),
        401,
        "invalid_credentials",
    )
    expect(
        call("POST", "/v1/session/password-login", body={"email": bootstrap["owner_email"], "password": "new-test-only-password-456"}),
        200,
    )
    expect(
        call(
            "POST",
            "/v1/session/local-login",
            headers={"Authorization": "Bearer incorrect-admin-token"},
        ),
        404,
    )
    expect(
        call(
            "POST",
            "/v1/session/local-login",
            headers={"Authorization": f"Bearer {BOOTSTRAP_TOKEN}"},
        ),
        404,
    )
    expect(call("GET", "/v1/session"), 200)
    expect(
        call(
            "POST",
            "/v1/projects",
            body={"slug": "blocked", "display_name": "Blocked"},
        ),
        403,
        "csrf_failed",
    )
    project = expect(
        call(
            "POST",
            "/v1/projects",
            body={"slug": "arena-ops", "display_name": "Arena Ops"},
            headers=browser_headers(),
        ),
        201,
    ).body
    expect(
        call(
            "POST",
            "/v1/projects",
            body={"slug": "arena-ops", "display_name": "Duplicate"},
            headers=browser_headers(),
        ),
        409,
        "already_exists",
    )
    listed = expect(call("GET", "/v1/projects"), 200).body
    assert [item["id"] for item in listed["items"]] == [project["id"]]
    expect(
        call(
            "POST",
            f"/v1/projects/{project['id']}/services",
            body={"slug": "bad-service", "environment": "invalid"},
            headers=browser_headers(),
        ),
        400,
        "invalid_request",
    )
    service = expect(
        call(
            "POST",
            f"/v1/projects/{project['id']}/services",
            body={"slug": "game-api", "environment": "production"},
            headers=browser_headers(),
        ),
        201,
    ).body
    assert service["game"] is None
    expect(
        call(
            "POST",
            f"/v1/projects/{project['id']}/services",
            body={"slug": "bad-game", "environment": "production", "game": {"kind": "minecraft_java", "host": "http://localhost", "port": 25565}},
            headers=browser_headers(),
        ),
        400,
        "invalid_request",
    )
    game = expect(
        call(
            "POST",
            f"/v1/projects/{project['id']}/services",
            body={"slug": "survival-01", "environment": "production", "game": {"kind": "minecraft_java", "host": "127.0.0.1", "port": 25565}},
            headers=browser_headers(),
        ),
        201,
    ).body
    assert game["game"] == {"kind": "minecraft_java", "host": "127.0.0.1", "port": 25565}
    expect(call("POST", f"/v1/services/{game['id']}/game-probe"), 403, "csrf_failed")
    expect(
        call("POST", f"/v1/services/{game['id']}/game-probe", headers=browser_headers()),
        400,
        "unsafe_probe_destination",
    )
    expect(
        call("POST", "/v1/services/00000000-0000-4000-8000-000000000099/game-probe", headers=browser_headers()),
        404,
        "not_found",
    )
    network_probe = {
        "kind": "https",
        "host": "127.0.0.1",
        "port": 443,
        "path": "/",
        "expected_status": None,
        "timeout_ms": 5000,
    }
    expect(
        call("POST", f"/v1/services/{service['id']}/network-probe", body=network_probe),
        403,
        "csrf_failed",
    )
    expect(
        call(
            "POST",
            f"/v1/services/{service['id']}/network-probe",
            body=network_probe,
            headers=browser_headers(),
        ),
        400,
        "unsafe_probe_destination",
    )
    expect(
        call(
            "POST",
            f"/v1/services/{service['id']}/network-probe",
            body={**network_probe, "host": "https://example.com"},
            headers=browser_headers(),
        ),
        400,
        "invalid_probe_request",
    )
    expect(
        call(
            "POST",
            "/v1/services/00000000-0000-4000-8000-000000000099/network-probe",
            body=network_probe,
            headers=browser_headers(),
        ),
        404,
        "not_found",
    )
    assert service["status"] == {
        "state": "unknown",
        "reported_state": None,
        "stale": False,
        "last_sequence": None,
        "observed_at": None,
        "updated_at": None,
    }
    services = expect(
        call("GET", f"/v1/projects/{project['id']}/services"), 200
    ).body
    assert services["items"][0]["status"]["state"] == "unknown"
    assert any(item["id"] == game["id"] and item["game"] == game["game"] for item in services["items"])
    credential = expect(
        call(
            "POST",
            f"/v1/services/{service['id']}/credentials",
            headers=browser_headers(),
        ),
        201,
    ).body
    assert credential["secret"].startswith("mks_sk_")
    assert len(credential["secret"].split("_")) == 5
    return (
        project["id"],
        service["id"],
        credential["credential_id"],
        credential["secret"],
    )


def mks_ingestion(
    project_id: str, service_id: str, credential_id: str, secret: str
) -> None:
    owner_session = expect(call("GET", "/v1/session"), 200).body
    owner_user_id = owner_session["user_id"]
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            raw=b"{bad",
            headers={"Content-Type": "application/json"},
        ),
        401,
        "unauthorized",
    )
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            raw=b"{bad",
            headers={
                **bearer(secret, str(uuid.uuid4())),
                "Content-Type": "application/json",
            },
        ),
        400,
        "invalid_payload",
    )
    tampered_parts = secret.split("_")
    tampered_parts[2] = uuid.uuid4().hex
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            raw=b"{}",
            headers={
                **bearer("_".join(tampered_parts), str(uuid.uuid4())),
                "Content-Type": "application/json",
            },
        ),
        401,
        "unauthorized",
    )
    sdk_environment = {
        **os.environ,
        "PYTHONPATH": str(pathlib.Path.cwd() / "sdk" / "python" / "src"),
        "MEERKATEER_URL": BASE_URL,
        "MEERKATEER_SERVICE_KEY": secret,
        "MEERKATEER_PROJECT": "arena-ops",
        "MEERKATEER_SERVICE": "game-api",
        "MEERKATEER_ENVIRONMENT": "production",
    }
    sdk = subprocess.run(
        [
            sys.executable,
            "-c",
            "from meerkateer_sdk import Meerkateer; "
            "Meerkateer.from_env().event('sdk_connected', message='SDK integration ready')",
        ],
        check=True,
        capture_output=True,
        text=True,
        env=sdk_environment,
        timeout=20,
    )
    assert secret not in sdk.stdout + sdk.stderr
    assert RUST_SDK_BIN, "MEERKATEER_RUST_SDK_BIN is required"
    rust_sdk = subprocess.run(
        [RUST_SDK_BIN],
        check=True,
        capture_output=True,
        text=True,
        env=sdk_environment,
        timeout=20,
    )
    assert secret not in rust_sdk.stdout + rust_sdk.stderr
    timestamp = utc(dt.timedelta(minutes=-4))
    heartbeat = {
        "interface_version": "1",
        "service": "game-api",
        "project": "arena-ops",
        "environment": "production",
        "status": "ok",
        "message": "healthy",
        "timestamp": timestamp,
    }
    heartbeat_key = str(uuid.uuid4())
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            raw=json.dumps(heartbeat).encode(),
            headers={
                **bearer(secret, str(uuid.uuid4())),
                "Content-Type": "text/plain",
            },
        ),
        400,
        "invalid_payload",
    )
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            raw=b"{" + (b"x" * (16 * 1024 + 1)) + b"}",
            headers={
                **bearer(secret, str(uuid.uuid4())),
                "Content-Type": "application/json",
            },
        ),
        413,
    )
    accepted = expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=heartbeat,
            headers=bearer(secret, heartbeat_key),
        ),
        202,
    )
    assert accepted.body["status"] == "accepted"
    projected = expect(
        call("GET", f"/v1/projects/{project_id}/services"), 200
    ).body["items"][0]["status"]
    assert projected["state"] == "unknown", projected
    assert projected["reported_state"] == "online", projected
    assert projected["stale"] is True, projected
    assert projected["observed_at"] == timestamp, projected
    duplicate = expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=heartbeat,
            headers=bearer(secret, heartbeat_key),
        ),
        200,
    )
    assert duplicate.body["status"] == "duplicate"
    changed = dict(heartbeat, status="down")
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=changed,
            headers=bearer(secret, heartbeat_key),
        ),
        409,
        "idempotency_conflict",
    )
    expect(
        call(
            "POST", "/v1/ingest/heartbeat", body=heartbeat, headers=bearer(secret)
        ),
        400,
        "idempotency_key_required",
    )
    for bad_timestamp in (utc(dt.timedelta(minutes=6)), utc(dt.timedelta(minutes=-16))):
        expect(
            call(
                "POST",
                "/v1/ingest/heartbeat",
                body=dict(heartbeat, timestamp=bad_timestamp),
                headers=bearer(secret, str(uuid.uuid4())),
            ),
            400,
            "invalid_payload",
        )
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=dict(heartbeat, project="another-tenant"),
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        400,
        "invalid_payload",
    )
    fresh_timestamp = utc()
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=dict(
                heartbeat,
                timestamp=fresh_timestamp,
                status="down",
                message="game process exited",
            ),
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    projected = expect(
        call("GET", f"/v1/projects/{project_id}/services"), 200
    ).body["items"][0]["status"]
    assert projected["state"] == "offline", projected
    assert projected["reported_state"] == "offline", projected
    assert projected["stale"] is False, projected
    assert projected["observed_at"] == fresh_timestamp, projected
    open_incidents = expect(
        call("GET", f"/v1/incidents?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert len(open_incidents) == 1 and open_incidents[0]["status"] == "open", open_incidents
    incident_id = open_incidents[0]["id"]
    expect(
        call("POST", f"/v1/incidents/{incident_id}/acknowledge"),
        403,
        "csrf_failed",
    )
    expect(
        call(
            "POST",
            f"/v1/incidents/{incident_id}/acknowledge",
            headers=browser_headers(),
        ),
        204,
    )
    expect(
        call(
            "POST",
            f"/v1/incidents/{incident_id}/acknowledge",
            headers=browser_headers(),
        ),
        204,
    )
    expect(
        call(
            "PUT",
            f"/v1/incidents/{incident_id}/assignment",
            body={"assigned": True},
            headers=browser_headers(),
        ),
        204,
    )
    expect(
        call(
            "PUT",
            f"/v1/incidents/{incident_id}/assignment",
            body={"assigned": True},
            headers=browser_headers(),
        ),
        204,
    )
    expect(
        call(
            "POST",
            f"/v1/incidents/{incident_id}/notes",
            body={"note": "   "},
            headers=browser_headers(),
        ),
        400,
        "invalid_request",
    )
    expect(
        call(
            "POST",
            f"/v1/incidents/{incident_id}/notes",
            body={"note": "ตรวจสอบแล้ว: process หยุดจริง"},
            headers=browser_headers(),
        ),
        204,
    )
    expect(
        call(
            "POST",
            f"/v1/incidents/{uuid.uuid4()}/notes",
            body={"note": "must stay tenant-hidden"},
            headers=browser_headers(),
        ),
        404,
        "not_found",
    )
    after_operator_action = expect(
        call("GET", f"/v1/projects/{project_id}/services"), 200
    ).body["items"][0]["status"]
    assert after_operator_action["state"] == "offline", after_operator_action
    open_incidents = expect(
        call("GET", f"/v1/incidents?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert open_incidents[0]["acknowledged_at"], open_incidents
    assert open_incidents[0]["acknowledged_by"] == owner_session["display_name"], open_incidents
    assert open_incidents[0]["assigned_to"] == owner_user_id, open_incidents
    assert open_incidents[0]["assignee"] == owner_session["display_name"], open_incidents
    activity = expect(
        call("GET", f"/v1/incidents/activity?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert [item["kind"] for item in activity] == ["note", "assigned", "acknowledged"], activity
    assert activity[0]["note"] == "ตรวจสอบแล้ว: process หยุดจริง", activity
    recovered_timestamp = utc(dt.timedelta(seconds=1))
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=dict(
                heartbeat,
                timestamp=recovered_timestamp,
                message="game process restarted",
            ),
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    projected = expect(
        call("GET", f"/v1/projects/{project_id}/services"), 200
    ).body["items"][0]["status"]
    assert projected["state"] == "online", projected
    assert projected["reported_state"] == "online", projected
    assert projected["observed_at"] == recovered_timestamp, projected
    incidents = expect(
        call("GET", f"/v1/incidents?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert len(incidents) == 1, incidents
    assert incidents[0]["status"] == "resolved", incidents
    assert incidents[0]["cause"] == "game process exited", incidents
    assert incidents[0]["resolved_at"] == recovered_timestamp, incidents
    assert incidents[0]["acknowledged_at"] and incidents[0]["assigned_to"] == owner_user_id, incidents
    expect(
        call(
            "PUT",
            f"/v1/incidents/{incident_id}/assignment",
            body={"assigned": False},
            headers=browser_headers(),
        ),
        204,
    )
    deliveries = expect(
        call("GET", f"/v1/alerts/deliveries?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert [item["transition"] for item in deliveries[:2]] == ["recovered", "down"], deliveries
    assert all(item["status"] == "skipped_unconfigured" for item in deliveries[:2]), deliveries
    assert all(item["replay_of"] is None for item in deliveries[:2]), deliveries
    policy = expect(call("GET", "/v1/alerts/policy"), 200).body
    assert policy == {
        "enabled": True,
        "notify_down": True,
        "notify_recovered": True,
        "cooldown_seconds": 0,
        "webhook_configured": False,
        "updated_at": None,
    }, policy
    expect(
        call(
            "PUT",
            "/v1/alerts/policy",
            body={"enabled": False, "notify_down": True, "notify_recovered": True, "cooldown_seconds": 120},
        ),
        403,
        "csrf_failed",
    )
    policy = expect(
        call(
            "PUT",
            "/v1/alerts/policy",
            body={"enabled": False, "notify_down": True, "notify_recovered": True, "cooldown_seconds": 120},
            headers=browser_headers(),
        ),
        200,
    ).body
    assert policy["enabled"] is False and policy["cooldown_seconds"] == 120 and policy["updated_at"], policy
    maintenance = expect(
        call(
            "POST",
            "/v1/maintenance-windows",
            body={
                "project_id": project_id,
                "service_id": service_id,
                "title": "Rolling upgrade",
                "reason": "planned game process restart",
                "starts_at": utc(dt.timedelta(minutes=-1)),
                "ends_at": utc(dt.timedelta(minutes=10)),
            },
            headers=browser_headers(),
        ),
        201,
    ).body
    assert maintenance["service"] == "game-api", maintenance
    operations_base = dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
    operation_time = lambda seconds: (operations_base + dt.timedelta(seconds=seconds)).isoformat().replace("+00:00", "Z")
    maintenance_down = operation_time(2)
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=dict(
                heartbeat,
                timestamp=maintenance_down,
                status="down",
                message="planned process restart",
            ),
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    # A repeated newer down is evidence, but must not open or notify a second incident.
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=dict(heartbeat, timestamp=operation_time(3), status="down"),
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    incidents = expect(
        call("GET", f"/v1/incidents?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert len(incidents) == 2 and incidents[0]["status"] == "open", incidents
    deliveries = expect(
        call("GET", f"/v1/alerts/deliveries?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert len(deliveries) == 3, deliveries
    assert deliveries[0]["status"] == "suppressed", deliveries
    assert deliveries[0]["suppression_reason"] == "planned game process restart", deliveries
    # Older evidence is retained but cannot recover the newer outage.
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=dict(heartbeat, timestamp=operation_time(1), message="late packet"),
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    incidents = expect(
        call("GET", f"/v1/incidents?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert incidents[0]["status"] == "open", incidents
    final_recovery = operation_time(4)
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body=dict(heartbeat, timestamp=final_recovery, message="upgrade complete"),
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    incidents = expect(
        call("GET", f"/v1/incidents?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert incidents[0]["status"] == "resolved" and incidents[0]["resolved_at"] == final_recovery, incidents
    windows = expect(
        call("GET", f"/v1/maintenance-windows?project_id={project_id}&limit=20"), 200
    ).body["items"]
    assert len(windows) == 1 and windows[0]["cancelled_at"] is None, windows
    expect(call("DELETE", f"/v1/maintenance-windows/{maintenance['id']}"), 403, "csrf_failed")
    expect(
        call(
            "DELETE",
            f"/v1/maintenance-windows/{maintenance['id']}",
            headers=browser_headers(),
        ),
        204,
    )
    summary = expect(call("GET", "/v1/admin/summary"), 200).body
    assert summary["open_incidents"] == 0 and summary["active_maintenance_windows"] == 0, summary
    assert summary["worker_status"] == "never_seen", summary
    assert summary["worker_last_cycle_at"] is None, summary
    assert summary["worker_last_cycle_claimed"] == 0, summary
    audit = expect(call("GET", "/v1/audit-events?limit=100"), 200).body["items"]
    actions = {item["action"] for item in audit}
    assert {
        "alert.policy.update",
        "incident.acknowledge",
        "incident.assign",
        "incident.unassign",
        "incident.note",
        "maintenance.create",
        "maintenance.cancel",
    } <= actions, actions
    event = {
        "interface_version": "1",
        "service": "game-api",
        "project": "arena-ops",
        "environment": "production",
        "level": "error",
        "kind": "database_unavailable",
        "message": "dependency unavailable",
        "count": 1,
        "timestamp": timestamp,
    }
    expect(
        call(
            "POST",
            "/v1/ingest/event",
            body=event,
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    expect(
        call("GET", f"/v1/services/{service_id}/timeline?limit=0"),
        400,
        "invalid_request",
    )
    timeline = expect(
        call("GET", f"/v1/services/{service_id}/timeline?limit=20"), 200
    ).body["items"]
    assert any(
        item["state"] == "offline" and item["message"] == "game process exited"
        for item in timeline
    ), timeline
    assert any(
        item["state"] == "online" and item["message"] == "game process restarted"
        for item in timeline
    ), timeline
    assert any(
        item["kind"] == "event"
        and item["severity"] == "error"
        and item["title"] == "database unavailable"
        for item in timeline
    ), timeline
    deploy = {
        "interface_version": "1",
        "service": "game-api",
        "project": "arena-ops",
        "environment": "production",
        "version": "1.2.3",
        "commit": "abcdef123456",
        "status": "finished",
        "timestamp": timestamp,
    }
    expect(
        call(
            "POST",
            "/v1/ingest/deploy",
            body=deploy,
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    timeline = expect(
        call("GET", f"/v1/services/{service_id}/timeline?limit=20"), 200
    ).body["items"]
    assert any(
        item["kind"] == "deploy" and item["title"] == "Deploy 1.2.3 finished"
        for item in timeline
    ), timeline
    rotated = expect(
        call(
            "POST",
            f"/v1/services/{service_id}/credentials/{credential_id}/rotate",
            body={"overlap_seconds": 60},
            headers=browser_headers(),
        ),
        201,
    ).body
    expect(
        call(
            "POST",
            "/v1/ingest/event",
            body=event,
            headers=bearer(secret, str(uuid.uuid4())),
        ),
        202,
    )
    expect(
        call(
            "POST",
            "/v1/ingest/event",
            body=event,
            headers=bearer(rotated["secret"], str(uuid.uuid4())),
        ),
        202,
    )
    expect(
        call(
            "DELETE",
            f"/v1/services/{service_id}/credentials/{rotated['credential_id']}",
            headers=browser_headers(),
        ),
        204,
    )
    expect(
        call(
            "POST",
            "/v1/ingest/event",
            body=event,
            headers=bearer(rotated["secret"], str(uuid.uuid4())),
        ),
        401,
        "unauthorized",
    )


def enroll_agent(project_id: str) -> tuple[str, str]:
    expect(
        call(
            "POST",
            f"/v1/projects/{project_id}/enrollment-tokens",
            body={"expires_in_seconds": 299},
            headers=browser_headers(),
        ),
        400,
        "invalid_expiry",
    )
    issued = expect(
        call(
            "POST",
            f"/v1/projects/{project_id}/enrollment-tokens",
            body={"expires_in_seconds": 600},
            headers=browser_headers(),
        ),
        201,
    ).body
    assert issued["project_id"] == project_id
    token = issued["secret"]
    expect(
        call(
            "POST",
            "/v1/agents/enroll",
            body={"installation_id": str(uuid.uuid4()), "display_name": "Invalid"},
            headers=bearer("malformed"),
        ),
        401,
        "invalid_enrollment_token",
    )
    with tempfile.TemporaryDirectory(prefix="meerkateer-agent-e2e-") as directory:
        config_path = pathlib.Path(directory) / "agent.json"
        command = [
            "--config",
            str(config_path),
            "enroll",
            "--server",
            BASE_URL,
            "--name",
            "E2E Game Host",
        ]
        enrolled = run_agent(command, token=token)
        assert enrolled["status"] == "enrolled"
        assert enrolled["project_id"] == project_id
        config = json.loads(config_path.read_text(encoding="utf-8"))
        assert config["credential"].startswith("mka_agent_")
        if os.name != "nt":
            assert config_path.stat().st_mode & 0o777 == 0o600
        doctor = run_agent(["--config", str(config_path), "doctor"])
        assert doctor["status"] == "ok" and doctor["pending_batch"] is False
        empty_snapshot = expect(
            call("GET", f"/v1/agents/{config['agent_id']}/telemetry"), 200
        ).body
        assert empty_snapshot["connection_state"] == "never_seen"
        assert empty_snapshot["collection_state"] == "unavailable"
        assert empty_snapshot["snapshot_stale"] is True
        assert empty_snapshot["processes"] == []
        assert len(empty_snapshot["missing_metrics"]) == 6

        config["server_url"] = "http://127.0.0.1:1/"
        config_path.write_text(json.dumps(config), encoding="utf-8")
        failed = subprocess.run(
            [
                AGENT_BIN,
                "--config",
                str(config_path),
                "run",
                "--once",
                "--watch-process",
                "python3",
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=20,
        )
        assert failed.returncode != 0
        assert "mka_agent_" not in failed.stdout + failed.stderr
        status_path = config_path.with_name("agent.status.json")
        failed_status = json.loads(status_path.read_text(encoding="utf-8"))
        assert failed_status["last_attempt_at"]
        assert failed_status["last_success_at"] is None
        assert failed_status["last_error_code"] == "connect_failed"
        assert failed_status["last_error"] == "Could not open a connection to the API"
        assert "firewall" in failed_status["last_error_hint"].lower()
        assert "mka_agent_" not in status_path.read_text(encoding="utf-8")
        pending = json.loads(config_path.read_text(encoding="utf-8"))
        assert len(pending["pending_batches"]) == 1
        pending_batch = pending["pending_batches"][0]
        assert pending_batch["first_sequence"] == 1
        pending_last_sequence = pending_batch["last_sequence"]
        assert pending_last_sequence >= 7
        assert len(pending_batch["records"]) == pending_last_sequence
        process_records = [
            record
            for record in pending_batch["records"]
            if record["metric"] == "process.running"
        ]
        assert len(process_records) == 1
        assert process_records[0]["attributes"] == {"process": "python3"}
        assert process_records[0]["value"] >= 1
        pending_batch_id = pending_batch["batch_id"]
        pending["server_url"] = f"{BASE_URL}/"
        config_path.write_text(json.dumps(pending), encoding="utf-8")

        sent = run_agent(["--config", str(config_path), "run", "--once"])
        assert (
            sent["status"] == "accepted"
            and sent["accepted_through_sequence"] == pending_last_sequence
        )
        assert sent["batch_id"] == pending_batch_id
        persisted = json.loads(config_path.read_text(encoding="utf-8"))
        assert persisted["next_sequence"] == pending_last_sequence + 1
        assert "pending_batch" not in persisted
        assert "pending_batches" not in persisted
        recovered_status = json.loads(status_path.read_text(encoding="utf-8"))
        assert recovered_status["last_success_at"]
        assert recovered_status["last_error_at"] is None
        assert recovered_status["last_error_code"] is None
        assert recovered_status["last_error"] is None
        assert recovered_status["last_error_hint"] is None
        snapshot = expect(
            call("GET", f"/v1/agents/{config['agent_id']}/telemetry"), 200
        ).body
        assert snapshot["agent_id"] == config["agent_id"]
        assert snapshot["connection_state"] == "online"
        assert snapshot["collection_state"] == "complete"
        assert snapshot["snapshot_stale"] is False
        assert 0 <= snapshot["cpu_usage_percent"] <= 100
        assert snapshot["memory"]["used_bytes"] <= snapshot["memory"]["total_bytes"]
        assert snapshot["disk"]["used_bytes"] <= snapshot["disk"]["total_bytes"]
        if snapshot["platform"] == "linux":
            assert snapshot["inodes"]["used"] <= snapshot["inodes"]["total"]
        assert snapshot["missing_metrics"] == []
        assert len(snapshot["processes"]) == 1
        assert snapshot["processes"][0]["name"] == "python3"
        assert snapshot["processes"][0]["running"] is True
        assert snapshot["processes"][0]["instances"] >= 1
        assert snapshot["services"] == []
        expect(call("GET", f"/v1/agents/{uuid.uuid4()}/telemetry"), 404, "not_found")
        old_agent_secret = persisted["credential"]
        local_rotation = run_agent(
            ["--config", str(config_path), "rotate-credential"]
        )
        assert local_rotation["status"] == "rotated"
        assert "secret" not in local_rotation
        rotated_config = json.loads(config_path.read_text(encoding="utf-8"))
        assert rotated_config["credential"] != old_agent_secret
        assert rotated_config["credential_id"] == local_rotation["credential_id"]
        installation_id = config["installation_id"]
        enrollment_body = {
            "installation_id": installation_id,
            "display_name": "E2E Game Host",
        }
        agent_id = config["agent_id"]
        agent_secret = rotated_config["credential"]

    expect(
        call(
            "POST", "/v1/agents/enroll", body=enrollment_body, headers=bearer(token)
        ),
        401,
        "invalid_enrollment_token",
    )
    duplicate_token = expect(
        call(
            "POST",
            "/v1/enrollment-tokens",
            body={"expires_in_seconds": 600},
            headers=browser_headers(),
        ),
        201,
    ).body["secret"]
    expect(
        call(
            "POST",
            "/v1/agents/enroll",
            body=enrollment_body,
            headers=bearer(duplicate_token),
        ),
        409,
        "installation_already_enrolled",
    )
    agents = expect(call("GET", "/v1/agents"), 200).body["items"]
    assert len(agents) == 1 and agents[0]["status"] == "active"
    assert agents[0]["connection_state"] == "online"
    workspace_agents = expect(
        call("GET", f"/v1/projects/{project_id}/agents"), 200
    ).body["items"]
    assert [item["id"] for item in workspace_agents] == [agent_id]
    expect(
        call(
            "DELETE",
            f"/v1/projects/{project_id}/agents/{agent_id}",
        ),
        403,
        "csrf_failed",
    )
    expect(call("GET", "/v1/session"), 200)
    expect(
        call(
            "DELETE",
            f"/v1/projects/{project_id}/agents/{agent_id}",
            headers=browser_headers(),
        ),
        204,
    )
    assert expect(call("GET", f"/v1/projects/{project_id}/agents"), 200).body[
        "items"
    ] == []
    for _ in range(2):
        expect(
            call(
                "PUT",
                f"/v1/projects/{project_id}/agents/{agent_id}",
                headers=browser_headers(),
            ),
            204,
        )
    workspace_agents = expect(
        call("GET", f"/v1/projects/{project_id}/agents"), 200
    ).body["items"]
    assert [item["id"] for item in workspace_agents] == [agent_id]
    return agent_id, agent_secret


def batch(agent_id: str, batch_id: str, first: int, last: int) -> dict[str, Any]:
    timestamp = utc()
    records: list[dict[str, Any]] = []
    for sequence in range(first, last + 1):
        records.append(
            {
                "type": "sample",
                "sequence": sequence,
                "record_id": str(uuid.uuid5(uuid.NAMESPACE_OID, f"{agent_id}:{sequence}")),
                "observed_at": timestamp,
                "metric": "host.cpu.utilization",
                "value": 0.25,
                "attributes": {"core": "aggregate"},
            }
        )
    return {
        "protocol_version": "1",
        "agent_id": agent_id,
        "batch_id": batch_id,
        "first_sequence": first,
        "last_sequence": last,
        "sent_at": timestamp,
        "records": records,
    }


def mka_ingestion(agent_id: str, secret: str) -> None:
    first_id = str(uuid.uuid4())
    first = batch(agent_id, first_id, 41, 42)
    tampered_parts = secret.split("_")
    tampered_parts[2] = uuid.uuid4().hex
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=first,
            headers=bearer("_".join(tampered_parts), first_id),
        ),
        401,
        "unauthorized",
    )
    expect(
        call(
            "POST",
            f"/v1/agents/{uuid.uuid4()}/credentials/rotate",
            headers=bearer(secret),
        ),
        401,
        "unauthorized",
    )
    accepted = expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=first,
            headers=bearer(secret, first_id),
        ),
        202,
    )
    assert accepted.body["accepted_through_sequence"] == 42
    assert accepted.headers.get("X-Meerkateer-Sequence-Gap") == "true"
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=first,
            headers=bearer(secret, first_id),
        ),
        200,
    )
    changed = json.loads(json.dumps(first))
    changed["records"][0]["value"] = 0.5
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=changed,
            headers=bearer(secret, first_id),
        ),
        409,
        "sequence_or_idempotency_conflict",
    )
    second_id = str(uuid.uuid4())
    second = batch(agent_id, second_id, 43, 43)
    accepted = expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=second,
            headers=bearer(secret, second_id),
        ),
        202,
    )
    assert accepted.headers.get("X-Meerkateer-Sequence-Gap") is None
    overlap_id = str(uuid.uuid4())
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=batch(agent_id, overlap_id, 42, 42),
            headers=bearer(secret, overlap_id),
        ),
        409,
        "sequence_or_idempotency_conflict",
    )
    unsafe_id = str(uuid.uuid4())
    unsafe = batch(agent_id, unsafe_id, 44, 44)
    unsafe["records"] = [
        {
            "type": "event",
            "sequence": 44,
            "record_id": str(uuid.uuid4()),
            "observed_at": utc(),
            "kind": "agent_check_failed",
            "severity": "error",
            "message": "password=mka_agent_secret",
            "attributes": {},
        }
    ]
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=unsafe,
            headers=bearer(secret, unsafe_id),
        ),
        400,
        "invalid_batch",
    )
    mismatch_id = str(uuid.uuid4())
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=batch(agent_id, str(uuid.uuid4()), 44, 44),
            headers=bearer(secret, mismatch_id),
        ),
        400,
        "invalid_batch",
    )
    future_id = str(uuid.uuid4())
    future = batch(agent_id, future_id, 44, 44)
    future["sent_at"] = utc(dt.timedelta(minutes=6))
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=future,
            headers=bearer(secret, future_id),
        ),
        400,
        "invalid_batch",
    )
    rotated = expect(
        call(
            "POST",
            f"/v1/agents/{agent_id}/credentials/rotate",
            headers=bearer(secret),
        ),
        201,
    ).body
    final_id = str(uuid.uuid4())
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=batch(agent_id, final_id, 44, 44),
            headers=bearer(rotated["secret"], final_id),
        ),
        202,
    )
    expect(
        call(
            "DELETE", f"/v1/agents/{agent_id}", headers=browser_headers()
        ),
        204,
    )
    revoked_id = str(uuid.uuid4())
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            body=batch(agent_id, revoked_id, 45, 45),
            headers=bearer(rotated["secret"], revoked_id),
        ),
        401,
        "unauthorized",
    )
    oversized = b"{" + (b"x" * (2 * 1024 * 1024 + 1)) + b"}"
    expect(
        call(
            "POST",
            "/v1/agent/telemetry",
            raw=oversized,
            headers={**bearer(secret, str(uuid.uuid4())), "Content-Type": "application/json"},
        ),
        413,
    )


def rate_limit_boundary() -> None:
    cookies.clear()
    limited: Response | None = None
    for _ in range(301):
        response = call("GET", "/v1/session")
        if response.status == 429:
            limited = response
            break
        expect(response, 401, "authentication_required")
    assert limited is not None, "authentication rate limit was not enforced"
    expect(limited, 429, "rate_limited")
    assert limited.headers.get("Retry-After") == "60", limited.headers


def internal_member_invitations() -> None:
    expect(
        call(
            "POST",
            "/v1/members/invitations",
            body={
                "username": "nina_ops",
                "display_name": "Nina Operator",
                "role": "viewer",
                "expires_in_seconds": 86_400,
            },
        ),
        403,
        "csrf_failed",
    )
    expect(
        call(
            "POST",
            "/v1/members/invitations",
            body={
                "username": "Nina Ops",
                "display_name": "Nina Operator",
                "role": "owner",
                "expires_in_seconds": 60,
            },
            headers=browser_headers(),
        ),
        400,
        "invalid_request",
    )
    issued = expect(
        call(
            "POST",
            "/v1/members/invitations",
            body={
                "username": "nina_ops",
                "display_name": "Nina Operator",
                "role": "viewer",
                "expires_in_seconds": 86_400,
            },
            headers=browser_headers(),
        ),
        201,
    ).body
    secret = issued["secret"]
    assert secret.startswith("mki_") and len(secret) < 160, issued
    invitations = expect(call("GET", "/v1/members/invitations"), 200).body["items"]
    assert invitations[0]["username"] == "nina_ops", invitations
    assert invitations[0]["status"] == "pending", invitations
    assert secret not in json.dumps(invitations), invitations
    preview = expect(
        call("POST", "/v1/member-invitations/inspect", body={"token": secret}), 200
    ).body
    assert preview["company"] == "E2E Operator", preview
    assert preview["username"] == "nina_ops" and preview["role"] == "viewer", preview
    expect(
        call(
            "POST",
            "/v1/member-invitations/accept",
            body={"token": secret, "password": "short"},
        ),
        400,
        "invalid_password",
    )
    accepted = expect(
        call(
            "POST",
            "/v1/member-invitations/accept",
            body={"token": secret, "password": "member-test-password-123"},
        ),
        200,
    ).body
    assert accepted["username"] == "nina_ops" and accepted["role"] == "viewer", accepted
    assert "internal.meerkateer.invalid" in accepted["email"], accepted
    first_member_session = {cookie.name: cookie.value for cookie in cookies}
    expect(
        call("POST", "/v1/member-invitations/inspect", body={"token": secret}),
        410,
        "invitation_unavailable",
    )
    expect(
        call(
            "POST",
            "/v1/member-invitations/accept",
            body={"token": secret, "password": "member-test-password-123"},
        ),
        410,
        "invitation_unavailable",
    )
    expect(call("GET", "/v1/members"), 403, "forbidden")

    cookies.clear()
    expect(
        call(
            "POST",
            "/v1/session/password-login",
            body={"email": "nina_ops", "password": "member-test-password-123"},
        ),
        200,
    )
    expect(
        call(
            "PUT",
            "/v1/session/password",
            body={
                "current_password": "wrong-current-password",
                "new_password": "member-test-password-789",
            },
            headers=browser_headers(),
        ),
        401,
        "invalid_current_password",
    )
    expect(
        call(
            "PUT",
            "/v1/session/password",
            body={
                "current_password": "member-test-password-123",
                "new_password": "member-test-password-789",
            },
            headers=browser_headers(),
        ),
        204,
    )
    expect(
        call(
            "PUT",
            "/v1/session/password",
            body={
                "current_password": "member-test-password-789",
                "new_password": "member-test-password-999",
            },
        ),
        403,
        "csrf_failed",
    )
    old_cookie = (
        f"meerkateer_session={first_member_session['meerkateer_session']}; "
        f"meerkateer_csrf={first_member_session['meerkateer_csrf']}"
    )
    expect(
        isolated_request("GET", "/v1/session", headers={"Cookie": old_cookie}),
        401,
        "authentication_required",
    )
    cookies.clear()
    expect(
        call(
            "POST",
            "/v1/session/password-login",
            body={"email": "nina_ops", "password": "member-test-password-123"},
        ),
        401,
        "invalid_credentials",
    )
    expect(
        call(
            "POST",
            "/v1/session/password-login",
            body={"email": "nina_ops", "password": "member-test-password-789"},
        ),
        200,
    )
    changed_member_session = {cookie.name: cookie.value for cookie in cookies}
    cookies.clear()
    expect(
        call(
            "POST",
            "/v1/session/password-login",
            body={
                "email": "e2e-owner@example.com",
                "password": "new-test-only-password-456",
            },
        ),
        200,
    )

    cancelled = expect(
        call(
            "POST",
            "/v1/members/invitations",
            body={
                "username": "cancelled_user",
                "display_name": "Cancelled User",
                "role": "viewer",
                "expires_in_seconds": 3_600,
            },
            headers=browser_headers(),
        ),
        201,
    ).body
    expect(
        call("DELETE", f"/v1/members/invitations/{cancelled['invitation']['id']}"),
        403,
        "csrf_failed",
    )
    expect(
        call(
            "DELETE",
            f"/v1/members/invitations/{cancelled['invitation']['id']}",
            headers=browser_headers(),
        ),
        204,
    )
    expect(
        call(
            "POST",
            "/v1/member-invitations/inspect",
            body={"token": cancelled["secret"]},
        ),
        410,
        "invitation_unavailable",
    )
    expect(
        call(
            "DELETE",
            f"/v1/members/invitations/{cancelled['invitation']['id']}",
            headers=browser_headers(),
        ),
        409,
        "invitation_not_pending",
    )

    concurrent = expect(
        call(
            "POST",
            "/v1/members/invitations",
            body={
                "username": "concurrent_user",
                "display_name": "Concurrent User",
                "role": "operator",
                "expires_in_seconds": 300,
            },
            headers=browser_headers(),
        ),
        201,
    ).body
    concurrent_secret = concurrent["secret"]
    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [
            executor.submit(
                isolated_call,
                "POST",
                "/v1/member-invitations/accept",
                {
                    "token": concurrent_secret,
                    "password": "concurrent-password-123",
                },
            )
            for _ in range(2)
        ]
        concurrent_results = [future.result() for future in futures]
    assert sorted(result.status for result in concurrent_results) == [200, 410], (
        concurrent_results
    )

    expiring = expect(
        call(
            "POST",
            "/v1/members/invitations",
            body={
                "username": "expired_user",
                "display_name": "Expired User",
                "role": "viewer",
                "expires_in_seconds": 300,
            },
            headers=browser_headers(),
        ),
        201,
    ).body
    expired_secret = expiring["secret"]
    force_invitation_expired(expired_secret)
    expect(
        call("POST", "/v1/member-invitations/inspect", body={"token": expired_secret}),
        410,
        "invitation_unavailable",
    )
    expect(
        call(
            "POST",
            "/v1/member-invitations/accept",
            body={"token": expired_secret, "password": "expired-password-123"},
        ),
        410,
        "invitation_unavailable",
    )

    declined = expect(
        call(
            "POST",
            "/v1/members/invitations",
            body={
                "username": "declined_user",
                "display_name": "Declined User",
                "role": "operator",
                "expires_in_seconds": 300,
            },
            headers=browser_headers(),
        ),
        201,
    ).body
    declined_secret = declined["secret"]
    expect(
        call(
            "POST", "/v1/member-invitations/decline", body={"token": declined_secret}
        ),
        204,
    )
    expect(
        call(
            "POST", "/v1/member-invitations/decline", body={"token": declined_secret}
        ),
        410,
        "invitation_unavailable",
    )
    members = expect(call("GET", "/v1/members"), 200).body["items"]
    assert any(member["username"] == "nina_ops" for member in members), members
    assert not any(member["username"] == "declined_user" for member in members), members
    nina_id = next(member["user_id"] for member in members if member["username"] == "nina_ops")
    owner_id = next(member["user_id"] for member in members if member["role"] == "owner")
    invitations = expect(call("GET", "/v1/members/invitations"), 200).body["items"]
    statuses = {item["username"]: item["status"] for item in invitations}
    assert statuses["nina_ops"] == "accepted", statuses
    assert statuses["concurrent_user"] == "accepted", statuses
    assert statuses["expired_user"] == "expired", statuses
    assert statuses["declined_user"] == "declined", statuses
    assert statuses["cancelled_user"] == "cancelled", statuses

    expect(
        call(
            "POST",
            f"/v1/members/{nina_id}/password-reset-links",
            body={"expires_in_seconds": 86_400},
        ),
        403,
        "csrf_failed",
    )
    expect(
        call(
            "POST",
            f"/v1/members/{owner_id}/password-reset-links",
            body={"expires_in_seconds": 86_400},
            headers=browser_headers(),
        ),
        400,
        "invalid_request",
    )
    first_reset = expect(
        call(
            "POST",
            f"/v1/members/{nina_id}/password-reset-links",
            body={"expires_in_seconds": 86_400},
            headers=browser_headers(),
        ),
        201,
    ).body
    assert first_reset["secret"].startswith("mkr_"), first_reset
    reset_preview = expect(
        call(
            "POST",
            "/v1/member-password-resets/inspect",
            body={"token": first_reset["secret"]},
        ),
        200,
    ).body
    assert reset_preview["username"] == "nina_ops", reset_preview

    replacement_reset = expect(
        call(
            "POST",
            f"/v1/members/{nina_id}/password-reset-links",
            body={"expires_in_seconds": 3_600},
            headers=browser_headers(),
        ),
        201,
    ).body
    expect(
        call(
            "POST",
            "/v1/member-password-resets/inspect",
            body={"token": first_reset["secret"]},
        ),
        410,
        "password_reset_unavailable",
    )
    reset_session = expect(
        isolated_call(
            "POST",
            "/v1/member-password-resets/accept",
            {
                "token": replacement_reset["secret"],
                "password": "member-reset-password-123",
            },
        ),
        200,
    ).body
    assert reset_session["username"] == "nina_ops", reset_session
    changed_cookie = (
        f"meerkateer_session={changed_member_session['meerkateer_session']}; "
        f"meerkateer_csrf={changed_member_session['meerkateer_csrf']}"
    )
    expect(
        isolated_request("GET", "/v1/session", headers={"Cookie": changed_cookie}),
        401,
        "authentication_required",
    )
    expect(
        call(
            "POST",
            "/v1/member-password-resets/accept",
            body={
                "token": replacement_reset["secret"],
                "password": "member-reset-password-123",
            },
        ),
        410,
        "password_reset_unavailable",
    )
    expect(
        isolated_call(
            "POST",
            "/v1/session/password-login",
            {"email": "nina_ops", "password": "member-test-password-789"},
        ),
        401,
        "invalid_credentials",
    )

    concurrent_reset = expect(
        call(
            "POST",
            f"/v1/members/{nina_id}/password-reset-links",
            body={"expires_in_seconds": 3_600},
            headers=browser_headers(),
        ),
        201,
    ).body
    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [
            executor.submit(
                isolated_call,
                "POST",
                "/v1/member-password-resets/accept",
                {
                    "token": concurrent_reset["secret"],
                    "password": "member-concurrent-reset-123",
                },
            )
            for _ in range(2)
        ]
        reset_results = [future.result() for future in futures]
    assert sorted(result.status for result in reset_results) == [200, 410], reset_results

    expired_reset = expect(
        call(
            "POST",
            f"/v1/members/{nina_id}/password-reset-links",
            body={"expires_in_seconds": 300},
            headers=browser_headers(),
        ),
        201,
    ).body
    force_token_expired(expired_reset["secret"], "member_password_resets")
    expect(
        call(
            "POST",
            "/v1/member-password-resets/inspect",
            body={"token": expired_reset["secret"]},
        ),
        410,
        "password_reset_unavailable",
    )
    audit = expect(call("GET", "/v1/audit-events?limit=100"), 200).body["items"]
    serialized_audit = json.dumps(audit)
    assert all(
        token not in serialized_audit
        for token in (
            secret,
            concurrent_secret,
            expired_secret,
            declined_secret,
            cancelled["secret"],
            first_reset["secret"],
            replacement_reset["secret"],
            concurrent_reset["secret"],
            expired_reset["secret"],
        )
    )
    actions = {item["action"] for item in audit}
    assert {
        "member.invitation_created",
        "member.invitation_accepted",
        "member.invitation_declined",
        "member.invitation_cancelled",
        "member.password_changed",
        "member.password_reset_created",
        "member.password_reset_completed",
    } <= actions, actions


def session_revocation() -> None:
    active = {cookie.name: cookie.value for cookie in cookies}
    assert "meerkateer_session" in active and "meerkateer_csrf" in active
    expect(call("DELETE", "/v1/session"), 403, "csrf_failed")
    expect(call("DELETE", "/v1/session", headers=browser_headers()), 204)

    old_cookie = (
        f"meerkateer_session={active['meerkateer_session']}; "
        f"meerkateer_csrf={active['meerkateer_csrf']}"
    )
    expect(
        call("GET", "/v1/session", headers={"Cookie": old_cookie}),
        401,
        "authentication_required",
    )
    expect(
        call(
            "POST",
            "/v1/session/password-login",
            body={
                "email": "e2e-owner@example.com",
                "password": "new-test-only-password-456",
            },
        ),
        200,
    )
    audit = expect(call("GET", "/v1/audit-events?limit=100"), 200).body["items"]
    assert any(item["action"] == "session.logout" for item in audit), audit


def main() -> None:
    project_id, service_id, credential_id, service_secret = bootstrap_and_inventory()
    mks_ingestion(project_id, service_id, credential_id, service_secret)
    agent_id, agent_secret = enroll_agent(project_id)
    mka_ingestion(agent_id, agent_secret)
    internal_member_invitations()
    session_revocation()
    rate_limit_boundary()
    print("Meerkateer identity, ingestion, and abuse-control E2E passed.")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"E2E failure: {error}", file=sys.stderr)
        raise
