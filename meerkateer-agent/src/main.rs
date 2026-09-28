use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail, ensure};
use atomic_write_file::OpenOptions;
use chrono::{DateTime, SecondsFormat, Utc};
use clap::{Parser, Subcommand};
use meerkateer_protocol::mka1::{PROTOCOL_VERSION, TelemetryBatch, TelemetryRecord};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use tokio::time;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

const CONFIG_VERSION: u8 = 1;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const DEFAULT_INTERVAL_SECONDS: u64 = 30;

#[derive(Debug, Parser)]
#[command(version, about = "Outbound-only Meerkateer host agent")]
struct Cli {
    /// Override the local credential/configuration file.
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Enroll this machine with a one-time token from the Console.
    Enroll {
        /// Meerkateer API base URL. HTTPS is required except on loopback.
        #[arg(long)]
        server: String,
        /// Machine name displayed in the Console.
        #[arg(long)]
        name: String,
    },
    /// Verify the local configuration without displaying credentials.
    Doctor,
    /// Send host heartbeat telemetry until stopped.
    Run {
        /// Send one batch and exit.
        #[arg(long)]
        once: bool,
        /// Seconds between heartbeat batches.
        #[arg(long, default_value_t = DEFAULT_INTERVAL_SECONDS, value_parser = clap::value_parser!(u64).range(5..=3600))]
        interval_seconds: u64,
    },
}

#[derive(Serialize, Deserialize)]
struct AgentConfig {
    version: u8,
    server_url: String,
    installation_id: Uuid,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_expires_at: Option<String>,
    next_sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_batch: Option<TelemetryBatch>,
}

#[derive(Debug, Serialize)]
struct DoctorResult<'a> {
    status: &'a str,
    platform: &'static str,
    architecture: &'static str,
    config_path: String,
    server_url: &'a str,
    enrolled: bool,
    agent_id: Option<Uuid>,
    project_id: Option<Uuid>,
    credential_expires_at: Option<&'a str>,
    pending_batch: bool,
    next_sequence: u64,
}

#[derive(Debug, Serialize)]
struct EnrollmentResult<'a> {
    status: &'static str,
    agent_id: Uuid,
    project_id: Option<Uuid>,
    credential_expires_at: &'a str,
    config_path: String,
}

#[derive(Debug, Serialize)]
struct SendResult {
    status: &'static str,
    agent_id: Uuid,
    batch_id: Uuid,
    accepted_through_sequence: u64,
}

#[derive(Debug, Serialize)]
struct EnrollRequest<'a> {
    installation_id: Uuid,
    display_name: &'a str,
}

#[derive(Debug, Deserialize)]
struct EnrollResponse {
    agent_id: Uuid,
    project_id: Option<Uuid>,
    credential_id: Uuid,
    secret: String,
    expires_at: String,
}

#[derive(Debug, Deserialize)]
struct TelemetryAck {
    protocol_version: String,
    agent_id: Uuid,
    batch_id: Uuid,
    status: String,
    accepted_through_sequence: u64,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    code: String,
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
    let config_path = resolve_config_path(cli.config)?;
    match cli.command {
        Command::Enroll { server, name } => enroll(&config_path, &server, &name).await,
        Command::Doctor => doctor(&config_path),
        Command::Run {
            once,
            interval_seconds,
        } => run(&config_path, once, interval_seconds).await,
    }
}

async fn enroll(config_path: &Path, server: &str, name: &str) -> anyhow::Result<()> {
    let server_url = validate_server_url(server)?;
    validate_display_name(name)?;
    let token = env::var("MEERKATEER_ENROLLMENT_TOKEN")
        .context("MEERKATEER_ENROLLMENT_TOKEN is required")?;
    ensure!(
        !token.trim().is_empty(),
        "MEERKATEER_ENROLLMENT_TOKEN is empty"
    );

    let mut config = match load_config(config_path) {
        Ok(existing) => {
            ensure!(
                existing.agent_id.is_none(),
                "this config is already enrolled; use a different --config path for another agent"
            );
            ensure!(
                existing.server_url == server_url.as_str() && existing.display_name == name,
                "an incomplete enrollment exists with different server/name settings"
            );
            existing
        }
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|value| value.kind() == std::io::ErrorKind::NotFound) =>
        {
            let pending = AgentConfig {
                version: CONFIG_VERSION,
                server_url: server_url.as_str().to_owned(),
                installation_id: Uuid::new_v4(),
                display_name: name.to_owned(),
                project_id: None,
                agent_id: None,
                credential_id: None,
                credential: None,
                credential_expires_at: None,
                next_sequence: 1,
                pending_batch: None,
            };
            save_config(config_path, &pending)?;
            pending
        }
        Err(error) => return Err(error),
    };

    let endpoint = server_url.join("v1/agents/enroll")?;
    let response = http_client()?
        .post(endpoint)
        .bearer_auth(token)
        .json(&EnrollRequest {
            installation_id: config.installation_id,
            display_name: &config.display_name,
        })
        .send()
        .await
        .context("agent enrollment request failed")?;
    let status = response.status();
    let body = read_bounded_body(response).await?;
    if status != StatusCode::CREATED {
        return Err(http_error("agent enrollment", status, &body));
    }
    let enrolled: EnrollResponse = serde_json::from_slice(&body)
        .context("Meerkateer returned an invalid enrollment response")?;
    ensure!(
        enrolled.secret.starts_with("mka_agent_"),
        "Meerkateer returned an invalid agent credential"
    );

    config.project_id = enrolled.project_id;
    config.agent_id = Some(enrolled.agent_id);
    config.credential_id = Some(enrolled.credential_id);
    config.credential = Some(enrolled.secret);
    config.credential_expires_at = Some(enrolled.expires_at);
    save_config(config_path, &config).context(
        "enrollment succeeded remotely but the credential could not be saved; issue a new token and use a new --config path",
    )?;

    let expires_at = config
        .credential_expires_at
        .as_deref()
        .context("enrollment response omitted credential expiry")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&EnrollmentResult {
            status: "enrolled",
            agent_id: enrolled.agent_id,
            project_id: enrolled.project_id,
            credential_expires_at: expires_at,
            config_path: config_path.display().to_string(),
        })?
    );
    Ok(())
}

