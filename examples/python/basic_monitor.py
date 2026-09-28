"""Minimal Meerkateer instrumentation for a long-running process."""

import time

from meerkateer_sdk import Meerkateer


client = Meerkateer.from_env()

while True:
    try:
        # Replace this with a bounded local health check for your process.
        client.heartbeat("ok", message="process responsive")
    except Exception as error:
        # Telemetry failure must not crash the monitored process.
        print(f"Meerkateer heartbeat failed: {type(error).__name__}")
    time.sleep(30)
