use std::{env, error::Error, sync::Arc, time::Duration};

use meerkateer_sdk::{DeployStatus, EventLevel, HeartbeatStatus, Meerkateer};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::RwLock,
    time,
};

const LANGUAGE: &str = "rust";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let client = Arc::new(Meerkateer::from_env()?);
    let state = Arc::new(RwLock::new(String::from("ok")));
    client
        .deploy("demo-1", "local", DeployStatus::Finished)
        .await?;
    signal(&client, &state, "ok").await?;

    let interval = env::var("DEMO_HEARTBEAT_SECONDS")
        .unwrap_or_else(|_| String::from("5"))
        .parse::<u64>()?;
    if interval == 0 {
        return Err("DEMO_HEARTBEAT_SECONDS must be positive".into());
    }
    let ticker_client = Arc::clone(&client);
    let ticker_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(interval));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let current = ticker_state.read().await.clone();
            if let Err(error) = ticker_client
                .heartbeat(status(&current), Some(format!("{LANGUAGE} demo heartbeat")))
                .await
            {
                eprintln!("heartbeat delivery failed: {error}");
            }
        }
    });

    let port = env::var("DEMO_PORT").unwrap_or_else(|_| String::from("19105"));
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&address).await?;
    println!("{LANGUAGE} demo listening on http://{address}");
    loop {
        let (stream, _) = listener.accept().await?;
        let request_client = Arc::clone(&client);
        let request_state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle(stream, request_client, request_state).await {
                eprintln!("request failed: {error}");
            }
        });
    }
}

async fn handle(
    mut stream: TcpStream,
    client: Arc<Meerkateer>,
    state: Arc<RwLock<String>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut buffer = [0_u8; 4096];
    let bytes = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..bytes]);
    let first_line = request.lines().next().unwrap_or_default();
    let (status_code, body) = if first_line == "GET /health HTTP/1.1" {
        let current = state.read().await.clone();
        (
            200,
            format!(r#"{{"language":"{LANGUAGE}","status":"{current}"}}"#),
        )
    } else if let Some(next) = scenario(first_line) {
        match signal(&client, &state, next).await {
            Ok(()) => (
                200,
                format!(r#"{{"language":"{LANGUAGE}","status":"{next}"}}"#),
            ),
            Err(error) => (
                502,
                format!(r#"{{"error":"telemetry_failed","detail":"{error}"}}"#),
            ),
        }
    } else {
        (404, String::from(r#"{"error":"not_found"}"#))
    };
    let reason = if status_code == 200 {
        "OK"
    } else if status_code == 404 {
        "Not Found"
    } else {
        "Bad Gateway"
    };
    let response = format!(
        "HTTP/1.1 {status_code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

fn scenario(request_line: &str) -> Option<&'static str> {
    match request_line {
        "POST /scenario/healthy HTTP/1.1" => Some("ok"),
        "POST /scenario/degraded HTTP/1.1" => Some("degraded"),
        "POST /scenario/down HTTP/1.1" => Some("down"),
        _ => None,
    }
}

fn status(value: &str) -> HeartbeatStatus {
    match value {
        "degraded" => HeartbeatStatus::Degraded,
        "down" => HeartbeatStatus::Down,
        _ => HeartbeatStatus::Ok,
    }
}

async fn signal(
    client: &Meerkateer,
    state: &RwLock<String>,
    next: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let message = match next {
        "degraded" => "demo dependency is slow",
        "down" => "demo process is unavailable",
        _ => "demo service recovered",
    };
    let previous = state.read().await.clone();
    *state.write().await = next.to_owned();
    client
        .heartbeat(status(next), Some(message.to_owned()))
        .await?;
    if previous != next {
        let (kind, level) = match next {
            "degraded" => ("demo_degraded", EventLevel::Warning),
            "down" => ("demo_failed", EventLevel::Error),
            _ => ("demo_recovered", EventLevel::Info),
        };
        client
            .event(kind, level, Some(message.to_owned()), 1)
            .await?;
    }
    Ok(())
}
