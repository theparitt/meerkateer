use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const INTERFACE: &str = "meerkateer";
pub const INTERFACE_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildInfo {
    pub built_at_utc: String,
    pub git_sha: String,
    pub git_branch: String,
}

impl BuildInfo {
    #[must_use]
    pub fn current() -> Self {
        Self {
            built_at_utc: option_env!("MEERKATEER_BUILD_TIME_UTC")
                .unwrap_or("unknown")
                .to_owned(),
            git_sha: option_env!("MEERKATEER_GIT_SHA")
                .unwrap_or("unknown")
                .to_owned(),
            git_branch: option_env!("MEERKATEER_GIT_BRANCH")
                .unwrap_or("unknown")
                .to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SafeCheckError {
    Unavailable,
    Timeout,
    Misconfigured,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<SafeCheckError>,
}

impl CheckResult {
    #[must_use]
    pub const fn ok() -> Self {
        Self {
            ok: true,
            latency_ms: None,
            error: None,
        }
    }

    #[must_use]
    pub const fn ok_with_latency(latency_ms: u64) -> Self {
        Self {
            ok: true,
            latency_ms: Some(latency_ms),
            error: None,
        }
    }

    #[must_use]
    pub const fn failed(error: SafeCheckError) -> Self {
        Self {
            ok: false,
            latency_ms: None,
            error: Some(error),
        }
    }

    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.ok == self.error.is_none()
    }
}

pub type Checks = BTreeMap<String, CheckResult>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub project: String,
    pub environment: String,
    pub interface: String,
    pub interface_version: String,
    pub version: String,
    pub build: BuildInfo,
    pub checks: Checks,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadyResponse {
    pub ready: bool,
    pub service: String,
    pub project: String,
    pub environment: String,
    pub interface: String,
    pub interface_version: String,
    pub checks: Checks,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub mode: String,
    pub build: BuildInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetadataEndpoints {
    pub health: String,
    pub ready: String,
    pub metrics: String,
    pub server_info: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Wire format mirrors the MKS-1 capability object.
pub struct MetadataCapabilities {
    pub health: bool,
    pub readiness: bool,
    pub prometheus_metrics: bool,
    pub server_info: bool,
    pub push_heartbeat: bool,
    pub push_event: bool,
    pub push_deploy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetadataResponse {
    pub interface: String,
    pub interface_version: String,
    pub service: String,
    pub project: String,
    pub environment: String,
    pub runtime: String,
    pub version: String,
    pub build: BuildInfo,
    pub endpoints: MetadataEndpoints,
    pub capabilities: MetadataCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeartbeatStatus {
    Ok,
    Degraded,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatIngest {
    pub interface_version: String,
    pub service: String,
    pub project: String,
    pub environment: String,
    pub status: HeartbeatStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EventIngest {
    pub interface_version: String,
    pub service: String,
    pub project: String,
    pub environment: String,
    pub level: EventLevel,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default = "default_event_count")]
    pub count: u32,
    pub timestamp: String,
}

const fn default_event_count() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeployStatus {
    Started,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeployIngest {
    pub interface_version: String,
    pub service: String,
    pub project: String,
    pub environment: String,
    pub version: String,
    pub commit: String,
    pub status: DeployStatus,
    pub timestamp: String,
}

impl HeartbeatIngest {
    #[must_use]
    pub fn has_valid_shape(&self) -> bool {
        valid_ingest_identity(
            &self.interface_version,
            &self.service,
            &self.project,
            &self.environment,
        ) && self
            .message
            .as_ref()
            .is_none_or(|message| message.chars().count() <= 1_024 && !looks_sensitive(message))
    }
}

impl EventIngest {
    #[must_use]
    pub fn has_valid_shape(&self) -> bool {
        valid_ingest_identity(
            &self.interface_version,
            &self.service,
            &self.project,
            &self.environment,
        ) && valid_event_kind(&self.kind)
            && self
                .message
                .as_ref()
                .is_none_or(|message| message.chars().count() <= 1_024 && !looks_sensitive(message))
            && (1..=1_000_000).contains(&self.count)
    }
}

impl DeployIngest {
    #[must_use]
    pub fn has_valid_shape(&self) -> bool {
        valid_ingest_identity(
            &self.interface_version,
            &self.service,
            &self.project,
            &self.environment,
        ) && (1..=128).contains(&self.version.chars().count())
            && (1..=128).contains(&self.commit.chars().count())
    }
}

fn valid_ingest_identity(
    interface_version: &str,
    service: &str,
    project: &str,
    environment: &str,
) -> bool {
    interface_version == INTERFACE_VERSION
        && (1..=64).contains(&service.len())
        && service.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric() || (index > 0 && matches!(character, '.' | '_' | '-'))
        })
        && (1..=64).contains(&project.len())
        && project.chars().enumerate().all(|(index, character)| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || (index > 0 && matches!(character, '_' | '-'))
        })
        && matches!(
            environment,
            "development" | "staging" | "production" | "test" | "local"
        )
}

fn valid_event_kind(kind: &str) -> bool {
    (3..=128).contains(&kind.len())
        && kind.split('_').count() >= 2
        && kind.split('_').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        })
        && !looks_sensitive(kind)
}

fn looks_sensitive(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "authorization",
        "password",
        "secret",
        "token",
        "player_name",
        "player_id",
        "steam_id",
        "ip_address",
        "chat",
        "mks_sk_",
        "mka_agent_",
        "mka_enroll_",
    ]
    .iter()
    .any(|forbidden| value.contains(forbidden))
}

#[cfg(test)]
mod tests {
    use super::{CheckResult, DeployIngest, EventIngest, HeartbeatIngest, SafeCheckError};

