# Meerkateer Node.js SDK

The dependency-free Node.js 22+ SDK sends MKS-1 heartbeats, events, and deployment facts.
It requires HTTPS except for loopback development, rejects redirects, bounds payloads and
responses, and retries transient failures with the same idempotency key. Install from this
repository until a package is published:

```sh
npm install ./sdk/node
```

Create a process in the Console and set the environment block it shows, including
`MEERKATEER_URL`, `MEERKATEER_SERVICE_KEY`, `MEERKATEER_PROJECT`, and
`MEERKATEER_SERVICE`. Then:

```js
import { Meerkateer } from "@meerkateer/sdk";

const watch = Meerkateer.fromEnv();
await watch.heartbeat("ok", { message: "worker ready" });
await watch.event("queue_delay", { level: "warning", message: "jobs delayed" });
await watch.deploy("1.2.3", "abcdef123456");
```

Send short operational descriptions only. Do not include order data, customer details,
credentials, request bodies, or stack traces. For durable application queues, pass the
same `idempotencyKey` again after a restart.
