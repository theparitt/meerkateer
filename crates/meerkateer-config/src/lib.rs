//! Validated process configuration with explicit Community and Cloud modes.

use std::{env, net::SocketAddr, str::FromStr};

use secrecy::SecretString;
use thiserror::Error;

const ENVIRONMENTS: [&str; 5] = ["development", "staging", "production", "test", "local"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentMode {
    Community,
    Cloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageProfile {
    Compact,
    HighAvailability,
}

impl FromStr for StorageProfile {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "compact" => Ok(Self::Compact),
            "ha" => Ok(Self::HighAvailability),
            _ => Err(ConfigError::InvalidValue {
                name: "MEERKATEER_STORAGE_PROFILE",
                reason: "expected compact or ha",
            }),
        }
    }
}

impl FromStr for DeploymentMode {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "community" => Ok(Self::Community),
            "cloud" => Ok(Self::Cloud),
            _ => Err(ConfigError::InvalidValue {
                name: "MEERKATEER_DEPLOYMENT_MODE",
                reason: "expected community or cloud",
            }),
        }
    }
}

impl DeploymentMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Community => "community",
            Self::Cloud => "cloud",
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{name} is required for the selected deployment mode")]
    Missing { name: &'static str },
    #[error("{name} is invalid: {reason}")]
    InvalidValue {
        name: &'static str,
        reason: &'static str,
    },
}

#[derive(Debug)]
pub struct ServerConfig {
    pub bind_address: SocketAddr,
    pub deployment_mode: DeploymentMode,
    pub storage_profile: StorageProfile,
    pub service_name: String,
    pub project: String,
    pub environment: String,
    pub metrics_enabled: bool,
    pub metrics_token: Option<SecretString>,
    pub database_url: Option<SecretString>,
    pub database_max_connections: u32,
    pub authentication_rate_limit_per_minute: u32,
    pub ingestion_rate_limit_per_minute: u32,
    pub heartbeat_stale_after_seconds: u32,
    pub bootstrap_token: Option<SecretString>,
    pub stripe_secret_key: Option<SecretString>,
    pub stripe_webhook_secret: Option<SecretString>,
}

impl ServerConfig {
    /// Load and validate configuration from the process environment.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when a value is malformed or a required Cloud secret
    /// is absent.
    pub fn load() -> Result<Self, ConfigError> {
        Self::from_source(|name| env::var(name).ok())
    }