fn doctor(config_path: &Path) -> anyhow::Result<()> {
    let config = load_config(config_path)?;
    validate_complete_config(&config)?;
    let expires_at = config
        .credential_expires_at
        .as_deref()
        .context("agent configuration is missing credential expiry")?;
    let expiry = DateTime::parse_from_rfc3339(expires_at)
        .context("agent credential expiry is invalid")?
        .with_timezone(&Utc);
    let status = if expiry <= Utc::now() {
        "credential_expired"
    } else if config.pending_batch.is_some() {
        "pending_retry"
    } else {
        "ok"
    };
    let result = DoctorResult {
        status,
        platform: env::consts::OS,
        architecture: env::consts::ARCH,
        config_path: config_path.display().to_string(),
        server_url: &config.server_url,
        enrolled: true,
        agent_id: config.agent_id,
        project_id: config.project_id,
        credential_expires_at: Some(expires_at),
        pending_batch: config.pending_batch.is_some(),
        next_sequence: config.next_sequence,
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    ensure!(
        expiry > Utc::now(),
        "agent credential has expired; re-enroll this machine"
    );
    Ok(())
}

async fn run(config_path: &Path, once: bool, interval_seconds: u64) -> anyhow::Result<()> {
    let client = http_client()?;
    info!(config_path = %config_path.display(), "starting Meerkateer agent");

    loop {
        let result = send_once(&client, config_path).await;
        match result {
            Ok(result) => println!("{}", serde_json::to_string(&result)?),
            Err(error) if once => return Err(error),
            Err(error) => {
                warn!(error = %error, "telemetry delivery failed; durable batch retained for retry");
            }
        }
        if once {
            return Ok(());
        }
        tokio::select! {
            () = time::sleep(Duration::from_secs(interval_seconds)) => {}
            result = tokio::signal::ctrl_c() => {
                result.context("failed to install shutdown signal")?;
                info!("agent shutdown requested");
                return Ok(());
            }
        }
    }
}

async fn send_once(client: &Client, config_path: &Path) -> anyhow::Result<SendResult> {
    let mut config = load_config(config_path)?;
    validate_complete_config(&config)?;
    ensure_credential_valid(&config)?;
    let server_url = validate_server_url(&config.server_url)?;
    let agent_id = config
        .agent_id
        .context("agent configuration is not enrolled")?;
    let credential = config
        .credential
        .as_deref()
        .context("agent configuration is missing its credential")?;

    if config.pending_batch.is_none() {
        config.pending_batch = Some(build_heartbeat_batch(agent_id, config.next_sequence)?);
        save_config(config_path, &config)?;
    }
    let batch = config
        .pending_batch
        .as_ref()
        .context("pending telemetry batch disappeared")?;
    ensure!(
        batch.agent_id == agent_id,
        "pending batch belongs to a different agent"
    );
    ensure!(
        batch.first_sequence == config.next_sequence,
        "pending batch sequence does not match local sequence state"
    );

    let endpoint = server_url.join("v1/agent/telemetry")?;
    let response = client
        .post(endpoint)
        .bearer_auth(credential)
        .header("Idempotency-Key", batch.batch_id.to_string())
        .json(batch)
        .send()
        .await
        .context("telemetry request failed")?;
    let status = response.status();
    let body = read_bounded_body(response).await?;
    if !matches!(status, StatusCode::OK | StatusCode::ACCEPTED) {
        return Err(http_error("telemetry delivery", status, &body));
    }
    let ack: TelemetryAck = serde_json::from_slice(&body)
        .context("Meerkateer returned an invalid telemetry acknowledgement")?;
    ensure!(
        ack.protocol_version == PROTOCOL_VERSION,
        "telemetry acknowledgement protocol mismatch"
    );
    ensure!(
        ack.agent_id == agent_id,
        "telemetry acknowledgement agent mismatch"
    );
    ensure!(
        ack.batch_id == batch.batch_id,
        "telemetry acknowledgement batch mismatch"
    );
    ensure!(ack.status == "accepted", "telemetry batch was not accepted");
    ensure!(
        ack.accepted_through_sequence == batch.last_sequence,
        "telemetry acknowledgement sequence mismatch"
    );
    let result = SendResult {
        status: "accepted",
        agent_id,
        batch_id: batch.batch_id,
        accepted_through_sequence: ack.accepted_through_sequence,
    };
    config.next_sequence = batch
        .last_sequence
        .checked_add(1)
        .context("agent sequence exhausted")?;
    config.pending_batch = None;
    save_config(config_path, &config)?;
    Ok(result)
}

fn build_heartbeat_batch(agent_id: Uuid, sequence: u64) -> anyhow::Result<TelemetryBatch> {
    ensure!(sequence > 0, "agent sequence must be positive");
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let attributes = BTreeMap::from([
        ("arch".to_owned(), env::consts::ARCH.to_owned()),
        ("os".to_owned(), env::consts::OS.to_owned()),
    ]);
    let record = TelemetryRecord::Sample {
        sequence,
        record_id: Uuid::new_v4(),
        observed_at: now.clone(),
        metric: "agent.heartbeat".to_owned(),
        value: 1.0,
        attributes,
    };
    let batch = TelemetryBatch {
        protocol_version: PROTOCOL_VERSION.to_owned(),
        agent_id,
        batch_id: Uuid::new_v4(),
        first_sequence: sequence,
        last_sequence: sequence,
        sent_at: now,
        records: vec![record],
    };
    ensure!(
        batch.has_valid_shape(),
        "agent produced an invalid telemetry batch"
    );
    Ok(batch)
}

fn validate_complete_config(config: &AgentConfig) -> anyhow::Result<()> {
    ensure!(
        config.version == CONFIG_VERSION,
        "unsupported agent config version"
    );
    ensure!(config.next_sequence > 0, "agent sequence state is invalid");
    ensure!(
        config.agent_id.is_some(),
        "agent enrollment is incomplete; run enroll again"
    );
    ensure!(
        config
            .credential
            .as_ref()
            .is_some_and(|value| !value.is_empty()),
        "agent credential is missing"
    );
    ensure!(
        config.credential_expires_at.is_some(),
        "agent credential expiry is missing"
    );
    Ok(())
}

fn ensure_credential_valid(config: &AgentConfig) -> anyhow::Result<()> {
    let expires_at = config
        .credential_expires_at
        .as_deref()
        .context("agent credential expiry is missing")?;
    let expiry = DateTime::parse_from_rfc3339(expires_at)
        .context("agent credential expiry is invalid")?
        .with_timezone(&Utc);
    ensure!(
        expiry > Utc::now(),
        "agent credential has expired; re-enroll this machine"
    );
    Ok(())
}

fn validate_server_url(value: &str) -> anyhow::Result<Url> {
    let mut url = Url::parse(value).context("--server must be an absolute URL")?;
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "--server must not contain credentials"
    );
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "--server must not contain a query or fragment"
    );
    ensure!(
        matches!(url.path(), "" | "/"),
        "--server must not contain a path"
    );
    let host = url.host_str().context("--server must include a host")?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && loopback),
        "HTTPS is required unless --server is loopback"
    );
    url.set_path("/");
    Ok(url)
}

