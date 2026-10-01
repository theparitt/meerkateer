#!/usr/bin/env python3
"""Check project policy files and invariants that should never silently drift."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
APACHE_2_SHA256 = "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30"
REQUIRED_FILES = (
    "LICENSE",
    "NOTICE",
    "README.md",
    "CONTRIBUTING.md",
    "DCO",
    "CODE_OF_CONDUCT.md",
    "SECURITY.md",
    "SUPPORT.md",
    "GOVERNANCE.md",
    "TRADEMARKS.md",
    "CHANGELOG.md",
    "docs/production-roadmap.md",
    "docs/roadmap-to-1.0.md",
    "docs/commercial-readiness-plan.md",
    "docs/game-server-beta.md",
    "docs/standards/mks-1.md",
    "docs/standards/mka-1.md",
)
IGNORED_DIRECTORIES = {".git", "node_modules", "target", "dist", "coverage", ".data"}


def main() -> int:
    failures: list[str] = []

    for relative in REQUIRED_FILES:
        if not (ROOT / relative).is_file():
            failures.append(f"missing required file: {relative}")

    license_path = ROOT / "LICENSE"
    if license_path.is_file():
        digest = hashlib.sha256(license_path.read_bytes()).hexdigest()
        if digest != APACHE_2_SHA256:
            failures.append("LICENSE is not the unmodified official Apache-2.0 text")

    for standard in ("mks-1.md", "mka-1.md"):
        path = ROOT / "docs" / "standards" / standard
        if path.is_file() and "**Status:** Draft" not in path.read_text(encoding="utf-8"):
            failures.append(f"{path.relative_to(ROOT)} must remain Draft before conformance")

    migrations = sorted(path.name for path in (ROOT / "migrations").glob("[0-9][0-9][0-9][0-9]_*.sql"))
    compose = (ROOT / "compose.yaml").read_text(encoding="utf-8")
    for index, filename in enumerate(migrations, start=1):
        if int(filename[:4]) != index:
            failures.append(f"migration sequence has a gap before {filename}")
        if compose.count(f"./migrations/{filename}:") != 1:
            failures.append(f"compose.yaml must mount {filename} exactly once")
    if not migrations:
        failures.append("no database migrations found")
    ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    if "for migration in migrations/*.sql; do" not in ci:
        failures.append("CI must apply the full migration directory")

    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    workspace_package = cargo.split("[workspace.package]", maxsplit=1)
    version_match = (
        re.search(r'^version = "([^"]+)"$', workspace_package[1], re.MULTILINE)
        if len(workspace_package) == 2
        else None
    )
    workspace_version = version_match.group(1) if version_match else ""
    if not workspace_version:
        failures.append("Cargo.toml must declare workspace.package.version")
    web_package = json.loads((ROOT / "meerkateer-web" / "package.json").read_text(encoding="utf-8"))
    if web_package.get("version") != workspace_version:
        failures.append("Rust workspace and web package versions must match")
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    roadmap = (ROOT / "docs" / "roadmap-to-1.0.md").read_text(encoding="utf-8")
    if f"| Code version | **{workspace_version}** |" not in readme:
        failures.append("README current version must match the Rust workspace version")
    if f"| Code version | `{workspace_version}` |" not in roadmap:
        failures.append("roadmap current version must match the Rust workspace version")

    schema_ids: dict[str, Path] = {}
    for path in sorted((ROOT / "schemas").glob("*/*.schema.json")):
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            failures.append(f"{path.relative_to(ROOT)} is not valid JSON: {error}")
            continue
        schema_id = payload.get("$id")
        if not isinstance(schema_id, str):
            failures.append(f"{path.relative_to(ROOT)} has no string $id")
        elif schema_id in schema_ids:
            failures.append(
                f"duplicate schema $id in {schema_ids[schema_id].relative_to(ROOT)} and "
                f"{path.relative_to(ROOT)}"
            )
        else:
            schema_ids[schema_id] = path

    for path in ROOT.rglob("*"):
        if not path.is_file() or IGNORED_DIRECTORIES.intersection(path.parts):
            continue
        if path.suffix.lower() not in {".md", ".json", ".py", ".toml", ".yml", ".yaml"}:
            continue
        data = path.read_bytes()
        if b"\r\n" in data:
            failures.append(f"{path.relative_to(ROOT)} uses CRLF instead of LF")
        for line_number, line in enumerate(data.splitlines(), start=1):
            if line.endswith((b" ", b"\t")):
                failures.append(
                    f"{path.relative_to(ROOT)}:{line_number} has trailing whitespace"
                )

    if failures:
        print("Repository validation failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(f"Repository policy valid; discovered {len(schema_ids)} unique schemas.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