    /// Build configuration from a caller-provided source, primarily for deterministic tests.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] under the same validation rules as [`Self::load`].
    pub fn from_source(source: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let deployment_mode = source("MEERKATEER_DEPLOYMENT_MODE")
            .unwrap_or_else(|| "community".to_owned())
            .parse()?;
        let storage_profile = source("MEERKATEER_STORAGE_PROFILE")
            .unwrap_or_else(|| "compact".to_owned())
            .parse()?;
        let bind_address = source("MEERKATEER_BIND_ADDR")
            .unwrap_or_else(|| "127.0.0.1:8080".to_owned())
            .parse()
            .map_err(|_| ConfigError::InvalidValue {
                name: "MEERKATEER_BIND_ADDR",
                reason: "expected IP:port",
            })?;
        let service_name =
            source("MEERKATEER_SERVICE_NAME").unwrap_or_else(|| "meerkateer-server".to_owned());
        validate_name("MEERKATEER_SERVICE_NAME", &service_name, false)?;
        let project =
            source("MEERKATEER_SERVICE_PROJECT").unwrap_or_else(|| "meerkateer".to_owned());
        validate_name("MEERKATEER_SERVICE_PROJECT", &project, true)?;
        let environment =
            source("MEERKATEER_SERVICE_ENVIRONMENT").unwrap_or_else(|| "development".to_owned());
        if !ENVIRONMENTS.contains(&environment.as_str()) {
            return Err(ConfigError::InvalidValue {
                name: "MEERKATEER_SERVICE_ENVIRONMENT",
                reason: "unsupported environment",
            });
        }
        let metrics_enabled = parse_bool(
            "MEERKATEER_METRICS_ENABLED",
            source("MEERKATEER_METRICS_ENABLED")
                .as_deref()
                .unwrap_or("true"),
        )?;
        let metrics_token = secret(source("MEERKATEER_METRICS_TOKEN"));
        let database_url = secret(source("MEERKATEER_DATABASE_URL"));
        let database_max_connections = source("MEERKATEER_DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|| "10".to_owned())
            .parse::<u32>()
            .ok()
            .filter(|value| (1..=100).contains(value))
            .ok_or(ConfigError::InvalidValue {
                name: "MEERKATEER_DATABASE_MAX_CONNECTIONS",
                reason: "expected an integer from 1 through 100",
            })?;
        let (
            authentication_rate_limit_per_minute,
            ingestion_rate_limit_per_minute,
            heartbeat_stale_after_seconds,
        ) = parse_operational_limits(&source)?;
        let bootstrap_token = secret(source("MEERKATEER_BOOTSTRAP_TOKEN"));
        let stripe_secret_key = secret(source("MEERKATEER_STRIPE_SECRET_KEY"));
        let stripe_webhook_secret = secret(source("MEERKATEER_STRIPE_WEBHOOK_SECRET"));

        if deployment_mode == DeploymentMode::Cloud {
            if stripe_secret_key.is_none() {
                return Err(ConfigError::Missing {
                    name: "MEERKATEER_STRIPE_SECRET_KEY",
                });
            }
            if stripe_webhook_secret.is_none() {
                return Err(ConfigError::Missing {
                    name: "MEERKATEER_STRIPE_WEBHOOK_SECRET",
                });
            }
        }
        if (deployment_mode == DeploymentMode::Cloud || environment == "production")
            && database_url.is_none()
        {
            return Err(ConfigError::Missing {
                name: "MEERKATEER_DATABASE_URL",
            });
        }
        if environment == "production" && metrics_enabled && metrics_token.is_none() {
            return Err(ConfigError::Missing {
                name: "MEERKATEER_METRICS_TOKEN",
            });
        }

        Ok(Self {
            bind_address,
            deployment_mode,
            storage_profile,
            service_name,
            project,
            environment,
            metrics_enabled,
            metrics_token,
            database_url,
            database_max_connections,
            authentication_rate_limit_per_minute,
            ingestion_rate_limit_per_minute,
            heartbeat_stale_after_seconds,
            bootstrap_token,
            stripe_secret_key,
            stripe_webhook_secret,
        })
    }
}

fn parse_operational_limits(
    source: &impl Fn(&str) -> Option<String>,
) -> Result<(u32, u32, u32), ConfigError> {
    let authentication = parse_bounded_u32(
        "MEERKATEER_AUTH_RATE_LIMIT_PER_MINUTE",
        source("MEERKATEER_AUTH_RATE_LIMIT_PER_MINUTE"),
        300,
        1,
        10_000,
    )?;
    let ingestion = parse_bounded_u32(
        "MEERKATEER_INGEST_RATE_LIMIT_PER_MINUTE",
        source("MEERKATEER_INGEST_RATE_LIMIT_PER_MINUTE"),
        1_200,
        1,
        100_000,
    )?;
    let heartbeat_stale = parse_bounded_u32(
        "MEERKATEER_HEARTBEAT_STALE_AFTER_SECONDS",
        source("MEERKATEER_HEARTBEAT_STALE_AFTER_SECONDS"),
        180,
        30,
        3_600,
    )?;
    Ok((authentication, ingestion, heartbeat_stale))
}

fn parse_bounded_u32(
    name: &'static str,
    value: Option<String>,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> Result<u32, ConfigError> {
    value
        .map_or_else(|| Some(default), |raw| raw.parse::<u32>().ok())
        .filter(|parsed| (minimum..=maximum).contains(parsed))
        .ok_or(ConfigError::InvalidValue {
            name,
            reason: "value is outside the supported range",
        })
}

fn secret(value: Option<String>) -> Option<SecretString> {
    value
        .filter(|item| !item.is_empty())
        .map(SecretString::from)
}

fn parse_bool(name: &'static str, value: &str) -> Result<bool, ConfigError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigError::InvalidValue {
            name,
            reason: "expected true or false",
        }),
    }
}

