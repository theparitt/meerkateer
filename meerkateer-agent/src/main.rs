mod collector;
mod diagnostics;
mod tui;

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail, ensure};
use atomic_write_file::OpenOptions;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use meerkateer_protocol::mka1::{PROTOCOL_VERSION, TelemetryBatch, TelemetryRecord};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use tokio::time;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::collector::HostSnapshot;

const CONFIG_VERSION: u8 = 1;
const SIGNAL_BACKUP_VERSION: u8 = 1;
const RUNTIME_STATUS_VERSION: u8 = 1;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_RUNTIME_ERROR_CHARS: usize = 240;
const DEFAULT_INTERVAL_SECONDS: u64 = 30;
const MAX_SPOOL_BATCHES: usize = 128;
const MAX_SPOOL_BYTES: usize = 4 * 1024 * 1024;
const MAX_DRAIN_BATCHES_PER_CYCLE: usize = 16;
const CREDENTIAL_ROTATION_LEAD_DAYS: i64 = 7;

#[derive(Debug, Parser)]
#[command(version, about = "Outbound-only Meerkateer host Controller")]
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
        /// Control plane destination. Cloud is reserved but not open yet.
        #[arg(long, value_enum, default_value_t = ControlPlane::SelfHosted)]
        destination: ControlPlane,
        /// Meerkateer API base URL. HTTPS is required except on loopback.
        #[arg(long)]
        server: String,
        /// Machine name displayed in the Console.
        #[arg(long)]
        name: String,
    },
    /// Verify the local configuration without displaying credentials.
    Doctor,
    /// Replace the machine credential atomically without re-enrollment.
    RotateCredential,
    /// Test local storage, URL, DNS, proxy/VPN-sensitive routing, TLS, and API readiness.
    TestConnection {
        /// Meerkateer API base URL. HTTPS is required except on loopback.
        #[arg(long)]
        server: String,
        /// Return a non-zero exit code when any required check fails.
        #[arg(long)]
        strict: bool,
    },
    /// Choose which bounded host signals this controller sends.
    Configure {
        /// Comma-separated signals: cpu,memory,disk,process,service.
        #[arg(long, value_delimiter = ',', required = true)]
        signals: Vec<SignalKind>,
        /// Exact process name to monitor when the process signal is enabled.
        #[arg(long = "watch-process")]
        watch_processes: Vec<String>,
        /// Exact systemd unit or Windows Service name to monitor.
        #[arg(long = "watch-service")]
        watch_services: Vec<String>,
    },
    /// Restore the previous bounded signal selection without changing machine identity.
    RollbackConfig,
    /// Open the local interactive dashboard and setup terminal UI.
    Tui,
    /// Inspect this host locally without requiring enrollment or sending data.
    #[command(visible_alias = "status")]
    Inspect {
        /// Exact process name to count (repeatable, for example java or server.exe).
        #[arg(long = "watch-process")]
        watch_processes: Vec<String>,
        /// Exact systemd unit or Windows Service name to inspect.
        #[arg(long = "watch-service")]
        watch_services: Vec<String>,
    },
    /// Send bounded host and process telemetry until stopped.
    Run {
        /// Send one batch and exit.
        #[arg(long)]
        once: bool,
        /// Seconds between heartbeat batches.
        #[arg(long, default_value_t = DEFAULT_INTERVAL_SECONDS, value_parser = clap::value_parser!(u64).range(5..=3600))]
        interval_seconds: u64,
        /// Exact process name to count (repeatable, for example java or server.exe).
        #[arg(long = "watch-process")]
        watch_processes: Vec<String>,
        /// Exact systemd unit or Windows Service name to count (repeatable).
        #[arg(long = "watch-service")]
        watch_services: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
enum SignalKind {
    Cpu,
    Memory,
    Disk,
    Process,
    Service,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum ControlPlane {
    #[default]
    #[value(name = "self-hosted")]
    SelfHosted,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SignalSelection {
    enabled: BTreeSet<SignalKind>,
    #[serde(default)]
    watched_processes: Vec<String>,
    #[serde(default)]
    watched_services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignalBackup {
    version: u8,
    signals: SignalSelection,
}

impl Default for SignalSelection {
    fn default() -> Self {
        Self {
            enabled: BTreeSet::from([SignalKind::Cpu, SignalKind::Memory, SignalKind::Disk]),
            watched_processes: Vec::new(),
            watched_services: Vec::new(),
        }
    }
}

impl SignalSelection {
    fn from_cli(
        signals: &[SignalKind],
        watched_processes: Vec<String>,
        watched_services: Vec<String>,
    ) -> anyhow::Result<Self> {
        collector::validate_watched_processes(&watched_processes)?;
        collector::validate_watched_services(&watched_services)?;
        let enabled = signals.iter().copied().collect::<BTreeSet<_>>();
        ensure!(
            enabled.contains(&SignalKind::Process) || watched_processes.is_empty(),
            "--watch-process requires the process signal"
        );
        ensure!(
            enabled.contains(&SignalKind::Service) || watched_services.is_empty(),
            "--watch-service requires the service signal"
        );
        ensure!(
            !signals.is_empty(),
            "select at least one signal; the heartbeat is always sent"
        );
        Ok(Self {
            enabled,
            watched_processes,
            watched_services,
        })
    }

    fn names(&self) -> Vec<&'static str> {
        let mut names = vec!["heartbeat"];
        if self.enabled.contains(&SignalKind::Cpu) {
            names.push("cpu");
        }
        if self.enabled.contains(&SignalKind::Memory) {
            names.push("memory");
        }
        if self.enabled.contains(&SignalKind::Disk) {
            names.push("disk");
        }
        if self.enabled.contains(&SignalKind::Process) {
            names.push("process");
        }
        if self.enabled.contains(&SignalKind::Service) {
            names.push("service");
        }
        names
    }
}

#[derive(Serialize, Deserialize)]
struct AgentConfig {
    version: u8,
    #[serde(default)]
    destination: ControlPlane,
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
    #[serde(default)]
    signals: SignalSelection,
    next_sequence: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pending_batches: Vec<TelemetryBatch>,
    // Read one-batch 0.1/0.2 configs and migrate them into pending_batches.
    // New writes omit this compatibility field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending_batch: Option<TelemetryBatch>,
}

#[derive(Debug, Serialize)]
struct DoctorResult<'a> {
    status: &'a str,
    platform: &'static str,
    architecture: &'static str,
    config_path: String,
    destination: ControlPlane,
    server_url: &'a str,
    enrolled: bool,
    agent_id: Option<Uuid>,
    project_id: Option<Uuid>,
    credential_expires_at: Option<&'a str>,
    signals: Vec<&'static str>,
    watched_processes: &'a [String],
    watched_services: &'a [String],
    pending_batch: bool,
    spool_batches: usize,
    spool_bytes: usize,
    spool_capacity_batches: usize,
    spool_capacity_bytes: usize,
    next_sequence: u64,
    last_error_code: Option<&'a str>,
    last_error: Option<&'a str>,
    last_error_hint: Option<&'a str>,
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
struct RotationResult<'a> {
    status: &'static str,
    credential_id: Uuid,
    expires_at: &'a str,
    previous_valid_until: &'a str,
}

#[derive(Debug, Serialize)]
struct SendResult {
    status: &'static str,
    agent_id: Uuid,
    batch_id: Uuid,
    accepted_through_sequence: u64,
    delivered_batches: usize,
    spool_batches_remaining: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct RuntimeStatus {
    version: u8,
    last_attempt_at: Option<String>,
    last_success_at: Option<String>,
    last_error_at: Option<String>,
    last_error_code: Option<String>,
    last_error: Option<String>,
    last_error_hint: Option<String>,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            version: RUNTIME_STATUS_VERSION,
            last_attempt_at: None,
            last_success_at: None,
            last_error_at: None,
            last_error_code: None,
            last_error: None,
            last_error_hint: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct ConfigureResult<'a> {
    status: &'static str,
    config_path: String,
    signals: Vec<&'static str>,
    watched_processes: &'a [String],
    watched_services: &'a [String],
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
struct RotateCredentialResponse {
    credential_id: Uuid,
    secret: String,
    expires_at: String,
    previous_valid_until: String,
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
        Command::Enroll {
            destination,
            server,
            name,
        } => enroll(&config_path, destination, &server, &name).await,
        Command::Doctor => doctor(&config_path),
        Command::RotateCredential => rotate_credential(&config_path).await,
        Command::TestConnection { server, strict } => {
            let report = diagnostics::test_self_hosted_connection(&server, &config_path).await;
            println!("{}", serde_json::to_string_pretty(&report)?);
            ensure!(
                !strict || report.is_ok(),
                "connection test failed; inspect the JSON checks above"
            );
            Ok(())
        }
        Command::Configure {
            signals,
            watch_processes,
            watch_services,
        } => configure(&config_path, &signals, watch_processes, watch_services),
        Command::RollbackConfig => rollback_signal_selection(&config_path),
        Command::Tui => tui::run(&config_path).await,
        Command::Inspect {
            watch_processes,
            watch_services,
        } => inspect(&watch_processes, &watch_services).await,
        Command::Run {
            once,
            interval_seconds,
            watch_processes,
            watch_services,
        } => {
            run(
                &config_path,
                once,
                interval_seconds,
                &watch_processes,
                &watch_services,
            )
            .await
        }
    }
}

fn configure(
    config_path: &Path,
    signals: &[SignalKind],
    watched_processes: Vec<String>,
    watched_services: Vec<String>,
) -> anyhow::Result<()> {
    let selection = SignalSelection::from_cli(signals, watched_processes, watched_services)?;
    let selection = save_signal_selection(config_path, selection)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&ConfigureResult {
            status: "configured",
            config_path: config_path.display().to_string(),
            signals: selection.names(),
            watched_processes: &selection.watched_processes,
            watched_services: &selection.watched_services,
        })?
    );
    Ok(())
}

fn save_signal_selection(
    config_path: &Path,
    selection: SignalSelection,
) -> anyhow::Result<SignalSelection> {
    collector::validate_watched_processes(&selection.watched_processes)?;
    collector::validate_watched_services(&selection.watched_services)?;
    ensure!(
        selection.enabled.contains(&SignalKind::Process) || selection.watched_processes.is_empty(),
        "configured process names require the process signal"
    );
    ensure!(
        selection.enabled.contains(&SignalKind::Service) || selection.watched_services.is_empty(),
        "configured service names require the service signal"
    );
    ensure!(
        !selection.enabled.is_empty(),
        "select at least one signal; the heartbeat is always sent"
    );
    let mut config = load_config(config_path)?;
    if config.signals != selection {
        save_signal_backup(
            &signal_backup_path(config_path),
            &SignalBackup {
                version: SIGNAL_BACKUP_VERSION,
                signals: config.signals.clone(),
            },
        )?;
    }
    config.signals = selection.clone();
    save_config(config_path, &config)?;
    Ok(selection)
}

fn rollback_signal_selection(config_path: &Path) -> anyhow::Result<()> {
    let backup = load_signal_backup(&signal_backup_path(config_path))?;
    let selection = save_signal_selection(config_path, backup.signals)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&ConfigureResult {
            status: "rolled_back",
            config_path: config_path.display().to_string(),
            signals: selection.names(),
            watched_processes: &selection.watched_processes,
            watched_services: &selection.watched_services,
        })?
    );
    Ok(())
}