fn validate_display_name(name: &str) -> anyhow::Result<()> {
    let length = name.chars().count();
    ensure!(
        (1..=128).contains(&length) && name.trim() == name,
        "--name must contain 1-128 characters without surrounding whitespace"
    );
    ensure!(
        !name.chars().any(char::is_control),
        "--name must not contain control characters"
    );
    Ok(())
}

fn http_client() -> anyhow::Result<Client> {
    Client::builder()
        .tls_backend_rustls()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("meerkateer-agent/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to initialize the HTTP client")
}

async fn read_bounded_body(mut response: reqwest::Response) -> anyhow::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        bail!("Meerkateer response exceeded {MAX_RESPONSE_BYTES} bytes");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .context("failed to read Meerkateer response")?
    {
        ensure!(
            body.len().saturating_add(chunk.len()) <= MAX_RESPONSE_BYTES,
            "Meerkateer response exceeded {MAX_RESPONSE_BYTES} bytes"
        );
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn http_error(operation: &str, status: StatusCode, body: &[u8]) -> anyhow::Error {
    let code = serde_json::from_slice::<ErrorResponse>(body).map_or_else(
        |_| "unexpected_response".to_owned(),
        |response| response.code,
    );
    anyhow::anyhow!("{operation} failed with HTTP {} ({code})", status.as_u16())
}

fn resolve_config_path(override_path: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(path);
    }
    if let Some(path) = env::var_os("MEERKATEER_AGENT_CONFIG") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(windows)]
    if let Some(base) = env::var_os("APPDATA") {
        return Ok(PathBuf::from(base).join("Meerkateer").join("agent.json"));
    }
    #[cfg(not(windows))]
    {
        if let Some(base) = env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(base).join("meerkateer").join("agent.json"));
        }
        if let Some(home) = env::var_os("HOME") {
            return Ok(PathBuf::from(home)
                .join(".config")
                .join("meerkateer")
                .join("agent.json"));
        }
    }
    bail!("could not determine the config directory; pass --config PATH")
}

