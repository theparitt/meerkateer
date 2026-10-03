#!/usr/bin/env python3
"""Two-company API isolation regression for every current tenant-owned ID route."""

from __future__ import annotations

import datetime as dt
import http.cookiejar
import json
import uuid
import urllib.parse

from api_e2e import BASE_URL, bearer, browser_headers, call, cookies, expect, utc


PASSWORD = "new-test-only-password-456"
RIVAL_SESSION = (
    "mks_session_22222222222242228222222222222222_"
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
)
RIVAL_CSRF = (
    "mks_csrf_"
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
)


def add_cookie(name: str, value: str, *, http_only: bool) -> None:
    parsed = urllib.parse.urlsplit(BASE_URL)
    assert parsed.hostname is not None
    cookies.set_cookie(
        http.cookiejar.Cookie(
            version=0,
            name=name,
            value=value,
            port=None,
            port_specified=False,
            domain=parsed.hostname,
            domain_specified=False,
            domain_initial_dot=False,
            path="/",
            path_specified=True,
            secure=parsed.scheme == "https",
            expires=None,
            discard=True,
            comment=None,
            comment_url=None,
            rest={"HttpOnly": None} if http_only else {},
            rfc2109=False,
        )
    )


def activate_rival_session() -> dict:
    cookies.clear()
    add_cookie("meerkateer_session", RIVAL_SESSION, http_only=True)
    add_cookie("meerkateer_csrf", RIVAL_CSRF, http_only=False)
    return expect(call("GET", "/v1/session"), 200).body


def login(email: str) -> dict:
    cookies.clear()
    return expect(
        call(
            "POST",
            "/v1/session/password-login",
            body={"email": email, "password": PASSWORD},
        ),
        200,
    ).body


def create_rival_objects() -> dict[str, str]:
    session = activate_rival_session()
    assert session["tenant_id"] == "22222222-2222-4222-8222-222222222222"
    project = expect(
        call(
            "POST",
            "/v1/projects",
            body={"slug": "rival-ops", "display_name": "Rival Ops"},
            headers=browser_headers(),
        ),
        201,
    ).body
    service = expect(
        call(
            "POST",
            f"/v1/projects/{project['id']}/services",
            body={"slug": "rival-api", "environment": "production"},
            headers=browser_headers(),
        ),
        201,
    ).body
    credential = expect(
        call(
            "POST",
            f"/v1/services/{service['id']}/credentials",
            headers=browser_headers(),
        ),
        201,
    ).body
    token = expect(
        call(
            "POST",
            f"/v1/projects/{project['id']}/enrollment-tokens",
            body={"expires_in_seconds": 600},
            headers=browser_headers(),
        ),
        201,
    ).body["secret"]
    agent = expect(
        call(
            "POST",
            "/v1/agents/enroll",
            body={
                "installation_id": str(uuid.uuid4()),
                "display_name": "Rival Host",
            },
            headers=bearer(token),
        ),
        201,
    ).body
    maintenance = expect(
        call(
            "POST",
            "/v1/maintenance-windows",
            body={
                "project_id": project["id"],
                "service_id": service["id"],
                "title": "Rival maintenance",
                "reason": "tenant isolation fixture",
                "starts_at": utc(dt.timedelta(minutes=-1)),
                "ends_at": utc(dt.timedelta(minutes=10)),
            },
            headers=browser_headers(),
        ),
        201,
    ).body
    heartbeat_id = str(uuid.uuid4())
    expect(
        call(
            "POST",
            "/v1/ingest/heartbeat",
            body={
                "interface_version": "1",
                "service": "rival-api",
                "project": "rival-ops",
                "environment": "production",
                "status": "down",
                "message": "tenant isolation fixture outage",
                "timestamp": utc(),
            },
            headers=bearer(credential["secret"], heartbeat_id),
        ),
        202,
    )
    incident = expect(
        call("GET", f"/v1/incidents?project_id={project['id']}&limit=20"), 200
    ).body["items"][0]
    assert incident["service_id"] == service["id"]
    return {
        "project": project["id"],
        "service": service["id"],
        "credential": credential["credential_id"],
        "agent": agent["agent_id"],
        "maintenance": maintenance["id"],
        "incident": incident["id"],
    }


def assert_not_found(method: str, path: str, body: dict | None = None) -> None:
    expect(
        call(method, path, body=body, headers=browser_headers()),
        404,
        "not_found",
    )


