use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL: &str = "meerkateer-agent";
pub const PROTOCOL_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPlatform {
    pub os: String,
    pub arch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnrollRequest {
    pub protocol: String,
    pub protocol_version: String,
    pub instance_id: Uuid,
    pub display_name: String,
    pub agent_version: String,
    pub platform: AgentPlatform,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum TelemetryRecord {
    #[serde(rename = "sample")]
    Sample {
        sequence: u64,
        record_id: Uuid,
        observed_at: String,
        metric: String,
        value: f64,
        attributes: BTreeMap<String, String>,
    },
    #[serde(rename = "event")]
    Event {
        sequence: u64,
        record_id: Uuid,
        observed_at: String,
        kind: String,
        severity: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        attributes: BTreeMap<String, String>,
    },
}

impl TelemetryRecord {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        match self {
            Self::Sample { sequence, .. } | Self::Event { sequence, .. } => *sequence,
        }
    }

    #[must_use]
    pub fn has_valid_shape(&self) -> bool {
        match self {
            Self::Sample {
                sequence,
                metric,
                value,
                attributes,
                ..
            } => {
                *sequence > 0
                    && value.is_finite()
                    && valid_metric(metric)
                    && valid_attributes(attributes)
            }
            Self::Event {
                sequence,
                kind,
                severity,
                message,
                attributes,
                ..
            } => {
                *sequence > 0
                    && valid_event_kind(kind)
                    && matches!(severity.as_str(), "info" | "warning" | "error" | "critical")
                    && message.as_ref().is_none_or(|message| {
                        message.chars().count() <= 1_024 && !looks_sensitive(message)
                    })
                    && valid_attributes(attributes)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryBatch {
    pub protocol_version: String,
    pub agent_id: Uuid,
    pub batch_id: Uuid,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub sent_at: String,
    pub records: Vec<TelemetryRecord>,
}

impl TelemetryBatch {
    #[must_use]
    pub fn has_valid_sequence_envelope(&self) -> bool {
        let Some(first) = self.records.first() else {
            return false;
        };
        let Some(last) = self.records.last() else {
            return false;
        };
        if first.sequence() != self.first_sequence || last.sequence() != self.last_sequence {
            return false;
        }
        self.records
            .windows(2)
            .all(|pair| pair[0].sequence() < pair[1].sequence())
    }

    #[must_use]
    pub fn has_valid_shape(&self) -> bool {
        self.protocol_version == PROTOCOL_VERSION
            && (1..=1_000).contains(&self.records.len())
            && self.first_sequence > 0
            && self.has_valid_sequence_envelope()
            && self.records.iter().all(TelemetryRecord::has_valid_shape)
    }
}

fn valid_metric(value: &str) -> bool {
    (2..=128).contains(&value.len())
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '.')
        })
        && !looks_sensitive(value)
}

fn valid_event_kind(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && value.split('_').count() >= 2
        && value.split('_').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        })
        && !looks_sensitive(value)
}

fn valid_attributes(attributes: &BTreeMap<String, String>) -> bool {
    attributes.len() <= 16
        && attributes.iter().all(|(key, value)| {
            (1..=64).contains(&key.len())
                && key
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_lowercase())
                && key.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '_' | '.')
                })
                && value.chars().count() <= 128
                && !looks_sensitive(key)
                && !looks_sensitive(value)
        })
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
    use std::collections::BTreeMap;

    use uuid::Uuid;

    use super::{PROTOCOL_VERSION, TelemetryBatch, TelemetryRecord};

    fn sample(sequence: u64) -> TelemetryRecord {
        TelemetryRecord::Sample {
            sequence,
            record_id: Uuid::new_v4(),
            observed_at: "2026-09-27T00:00:00Z".to_owned(),
            metric: "host.cpu.utilization".to_owned(),
            value: 0.25,
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn validates_monotonic_sequence_envelope() {
        let batch = TelemetryBatch {
            protocol_version: PROTOCOL_VERSION.to_owned(),
            agent_id: Uuid::new_v4(),
            batch_id: Uuid::new_v4(),
            first_sequence: 10,
            last_sequence: 11,
            sent_at: "2026-09-27T00:00:01Z".to_owned(),
            records: vec![sample(10), sample(11)],
        };
        assert!(batch.has_valid_sequence_envelope());
        assert!(batch.has_valid_shape());
    }

    #[test]
    fn rejects_sensitive_or_non_finite_telemetry() {
        let sensitive = TelemetryRecord::Event {
            sequence: 1,
            record_id: Uuid::new_v4(),
            observed_at: "2026-09-27T00:00:00Z".to_owned(),
            kind: "game_server_failed".to_owned(),
            severity: "error".to_owned(),
            message: Some("password=mka_agent_secret".to_owned()),
            attributes: BTreeMap::new(),
        };
        assert!(!sensitive.has_valid_shape());

        let non_finite = TelemetryRecord::Sample {
            sequence: 2,
            record_id: Uuid::new_v4(),
            observed_at: "2026-09-27T00:00:00Z".to_owned(),
            metric: "host.cpu.utilization".to_owned(),
            value: f64::NAN,
            attributes: BTreeMap::new(),
        };
        assert!(!non_finite.has_valid_shape());
    }
}
