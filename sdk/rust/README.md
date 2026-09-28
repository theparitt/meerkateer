# Meerkateer Rust SDK

The Rust SDK provides async, typed MKS-1 heartbeat, event, and deployment delivery for
game servers and SME services. It uses Rustls, rejects redirects, requires HTTPS away from
loopback, bounds responses, redacts credentials, and retries transient failures with the same
idempotency key.

During workspace development, add:

```toml
[dependencies]
meerkateer-sdk = { path = "../meerkateer/sdk/rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Set the environment block shown by the Console, then:

```rust
use meerkateer_sdk::{EventLevel, HeartbeatStatus, Meerkateer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Meerkateer::from_env()?;
    client
        .heartbeat(HeartbeatStatus::Ok, Some("game loop healthy".to_owned()))
        .await?;
    client
        .event(
            "matchmaker_unavailable",
            EventLevel::Error,
            Some("dependency unavailable".to_owned()),
            1,
        )
        .await?;
    Ok(())
}
```

For retries across an application-owned durable queue, use the `*_with_idempotency` methods
and retain the UUID with the queued payload until an acknowledgement is received.
