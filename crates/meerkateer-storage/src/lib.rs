//! Storage boundaries keep domain behavior independent from a deployment profile.

use std::{collections::BTreeMap, sync::RwLock};

use meerkateer_domain::ServiceState;
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSnapshot {
    pub tenant_id: Uuid,
    pub service_id: Uuid,
    pub state: ServiceState,
    pub last_sequence: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StorageError {
    #[error("storage is temporarily unavailable")]
    Unavailable,
    #[error("telemetry sequence has already been accepted")]
    Duplicate,
}

/// Transactional control-plane state. Implementations must scope every lookup by tenant.
pub trait ControlStore: Send + Sync + std::fmt::Debug {
    /// Read a snapshot through an explicitly tenant-scoped key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Unavailable`] when the backing store cannot be read.
    fn service_snapshot(
        &self,
        tenant_id: Uuid,
        service_id: Uuid,
    ) -> Result<Option<ServiceSnapshot>, StorageError>;

    /// Insert or replace the current tenant-scoped service snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Unavailable`] when the backing store cannot be written.
    fn put_service_snapshot(&self, snapshot: ServiceSnapshot) -> Result<(), StorageError>;
}

/// Append-only numeric telemetry. A duplicate sequence must never be applied twice.
pub trait TelemetryStore: Send + Sync + std::fmt::Debug {
    /// Append one bounded numeric sample under a monotonic sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Duplicate`] for an accepted sequence or
    /// [`StorageError::Unavailable`] when durable storage cannot accept the sample.
    fn append_sample(
        &self,
        tenant_id: Uuid,
        service_id: Uuid,
        sequence: u64,
        metric: &str,
        value: f64,
    ) -> Result<(), StorageError>;
}

/// Deterministic adapter for tests and the Phase 1 local integration harness.
#[derive(Debug, Default)]
pub struct InMemoryControlStore {
    snapshots: RwLock<BTreeMap<(Uuid, Uuid), ServiceSnapshot>>,
}

impl ControlStore for InMemoryControlStore {
    fn service_snapshot(
        &self,
        tenant_id: Uuid,
        service_id: Uuid,
    ) -> Result<Option<ServiceSnapshot>, StorageError> {
        let values = self
            .snapshots
            .read()
            .map_err(|_| StorageError::Unavailable)?;
        Ok(values.get(&(tenant_id, service_id)).cloned())
    }

    fn put_service_snapshot(&self, snapshot: ServiceSnapshot) -> Result<(), StorageError> {
        let mut values = self
            .snapshots
            .write()
            .map_err(|_| StorageError::Unavailable)?;
        values.insert((snapshot.tenant_id, snapshot.service_id), snapshot);
        Ok(())
    }
}

/// `PostgreSQL` adapter entry point. Tenant queries are only exposed through a
/// transaction that installs a transaction-local RLS context.
#[derive(Debug, Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Begin a tenant-scoped transaction and install the RLS context with `SET LOCAL`
    /// semantics. Returning the pooled connection can never retain this tenant ID.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Unavailable`] if the transaction or RLS context cannot
    /// be established.
    pub async fn begin_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<TenantTransaction<'_>, StorageError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| StorageError::Unavailable)?;
        sqlx::query("SELECT set_config('meerkateer.tenant_id', $1, true)")
            .bind(tenant_id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(|_| StorageError::Unavailable)?;
        Ok(TenantTransaction {
            tenant_id,
            transaction,
        })
    }
}

#[derive(Debug)]
pub struct TenantTransaction<'a> {
    tenant_id: Uuid,
    transaction: Transaction<'a, Postgres>,
}

impl TenantTransaction<'_> {
    #[must_use]
    pub const fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    /// Count services visible to the current tenant. This narrow method exists for
    /// the integration harness and will be replaced by paginated repositories.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Unavailable`] when `PostgreSQL` cannot serve the query.
    pub async fn service_count(&mut self) -> Result<i64, StorageError> {
        sqlx::query_scalar("SELECT count(*) FROM services")
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(|_| StorageError::Unavailable)
    }

    /// Commit the tenant-scoped transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Unavailable`] if `PostgreSQL` cannot commit.
    pub async fn commit(self) -> Result<(), StorageError> {
        self.transaction
            .commit()
            .await
            .map_err(|_| StorageError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use meerkateer_domain::ServiceState;
    use uuid::Uuid;

    use super::{ControlStore, InMemoryControlStore, ServiceSnapshot};

    #[test]
    fn tenant_is_part_of_every_storage_key() {
        let store = InMemoryControlStore::default();
        let service_id = Uuid::new_v4();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let write = store.put_service_snapshot(ServiceSnapshot {
            tenant_id: tenant_a,
            service_id,
            state: ServiceState::Online,
            last_sequence: 7,
        });
        assert!(write.is_ok());
        assert!(matches!(
            store.service_snapshot(tenant_a, service_id),
            Ok(Some(_))
        ));
        assert!(matches!(
            store.service_snapshot(tenant_b, service_id),
            Ok(None)
        ));
    }
}
