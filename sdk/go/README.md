# Meerkateer Go SDK

The standard-library Go SDK sends MKS-1 heartbeat, event, and deployment facts. It
requires HTTPS except for loopback development, rejects redirects, bounds payloads and
responses, and retries transient failures with the same idempotency key.

From your Go service, add the module from this repository:

```sh
go get github.com/theparitt/meerkateer/sdk/go@main
```

Create a process in the Console and set its `MEERKATEER_URL`,
`MEERKATEER_SERVICE_KEY`, `MEERKATEER_PROJECT`, and `MEERKATEER_SERVICE` values:

```go
package main

import (
    "context"
    "log"

    meerkateer "github.com/theparitt/meerkateer/sdk/go"
)

func main() {
    watch, err := meerkateer.FromEnv()
    if err != nil { log.Fatal(err) }
    if err := watch.Heartbeat(context.Background(), "ok", "worker ready"); err != nil {
        log.Print(err)
    }
}
```

Only send short operational descriptions. Do not send order data, customer details,
credentials, request bodies, or stack traces. Use `HeartbeatWithID` with a retained UUID
when your application owns a durable retry queue.
