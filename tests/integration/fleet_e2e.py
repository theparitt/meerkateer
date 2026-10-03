#!/usr/bin/env python3
"""Concurrent real-agent fleet scenarios for failure detection and recovery."""

from __future__ import annotations

import concurrent.futures
import json
import os
import pathlib
import random
import subprocess
import tempfile
import time
from typing import Any

from api_e2e import (
    BASE_URL,
    browser_headers,
    call,
    cookies,
    expect,
)


AGENT_BIN = os.environ.get("MEERKATEER_AGENT_BIN")
FLEET_SIZE = 10
FAILED_COUNT = 4
RECOVERED_COUNT = 2
RANDOM_SEED = 20260928
STALE_AFTER_SECONDS = int(os.environ.get("MEERKATEER_FLEET_STALE_AFTER_SECONDS", "30"))


def parse_agent_output(completed: subprocess.CompletedProcess[str]) -> dict[str, Any]:
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
        if isinstance(parsed, dict) and "status" in parsed:
            return parsed
    raise AssertionError(("agent emitted no result JSON", completed.stdout, completed.stderr))


def enroll(config_path: pathlib.Path, index: int, token: str) -> dict[str, Any]:
    assert AGENT_BIN, "MEERKATEER_AGENT_BIN is required"
    environment = {**os.environ, "MEERKATEER_ENROLLMENT_TOKEN": token}
    completed = subprocess.run(
        [
            AGENT_BIN,
            "--config",
            str(config_path),
            "enroll",
            "--server",
            BASE_URL,
            "--name",
            f"Fleet Sim {index:02d}",
        ],
        check=True,
        capture_output=True,
        text=True,
        env=environment,
        timeout=30,
    )
    return parse_agent_output(completed)


def run_once(config_path: pathlib.Path) -> dict[str, Any]:
    assert AGENT_BIN, "MEERKATEER_AGENT_BIN is required"
    completed = subprocess.run(
        [AGENT_BIN, "--config", str(config_path), "run", "--once"],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    parsed = parse_agent_output(completed)
    assert parsed["status"] == "accepted", parsed
    return parsed


def parallel(function: Any, items: list[Any]) -> list[Any]:
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(items)) as executor:
        futures = [executor.submit(function, item) for item in items]
        return [future.result(timeout=45) for future in futures]


def fleet_states(project_id: str, agent_ids: set[str]) -> dict[str, str]:
    agents = expect(call("GET", f"/v1/projects/{project_id}/agents"), 200).body["items"]
    fleet = {
        agent["id"]: agent["connection_state"]
        for agent in agents
        if agent["id"] in agent_ids
    }
    assert set(fleet) == agent_ids, (fleet, agent_ids)
    return fleet


def retry_after_network_failure(config_path: pathlib.Path) -> None:
    assert AGENT_BIN, "MEERKATEER_AGENT_BIN is required"
    config = json.loads(config_path.read_text(encoding="utf-8"))
    server_url = config["server_url"]
    next_sequence = config["next_sequence"]
    config["server_url"] = "http://127.0.0.1:1/"
    config_path.write_text(json.dumps(config), encoding="utf-8")

    failed = subprocess.run(
        [AGENT_BIN, "--config", str(config_path), "run", "--once"],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert failed.returncode != 0, failed.stdout
    assert "mka_agent_" not in failed.stdout + failed.stderr
    pending = json.loads(config_path.read_text(encoding="utf-8"))
    assert len(pending["pending_batches"]) == 1, pending
    pending_batch = pending["pending_batches"][0]
    assert pending_batch["first_sequence"] == next_sequence, pending
    pending_last_sequence = pending_batch["last_sequence"]
    pending_batch_id = pending_batch["batch_id"]

    pending["server_url"] = server_url
    config_path.write_text(json.dumps(pending), encoding="utf-8")
    accepted = run_once(config_path)
    assert accepted["batch_id"] == pending_batch_id, accepted
    persisted = json.loads(config_path.read_text(encoding="utf-8"))
    assert "pending_batch" not in persisted, persisted
    assert "pending_batches" not in persisted, persisted
    assert persisted["next_sequence"] == pending_last_sequence + 1, persisted


def main() -> None:
    assert STALE_AFTER_SECONDS >= 30
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
    projects = expect(call("GET", "/v1/projects"), 200).body["items"]
    project = next(item for item in projects if item["slug"] == "arena-ops")
    project_id = project["id"]

    tokens = [
        expect(
            call(
                "POST",
                f"/v1/projects/{project_id}/enrollment-tokens",
                body={"expires_in_seconds": 600},
                headers=browser_headers(),
            ),
            201,
        ).body["secret"]
        for _ in range(FLEET_SIZE)
    ]

    with tempfile.TemporaryDirectory(prefix="meerkateer-fleet-e2e-") as directory:
        configs = [pathlib.Path(directory) / f"agent-{index:02d}.json" for index in range(FLEET_SIZE)]
        with concurrent.futures.ThreadPoolExecutor(max_workers=FLEET_SIZE) as executor:
            futures = [
                executor.submit(enroll, configs[index], index, tokens[index])
                for index in range(FLEET_SIZE)
            ]
            enrolled = [future.result(timeout=45) for future in futures]
        agent_ids = {str(item["agent_id"]) for item in enrolled}
        assert len(agent_ids) == FLEET_SIZE, enrolled

        first_batches = parallel(run_once, configs)
        assert len({item["batch_id"] for item in first_batches}) == FLEET_SIZE
        initial = fleet_states(project_id, agent_ids)
        assert set(initial.values()) == {"online"}, initial

        randomizer = random.Random(RANDOM_SEED)
        failed_indexes = set(randomizer.sample(range(FLEET_SIZE), FAILED_COUNT))
        healthy_indexes = [index for index in range(FLEET_SIZE) if index not in failed_indexes]
        time.sleep(STALE_AFTER_SECONDS + 1.5)
        parallel(run_once, [configs[index] for index in healthy_indexes])

        degraded = fleet_states(project_id, agent_ids)
        failed_ids = {str(enrolled[index]["agent_id"]) for index in failed_indexes}
        assert {agent_id for agent_id, state in degraded.items() if state == "stale"} == failed_ids, degraded
        assert sum(state == "online" for state in degraded.values()) == FLEET_SIZE - FAILED_COUNT, degraded

        recovered_indexes = sorted(randomizer.sample(sorted(failed_indexes), RECOVERED_COUNT))
        parallel(run_once, [configs[index] for index in recovered_indexes])
        recovered = fleet_states(project_id, agent_ids)
        remaining_failed_ids = failed_ids - {
            str(enrolled[index]["agent_id"]) for index in recovered_indexes
        }
        assert {agent_id for agent_id, state in recovered.items() if state == "stale"} == remaining_failed_ids, recovered
        assert sum(state == "online" for state in recovered.values()) == FLEET_SIZE - len(remaining_failed_ids), recovered

        retry_after_network_failure(configs[healthy_indexes[0]])
        final = fleet_states(project_id, agent_ids)
        assert final[str(enrolled[healthy_indexes[0]]["agent_id"])] == "online", final

    print(
        "Concurrent fleet E2E passed: "
        f"{FLEET_SIZE} agents, {FAILED_COUNT} detected stale, "
        f"{RECOVERED_COUNT} recovered, deterministic seed {RANDOM_SEED}."
    )


if __name__ == "__main__":
    main()
