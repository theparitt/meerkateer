//! Bounded, idempotent MKS-1 client for Rust services and game servers.

use std::{env, fmt, net::IpAddr, time::Duration};

use chrono::{SecondsFormat, Utc};
use meerkateer_protocol::mks1::{DeployIngest, EventIngest, HeartbeatIngest, INTERFACE_VERSION};
pub use meerkateer_protocol::mks1::{DeployStatus, EventLevel, HeartbeatStatus};
use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::sleep;
use uuid::Uuid;

const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_RETRIES: usize = 2;

/// A safe SDK error. Credential material is never included in its display text.
#[derive(Debug, Error)]
pub enum SdkError {
    #[error("invalid Meerkateer configuration: {0}")]
    InvalidConfiguration(&'static str),
    #[error("telemetry transport failed")]
    Transport(#[source] reqwest::Error),
    #[error("Meerkateer rejected telemetry with HTTP {status} ({code})")]
    Rejected { status: u16, code: String },
    #[error("Meerkateer returned an invalid acknowledgement: {0}")]
    InvalidAcknowledgement(&'static str),
    #[error("failed to encode telemetry")]
    Encoding(#[source] serde_json::Error),
}

/// A durable acknowledgement returned for a newly accepted or exactly replayed fact.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Acknowledgement {
    pub status: String,
    pub idempotency_key: Uuid,
    pub received_at: String,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    code: String,
}

/// Async client bound to one process identity in one workspace.
pub struct Meerkateer {
    http: Client,
    base_url: Url,
    service_key: SecretString,
    project: String,
    service: String,
    environment: String,
}

impl fmt::Debug for Meerkateer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Meerkateer")
            .field("http", &self.http)
            .field("base_url", &self.base_url)
            .field("service_key", &"[REDACTED]")
            .field("project", &self.project)
            .field("service", &self.service)
            .field("environment", &self.environment)
            .finish()
    }
}

impl Meerkateer {
    /// Create a client. HTTPS is required except for loopback development URLs.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError`] when the URL, identity, environment, key, or HTTP client
    /// configuration is invalid.
    pub fn new(
        url: &str,
        service_key: impl Into<String>,
        project: impl Into<String>,
        service: impl Into<String>,
        environment: impl Into<String>,
    ) -> Result<Self, SdkError> {
        let base_url = validate_url(url)?;
        let service_key = service_key.into();
        if !service_key.starts_with("mks_sk_") {
            return Err(SdkError::InvalidConfiguration(
                "service key has an unsupported shape",
            ));
        }
        let project = project.into();
        let service = service.into();
        let environment = environment.into();
        if project.is_empty() || service.is_empty() {
            return Err(SdkError::InvalidConfiguration(
                "project and service are required",
            ));
        }
        if !matches!(
            environment.as_str(),
            "development" | "staging" | "production" | "test" | "local"
        ) {
            return Err(SdkError::InvalidConfiguration("environment is unsupported"));
        }
        let http = Client::builder()
            .tls_backend_rustls()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .user_agent(concat!("meerkateer-rust-sdk/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(SdkError::Transport)?;
        Ok(Self {
            http,
            base_url,
            service_key: SecretString::from(service_key),
            project,
            service,
            environment,
        })
    }

    /// Construct a client from the environment block shown by the Console.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError`] when a required environment variable is missing or invalid.
    pub fn from_env() -> Result<Self, SdkError> {
        Self::new(
            &required_env("MEERKATEER_URL")?,
            required_env("MEERKATEER_SERVICE_KEY")?,
            required_env("MEERKATEER_PROJECT")?,
            required_env("MEERKATEER_SERVICE")?,
            env::var("MEERKATEER_ENVIRONMENT").unwrap_or_else(|_| "production".to_owned()),
        )
    }

    /// Report current process health with an automatically generated idempotency key.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError`] when the payload is invalid, delivery fails, or the server
    /// rejects or cannot acknowledge the fact.
    pub async fn heartbeat(
        &self,
        status: HeartbeatStatus,
        message: Option<String>,
    ) -> Result<Acknowledgement, SdkError> {
        self.heartbeat_with_idempotency(status, message, Uuid::new_v4())
            .await
    }

    /// Report current process health using a caller-owned idempotency key.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError`] when the payload is invalid, delivery fails, or the server
    /// rejects or cannot acknowledge the fact.
    pub async fn heartbeat_with_idempotency(
        &self,
        status: HeartbeatStatus,
        message: Option<String>,
        idempotency_key: Uuid,
    ) -> Result<Acknowledgement, SdkError> {
        let payload = HeartbeatIngest {
            interface_version: INTERFACE_VERSION.to_owned(),
            service: self.service.clone(),
            project: self.project.clone(),
            environment: self.environment.clone(),
            status,
            message,
            timestamp: now(),
        };
        if !payload.has_valid_shape() {
            return Err(SdkError::InvalidConfiguration(
                "heartbeat payload is unsafe or invalid",
            ));
        }
        self.send("v1/ingest/heartbeat", &payload, idempotency_key)
            .await
    }

    /// Report a bounded operational event.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError`] when the payload is invalid, delivery fails, or the server
    /// rejects or cannot acknowledge the fact.
    pub async fn event(
        &self,
        kind: impl Into<String>,
        level: EventLevel,
        message: Option<String>,
        count: u32,
    ) -> Result<Acknowledgement, SdkError> {
        self.event_with_idempotency(kind, level, message, count, Uuid::new_v4())
            .await
    }

    /// Report an operational event using a caller-owned idempotency key.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError`] when the payload is invalid, delivery fails, or the server
    /// rejects or cannot acknowledge the fact.
    pub async fn event_with_idempotency(
        &self,
        kind: impl Into<String>,
        level: EventLevel,
        message: Option<String>,
        count: u32,
        idempotency_key: Uuid,
    ) -> Result<Acknowledgement, SdkError> {
        let payload = EventIngest {
            interface_version: INTERFACE_VERSION.to_owned(),
            service: self.service.clone(),
            project: self.project.clone(),
            environment: self.environment.clone(),
            level,
            kind: kind.into(),
            message,
            count,
            timestamp: now(),
        };
        if !payload.has_valid_shape() {
            return Err(SdkError::InvalidConfiguration(
                "event payload is unsafe or invalid",
            ));
        }
        self.send("v1/ingest/event", &payload, idempotency_key)
            .await
    }

    /// Report a deployment transition.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError`] when the payload is invalid, delivery fails, or the server
    /// rejects or cannot acknowledge the fact.
    pub async fn deploy(
        &self,
        version: impl Into<String>,
        commit: impl Into<String>,
        status: DeployStatus,
    ) -> Result<Acknowledgement, SdkError> {
        self.deploy_with_idempotency(version, commit, status, Uuid::new_v4())
            .await
    }

    /// Report a deployment transition using a caller-owned idempotency key.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError`] when the payload is invalid, delivery fails, or the server
    /// rejects or cannot acknowledge the fact.
    pub async fn deploy_with_idempotency(
        &self,
        version: impl Into<String>,
        commit: impl Into<String>,
        status: DeployStatus,
        idempotency_key: Uuid,
    ) -> Result<Acknowledgement, SdkError> {
        let payload = DeployIngest {
            interface_version: INTERFACE_VERSION.to_owned(),
            service: self.service.clone(),
            project: self.project.clone(),
            environment: self.environment.clone(),
            version: version.into(),
            commit: commit.into(),
            status,
            timestamp: now(),
        };
        if !payload.has_valid_shape() {
            return Err(SdkError::InvalidConfiguration(
                "deployment payload is unsafe or invalid",
            ));
        }
        self.send("v1/ingest/deploy", &payload, idempotency_key)
            .await
    }

    async fn send(
        &self,
        path: &str,
        payload: &impl Serialize,
        idempotency_key: Uuid,
    ) -> Result<Acknowledgement, SdkError> {
        let endpoint = self
            .base_url
            .join(path)
            .map_err(|_| SdkError::InvalidConfiguration("endpoint URL is invalid"))?;
        let body = serde_json::to_vec(payload).map_err(SdkError::Encoding)?;
        let mut last_failure = None;
        for attempt in 0..=MAX_RETRIES {
            let response = self
                .http
                .post(endpoint.clone())
                .bearer_auth(self.service_key.expose_secret())
                .header("Idempotency-Key", idempotency_key.to_string())
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone())
                .send()
                .await;
            match response {
                Ok(response) => {
                    let status = response.status();
                    let response_body = read_bounded(response).await?;
                    if matches!(status, StatusCode::OK | StatusCode::ACCEPTED) {
                        let acknowledgement: Acknowledgement =
                            serde_json::from_slice(&response_body).map_err(|_| {
                                SdkError::InvalidAcknowledgement("response was not valid JSON")
                            })?;
                        if acknowledgement.idempotency_key != idempotency_key
                            || !matches!(acknowledgement.status.as_str(), "accepted" | "duplicate")
                        {
                            return Err(SdkError::InvalidAcknowledgement(
                                "identity or status did not match the request",
                            ));
                        }
                        return Ok(acknowledgement);
                    }
                    let failure = rejected(status, &response_body);
                    if !retryable_status(status) || attempt == MAX_RETRIES {
                        return Err(failure);
                    }
                    last_failure = Some(failure);
                }
                Err(error) => {
                    let failure = SdkError::Transport(error);
                    if attempt == MAX_RETRIES {
                        return Err(failure);
                    }
                    last_failure = Some(failure);
                }
            }
            sleep(Duration::from_millis(250 * (1 << attempt))).await;
        }
        Err(last_failure.unwrap_or(SdkError::InvalidAcknowledgement(
            "retry loop ended unexpectedly",
        )))
    }
}

fn validate_url(value: &str) -> Result<Url, SdkError> {
    let mut url =
        Url::parse(value).map_err(|_| SdkError::InvalidConfiguration("URL must be absolute"))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(SdkError::InvalidConfiguration(
            "URL must not contain credentials, query, or fragment",
        ));
    }
    if !matches!(url.path(), "" | "/") {
        return Err(SdkError::InvalidConfiguration(
            "URL must not contain a path",
        ));
    }
    let host = url
        .host_str()
        .ok_or(SdkError::InvalidConfiguration("URL must include a host"))?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(SdkError::InvalidConfiguration(
            "HTTPS is required unless URL is loopback",
        ));
    }
    url.set_path("/");
    Ok(url)
}

