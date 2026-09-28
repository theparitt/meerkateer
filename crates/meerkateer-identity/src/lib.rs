//! Deny-by-default tenant authorization and one-time machine credentials.

use std::{fmt, str::FromStr};

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{Error as PasswordHashError, SaltString},
};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

const SERVICE_KEY_SCHEME: &str = "mks_sk";
const ENROLLMENT_TOKEN_SCHEME: &str = "mka_enroll";
const AGENT_KEY_SCHEME: &str = "mka_agent";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Admin,
    Operator,
    Viewer,
}

impl FromStr for Role {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "operator" => Ok(Self::Operator),
            "viewer" => Ok(Self::Viewer),
            _ => Err(IdentityError::InvalidRole),
        }
    }
}

impl Role {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
        }
    }

    /// Authorization is centralized and deny-by-default. New actions are not granted
    /// until they are explicitly added to this matrix and its tests.
    #[must_use]
    pub const fn allows(self, action: Action) -> bool {
        match self {
            Self::Owner => true,
            Self::Admin => !matches!(action, Action::TenantDelete | Action::BillingManage),
            Self::Operator => matches!(
                action,
                Action::Read
                    | Action::ServiceWrite
                    | Action::CredentialRotate
                    | Action::MaintenanceWrite
            ),
            Self::Viewer => matches!(action, Action::Read),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Read,
    TenantManage,
    TenantDelete,
    MemberManage,
    ServiceWrite,
    CredentialIssue,
    CredentialRotate,
    MaintenanceWrite,
    AgentManage,
    BillingManage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Principal {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub role: Role,
}

impl Principal {
    /// Require both the requested tenant and action to match the principal.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Forbidden`] rather than revealing whether a resource
    /// exists in another tenant.
    pub fn authorize(self, tenant_id: Uuid, action: Action) -> Result<(), IdentityError> {
        if self.tenant_id == tenant_id && self.role.allows(action) {
            Ok(())
        } else {
            Err(IdentityError::Forbidden)
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
    #[error("access is forbidden")]
    Forbidden,
    #[error("role is invalid")]
    InvalidRole,
    #[error("credential is invalid")]
    InvalidCredential,
    #[error("credential hashing failed")]
    Hashing,
}

/// A newly-issued service key. The secret is returned once and must not be logged.
pub struct IssuedServiceKey {
    pub key_id: Uuid,
    pub tenant_id: Uuid,
    pub prefix: String,
    pub secret: SecretString,
    pub password_hash: String,
}

impl fmt::Debug for IssuedServiceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedServiceKey")
            .field("key_id", &self.key_id)
            .field("tenant_id", &self.tenant_id)
            .field("prefix", &self.prefix)
            .field("secret", &"[REDACTED]")
            .field("password_hash", &"[REDACTED]")
            .finish()
    }
}

impl IssuedServiceKey {
    /// Create a high-entropy key and an Argon2id hash suitable for persistent storage.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Hashing`] if the salt or Argon2 operation fails.
    pub fn issue(tenant_id: Uuid) -> Result<Self, IdentityError> {
        let key_id = Uuid::new_v4();
        let public = key_id.simple().to_string();
        let prefix = public[..12].to_owned();
        let random = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let presented = format!(
            "{SERVICE_KEY_SCHEME}_{}_{prefix}_{random}",
            tenant_id.simple()
        );
        let password_hash = hash_credential(&presented)?;
        Ok(Self {
            key_id,
            tenant_id,
            prefix,
            secret: SecretString::from(presented),
            password_hash,
        })
    }

    #[must_use]
    pub fn presented_prefix(value: &str) -> Option<&str> {
        let mut pieces = value.split('_');
        match (
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
        ) {
            (Some("mks"), Some("sk"), Some(tenant), Some(prefix), Some(secret), None)
                if tenant.len() == 32
                    && Uuid::parse_str(tenant).is_ok()
                    && prefix.len() == 12
                    && secret.len() == 64 =>
            {
                Some(prefix)
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn presented_tenant(value: &str) -> Option<Uuid> {
        let mut pieces = value.split('_');
        match (pieces.next(), pieces.next(), pieces.next()) {
            (Some("mks"), Some("sk"), Some(tenant)) if tenant.len() == 32 => {
                Uuid::parse_str(tenant).ok()
            }
            _ => None,
        }
    }

    /// Verify a presented key against its stored Argon2id PHC string.
    ///
    /// # Errors
    ///
    /// Returns one stable [`IdentityError::InvalidCredential`] for malformed keys,
    /// malformed stored hashes, and mismatches.
    pub fn verify(presented: &SecretString, stored_hash: &str) -> Result<(), IdentityError> {
        verify_credential(
            presented,
            stored_hash,
            Self::presented_prefix(presented.expose_secret()).is_some(),
        )
    }
}

fn hash_credential(presented: &str) -> Result<String, IdentityError> {
    let salt_source = Uuid::new_v4();
    let salt =
        SaltString::encode_b64(salt_source.as_bytes()).map_err(|_| IdentityError::Hashing)?;
    Argon2::default()
        .hash_password(presented.as_bytes(), &salt)
        .map_err(|_| IdentityError::Hashing)
        .map(|hash| hash.to_string())
}

/// Hash a Community owner's password with the same salted Argon2id policy as other secrets.
///
/// # Errors
///
/// Returns [`IdentityError::Hashing`] if Argon2id cannot produce a hash.
pub fn hash_local_password(password: &SecretString) -> Result<String, IdentityError> {
    hash_credential(password.expose_secret())
}

/// Verify a Community owner's password without exposing the stored hash.
///
/// # Errors
///
/// Returns [`IdentityError::InvalidCredential`] for an invalid password or hash.
pub fn verify_local_password(
    password: &SecretString,
    stored_hash: &str,
) -> Result<(), IdentityError> {
    verify_credential(password, stored_hash, true)
}

fn verify_credential(
    presented: &SecretString,
    stored_hash: &str,
    structurally_valid: bool,
) -> Result<(), IdentityError> {
    if !structurally_valid {
        return Err(IdentityError::InvalidCredential);
    }
    let parsed = PasswordHash::new(stored_hash).map_err(map_password_error)?;
    Argon2::default()
        .verify_password(presented.expose_secret().as_bytes(), &parsed)
        .map_err(map_password_error)
}

pub struct IssuedEnrollmentToken {
    pub token_id: Uuid,
    pub tenant_id: Uuid,
    pub prefix: String,
    pub secret: SecretString,
    pub password_hash: String,
}

impl fmt::Debug for IssuedEnrollmentToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedEnrollmentToken")
            .field("token_id", &self.token_id)
            .field("tenant_id", &self.tenant_id)
            .field("prefix", &self.prefix)
            .field("secret", &"[REDACTED]")
            .field("password_hash", &"[REDACTED]")
            .finish()
    }
}

impl IssuedEnrollmentToken {
    /// Issue one tenant-scoped token. Expiry and one-time consumption are enforced by storage.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Hashing`] if Argon2id hashing fails.
    pub fn issue(tenant_id: Uuid) -> Result<Self, IdentityError> {
        let token_id = Uuid::new_v4();
        let prefix = token_id.simple().to_string()[..12].to_owned();
        let random = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let presented = format!(
            "{ENROLLMENT_TOKEN_SCHEME}_{}_{prefix}_{random}",
            tenant_id.simple()
        );
        let password_hash = hash_credential(&presented)?;
        Ok(Self {
            token_id,
            tenant_id,
            prefix,
            secret: SecretString::from(presented),
            password_hash,
        })
    }

    #[must_use]
    pub fn presented_scope(value: &str) -> Option<(Uuid, &str)> {
        let mut pieces = value.split('_');
        match (
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
        ) {
            (Some("mka"), Some("enroll"), Some(tenant), Some(prefix), Some(secret), None)
                if tenant.len() == 32
                    && prefix.len() == 12
                    && secret.len() == 64
                    && prefix.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
            {
                Uuid::parse_str(tenant).ok().map(|tenant| (tenant, prefix))
            }
            _ => None,
        }
    }

    /// Verify a presented enrollment token without revealing the failure reason.
    ///
    /// # Errors
    ///
    /// Returns one stable [`IdentityError::InvalidCredential`] for malformed or mismatched tokens.
    pub fn verify(presented: &SecretString, stored_hash: &str) -> Result<(), IdentityError> {
        verify_credential(
            presented,
            stored_hash,
            Self::presented_scope(presented.expose_secret()).is_some(),
        )
    }
}

pub struct IssuedAgentCredential {
    pub credential_id: Uuid,
    pub tenant_id: Uuid,
    pub agent_id: Uuid,
    pub prefix: String,
    pub secret: SecretString,
    pub password_hash: String,
}

impl fmt::Debug for IssuedAgentCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedAgentCredential")
            .field("credential_id", &self.credential_id)
            .field("tenant_id", &self.tenant_id)
            .field("agent_id", &self.agent_id)
            .field("prefix", &self.prefix)
            .field("secret", &"[REDACTED]")
            .field("password_hash", &"[REDACTED]")
            .finish()
    }
}

impl IssuedAgentCredential {
    /// Issue a credential scoped to one tenant and one agent.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Hashing`] if Argon2id hashing fails.
    pub fn issue(tenant_id: Uuid, agent_id: Uuid) -> Result<Self, IdentityError> {
        let credential_id = Uuid::new_v4();
        let prefix = credential_id.simple().to_string()[..12].to_owned();
        let random = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let presented = format!(
            "{AGENT_KEY_SCHEME}_{}_{}_{prefix}_{random}",
            tenant_id.simple(),
            agent_id.simple()
        );
        let password_hash = hash_credential(&presented)?;
        Ok(Self {
            credential_id,
            tenant_id,
            agent_id,
            prefix,
            secret: SecretString::from(presented),
            password_hash,
        })
    }

    #[must_use]
    pub fn presented_scope(value: &str) -> Option<(Uuid, Uuid, &str)> {
        let mut pieces = value.split('_');
        match (
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
        ) {
            (
                Some("mka"),
                Some("agent"),
                Some(tenant),
                Some(agent),
                Some(prefix),
                Some(secret),
                None,
            ) if tenant.len() == 32
                && agent.len() == 32
                && prefix.len() == 12
                && secret.len() == 64 =>
            {
                Some((
                    Uuid::parse_str(tenant).ok()?,
                    Uuid::parse_str(agent).ok()?,
                    prefix,
                ))
            }
            _ => None,
        }
    }

    /// Verify a presented agent credential without revealing the failure reason.
    ///
    /// # Errors
    ///
    /// Returns one stable [`IdentityError::InvalidCredential`] for malformed or mismatched keys.
    pub fn verify(presented: &SecretString, stored_hash: &str) -> Result<(), IdentityError> {
        verify_credential(
            presented,
            stored_hash,
            Self::presented_scope(presented.expose_secret()).is_some(),
        )
    }
}

fn map_password_error(_: PasswordHashError) -> IdentityError {
    IdentityError::InvalidCredential
}

/// A high-entropy browser session credential and its lookup digest.
pub struct IssuedSession {
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub secret: SecretString,
    pub digest: [u8; 32],
    pub csrf_secret: SecretString,
    pub csrf_digest: [u8; 32],
}

impl fmt::Debug for IssuedSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedSession")
            .field("session_id", &self.session_id)
            .field("tenant_id", &self.tenant_id)
            .field("secret", &"[REDACTED]")
            .field("digest", &"[REDACTED]")
            .field("csrf_secret", &"[REDACTED]")
            .field("csrf_digest", &"[REDACTED]")
            .finish()
    }
}