fn load_config(path: &Path) -> anyhow::Result<AgentConfig> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let config: AgentConfig = serde_json::from_slice(&bytes)
        .with_context(|| format!("{} is not valid agent configuration", path.display()))?;
    ensure!(
        config.version == CONFIG_VERSION,
        "unsupported agent config version"
    );
    Ok(config)
}

fn save_config(path: &Path, config: &AgentConfig) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    secure_directory(parent)?;
    let bytes = serde_json::to_vec_pretty(config)?;
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use atomic_write_file::unix::OpenOptionsExt as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        options.preserve_mode(false).mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to securely open {}", path.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.write_all(b"\n")?;
    file.commit()
        .with_context(|| format!("failed to atomically save {}", path.display()))?;
    Ok(())
}

#[cfg(unix)]
fn secure_directory(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to secure {}", path.display()))
}

#[cfg(not(unix))]
fn secure_directory(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn test_config() -> AgentConfig {
        AgentConfig {
            version: CONFIG_VERSION,
            server_url: "http://127.0.0.1:6510/".to_owned(),
            installation_id: Uuid::new_v4(),
            display_name: "Test host".to_owned(),
            project_id: Some(Uuid::new_v4()),
            agent_id: Some(Uuid::new_v4()),
            credential_id: Some(Uuid::new_v4()),
            credential: Some("mka_agent_test_secret_material".to_owned()),
            credential_expires_at: Some("2099-01-01T00:00:00Z".to_owned()),
            next_sequence: 7,
            pending_batch: None,
        }
    }

    #[test]
    fn permits_only_https_or_loopback_http() {
        assert!(validate_server_url("https://ops.example.com").is_ok());
        assert!(validate_server_url("http://127.0.0.1:6510").is_ok());
        assert!(validate_server_url("http://[::1]:6510").is_ok());
        assert!(validate_server_url("http://ops.example.com").is_err());
        assert!(validate_server_url("https://user:pass@ops.example.com").is_err());
        assert!(validate_server_url("https://ops.example.com/path").is_err());
    }

    #[test]
    fn heartbeat_batch_uses_requested_sequence_and_safe_attributes() -> anyhow::Result<()> {
        let agent_id = Uuid::new_v4();
        let batch = build_heartbeat_batch(agent_id, 42)?;
        assert_eq!(batch.agent_id, agent_id);
        assert_eq!(batch.first_sequence, 42);
        assert_eq!(batch.last_sequence, 42);
        assert!(batch.has_valid_shape());
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn config_is_atomic_and_owner_only() -> anyhow::Result<()> {
        let directory = env::temp_dir().join(format!("meerkateer-agent-test-{}", Uuid::new_v4()));
        let path = directory.join("agent.json");
        let config = test_config();
        save_config(&path, &config)?;
        let loaded = load_config(&path)?;
        assert_eq!(loaded.agent_id, config.agent_id);
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(&directory)?.permissions().mode() & 0o777,
            0o700
        );
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn persisted_pending_batch_survives_reload() -> anyhow::Result<()> {
        let directory = env::temp_dir().join(format!("meerkateer-agent-test-{}", Uuid::new_v4()));
        let path = directory.join("agent.json");
        let mut config = test_config();
        config.pending_batch = Some(build_heartbeat_batch(
            config.agent_id.context("test agent missing")?,
            config.next_sequence,
        )?);
        let batch_id = config
            .pending_batch
            .as_ref()
            .context("test batch missing")?
            .batch_id;
        save_config(&path, &config)?;
        let loaded = load_config(&path)?;
        assert_eq!(
            loaded
                .pending_batch
                .context("pending batch missing")?
                .batch_id,
            batch_id
        );
        fs::remove_dir_all(directory)?;
        Ok(())
    }
}