fn signal_backup_path(config_path: &Path) -> PathBuf {
    let stem = config_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("agent");
    config_path.with_file_name(format!("{stem}.signals.previous.json"))
}

fn load_signal_backup(path: &Path) -> anyhow::Result<SignalBackup> {
    let bytes = fs::read(path).with_context(|| {
        format!(
            "no previous signal configuration is available at {}",
            path.display()
        )
    })?;
    let backup: SignalBackup = serde_json::from_slice(&bytes)
        .with_context(|| format!("{} is not a valid signal backup", path.display()))?;
    ensure!(
        backup.version == SIGNAL_BACKUP_VERSION,
        "unsupported signal backup version"
    );
    collector::validate_watched_processes(&backup.signals.watched_processes)?;
    collector::validate_watched_services(&backup.signals.watched_services)?;
    Ok(backup)
}

fn save_signal_backup(path: &Path, backup: &SignalBackup) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    secure_directory(parent)?;
    let bytes = serde_json::to_vec_pretty(backup)?;
    #[cfg(unix)]
    let options = {
        use atomic_write_file::unix::OpenOptionsExt as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut options = OpenOptions::new();
        options.preserve_mode(false).mode(0o600);
        options
    };
    #[cfg(not(unix))]
    let options = OpenOptions::new();
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to securely open {}", path.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.write_all(b"\n")?;
    file.commit()
        .with_context(|| format!("failed to atomically save {}", path.display()))
}

