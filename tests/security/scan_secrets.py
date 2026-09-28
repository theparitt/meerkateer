#!/usr/bin/env python3
"""Fail on common committed credential shapes without printing the credential."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SKIP_DIRS = {".git", "node_modules", "target", "dist", ".data"}
PATTERNS = {
    "AWS access key": re.compile(rb"\bAKIA[0-9A-Z]{16}\b"),
    "GitHub token": re.compile(rb"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{30,}\b"),
    "GitHub fine-grained token": re.compile(rb"\bgithub_pat_[A-Za-z0-9_]{40,}\b"),
    "Stripe secret key": re.compile(rb"\bsk_(?:test|live)_[A-Za-z0-9]{12,}\b"),
    "Stripe webhook secret": re.compile(rb"\bwhsec_[A-Za-z0-9]{12,}\b"),
    "private key": re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
}


def main() -> int:
    findings: list[tuple[Path, str]] = []
    for path in ROOT.rglob("*"):
        if not path.is_file() or any(part in SKIP_DIRS for part in path.parts):
            continue
        try:
            data = path.read_bytes()
        except OSError:
            continue
        if b"\x00" in data:
            continue
        for label, pattern in PATTERNS.items():
            if pattern.search(data):
                findings.append((path.relative_to(ROOT), label))

    if findings:
        print("Potential committed secrets found (values intentionally omitted):", file=sys.stderr)
        for path, label in findings:
            print(f"- {path}: {label}", file=sys.stderr)
        return 1

    print("No known credential shapes found.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