impl IssuedSession {
    #[must_use]
    pub fn issue(tenant_id: Uuid) -> Self {
        let session_id = Uuid::new_v4();
        let token = format!(
            "mks_session_{}_{}{}",
            tenant_id.simple(),
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let digest = Sha256::digest(token.as_bytes()).into();
        let csrf_token = format!(
            "mks_csrf_{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let csrf_digest = Sha256::digest(csrf_token.as_bytes()).into();
        Self {
            session_id,
            tenant_id,
            secret: SecretString::from(token),
            digest,
            csrf_secret: SecretString::from(csrf_token),
            csrf_digest,
        }
    }

    #[must_use]
    pub fn digest(value: &SecretString) -> [u8; 32] {
        Sha256::digest(value.expose_secret().as_bytes()).into()
    }

    #[must_use]
    pub fn presented_tenant(value: &SecretString) -> Option<Uuid> {
        let mut pieces = value.expose_secret().split('_');
        match (
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
            pieces.next(),
        ) {
            (Some("mks"), Some("session"), Some(tenant), Some(secret), None)
                if tenant.len() == 32 && secret.len() == 64 =>
            {
                Uuid::parse_str(tenant).ok()
            }
            _ => None,
        }
    }
}

#[must_use]
pub fn constant_time_secret_eq(left: &str, right: &str) -> bool {
    bool::from(left.as_bytes().ct_eq(right.as_bytes()))
}

#[cfg(test)]
mod tests {
    use secrecy::{ExposeSecret, SecretString};
    use uuid::Uuid;

    use super::{
        Action, IdentityError, IssuedAgentCredential, IssuedEnrollmentToken, IssuedServiceKey,
        IssuedSession, Principal, Role, constant_time_secret_eq,
    };

    #[test]
    fn role_matrix_is_deny_by_default_for_sensitive_actions() {
        assert!(Role::Owner.allows(Action::BillingManage));
        assert!(!Role::Admin.allows(Action::BillingManage));
        assert!(Role::Operator.allows(Action::CredentialRotate));
        assert!(!Role::Operator.allows(Action::CredentialIssue));
        assert!(!Role::Operator.allows(Action::AgentManage));
        assert!(Role::Admin.allows(Action::AgentManage));
        assert!(Role::Viewer.allows(Action::Read));
        assert!(!Role::Viewer.allows(Action::ServiceWrite));
    }

    #[test]
    fn principal_cannot_cross_tenant_even_with_owner_role() {
        let principal = Principal {
            user_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            role: Role::Owner,
        };
        assert_eq!(
            principal.authorize(Uuid::new_v4(), Action::Read),
            Err(IdentityError::Forbidden)
        );
    }

    #[test]
    fn issued_key_is_one_time_secret_with_slow_hash() {
        let tenant_id = Uuid::new_v4();
        let issued = IssuedServiceKey::issue(tenant_id);
        assert!(issued.is_ok());
        let Ok(issued) = issued else {
            return;
        };
        assert!(issued.password_hash.starts_with("$argon2id$"));
        assert_eq!(
            IssuedServiceKey::presented_prefix(issued.secret.expose_secret()),
            Some(issued.prefix.as_str())
        );
        assert_eq!(
            IssuedServiceKey::presented_tenant(issued.secret.expose_secret()),
            Some(tenant_id)
        );
        assert!(IssuedServiceKey::verify(&issued.secret, &issued.password_hash).is_ok());
        assert_eq!(
            IssuedServiceKey::verify(
                &SecretString::from(
                    "mks_sk_00000000000000000000000000000000_000000000000_0000000000000000000000000000000000000000000000000000000000000000"
                        .to_owned()
                ),
                &issued.password_hash
            ),
            Err(IdentityError::InvalidCredential)
        );
        assert!(!format!("{issued:?}").contains(issued.secret.expose_secret()));
    }

    #[test]
    fn machine_credentials_are_scoped_hashed_and_redacted() {
        let tenant_id = Uuid::new_v4();
        let enrollment = IssuedEnrollmentToken::issue(tenant_id);
        assert!(enrollment.is_ok());
        let Ok(enrollment) = enrollment else {
            return;
        };
        assert_eq!(
            IssuedEnrollmentToken::presented_scope(enrollment.secret.expose_secret()),
            Some((tenant_id, enrollment.prefix.as_str()))
        );
        assert!(
            IssuedEnrollmentToken::verify(&enrollment.secret, &enrollment.password_hash).is_ok()
        );
        assert!(!format!("{enrollment:?}").contains(enrollment.secret.expose_secret()));

        let agent_id = Uuid::new_v4();
        let credential = IssuedAgentCredential::issue(tenant_id, agent_id);
        assert!(credential.is_ok());
        let Ok(credential) = credential else {
            return;
        };
        assert_eq!(
            IssuedAgentCredential::presented_scope(credential.secret.expose_secret()),
            Some((tenant_id, agent_id, credential.prefix.as_str()))
        );
        assert!(
            IssuedAgentCredential::verify(&credential.secret, &credential.password_hash).is_ok()
        );
        assert!(!format!("{credential:?}").contains(credential.secret.expose_secret()));
    }

    #[test]
    fn session_secret_is_digestible_and_redacted() {
        let tenant_id = Uuid::new_v4();
        let session = IssuedSession::issue(tenant_id);
        assert_eq!(IssuedSession::digest(&session.secret), session.digest);
        assert_eq!(
            IssuedSession::digest(&session.csrf_secret),
            session.csrf_digest
        );
        assert!(session.secret.expose_secret().starts_with("mks_session_"));
        assert_eq!(
            IssuedSession::presented_tenant(&session.secret),
            Some(tenant_id)
        );
        assert!(!format!("{session:?}").contains(session.secret.expose_secret()));
        assert!(!format!("{session:?}").contains(session.csrf_secret.expose_secret()));
    }

    #[test]
    fn bootstrap_secret_comparison_is_constant_time_for_equal_length_values() {
        assert!(constant_time_secret_eq("fixture-a", "fixture-a"));
        assert!(!constant_time_secret_eq("fixture-a", "fixture-b"));
        assert!(!constant_time_secret_eq("short", "longer"));
    }
}