async fn inspect(watched_processes: &[String], watched_services: &[String]) -> anyhow::Result<()> {
    let snapshot = collector::collect(watched_processes, watched_services).await?;
    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}

async fn enroll(
    config_path: &Path,
    destination: ControlPlane,
    server: &str,
    name: &str,
) -> anyhow::Result<()> {
    let token = Zeroizing::new(
        env::var("MEERKATEER_ENROLLMENT_TOKEN")
            .context("MEERKATEER_ENROLLMENT_TOKEN is required")?,
    );
    ensure!(
        !token.trim().is_empty(),
        "MEERKATEER_ENROLLMENT_TOKEN is empty"
    );
    let outcome = enroll_with_token(config_path, destination, server, name, token.as_str()).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&EnrollmentResult {
            status: "enrolled",
            agent_id: outcome.agent_id,
            project_id: outcome.project_id,
            credential_expires_at: &outcome.credential_expires_at,
            config_path: config_path.display().to_string(),
        })?
    );
    Ok(())
}

#[derive(Debug)]
struct EnrollmentOutcome {
    agent_id: Uuid,
    project_id: Option<Uuid>,
    credential_expires_at: String,
}

async fn enroll_with_token(
    config_path: &Path,
    destination: ControlPlane,
    server: &str,
    name: &str,
    token: &str,
) -> anyhow::Result<EnrollmentOutcome> {
    ensure!(
        destination == ControlPlane::SelfHosted,
        "Meerkateer Cloud is not open yet; choose self-hosted Community"
    );
    let server_url = validate_server_url(server)?;
    validate_display_name(name)?;
    ensure!(!token.trim().is_empty(), "enrollment token is empty");

    let mut config = match load_config(config_path) {
        Ok(existing) => {
            ensure!(
                existing.agent_id.is_none(),
                "this config is already enrolled; use a different --config path for another agent"
            );
            ensure!(
                existing.destination == destination
                    && existing.server_url == server_url.as_str()
                    && existing.display_name == name,
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
                destination,
                server_url: server_url.as_str().to_owned(),
                installation_id: Uuid::new_v4(),
                display_name: name.to_owned(),
                project_id: None,
                agent_id: None,
                credential_id: None,
                credential: None,
                credential_expires_at: None,
                signals: SignalSelection::default(),
                next_sequence: 1,
                pending_batches: Vec::new(),
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
    Ok(EnrollmentOutcome {
        agent_id: enrolled.agent_id,
        project_id: enrolled.project_id,
        credential_expires_at: expires_at.to_owned(),
    })
}

async fn rotate_credential(config_path: &Path) -> anyhow::Result<()> {
    let client = http_client()?;
    let mut config = load_config(config_path)?;
    validate_complete_config(&config)?;
    ensure_credential_valid(&config)?;
    let rotated = rotate_local_credential(&client, config_path, &mut config).await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&RotationResult {
            status: "rotated",
            credential_id: rotated.credential_id,
            expires_at: &rotated.expires_at,
            previous_valid_until: &rotated.previous_valid_until,
        })?
    );
    Ok(())
}

async fn rotate_local_credential(
    client: &Client,
    config_path: &Path,
    config: &mut AgentConfig,
) -> anyhow::Result<RotateCredentialResponse> {
    let server_url = validate_server_url(&config.server_url)?;
    let agent_id = config
        .agent_id
        .context("agent configuration is not enrolled")?;
    let credential = Zeroizing::new(
        config
            .credential
            .as_deref()
            .context("agent configuration is missing its credential")?
            .to_owned(),
    );
    let endpoint = server_url.join(&format!("v1/agents/{agent_id}/credentials/rotate"))?;
    let response = client
        .post(endpoint)
        .bearer_auth(credential.as_str())
        .send()
        .await
        .context("credential rotation request failed")?;
    let status = response.status();
    let body = read_bounded_body(response).await?;
    if status != StatusCode::CREATED {
        return Err(http_error("credential rotation", status, &body));
    }
    let rotated: RotateCredentialResponse = serde_json::from_slice(&body)
        .context("Meerkateer returned an invalid credential rotation response")?;
    ensure!(
        rotated.secret.starts_with("mka_agent_"),
        "Meerkateer returned an invalid replacement credential"
    );
    let expiry = DateTime::parse_from_rfc3339(&rotated.expires_at)
        .context("replacement credential expiry is invalid")?
        .with_timezone(&Utc);
    let overlap = DateTime::parse_from_rfc3339(&rotated.previous_valid_until)
        .context("previous credential overlap is invalid")?
        .with_timezone(&Utc);
    ensure!(
        expiry > Utc::now(),
        "replacement credential is already expired"
    );
    ensure!(
        overlap > Utc::now() && overlap < expiry,
        "credential overlap window is invalid"
    );
    config.credential_id = Some(rotated.credential_id);
    config.credential = Some(rotated.secret.clone());
    config.credential_expires_at = Some(rotated.expires_at.clone());
    save_config(config_path, config).context(
        "credential rotated remotely but could not be saved; retry within the overlap window",
    )?;
    Ok(rotated)
}

fn doctor(config_path: &Path) -> anyhow::Result<()> {
    let config = load_config(config_path)?;
    validate_complete_config(&config)?;
    let runtime = load_runtime_status(&runtime_status_path(config_path))?;
    let expires_at = config
        .credential_expires_at
        .as_deref()
        .context("agent configuration is missing credential expiry")?;
    let expiry = DateTime::parse_from_rfc3339(expires_at)
        .context("agent credential expiry is invalid")?
        .with_timezone(&Utc);
    let status = if expiry <= Utc::now() {
        "credential_expired"
    } else if !config.pending_batches.is_empty() {
        "pending_retry"
    } else {
        "ok"
    };
    let result = DoctorResult {
        status,
        platform: env::consts::OS,
        architecture: env::consts::ARCH,
        config_path: config_path.display().to_string(),
        destination: config.destination,
        server_url: &config.server_url,
        enrolled: true,
        agent_id: config.agent_id,
        project_id: config.project_id,
        credential_expires_at: Some(expires_at),
        signals: config.signals.names(),
        watched_processes: &config.signals.watched_processes,
        watched_services: &config.signals.watched_services,
        pending_batch: !config.pending_batches.is_empty(),
        spool_batches: config.pending_batches.len(),
        spool_bytes: spool_size_bytes(&config)?,
        spool_capacity_batches: MAX_SPOOL_BATCHES,
        spool_capacity_bytes: MAX_SPOOL_BYTES,
        next_sequence: config.next_sequence,
        last_error_code: runtime.last_error_code.as_deref(),
        last_error: runtime.last_error.as_deref(),
        last_error_hint: runtime.last_error_hint.as_deref(),
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    ensure!(
        expiry > Utc::now(),
        "agent credential has expired; re-enroll this machine"
    );
    Ok(())
}

async fn run(
    config_path: &Path,
    once: bool,
    interval_seconds: u64,
    watched_processes: &[String],
    watched_services: &[String],
) -> anyhow::Result<()> {
    collector::validate_watched_processes(watched_processes)?;
    collector::validate_watched_services(watched_services)?;
    let client = http_client()?;
    info!(
        config_path = %config_path.display(),
        watched_processes = watched_processes.len(),
        watched_services = watched_services.len(),
        "starting Meerkateer Controller"
    );

    loop {
        let result = send_cycle(
            &client,
            config_path,
            watched_processes,
            watched_services,
            !once,
        )
        .await;
        if let Err(error) = record_runtime_status(config_path, result.as_ref().err()) {
            warn!(error = %error, "could not persist local Controller runtime status");
        }
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

fn record_runtime_status(config_path: &Path, error: Option<&anyhow::Error>) -> anyhow::Result<()> {
    let status_path = runtime_status_path(config_path);
    let mut status = load_runtime_status(&status_path)?;
    let now = Utc::now().to_rfc3339();
    status.version = RUNTIME_STATUS_VERSION;
    status.last_attempt_at = Some(now.clone());
    if let Some(error) = error {
        let classified = diagnostics::classify_error(error);
        status.last_error_at = Some(now);
        status.last_error_code = Some(classified.code.to_owned());
        status.last_error = Some(sanitize_runtime_message(error, &classified.message));
        status.last_error_hint = Some(classified.hint.to_owned());
    } else {
        status.last_success_at = Some(now);
        status.last_error_at = None;
        status.last_error_code = None;
        status.last_error = None;
        status.last_error_hint = None;
    }
    save_runtime_status(&status_path, &status)
}

fn runtime_status_path(config_path: &Path) -> PathBuf {
    let stem = config_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("agent");
    config_path.with_file_name(format!("{stem}.status.json"))
}

fn load_runtime_status(path: &Path) -> anyhow::Result<RuntimeStatus> {
    match fs::read(path) {
        Ok(bytes) => {
            let status: RuntimeStatus = serde_json::from_slice(&bytes)
                .with_context(|| format!("{} is not valid runtime status", path.display()))?;
            ensure!(
                status.version == RUNTIME_STATUS_VERSION,
                "unsupported Controller runtime status version"
            );
            Ok(status)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(RuntimeStatus::default()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn save_runtime_status(path: &Path, status: &RuntimeStatus) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    secure_directory(parent)?;
    let bytes = serde_json::to_vec_pretty(status)?;
    #[cfg(unix)]
    let options = {
        use atomic_write_file::unix::OpenOptionsExt as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut options = OpenOptions::new();
        options.preserve_mode(false).mode(0o600);
        options
    };
    #[cfg(not(unix))]
    let options = OpenOptions::new();
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to securely open {}", path.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.write_all(b"\n")?;
    file.commit()
        .with_context(|| format!("failed to atomically save {}", path.display()))
}

fn sanitize_runtime_message(error: &anyhow::Error, classified_message: &str) -> String {
    let value = error.to_string();
    let normalized = value.to_ascii_lowercase();
    if [
        "mka_agent_",
        "mka_enroll_",
        "authorization",
        "password",
        "secret",
        "token",
    ]
    .iter()
    .any(|sensitive| normalized.contains(sensitive))
    {
        return "delivery failed; sensitive error detail was redacted".to_owned();
    }
    classified_message
        .chars()
        .take(MAX_RUNTIME_ERROR_CHARS)
        .collect()
}

async fn send_cycle(
    client: &Client,
    config_path: &Path,
    watched_process_overrides: &[String],
    watched_service_overrides: &[String],
    collect_while_backlogged: bool,
) -> anyhow::Result<SendResult> {
    let mut config = load_config(config_path)?;
    validate_complete_config(&config)?;
    ensure_credential_valid(&config)?;
    if credential_rotation_due(&config)?
        && let Err(error) = rotate_local_credential(client, config_path, &mut config).await
    {
        warn!(
            error = %error,
            "automatic credential rotation failed; continuing inside the valid credential window"
        );
    }
    let server_url = validate_server_url(&config.server_url)?;
    let agent_id = config
        .agent_id
        .context("agent configuration is not enrolled")?;
    let credential = Zeroizing::new(
        config
            .credential
            .as_deref()
            .context("agent configuration is missing its credential")?
            .to_owned(),
    );

    if config.pending_batches.is_empty() || collect_while_backlogged {
        let mut signals = config.signals.clone();
        if !watched_process_overrides.is_empty() {
            collector::validate_watched_processes(watched_process_overrides)?;
            signals.enabled.insert(SignalKind::Process);
            signals.watched_processes = watched_process_overrides.to_vec();
        }
        if !watched_service_overrides.is_empty() {
            collector::validate_watched_services(watched_service_overrides)?;
            signals.enabled.insert(SignalKind::Service);
            signals.watched_services = watched_service_overrides.to_vec();
        }
        if config.pending_batches.len() < MAX_SPOOL_BATCHES {
            let snapshot =
                collector::collect(&signals.watched_processes, &signals.watched_services).await?;
            let sequence = next_collection_sequence(&config)?;
            let batch = build_host_batch(agent_id, sequence, &snapshot, &signals)?;
            config.pending_batches.push(batch);
            if spool_size_bytes(&config)? > MAX_SPOOL_BYTES {
                config.pending_batches.pop();
            }
            save_config(config_path, &config)?;
        }
    }
    ensure!(
        !config.pending_batches.is_empty(),
        "offline spool is full and contains no deliverable batch"
    );

    drain_spool(
        client,
        config_path,
        &mut config,
        &server_url,
        agent_id,
        credential.as_str(),
    )
    .await
}

async fn drain_spool(
    client: &Client,
    config_path: &Path,
    config: &mut AgentConfig,
    server_url: &Url,
    agent_id: Uuid,
    credential: &str,
) -> anyhow::Result<SendResult> {
    let endpoint = server_url.join("v1/agent/telemetry")?;
    let mut last_result = None;
    for delivered in 0..MAX_DRAIN_BATCHES_PER_CYCLE {
        let Some(batch) = config.pending_batches.first().cloned() else {
            break;
        };
        ensure!(
            batch.agent_id == agent_id,
            "pending batch belongs to a different agent"
        );
        ensure!(
            batch.first_sequence == config.next_sequence,
            "pending batch sequence does not match local sequence state"
        );

        let response = client
            .post(endpoint.clone())
            .bearer_auth(credential)
            .header("Idempotency-Key", batch.batch_id.to_string())
            .json(&batch)
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
        config.next_sequence = batch
            .last_sequence
            .checked_add(1)
            .context("agent sequence exhausted")?;
        config.pending_batches.remove(0);
        save_config(config_path, config)?;
        last_result = Some(SendResult {
            status: "accepted",
            agent_id,
            batch_id: batch.batch_id,
            accepted_through_sequence: ack.accepted_through_sequence,
            delivered_batches: delivered + 1,
            spool_batches_remaining: config.pending_batches.len(),
        });
    }
    last_result.context("offline spool contained no deliverable batch")
}

fn next_collection_sequence(config: &AgentConfig) -> anyhow::Result<u64> {
    config
        .pending_batches
        .last()
        .map_or(Ok(config.next_sequence), |batch| {
            batch
                .last_sequence
                .checked_add(1)
                .context("agent sequence exhausted")
        })
}

fn spool_size_bytes(config: &AgentConfig) -> anyhow::Result<usize> {
    Ok(serde_json::to_vec(&config.pending_batches)?.len())
}

fn build_host_batch(
    agent_id: Uuid,
    sequence: u64,
    snapshot: &HostSnapshot,
    signals: &SignalSelection,
) -> anyhow::Result<TelemetryBatch> {
    ensure!(sequence > 0, "agent sequence must be positive");
    let attributes = BTreeMap::from([
        ("arch".to_owned(), env::consts::ARCH.to_owned()),
        ("os".to_owned(), env::consts::OS.to_owned()),
    ]);
    let mut metrics = vec![("agent.heartbeat", 1.0)];
    if signals.enabled.contains(&SignalKind::Cpu) {
        metrics.push(("host.cpu.utilization", snapshot.cpu_usage_percent / 100.0));
    }
    if signals.enabled.contains(&SignalKind::Memory) {
        metrics.extend([
            (
                "host.memory.used_bytes",
                bytes_as_metric(snapshot.memory_used_bytes),
            ),
            (
                "host.memory.total_bytes",
                bytes_as_metric(snapshot.memory_total_bytes),
            ),
        ]);
    }
    if signals.enabled.contains(&SignalKind::Disk) {
        metrics.extend([
            (
                "host.disk.used_bytes",
                bytes_as_metric(snapshot.disk_used_bytes),
            ),
            (
                "host.disk.total_bytes",
                bytes_as_metric(snapshot.disk_total_bytes),
            ),
        ]);
        if let (Some(used), Some(total)) = (snapshot.inode_used, snapshot.inode_total) {
            metrics.extend([
                ("host.disk.inodes_used", bytes_as_metric(used)),
                ("host.disk.inodes_total", bytes_as_metric(total)),
            ]);
        }
    }
    let process_capacity = if signals.enabled.contains(&SignalKind::Process) {
        snapshot.watched_processes.len()
    } else {
        0
    };
    let service_capacity = if signals.enabled.contains(&SignalKind::Service) {
        snapshot.watched_services.len()
    } else {
        0
    };
    let mut records = Vec::with_capacity(metrics.len() + process_capacity + service_capacity);
    for (offset, (metric, value)) in metrics.into_iter().enumerate() {
        let record_sequence = sequence
            .checked_add(u64::try_from(offset)?)
            .context("agent sequence exhausted")?;
        records.push(TelemetryRecord::Sample {
            sequence: record_sequence,
            record_id: Uuid::new_v4(),
            observed_at: snapshot.collected_at.clone(),
            metric: metric.to_owned(),
            value,
            attributes: attributes.clone(),
        });
    }
    if signals.enabled.contains(&SignalKind::Process) {
        append_process_records(&mut records, sequence, snapshot)?;
    }
    if signals.enabled.contains(&SignalKind::Service) {
        append_service_records(&mut records, sequence, snapshot)?;
    }
    let last_sequence = records
        .last()
        .map(TelemetryRecord::sequence)
        .context("host collector produced no records")?;
    let batch = TelemetryBatch {
        protocol_version: PROTOCOL_VERSION.to_owned(),
        agent_id,
        batch_id: Uuid::new_v4(),
        first_sequence: sequence,
        last_sequence,
        sent_at: snapshot.collected_at.clone(),
        records,
    };
    ensure!(
        batch.has_valid_shape(),
        "agent produced an invalid telemetry batch"
    );
    Ok(batch)
}

fn append_process_records(
    records: &mut Vec<TelemetryRecord>,
    sequence: u64,
    snapshot: &HostSnapshot,
) -> anyhow::Result<()> {
    for process in &snapshot.watched_processes {
        let record_sequence = sequence
            .checked_add(u64::try_from(records.len())?)
            .context("agent sequence exhausted")?;
        records.push(TelemetryRecord::Sample {
            sequence: record_sequence,
            record_id: Uuid::new_v4(),
            observed_at: snapshot.collected_at.clone(),
            metric: "process.running".to_owned(),
            value: f64::from(process.instances),
            attributes: BTreeMap::from([("process".to_owned(), process.name.clone())]),
        });
    }
    Ok(())
}

fn append_service_records(
    records: &mut Vec<TelemetryRecord>,
    sequence: u64,
    snapshot: &HostSnapshot,
) -> anyhow::Result<()> {
    for service in &snapshot.watched_services {
        let record_sequence = sequence
            .checked_add(u64::try_from(records.len())?)
            .context("agent sequence exhausted")?;
        records.push(TelemetryRecord::Sample {
            sequence: record_sequence,
            record_id: Uuid::new_v4(),
            observed_at: snapshot.collected_at.clone(),
            metric: "os.service.running".to_owned(),
            value: if service.running == Some(true) {
                1.0
            } else {
                0.0
            },
            attributes: BTreeMap::from([
                ("service".to_owned(), service.name.clone()),
                ("state".to_owned(), service.state.clone()),
                ("known".to_owned(), service.running.is_some().to_string()),
            ]),
        });
    }
    Ok(())
}

fn bytes_as_metric(value: u64) -> f64 {
    // MKS/MKA sample values are f64. Integer-byte precision remains exact through
    // 8 PiB, well above the supported single-host capacity envelope.
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

fn validate_complete_config(config: &AgentConfig) -> anyhow::Result<()> {
    ensure!(
        config.version == CONFIG_VERSION,
        "unsupported agent config version"
    );
    ensure!(
        config.destination == ControlPlane::SelfHosted,
        "Meerkateer Cloud is not open yet; this Controller must use self-hosted Community"
    );
    ensure!(config.next_sequence > 0, "agent sequence state is invalid");
    ensure!(
        config.pending_batch.is_none(),
        "legacy pending batch was not migrated into the offline spool"
    );
    ensure!(
        config.pending_batches.len() <= MAX_SPOOL_BATCHES,
        "offline spool exceeds its batch ceiling"
    );
    ensure!(
        spool_size_bytes(config)? <= MAX_SPOOL_BYTES,
        "offline spool exceeds its disk ceiling"
    );
    collector::validate_watched_processes(&config.signals.watched_processes)?;
    collector::validate_watched_services(&config.signals.watched_services)?;
    ensure!(
        config.signals.enabled.contains(&SignalKind::Process)
            || config.signals.watched_processes.is_empty(),
        "configured process names require the process signal"
    );
    ensure!(
        config.signals.enabled.contains(&SignalKind::Service)
            || config.signals.watched_services.is_empty(),
        "configured service names require the service signal"
    );
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
    let agent_id = config.agent_id.context("agent enrollment is incomplete")?;
    let mut expected_sequence = config.next_sequence;
    for batch in &config.pending_batches {
        ensure!(
            batch.agent_id == agent_id,
            "offline spool contains another agent's batch"
        );
        ensure!(
            batch.has_valid_shape(),
            "offline spool contains an invalid batch"
        );
        ensure!(
            batch.first_sequence == expected_sequence,
            "offline spool sequence is not contiguous"
        );
        expected_sequence = batch
            .last_sequence
            .checked_add(1)
            .context("agent sequence exhausted")?;
    }
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

fn credential_rotation_due(config: &AgentConfig) -> anyhow::Result<bool> {
    let expires_at = config
        .credential_expires_at
        .as_deref()
        .context("agent credential expiry is missing")?;
    let expiry = DateTime::parse_from_rfc3339(expires_at)
        .context("agent credential expiry is invalid")?
        .with_timezone(&Utc);
    Ok(expiry <= Utc::now() + ChronoDuration::days(CREDENTIAL_ROTATION_LEAD_DAYS))
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
        return validate_config_path(path);
    }
    if let Some(path) = env::var_os("MEERKATEER_AGENT_CONFIG") {
        return validate_config_path(PathBuf::from(path));
    }
    #[cfg(windows)]
    if let Some(base) = env::var_os("APPDATA") {
        return validate_config_path(PathBuf::from(base).join("Meerkateer").join("agent.json"));
    }
    #[cfg(not(windows))]
    {
        if let Some(base) = env::var_os("XDG_CONFIG_HOME") {
            return validate_config_path(PathBuf::from(base).join("meerkateer").join("agent.json"));
        }
        if let Some(home) = env::var_os("HOME") {
            return validate_config_path(
                PathBuf::from(home)
                    .join(".config")
                    .join("meerkateer")
                    .join("agent.json"),
            );
        }
    }
    bail!("could not determine the config directory; pass --config PATH")
}

fn validate_config_path(path: PathBuf) -> anyhow::Result<PathBuf> {
    ensure!(
        !path.as_os_str().is_empty(),
        "Controller config path is empty; fix MEERKATEER_AGENT_CONFIG or pass --config PATH"
    );
    ensure!(
        !path.is_dir(),
        "Controller config path points to a directory; use a file such as agent.json"
    );
    Ok(path)
}

fn load_config(path: &Path) -> anyhow::Result<AgentConfig> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut config: AgentConfig = serde_json::from_slice(&bytes)
        .with_context(|| format!("{} is not valid agent configuration", path.display()))?;
    ensure!(
        config.version == CONFIG_VERSION,
        "unsupported agent config version"
    );
    if let Some(legacy) = config.pending_batch.take()
        && config
            .pending_batches
            .first()
            .is_none_or(|current| current.batch_id != legacy.batch_id)
    {
        config.pending_batches.insert(0, legacy);
    }
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
    #[cfg(unix)]
    let options = {
        use atomic_write_file::unix::OpenOptionsExt as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut options = OpenOptions::new();
        options.preserve_mode(false).mode(0o600);
        options
    };
    #[cfg(not(unix))]
    let options = OpenOptions::new();
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

    fn test_snapshot(watched_processes: Vec<collector::ProcessObservation>) -> HostSnapshot {
        HostSnapshot {
            collected_at: "2026-09-30T00:00:00Z".to_owned(),
            platform: "linux",
            architecture: "x86_64",
            cpu_usage_percent: 25.0,
            memory_total_bytes: 16_000,
            memory_used_bytes: 8_000,
            disk_total_bytes: 100_000,
            disk_used_bytes: 40_000,
            disk_count: 1,
            inode_total: Some(10_000),
            inode_used: Some(4_000),
            watched_processes,
            watched_services: Vec::new(),
        }
    }

    fn test_config() -> AgentConfig {
        AgentConfig {
            version: CONFIG_VERSION,
            destination: ControlPlane::SelfHosted,
            server_url: "http://127.0.0.1:6510/".to_owned(),
            installation_id: Uuid::new_v4(),
            display_name: "Test host".to_owned(),
            project_id: Some(Uuid::new_v4()),
            agent_id: Some(Uuid::new_v4()),
            credential_id: Some(Uuid::new_v4()),
            credential: Some("mka_agent_test_secret_material".to_owned()),
            credential_expires_at: Some("2099-01-01T00:00:00Z".to_owned()),
            signals: SignalSelection::default(),
            next_sequence: 7,
            pending_batches: Vec::new(),
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
    fn rejects_empty_or_directory_config_paths() {
        assert!(validate_config_path(PathBuf::new()).is_err());
        assert!(validate_config_path(env::temp_dir()).is_err());
        assert!(validate_config_path(env::temp_dir().join("agent.json")).is_ok());
    }

    #[test]
    fn host_batch_uses_contiguous_sequences_and_safe_metrics() -> anyhow::Result<()> {
        let agent_id = Uuid::new_v4();
        let mut snapshot = test_snapshot(vec![collector::ProcessObservation {
            name: "java".to_owned(),
            running: true,
            instances: 2,
        }]);
        snapshot
            .watched_services
            .push(collector::ServiceObservation {
                name: "minecraft.service".to_owned(),
                running: Some(false),
                state: "failed".to_owned(),
            });
        let signals = SignalSelection {
            enabled: BTreeSet::from([
                SignalKind::Cpu,
                SignalKind::Memory,
                SignalKind::Disk,
                SignalKind::Process,
                SignalKind::Service,
            ]),
            watched_processes: vec!["java".to_owned()],
            watched_services: vec!["minecraft.service".to_owned()],
        };
        let batch = build_host_batch(agent_id, 42, &snapshot, &signals)?;
        assert_eq!(batch.agent_id, agent_id);
        assert_eq!(batch.first_sequence, 42);
        assert_eq!(batch.last_sequence, 51);
        assert_eq!(batch.records.len(), 10);
        assert!(batch.has_valid_shape());
        assert!(batch.records.iter().any(|record| matches!(
            record,
            TelemetryRecord::Sample { metric, value, .. }
                if metric == "process.running" && (*value - 2.0).abs() < f64::EPSILON
        )));
        assert!(batch.records.iter().any(|record| matches!(
            record,
            TelemetryRecord::Sample { metric, value, attributes, .. }
                if metric == "os.service.running"
                    && (*value).abs() < f64::EPSILON
                    && attributes.get("state").is_some_and(|state| state == "failed")
        )));
        Ok(())
    }

    #[test]
    fn signal_selection_limits_the_emitted_metrics() -> anyhow::Result<()> {
        let snapshot = test_snapshot(vec![collector::ProcessObservation {
            name: "java".to_owned(),
            running: true,
            instances: 1,
        }]);
        let signals = SignalSelection {
            enabled: BTreeSet::from([SignalKind::Memory]),
            watched_processes: Vec::new(),
            watched_services: Vec::new(),
        };
        let batch = build_host_batch(Uuid::new_v4(), 1, &snapshot, &signals)?;
        let metrics = batch
            .records
            .iter()
            .map(|record| match record {
                TelemetryRecord::Sample { metric, .. } => metric.as_str(),
                TelemetryRecord::Event { .. } => "event",
            })
            .collect::<Vec<_>>();
        assert_eq!(
            metrics,
            vec![
                "agent.heartbeat",
                "host.memory.used_bytes",
                "host.memory.total_bytes"
            ]
        );
        Ok(())
    }

    #[test]
    fn existing_configs_receive_safe_default_signals() -> anyhow::Result<()> {
        let config: AgentConfig = serde_json::from_value(serde_json::json!({
            "version": CONFIG_VERSION,
            "server_url": "http://127.0.0.1:6510/",
            "installation_id": Uuid::new_v4(),
            "display_name": "Existing host",
            "agent_id": Uuid::new_v4(),
            "credential_id": Uuid::new_v4(),
            "credential": "mka_agent_test_secret_material",
            "credential_expires_at": "2099-01-01T00:00:00Z",
            "next_sequence": 1
        }))?;
        assert_eq!(
            config.signals.names(),
            vec!["heartbeat", "cpu", "memory", "disk"]
        );
        assert_eq!(config.destination, ControlPlane::SelfHosted);
        assert!(config.signals.watched_processes.is_empty());
        Ok(())
    }

    #[test]
    fn credential_rotation_starts_before_expiry_without_waiting_until_failure() -> anyhow::Result<()>
    {
        let mut config = test_config();
        config.credential_expires_at = Some((Utc::now() + ChronoDuration::days(6)).to_rfc3339());
        assert!(credential_rotation_due(&config)?);
        config.credential_expires_at = Some((Utc::now() + ChronoDuration::days(8)).to_rfc3339());
        assert!(!credential_rotation_due(&config)?);
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
    fn legacy_pending_batch_migrates_into_bounded_spool() -> anyhow::Result<()> {
        let directory = env::temp_dir().join(format!("meerkateer-agent-test-{}", Uuid::new_v4()));
        let path = directory.join("agent.json");
        let mut config = test_config();
        config.pending_batch = Some(build_host_batch(
            config.agent_id.context("test agent missing")?,
            config.next_sequence,
            &test_snapshot(Vec::new()),
            &config.signals,
        )?);
        let batch_id = config
            .pending_batch
            .as_ref()
            .context("test batch missing")?
            .batch_id;
        save_config(&path, &config)?;
        let loaded = load_config(&path)?;
        assert!(loaded.pending_batch.is_none());
        assert_eq!(loaded.pending_batches.len(), 1);
        assert_eq!(loaded.pending_batches[0].batch_id, batch_id);
        validate_complete_config(&loaded)?;
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn offline_spool_preserves_order_and_survives_reload() -> anyhow::Result<()> {
        let directory = env::temp_dir().join(format!("meerkateer-spool-test-{}", Uuid::new_v4()));
        let path = directory.join("agent.json");
        let mut config = test_config();
        for _ in 0..3 {
            let sequence = next_collection_sequence(&config)?;
            config.pending_batches.push(build_host_batch(
                config.agent_id.context("test agent missing")?,
                sequence,
                &test_snapshot(Vec::new()),
                &config.signals,
            )?);
        }
        save_config(&path, &config)?;
        let loaded = load_config(&path)?;
        validate_complete_config(&loaded)?;
        assert_eq!(loaded.pending_batches.len(), 3);
        assert_eq!(loaded.pending_batches[0].first_sequence, 7);
        assert_eq!(
            loaded.pending_batches[1].first_sequence,
            loaded.pending_batches[0].last_sequence + 1
        );
        assert_eq!(
            loaded.pending_batches[2].first_sequence,
            loaded.pending_batches[1].last_sequence + 1
        );
        assert!(spool_size_bytes(&loaded)? <= MAX_SPOOL_BYTES);
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn offline_spool_holds_thirty_minutes_at_default_interval() -> anyhow::Result<()> {
        let mut config = test_config();
        let expected_batches = usize::try_from(30 * 60 / DEFAULT_INTERVAL_SECONDS)?;
        assert!(expected_batches < MAX_SPOOL_BATCHES);
        for _ in 0..expected_batches {
            let sequence = next_collection_sequence(&config)?;
            config.pending_batches.push(build_host_batch(
                config.agent_id.context("test agent missing")?,
                sequence,
                &test_snapshot(Vec::new()),
                &config.signals,
            )?);
        }
        validate_complete_config(&config)?;
        assert_eq!(config.pending_batches.len(), expected_batches);
        assert!(spool_size_bytes(&config)? < MAX_SPOOL_BYTES);
        assert_eq!(
            config
                .pending_batches
                .last()
                .context("spool missing")?
                .last_sequence
                + 1,
            next_collection_sequence(&config)?
        );
        Ok(())
    }

    #[test]
    fn signal_configuration_rolls_back_without_changing_identity_or_spool() -> anyhow::Result<()> {
        let directory =
            env::temp_dir().join(format!("meerkateer-rollback-test-{}", Uuid::new_v4()));
        let path = directory.join("agent.json");
        let config = test_config();
        let agent_id = config.agent_id;
        let credential_id = config.credential_id;
        save_config(&path, &config)?;
        save_signal_selection(
            &path,
            SignalSelection {
                enabled: BTreeSet::from([SignalKind::Cpu, SignalKind::Process]),
                watched_processes: vec!["java".to_owned()],
                watched_services: Vec::new(),
            },
        )?;
        rollback_signal_selection(&path)?;
        let restored = load_config(&path)?;
        assert_eq!(restored.signals, SignalSelection::default());
        assert_eq!(restored.agent_id, agent_id);
        assert_eq!(restored.credential_id, credential_id);
        assert_eq!(restored.next_sequence, 7);
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(signal_backup_path(&path))?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn runtime_status_is_separate_atomic_and_owner_only() -> anyhow::Result<()> {
        let directory = env::temp_dir().join(format!("meerkateer-status-test-{}", Uuid::new_v4()));
        let config_path = directory.join("agent.json");
        record_runtime_status(&config_path, None)?;
        let status_path = runtime_status_path(&config_path);
        let status = load_runtime_status(&status_path)?;
        assert!(status.last_attempt_at.is_some());
        assert!(status.last_success_at.is_some());
        assert!(status.last_error.is_none());
        assert_eq!(
            fs::metadata(&status_path)?.permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn runtime_errors_are_bounded_and_secret_safe() {
        let long = "network unavailable ".repeat(40);
        assert!(
            sanitize_runtime_message(&anyhow::anyhow!(long), &"network unavailable ".repeat(40))
                .chars()
                .count()
                <= 240
        );
        assert_eq!(
            sanitize_runtime_message(
                &anyhow::anyhow!("bad token mka_agent_do_not_show"),
                "should not appear"
            ),
            "delivery failed; sensitive error detail was redacted"
        );
    }
}