async fn read_bounded(mut response: reqwest::Response) -> Result<Vec<u8>, SdkError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(SdkError::InvalidAcknowledgement(
            "response exceeded the size limit",
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(SdkError::Transport)? {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(SdkError::InvalidAcknowledgement(
                "response exceeded the size limit",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn rejected(status: StatusCode, body: &[u8]) -> SdkError {
    let code = serde_json::from_slice::<ErrorResponse>(body)
        .map_or_else(|_| "unexpected_response".to_owned(), |value| value.code);
    SdkError::Rejected {
        status: status.as_u16(),
        code,
    }
}

fn retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn required_env(name: &'static str) -> Result<String, SdkError> {
    env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or(SdkError::InvalidConfiguration(name))
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::{HeartbeatStatus, Meerkateer, SdkError};

    fn client() -> Meerkateer {
        Meerkateer::new(
            "http://127.0.0.1:6510",
            "mks_sk_test_credential",
            "arena",
            "game-server-01",
            "production",
        )
        .unwrap_or_else(|_| std::process::abort())
    }

    #[test]
    fn permits_https_and_loopback_http_only() {
        assert!(
            Meerkateer::new(
                "https://ops.example.com",
                "mks_sk_test",
                "a",
                "b",
                "production"
            )
            .is_ok()
        );
        assert!(
            Meerkateer::new(
                "http://localhost:6510",
                "mks_sk_test",
                "a",
                "b",
                "production"
            )
            .is_ok()
        );
        assert!(
            Meerkateer::new(
                "http://ops.example.com",
                "mks_sk_test",
                "a",
                "b",
                "production"
            )
            .is_err()
        );
        assert!(
            Meerkateer::new(
                "https://user:pass@ops.example.com",
                "mks_sk_test",
                "a",
                "b",
                "production"
            )
            .is_err()
        );
    }

    #[test]
    fn debug_output_redacts_service_key() {
        let output = format!("{:?}", client());
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("mks_sk_test_credential"));
    }

    #[tokio::test]
    async fn rejects_sensitive_payload_before_network() {
        let error = client()
            .heartbeat(
                HeartbeatStatus::Down,
                Some("password=mks_sk_secret".to_owned()),
            )
            .await;
        assert!(matches!(error, Err(SdkError::InvalidConfiguration(_))));
    }
}
