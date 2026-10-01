#!/usr/bin/env python3
"""Provision and exercise five real SDK demo servers against a local Meerkateer."""

from __future__ import annotations

import argparse
import concurrent.futures
import http.cookiejar
import json
import os
import pathlib
import stat
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Any


BASE_URL = os.environ.get("MEERKATEER_LAB_URL", "http://127.0.0.1:18110")
OWNER_EMAIL = "sdk-lab@example.invalid"
OWNER_PASSWORD = "local-sdk-lab-password-123"
PROJECT_SLUG = "sdk-lab"
LANGUAGES = {
    "python": 19101,
    "node": 19102,
    "go": 19103,
    "php": 19104,
    "rust": 19105,
}


@dataclass
class Response:
    status: int
    body: Any


cookies = http.cookiejar.CookieJar()
client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookies))


def request(
    method: str,
    path: str,
    *,
    body: dict[str, Any] | None = None,
    headers: dict[str, str] | None = None,
    base_url: str = BASE_URL,
) -> Response:
    data = json.dumps(body, separators=(",", ":")).encode() if body is not None else None
    request_headers = dict(headers or {})
    if body is not None:
        request_headers["Content-Type"] = "application/json"
    message = urllib.request.Request(
        f"{base_url}{path}", data=data, method=method, headers=request_headers
    )
    try:
        response = client.open(message, timeout=15)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    parsed = json.loads(raw) if raw else None
    return Response(response.status, parsed)


def expect(response: Response, status: int) -> Any:
    if response.status != status:
        raise AssertionError((response.status, response.body))
    return response.body


def csrf() -> str:
    for cookie in cookies:
        if cookie.name == "meerkateer_csrf":
            return cookie.value
    raise AssertionError("CSRF cookie was not issued")


def login() -> None:
    cookies.clear()
    expect(
        request(
            "POST",
            "/v1/session/password-login",
            body={"email": OWNER_EMAIL, "password": OWNER_PASSWORD},
        ),
        200,
    )


def prepare(output: pathlib.Path) -> None:
    instance = expect(request("GET", "/v1/instance"), 200)
    if instance["setup_required"]:
        bootstrap_token = os.environ.get("MEERKATEER_BOOTSTRAP_TOKEN")
        if not bootstrap_token:
            raise AssertionError("MEERKATEER_BOOTSTRAP_TOKEN is required")
        expect(
            request(
                "POST",
                "/v1/bootstrap",
                body={
                    "tenant_slug": "sdk-lab-company",
                    "tenant_name": "SDK Lab Company",
                    "owner_email": OWNER_EMAIL,
                    "owner_name": "SDK Lab Owner",
                    "owner_password": OWNER_PASSWORD,
                },
                headers={"Authorization": f"Bearer {bootstrap_token}"},
            ),
            201,
        )
    login()
    projects = expect(request("GET", "/v1/projects"), 200)["items"]
    project = next((item for item in projects if item["slug"] == PROJECT_SLUG), None)
    if project is None:
        project = expect(
            request(
                "POST",
                "/v1/projects",
                body={"slug": PROJECT_SLUG, "display_name": "Multi-language SDK Lab"},
                headers={"X-Meerkateer-CSRF": csrf()},
            ),
            201,
        )
    services = expect(request("GET", f"/v1/projects/{project['id']}/services"), 200)[
        "items"
    ]
    keys: dict[str, str] = {}
    for language in LANGUAGES:
        slug = f"{language}-demo"
        service = next((item for item in services if item["slug"] == slug), None)
        if service is None:
            service = expect(
                request(
                    "POST",
                    f"/v1/projects/{project['id']}/services",
                    body={"slug": slug, "environment": "local"},
                    headers={"X-Meerkateer-CSRF": csrf()},
                ),
                201,
            )
        credential = expect(
            request(
                "POST",
                f"/v1/services/{service['id']}/credentials",
                headers={"X-Meerkateer-CSRF": csrf()},
            ),
            201,
        )
        keys[language] = credential["secret"]

    lines = [
        f"MEERKATEER_URL={BASE_URL}",
        f"MEERKATEER_PROJECT={PROJECT_SLUG}",
        "DEMO_HEARTBEAT_SECONDS=5",
        *[f"{language.upper()}_SERVICE_KEY={keys[language]}" for language in LANGUAGES],
    ]
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")
    output.chmod(stat.S_IRUSR | stat.S_IWUSR)
    print(f"Provisioned workspace {PROJECT_SLUG} with {len(LANGUAGES)} SDK services.")


