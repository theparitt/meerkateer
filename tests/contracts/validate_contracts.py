#!/usr/bin/env python3
"""Validate Meerkateer schemas, fixtures, and invariants schemas cannot express."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

from jsonschema import FormatChecker
from jsonschema.validators import validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = ROOT / "schemas"
FIXTURE_ROOT = Path(__file__).resolve().parent / "fixtures"

FORBIDDEN_KEYS = {
    "access_token",
    "agent_credential",
    "api_key",
    "authorization",
    "chat",
    "cookie",
    "credential",
    "database_url",
    "email",
    "enrollment_token",
    "ip_address",
    "password",
    "player_id",
    "player_name",
    "phone",
    "private_key",
    "refresh_token",
    "service_key",
    "session_token",
    "steam_id",
}
SECRET_VALUE = re.compile(
    r"(?i)(?:\bbearer\s+\S+|\b(?:sk_(?:test|live)|whsec)_[A-Za-z0-9]{8,})"
)


def load_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def scan_unsafe(value: Any, location: str = "$") -> list[str]:
    errors: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_location = f"{location}.{key}"
            if key.lower() in FORBIDDEN_KEYS:
                errors.append(f"{child_location}: forbidden sensitive field")
            errors.extend(scan_unsafe(child, child_location))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            errors.extend(scan_unsafe(child, f"{location}[{index}]"))
    elif isinstance(value, str) and SECRET_VALUE.search(value):
        errors.append(f"{location}: value resembles a credential")
    return errors


def semantic_errors(schema_name: str, instance: Any) -> list[str]:
    # Enrollment returns the initial agent credential exactly once by design.
    errors = [] if schema_name == "enroll-response" else scan_unsafe(instance)
    if not isinstance(instance, dict):
        return errors

    checks = instance.get("checks")
    if schema_name in {"health", "ready"} and isinstance(checks, dict):
        all_ok = all(
            isinstance(check, dict) and check.get("ok") is True
            for check in checks.values()
        )
        if schema_name == "health":
            expected = "ok" if all_ok else "degraded"
            if instance.get("status") != expected:
                errors.append(f"$.status: must be {expected!r} for the supplied checks")
        else:
            if instance.get("ready") is not all_ok:
                errors.append(f"$.ready: must be {all_ok!r} for the supplied checks")

    if schema_name == "metadata":
        endpoints = instance.get("endpoints", {})
        capabilities = instance.get("capabilities", {})
        if isinstance(endpoints, dict) and isinstance(capabilities, dict):
            if capabilities.get("server_info") is True and "server_info" not in endpoints:
                errors.append(
                    "$.endpoints.server_info: required when server_info capability is true"
                )

    if schema_name == "check-in":
        collectors = instance.get("collectors")
        if isinstance(collectors, dict):
            all_ok = all(
                isinstance(collector, dict) and collector.get("ok") is True
                for collector in collectors.values()
            )
            expected = "ok" if all_ok else "degraded"
            if instance.get("state") != expected:
                errors.append(f"$.state: must be {expected!r} for the supplied collectors")

    if schema_name == "config":
        checks = instance.get("checks")
        if isinstance(checks, list):
            check_ids: set[str] = set()
            for index, check in enumerate(checks):
                if not isinstance(check, dict):
                    continue
                check_id = check.get("check_id")
                if isinstance(check_id, str):
                    if check_id in check_ids:
                        errors.append(f"$.checks[{index}].check_id: duplicate check ID")
                    check_ids.add(check_id)
                interval = check.get("interval_seconds")
                timeout = check.get("timeout_ms")
                if isinstance(interval, int) and isinstance(timeout, int):
                    if timeout >= interval * 1000:
                        errors.append(
                            f"$.checks[{index}].timeout_ms: must be shorter than interval"
                        )
                parameters = check.get("parameters")
                if isinstance(parameters, dict):
                    minimum = parameters.get("expected_status_min")
                    maximum = parameters.get("expected_status_max")
                    if isinstance(minimum, int) and isinstance(maximum, int) and minimum > maximum:
                        errors.append(
                            f"$.checks[{index}].parameters: status minimum exceeds maximum"
                        )

    if schema_name == "telemetry":
        records = instance.get("records")
        if isinstance(records, list) and records:
            sequences = [record.get("sequence") for record in records if isinstance(record, dict)]
            record_ids = [record.get("record_id") for record in records if isinstance(record, dict)]
            if len(sequences) == len(records) and all(isinstance(item, int) for item in sequences):
                if any(current <= previous for previous, current in zip(sequences, sequences[1:])):
                    errors.append("$.records: sequences must be strictly increasing")
                if instance.get("first_sequence") != sequences[0]:
                    errors.append("$.first_sequence: must match the first record")
                if instance.get("last_sequence") != sequences[-1]:
                    errors.append("$.last_sequence: must match the last record")
            if len(record_ids) != len(set(record_ids)):
                errors.append("$.records: record IDs must be unique within a batch")

    if schema_name == "telemetry-ack":
        status = instance.get("status")
        rejected = instance.get("rejected")
        if status == "accepted" and rejected:
            errors.append("$.rejected: accepted acknowledgement cannot reject records")
        if status == "partial" and not rejected:
            errors.append("$.rejected: partial acknowledgement must list rejected records")
    return errors


def validate_schema(path: Path) -> tuple[Any, Any]:
    schema = load_json(path)
    validator_class = validator_for(schema)
    validator_class.check_schema(schema)
    return schema, validator_class(schema, format_checker=FormatChecker())


def main() -> int:
    failures: list[str] = []
    validators: dict[tuple[str, str], Any] = {}

    for schema_path in sorted(SCHEMA_ROOT.glob("*/*.schema.json")):
        try:
            _, validator = validate_schema(schema_path)
            validators[(schema_path.parent.name, schema_path.name.removesuffix(".schema.json"))] = validator
        except Exception as error:  # schema compiler errors have several concrete types
            failures.append(f"{schema_path.relative_to(ROOT)}: invalid schema: {error}")

    fixture_count = 0
    for fixture_path in sorted(FIXTURE_ROOT.glob("*/*.json")):
        fixture_count += 1
        parts = fixture_path.name.split(".")
        if len(parts) < 4 or parts[1] not in {"valid", "invalid"}:
            failures.append(f"{fixture_path.relative_to(ROOT)}: invalid fixture filename")
            continue

        protocol = fixture_path.parent.name
        schema_name, expectation = parts[0], parts[1]
        validator = validators.get((protocol, schema_name))
        if validator is None:
            failures.append(
                f"{fixture_path.relative_to(ROOT)}: no schema for {protocol}/{schema_name}"
            )
            continue

        try:
            instance = load_json(fixture_path)
        except Exception as error:
            failures.append(f"{fixture_path.relative_to(ROOT)}: invalid JSON: {error}")
            continue

        errors = [
            f"{'.'.join(str(part) for part in error.absolute_path) or '$'}: {error.message}"
            for error in validator.iter_errors(instance)
        ]
        errors.extend(semantic_errors(schema_name, instance))

        if expectation == "valid" and errors:
            failures.append(
                f"{fixture_path.relative_to(ROOT)}: expected valid, got: {'; '.join(errors)}"
            )
        elif expectation == "invalid" and not errors:
            failures.append(
                f"{fixture_path.relative_to(ROOT)}: expected invalid but all checks passed"
            )

    if fixture_count == 0:
        failures.append("no contract fixtures found")

    if failures:
        print("Contract validation failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(f"Validated {len(validators)} schemas and {fixture_count} fixtures.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
