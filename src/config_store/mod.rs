pub mod seed;
pub mod sqlite;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(test)]
pub mod tests;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::monitor::model::Monitor;

#[derive(Debug, thiserror::Error)]
pub enum ConfigStoreError {
    #[error("Monitor not found: {0}")]
    NotFound(String),
    #[error("Monitor already exists: {0}")]
    AlreadyExists(String),
    #[error("Version conflict for monitor {name}: expected {expected}, found {actual}")]
    VersionConflict {
        name: String,
        expected: i64,
        actual: i64,
    },
    #[error("Database error: {0}")]
    Database(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone)]
pub struct StoredMonitor {
    pub monitor: Monitor,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ConfigStore: Send + Sync + 'static {
    /// List all monitors.
    async fn list_monitors(&self) -> Result<Vec<StoredMonitor>, ConfigStoreError>;

    /// Get a single monitor by name.
    async fn get_monitor(&self, name: &str) -> Result<StoredMonitor, ConfigStoreError>;

    /// Create a new monitor. Returns the StoredMonitor with version=1.
    /// Fails with AlreadyExists if name is taken.
    async fn create_monitor(&self, monitor: &Monitor) -> Result<StoredMonitor, ConfigStoreError>;

    /// Update an existing monitor with optimistic concurrency control.
    /// Fails with VersionConflict if versions don't match, NotFound if missing.
    async fn update_monitor(
        &self,
        name: &str,
        monitor: &Monitor,
        expected_version: i64,
    ) -> Result<StoredMonitor, ConfigStoreError>;

    /// Delete a monitor by name. Fails with NotFound if missing.
    async fn delete_monitor(&self, name: &str) -> Result<(), ConfigStoreError>;

    /// Get a fingerprint of all monitors (hash of names+versions).
    /// Used by periodic poll to cheaply detect changes.
    async fn get_config_fingerprint(&self) -> Result<u64, ConfigStoreError>;
}

/// A no-op config store used as a default when no database is configured.
/// All operations return NotFound/empty. Used for backward compatibility with AppState::new().
pub struct NoopConfigStore;

#[async_trait]
impl ConfigStore for NoopConfigStore {
    async fn list_monitors(&self) -> Result<Vec<StoredMonitor>, ConfigStoreError> {
        Ok(vec![])
    }
    async fn get_monitor(&self, name: &str) -> Result<StoredMonitor, ConfigStoreError> {
        Err(ConfigStoreError::NotFound(name.to_string()))
    }
    async fn create_monitor(&self, _monitor: &Monitor) -> Result<StoredMonitor, ConfigStoreError> {
        Err(ConfigStoreError::Database(
            "NoopConfigStore: not supported".into(),
        ))
    }
    async fn update_monitor(
        &self,
        name: &str,
        _monitor: &Monitor,
        _expected_version: i64,
    ) -> Result<StoredMonitor, ConfigStoreError> {
        Err(ConfigStoreError::NotFound(name.to_string()))
    }
    async fn delete_monitor(&self, name: &str) -> Result<(), ConfigStoreError> {
        Err(ConfigStoreError::NotFound(name.to_string()))
    }
    async fn get_config_fingerprint(&self) -> Result<u64, ConfigStoreError> {
        Ok(0)
    }
}
