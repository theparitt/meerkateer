#!/usr/bin/env python3
"""Fail when a proposed release tag disagrees with public version declarations."""

from __future__ import annotations

import json
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: scripts/check-release-version.py <version-or-v-tag>", file=sys.stderr)
        return 2
    expected = sys.argv[1].removeprefix("v")
    if not expected or any(character.isspace() for character in expected):
        print("release version is empty or contains whitespace", file=sys.stderr)
        return 2

    cargo_text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    workspace_package = re.search(
        r"(?ms)^\[workspace\.package\]\s*(.*?)(?=^\[|\Z)", cargo_text
    )
    cargo_version = (
        re.search(r'(?m)^version\s*=\s*"([^"]+)"\s*$', workspace_package.group(1))
        if workspace_package
        else None
    )
    if cargo_version is None:
        print("Cargo workspace version declaration is missing", file=sys.stderr)
        return 2
    package = json.loads((ROOT / "meerkateer-web/package.json").read_text(encoding="utf-8"))
    openapi = json.loads((ROOT / "openapi/meerkateer.openapi.json").read_text(encoding="utf-8"))
    declared = {
        "Cargo workspace": cargo_version.group(1),
        "web package": package["version"],
        "OpenAPI": openapi["info"]["version"],
    }
    failures = [f"{name} declares {value}" for name, value in declared.items() if value != expected]
    if failures:
        print(f"release tag expects {expected}, but " + "; ".join(failures), file=sys.stderr)
        return 1
    print(f"Release declarations agree on {expected}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
