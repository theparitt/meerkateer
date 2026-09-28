#!/usr/bin/env python3
"""Check the committed API description without depending on a code generator."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DOCUMENT = ROOT / "openapi" / "meerkateer.openapi.json"


def walk(value: Any):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk(child)


def main() -> None:
    document = json.loads(DOCUMENT.read_text(encoding="utf-8"))
    assert document["openapi"].startswith("3.1."), "OpenAPI 3.1 is required"
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    package_section = cargo.split("[workspace.package]", 1)[1].split("[", 1)[0]
    version_match = re.search(r'^version = "([^"]+)"$', package_section, re.MULTILINE)
    assert version_match, "workspace package version is missing"
    expected_version = version_match.group(1)
    assert document["info"]["version"] == expected_version, "API and workspace versions differ"

    operation_ids: list[str] = []
    for path, path_item in document["paths"].items():
        assert path.startswith("/"), f"invalid API path: {path}"
        for method, operation in path_item.items():
            if method not in {"get", "put", "post", "patch", "delete", "options", "head"}:
                continue
            operation_id = operation.get("operationId")
            assert operation_id, f"{method.upper()} {path} has no operationId"
            operation_ids.append(operation_id)
            assert operation.get("responses"), f"{operation_id} has no responses"
    assert len(operation_ids) == len(set(operation_ids)), "operationId values must be unique"

    for node in walk(document):
        reference = node.get("$ref")
        if not reference:
            continue
        prefix = "#/components/"
        assert reference.startswith(prefix), f"external OpenAPI ref is not allowed: {reference}"
        target: Any = document
        for raw_segment in reference.removeprefix("#/").split("/"):
            segment = raw_segment.replace("~1", "/").replace("~0", "~")
            assert isinstance(target, dict) and segment in target, (
                f"unresolved OpenAPI ref: {reference}"
            )
            target = target[segment]

    print(f"OpenAPI valid; discovered {len(operation_ids)} operations.")


if __name__ == "__main__":
    main()