    #[test]
    fn successful_check_omits_error() {
        let value = serde_json::to_value(CheckResult::ok()).unwrap_or_default();
        assert_eq!(value, serde_json::json!({ "ok": true }));
    }

    #[test]
    fn failed_check_uses_safe_error() {
        let value =
            serde_json::to_value(CheckResult::failed(SafeCheckError::Timeout)).unwrap_or_default();
        assert_eq!(
            value,
            serde_json::json!({ "ok": false, "error": "timeout" })
        );
    }

    #[test]
    fn ingest_contract_fixtures_deserialize_and_validate() {
        let heartbeat: Result<HeartbeatIngest, _> = serde_json::from_str(include_str!(
            "../../../tests/contracts/fixtures/mks-1/heartbeat.valid.ok.json"
        ));
        let event: Result<EventIngest, _> = serde_json::from_str(include_str!(
            "../../../tests/contracts/fixtures/mks-1/event.valid.database-unavailable.json"
        ));
        let deploy: Result<DeployIngest, _> = serde_json::from_str(include_str!(
            "../../../tests/contracts/fixtures/mks-1/deploy.valid.finished.json"
        ));
        assert!(heartbeat.is_ok_and(|payload| payload.has_valid_shape()));
        assert!(event.is_ok_and(|payload| payload.has_valid_shape()));
        assert!(deploy.is_ok_and(|payload| payload.has_valid_shape()));
    }

    #[test]
    fn ingest_rejects_unknown_fields_and_invalid_shapes() {
        let unknown = serde_json::from_value::<HeartbeatIngest>(serde_json::json!({
            "interface_version": "1",
            "service": "game-api",
            "project": "arena_ops",
            "environment": "production",
            "status": "ok",
            "timestamp": "2026-09-27T00:00:01Z",
            "authorization": "must-not-be-accepted"
        }));
        assert!(unknown.is_err());

        let invalid = EventIngest {
            interface_version: "1".to_owned(),
            service: "game-api".to_owned(),
            project: "arena_ops".to_owned(),
            environment: "production".to_owned(),
            level: super::EventLevel::Error,
            kind: "single".to_owned(),
            message: None,
            count: 1,
            timestamp: "2026-09-27T00:00:01Z".to_owned(),
        };
        assert!(!invalid.has_valid_shape());

        let sensitive = HeartbeatIngest {
            interface_version: "1".to_owned(),
            service: "game-api".to_owned(),
            project: "arena_ops".to_owned(),
            environment: "production".to_owned(),
            status: super::HeartbeatStatus::Down,
            message: Some("password=mks_sk_must_not_persist".to_owned()),
            timestamp: "2026-09-27T00:00:01Z".to_owned(),
        };
        assert!(!sensitive.has_valid_shape());
    }
}