fn validate_name(name: &'static str, value: &str, lowercase: bool) -> Result<(), ConfigError> {
    let is_valid = !value.is_empty()
        && value.len() <= 64
        && value.chars().enumerate().all(|(index, character)| {
            let allowed = character.is_ascii_alphanumeric()
                || (!lowercase && character == '.')
                || character == '_'
                || character == '-';
            allowed && (index > 0 || character.is_ascii_alphanumeric())
        })
        && (!lowercase || value == value.to_ascii_lowercase());
    if is_valid {
        Ok(())
    } else {
        Err(ConfigError::InvalidValue {
            name,
            reason: "does not satisfy the MKS-1 identity rules",
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{ConfigError, DeploymentMode, ServerConfig};

    fn source(values: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let values = values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<HashMap<_, _>>();
        move |name| values.get(name).cloned()
    }

    #[test]
    fn community_starts_without_stripe() {
        let config = ServerConfig::from_source(source(&[]));
        assert!(matches!(
            config.map(|value| value.deployment_mode),
            Ok(DeploymentMode::Community)
        ));
    }

    #[test]
    fn cloud_fails_closed_without_stripe_secrets() {
        let config = ServerConfig::from_source(source(&[("MEERKATEER_DEPLOYMENT_MODE", "cloud")]));
        assert_eq!(
            config.err(),
            Some(ConfigError::Missing {
                name: "MEERKATEER_STRIPE_SECRET_KEY"
            })
        );
    }

    #[test]
    fn rejects_uppercase_project_slug() {
        let config =
            ServerConfig::from_source(source(&[("MEERKATEER_SERVICE_PROJECT", "Arena_Ops")]));
        assert!(matches!(config, Err(ConfigError::InvalidValue { .. })));
    }

    #[test]
    fn rejects_dot_in_project_slug() {
        let config =
            ServerConfig::from_source(source(&[("MEERKATEER_SERVICE_PROJECT", "arena.ops")]));
        assert!(matches!(config, Err(ConfigError::InvalidValue { .. })));
    }

    #[test]
    fn production_fails_closed_without_database() {
        let config =
            ServerConfig::from_source(source(&[("MEERKATEER_SERVICE_ENVIRONMENT", "production")]));
        assert_eq!(
            config.err(),
            Some(ConfigError::Missing {
                name: "MEERKATEER_DATABASE_URL"
            })
        );
    }

    #[test]
    fn production_metrics_require_authentication() {
        let config = ServerConfig::from_source(source(&[
            ("MEERKATEER_SERVICE_ENVIRONMENT", "production"),
            (
                "MEERKATEER_DATABASE_URL",
                "postgres://fixture.invalid/meerkateer",
            ),
        ]));
        assert_eq!(
            config.err(),
            Some(ConfigError::Missing {
                name: "MEERKATEER_METRICS_TOKEN"
            })
        );
    }

    #[test]
    fn rejects_unbounded_authentication_rate_limit() {
        let config =
            ServerConfig::from_source(source(&[("MEERKATEER_AUTH_RATE_LIMIT_PER_MINUTE", "0")]));
        assert_eq!(
            config.err(),
            Some(ConfigError::InvalidValue {
                name: "MEERKATEER_AUTH_RATE_LIMIT_PER_MINUTE",
                reason: "value is outside the supported range",
            })
        );
    }
}
