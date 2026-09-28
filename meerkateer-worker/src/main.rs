use std::time::Duration;

use anyhow::{Context, ensure};
use clap::Parser;
use meerkateer_config::ServerConfig;
use secrecy::ExposeSecret;
use serde::Deserialize;
use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::time;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(version, about = "Meerkateer durable background worker")]
struct Cli {
    /// Claim one batch, finish it, and exit.
    #[arg(long)]
    once: bool,

    /// Maximum number of outbox records claimed in one transaction.
    #[arg(long, default_value_t = 50)]
    batch_size: u16,

    /// Move a repeatedly failing record to the dead-letter queue at this attempt.
    #[arg(long, default_value_t = 5)]
    max_attempts: u16,

    /// Delay between claim cycles when no shutdown was requested.
    #[arg(long, default_value_t = 1_000)]
    poll_interval_ms: u64,
}

#[derive(Debug)]
struct ClaimedOutbox {
    id: Uuid,
    tenant_id: Uuid,
    topic: String,
    payload: Value,
    attempts: u32,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CycleStats {
    claimed: usize,
    completed: usize,
    retried: usize,
    dead_lettered: usize,
}

#[derive(Debug, Deserialize)]
struct IngestEvent {
    service_id: Uuid,
    idempotency_key: Uuid,
    message_kind: String,
}

#[derive(Debug, Deserialize)]
struct AgentTelemetryEvent {
    agent_id: Uuid,
    batch_id: Uuid,
    first_sequence: u64,
    last_sequence: u64,
    #[serde(rename = "gap_detected")]
    _gap_detected: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let cli = Cli::parse();
    validate_cli(&cli)?;
    let config = ServerConfig::load().context("invalid worker configuration")?;
    let database_url = config
        .database_url
        .as_ref()
        .context("MEERKATEER_DATABASE_URL is required by the worker")?;
    let database = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url.expose_secret())
        .await
        .context("failed to connect to the control database")?;
    let worker_id = Uuid::new_v4();
    info!(
        %worker_id,
        deployment_mode = config.deployment_mode.as_str(),
        environment = %config.environment,
        batch_size = cli.batch_size,
        max_attempts = cli.max_attempts,
        "starting Meerkateer worker"
    );

    run_cycle(
        &database,
        worker_id,
        i32::from(cli.batch_size),
        u32::from(cli.max_attempts),
    )
    .await?;
    if cli.once {
        return Ok(());
    }

    let mut interval = time::interval(Duration::from_millis(cli.poll_interval_ms));
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    interval.tick().await;
    loop {
        tokio::select! {
            _ = interval.tick() => {
                run_cycle(
                    &database,
                    worker_id,
                    i32::from(cli.batch_size),
                    u32::from(cli.max_attempts),
                ).await?;
            },
            result = tokio::signal::ctrl_c() => {
                result.context("failed to install shutdown signal")?;
                info!(%worker_id, "worker shutdown requested");
                return Ok(());
            }
        }
    }
}

fn validate_cli(cli: &Cli) -> anyhow::Result<()> {
    ensure!(
        (1..=100).contains(&cli.batch_size),
        "--batch-size must be between 1 and 100"
    );
    ensure!(
        (1..=100).contains(&cli.max_attempts),
        "--max-attempts must be between 1 and 100"
    );
    ensure!(
        (100..=60_000).contains(&cli.poll_interval_ms),
        "--poll-interval-ms must be between 100 and 60000"
    );
    Ok(())
}

async fn run_cycle(
    database: &PgPool,
    worker_id: Uuid,
    batch_size: i32,
    max_attempts: u32,
) -> anyhow::Result<CycleStats> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, Value, i32)>(
        "SELECT id, tenant_id, topic, payload, attempts \
         FROM meerkateer_claim_outbox($1, $2)",
    )
    .bind(worker_id)
    .bind(batch_size)
    .fetch_all(database)
    .await
    .context("failed to claim outbox records")?;

    let mut stats = CycleStats {
        claimed: rows.len(),
        ..CycleStats::default()
    };
    for (id, tenant_id, topic, payload, attempts) in rows {
        let event = ClaimedOutbox {
            id,
            tenant_id,
            topic,
            payload,
            attempts: u32::try_from(attempts).context("database returned a negative attempt")?,
        };
        match dispatch(&event) {
            Ok(()) => {
                complete(database, worker_id, event.id).await?;
                stats.completed += 1;
            }
            Err(error) if event.attempts >= max_attempts => {
                dead_letter(database, worker_id, event.id, error).await?;
                warn!(
                    outbox_id = %event.id,
                    tenant_id = %event.tenant_id,
                    topic = %event.topic,
                    attempts = event.attempts,
                    reason = error,
                    "outbox record moved to dead-letter queue"
                );
                stats.dead_lettered += 1;
            }
            Err(error) => {
                let delay_seconds = retry_delay_seconds(event.attempts);
                retry(database, worker_id, event.id, error, delay_seconds).await?;
                warn!(
                    outbox_id = %event.id,
                    tenant_id = %event.tenant_id,
                    topic = %event.topic,
                    attempts = event.attempts,
                    delay_seconds,
                    reason = error,
                    "outbox record scheduled for retry"
                );
                stats.retried += 1;
            }
        }
    }

    if stats.claimed > 0 {
        info!(
            %worker_id,
            claimed = stats.claimed,
            completed = stats.completed,
            retried = stats.retried,
            dead_lettered = stats.dead_lettered,
            "worker cycle complete"
        );
    }
    Ok(stats)
}