def attack_from_primary(rival: dict[str, str]) -> None:
    session = login("e2e-owner@example.com")
    assert session["tenant_id"] != "22222222-2222-4222-8222-222222222222"
    primary_projects = expect(call("GET", "/v1/projects"), 200).body["items"]
    primary_project = next(item for item in primary_projects if item["slug"] == "arena-ops")
    primary_token = expect(
        call(
            "POST",
            f"/v1/projects/{primary_project['id']}/enrollment-tokens",
            body={"expires_in_seconds": 600},
            headers=browser_headers(),
        ),
        201,
    ).body["secret"]
    primary_agent = expect(
        call(
            "POST",
            "/v1/agents/enroll",
            body={
                "installation_id": str(uuid.uuid4()),
                "display_name": "Primary isolation probe",
            },
            headers=bearer(primary_token),
        ),
        201,
    ).body

    scoped_reads = [
        call("GET", "/v1/projects"),
        call("GET", "/v1/agents"),
        call(
            "GET",
            f"/v1/incidents?project_id={primary_project['id']}&limit=100",
        ),
        call(
            "GET",
            f"/v1/incidents/activity?project_id={primary_project['id']}&limit=100",
        ),
        call(
            "GET",
            f"/v1/maintenance-windows?project_id={primary_project['id']}&limit=100",
        ),
        call(
            "GET",
            f"/v1/alerts/deliveries?project_id={primary_project['id']}&limit=100",
        ),
        call("GET", "/v1/audit-events?limit=100"),
        call("GET", "/v1/admin/summary"),
    ]
    for response in scoped_reads:
        expect(response, 200)
        encoded = json.dumps(response.body, separators=(",", ":"))
        assert not any(identifier in encoded for identifier in rival.values()), encoded

    assert_not_found("GET", f"/v1/projects/{rival['project']}/services")
    assert_not_found(
        "POST",
        f"/v1/projects/{rival['project']}/services",
        {"slug": "intrusion", "environment": "production"},
    )
    assert_not_found("GET", f"/v1/services/{rival['service']}/timeline")
    assert_not_found("POST", f"/v1/services/{rival['service']}/game-probe")
    assert_not_found("POST", f"/v1/services/{rival['service']}/credentials")
    assert_not_found(
        "POST",
        f"/v1/services/{rival['service']}/credentials/{rival['credential']}/rotate",
        {"overlap_seconds": 60},
    )
    assert_not_found(
        "DELETE",
        f"/v1/services/{rival['service']}/credentials/{rival['credential']}",
    )
    assert_not_found(
        "POST", f"/v1/projects/{rival['project']}/enrollment-tokens", {"expires_in_seconds": 600}
    )
    assert_not_found("GET", f"/v1/agents/{rival['agent']}/telemetry")
    expect(
        call(
            "POST",
            f"/v1/agents/{rival['agent']}/credentials/rotate",
            headers=bearer(primary_agent["secret"]),
        ),
        401,
        "unauthorized",
    )
    assert_not_found("DELETE", f"/v1/agents/{rival['agent']}")
    assert_not_found(
        "PUT", f"/v1/projects/{primary_project['id']}/agents/{rival['agent']}"
    )
    assert_not_found(
        "PUT", f"/v1/projects/{rival['project']}/agents/{rival['agent']}"
    )
    assert_not_found("DELETE", f"/v1/maintenance-windows/{rival['maintenance']}")
    assert_not_found("POST", f"/v1/incidents/{rival['incident']}/acknowledge")
    assert_not_found(
        "PUT", f"/v1/incidents/{rival['incident']}/assignment", {"assigned": True}
    )
    assert_not_found(
        "POST",
        f"/v1/incidents/{rival['incident']}/notes",
        {"note": "cross-tenant write must fail"},
    )


def assert_rival_unchanged(rival: dict[str, str]) -> None:
    activate_rival_session()
    projects = expect(call("GET", "/v1/projects"), 200).body["items"]
    assert [item["id"] for item in projects] == [rival["project"]]
    services = expect(
        call("GET", f"/v1/projects/{rival['project']}/services"), 200
    ).body["items"]
    assert [item["id"] for item in services] == [rival["service"]]
    agents = expect(call("GET", "/v1/agents"), 200).body["items"]
    assert [item["id"] for item in agents] == [rival["agent"]]
    incidents = expect(
        call("GET", f"/v1/incidents?project_id={rival['project']}&limit=20"), 200
    ).body["items"]
    assert [item["id"] for item in incidents] == [rival["incident"]]
    assert incidents[0]["status"] == "open"


def assert_role_boundaries() -> None:
    viewer = login("viewer@example.com")
    assert viewer["role"] == "viewer"
    projects = expect(call("GET", "/v1/projects"), 200).body["items"]
    primary_project = next(item for item in projects if item["slug"] == "arena-ops")
    expect(
        call(
            "POST",
            "/v1/projects",
            body={"slug": "viewer-write", "display_name": "Viewer write"},
            headers=browser_headers(),
        ),
        403,
        "forbidden",
    )
    expect(call("GET", "/v1/admin/summary"), 403, "forbidden")

    operator = login("operator@example.com")
    assert operator["role"] == "operator"
    service = expect(
        call(
            "POST",
            f"/v1/projects/{primary_project['id']}/services",
            body={"slug": "operator-service", "environment": "test"},
            headers=browser_headers(),
        ),
        201,
    ).body
    expect(
        call(
            "POST",
            f"/v1/services/{service['id']}/credentials",
            headers=browser_headers(),
        ),
        403,
        "forbidden",
    )

    admin = login("admin@example.com")
    assert admin["role"] == "admin"
    expect(call("GET", "/v1/admin/summary"), 200)
    expect(
        call(
            "POST",
            "/v1/projects",
            body={"slug": "admin-workspace", "display_name": "Admin workspace"},
            headers=browser_headers(),
        ),
        201,
    )


def main() -> None:
    rival = create_rival_objects()
    attack_from_primary(rival)
    assert_role_boundaries()
    assert_rival_unchanged(rival)
    assert BASE_URL.startswith("http://127.0.0.1:")
    print("Two-company API isolation passed for list, direct-ID, nested-ID, and write routes.")


if __name__ == "__main__":
    main()
