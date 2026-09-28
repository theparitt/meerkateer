# Meerkateer Python SDK

The dependency-free Python SDK sends authenticated MKS-1 heartbeat, event, and deployment
facts. It requires HTTPS except for loopback development URLs, bounds response bodies, retries
transient failures with the same idempotency key, and never includes the service key in errors.

Install directly from this repository:

```sh
python3 -m pip install ./sdk/python
```

Copy the one-time SDK key shown after creating a process in the Console and set:

```sh
export MEERKATEER_URL=https://meerkateer.example.com
export MEERKATEER_SERVICE_KEY='<one-time-key>'
export MEERKATEER_PROJECT=arena
export MEERKATEER_SERVICE=game-server-01
export MEERKATEER_ENVIRONMENT=production
```

Then instrument the process:

```python
from meerkateer_sdk import Meerkateer

meerkateer = Meerkateer.from_env()
meerkateer.heartbeat("ok", message="game loop healthy")

try:
    start_game_server()
except Exception:
    meerkateer.event(
        "game_server_failed",
        level="error",
        message="process startup failed",
    )
    meerkateer.heartbeat("down", message="game process exited")
    raise
```

Do not send player names, player identifiers, chat, IP addresses, credentials, database rows,
query text, or exception dumps. Use bounded operational causes such as `process startup failed`.