fn dispatch(event: &ClaimedOutbox) -> Result<(), &'static str> {
    match event.topic.as_str() {
        "ingest.heartbeat" | "ingest.event" | "ingest.deploy" => {
            let payload: IngestEvent = serde_json::from_value(event.payload.clone())
                .map_err(|_| "invalid ingest outbox payload")?;
            let expected_kind = event
                .topic
                .strip_prefix("ingest.")
                .ok_or("invalid ingest outbox topic")?;
            if payload.service_id.is_nil()
                || payload.idempotency_key.is_nil()
                || payload.message_kind != expected_kind
            {
                return Err("ingest outbox identity or kind mismatch");
            }
            Ok(())
        }
        "agent.telemetry" => {
            let payload: AgentTelemetryEvent = serde_json::from_value(event.payload.clone())
                .map_err(|_| "invalid agent telemetry outbox payload")?;
            if payload.agent_id.is_nil()
                || payload.batch_id.is_nil()
                || payload.first_sequence == 0
                || payload.last_sequence < payload.first_sequence
            {
                return Err("invalid agent telemetry outbox sequence");
            }
            Ok(())
        }
        _ => Err("unsupported outbox topic"),
    }
}

const fn retry_delay_seconds(attempts: u32) -> i32 {
    match attempts {
        0 | 1 => 5,
        2 => 10,
        3 => 20,
        4 => 40,
        5 => 80,
        6 => 160,
        _ => 300,
    }
}

async fn complete(database: &PgPool, worker_id: Uuid, outbox_id: Uuid) -> anyhow::Result<()> {
    let changed: bool = sqlx::query_scalar("SELECT meerkateer_complete_outbox($1, $2)")
        .bind(worker_id)
        .bind(outbox_id)
        .fetch_one(database)
        .await
        .context("failed to complete outbox record")?;
    ensure!(changed, "outbox completion lease was lost");
    Ok(())
}

async fn retry(
    database: &PgPool,
    worker_id: Uuid,
    outbox_id: Uuid,
    error: &str,
    delay_seconds: i32,
) -> anyhow::Result<()> {
    let changed: bool = sqlx::query_scalar("SELECT meerkateer_retry_outbox($1, $2, $3, $4)")
        .bind(worker_id)
        .bind(outbox_id)
        .bind(error)
        .bind(delay_seconds)
        .fetch_one(database)
        .await
        .context("failed to retry outbox record")?;
    ensure!(changed, "outbox retry lease was lost");
    Ok(())
}

async fn dead_letter(
    database: &PgPool,
    worker_id: Uuid,
    outbox_id: Uuid,
    error: &str,
) -> anyhow::Result<()> {
    let changed: bool = sqlx::query_scalar("SELECT meerkateer_dead_letter_outbox($1, $2, $3)")
        .bind(worker_id)
        .bind(outbox_id)
        .bind(error)
        .fetch_one(database)
        .await
        .context("failed to dead-letter outbox record")?;
    ensure!(changed, "outbox dead-letter lease was lost");
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::{ClaimedOutbox, dispatch, retry_delay_seconds};

    fn event(topic: &str, payload: serde_json::Value) -> ClaimedOutbox {
        ClaimedOutbox {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            topic: topic.to_owned(),
            payload,
            attempts: 1,
        }
    }

    #[test]
    fn accepts_supported_ingest_topic_with_matching_kind() {
        let event = event(
            "ingest.heartbeat",
            json!({
                "service_id": Uuid::new_v4(),
                "idempotency_key": Uuid::new_v4(),
                "message_kind": "heartbeat"
            }),
        );
        assert!(dispatch(&event).is_ok());
    }

    #[test]
    fn rejects_ingest_kind_mismatch() {
        let event = event(
            "ingest.event",
            json!({
                "service_id": Uuid::new_v4(),
                "idempotency_key": Uuid::new_v4(),
                "message_kind": "deploy"
            }),
        );
        assert_eq!(
            dispatch(&event),
            Err("ingest outbox identity or kind mismatch")
        );
    }

    #[test]
    fn accepts_valid_agent_batch_and_rejects_invalid_sequence() {
        let valid = event(
            "agent.telemetry",
            json!({
                "agent_id": Uuid::new_v4(),
                "batch_id": Uuid::new_v4(),
                "first_sequence": 3,
                "last_sequence": 7,
                "gap_detected": true
            }),
        );
        assert!(dispatch(&valid).is_ok());

        let invalid = event(
            "agent.telemetry",
            json!({
                "agent_id": Uuid::new_v4(),
                "batch_id": Uuid::new_v4(),
                "first_sequence": 8,
                "last_sequence": 7,
                "gap_detected": false
            }),
        );
        assert_eq!(
            dispatch(&invalid),
            Err("invalid agent telemetry outbox sequence")
        );
    }

    #[test]
    fn rejects_unknown_topics_without_exposing_payload() {
        let event = event("billing.unknown", json!({"secret": "do-not-log"}));
        assert_eq!(dispatch(&event), Err("unsupported outbox topic"));
    }

    #[test]
    fn retry_backoff_is_bounded() {
        assert_eq!(retry_delay_seconds(1), 5);
        assert_eq!(retry_delay_seconds(4), 40);
        assert_eq!(retry_delay_seconds(6), 160);
        assert_eq!(retry_delay_seconds(50), 300);
    }
}