def demo_request(language: str, method: str, path: str) -> Any:
    port = LANGUAGES[language]
    return expect(request(method, path, base_url=f"http://127.0.0.1:{port}"), 200)


def wait_for_demos(selected: list[str] | None = None) -> None:
    pending = set(selected or LANGUAGES)
    deadline = time.monotonic() + 180
    while pending and time.monotonic() < deadline:
        for language in list(pending):
            try:
                result = demo_request(language, "GET", "/health")
                if result == {"language": language, "status": "ok"}:
                    pending.remove(language)
            except (OSError, AssertionError, json.JSONDecodeError):
                pass
        if pending:
            time.sleep(1)
    if pending:
        raise AssertionError(f"demo servers not ready: {sorted(pending)}")
    print(f"Ready: {', '.join(selected or LANGUAGES)}.")


def services_by_slug() -> dict[str, dict[str, Any]]:
    projects = expect(request("GET", "/v1/projects"), 200)["items"]
    project = next(item for item in projects if item["slug"] == PROJECT_SLUG)
    services = expect(request("GET", f"/v1/projects/{project['id']}/services"), 200)[
        "items"
    ]
    return {item["slug"]: item for item in services}


def assert_states(expected: str, *, languages: list[str] | None = None) -> None:
    selected = languages or list(LANGUAGES)
    services = services_by_slug()
    for language in selected:
        status = services[f"{language}-demo"]["status"]
        if status["state"] != expected or status["stale"]:
            raise AssertionError((language, expected, status))


def wait_for_states(expected: str, *, languages: list[str] | None = None) -> None:
    deadline = time.monotonic() + 12
    last_error: AssertionError | None = None
    while time.monotonic() < deadline:
        try:
            assert_states(expected, languages=languages)
            return
        except AssertionError as error:
            last_error = error
            time.sleep(0.5)
    raise last_error or AssertionError(f"services did not resolve to {expected}")


def post_all(scenario: str) -> None:
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(LANGUAGES)) as executor:
        futures = [
            executor.submit(demo_request, language, "POST", f"/scenario/{scenario}")
            for language in LANGUAGES
        ]
        results = [future.result(timeout=30) for future in futures]
    expected = "ok" if scenario == "healthy" else scenario
    assert {item["status"] for item in results} == {expected}


def exercise() -> None:
    wait_for_demos()
    login()
    for scenario, expected in (
        ("degraded", "degraded"),
        ("down", "offline"),
        ("healthy", "online"),
    ):
        # Python, PHP, and Rust intentionally emit protocol timestamps at second
        # precision. Keep state transitions in distinct observation seconds.
        time.sleep(1.1)
        post_all(scenario)
        wait_for_states(expected)
    services = services_by_slug()
    for language in LANGUAGES:
        service = services[f"{language}-demo"]
        timeline = expect(
            request("GET", f"/v1/services/{service['id']}/timeline?limit=100"), 200
        )["items"]
        titles = {item["title"] for item in timeline}
        for expected_title in {"demo degraded", "demo failed", "demo recovered"}:
            if expected_title not in titles:
                raise AssertionError((language, expected_title, sorted(titles)))
    print("All five SDKs reported degraded, down, and recovery with visible timeline events.")


def wait_stale(language: str) -> None:
    login()
    deadline = time.monotonic() + 50
    while time.monotonic() < deadline:
        service = services_by_slug()[f"{language}-demo"]
        status = service["status"]
        if status["stale"] and status["state"] == "unknown":
            print(f"Detected stopped {language} server as stale/unknown.")
            return
        time.sleep(1)
    raise AssertionError(f"{language} server did not become stale")


def recover(language: str) -> None:
    wait_for_demos([language])
    login()
    demo_request(language, "POST", "/scenario/healthy")
    wait_for_states("online", languages=[language])
    print(f"Recovered {language} server to online.")


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    prepare_parser = subparsers.add_parser("prepare")
    prepare_parser.add_argument("--output", required=True, type=pathlib.Path)
    subparsers.add_parser("wait")
    subparsers.add_parser("exercise")
    stale_parser = subparsers.add_parser("wait-stale")
    stale_parser.add_argument("language", choices=LANGUAGES)
    recover_parser = subparsers.add_parser("recover")
    recover_parser.add_argument("language", choices=LANGUAGES)
    arguments = parser.parse_args()
    if arguments.command == "prepare":
        prepare(arguments.output)
    elif arguments.command == "wait":
        wait_for_demos()
    elif arguments.command == "exercise":
        exercise()
    elif arguments.command == "wait-stale":
        wait_stale(arguments.language)
    elif arguments.command == "recover":
        recover(arguments.language)


if __name__ == "__main__":
    main()
