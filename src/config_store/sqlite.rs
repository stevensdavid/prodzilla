use std::hash::{DefaultHasher, Hash, Hasher};

use async_trait::async_trait;
use chrono::Utc;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;

use crate::config_store::{ConfigStore, ConfigStoreError, StoredMonitor};
use crate::monitor::model::Monitor;

pub struct SqliteConfigStore {
    pool: SqlitePool,
}

impl SqliteConfigStore {
    /// Create a new file-backed SQLite config store.
    pub async fn new(db_path: &str) -> Result<Self, ConfigStoreError> {
        let url = format!("sqlite:{}?mode=rwc", db_path);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let store = Self { pool };
        store.run_migrations().await?;
        Ok(store)
    }

    /// Create an in-memory SQLite config store (for tests).
    pub async fn new_in_memory() -> Result<Self, ConfigStoreError> {
        // SQLite in-memory DBs are per-connection, so pool of 1 ensures consistency.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let store = Self { pool };
        store.run_migrations().await?;
        Ok(store)
    }

    async fn run_migrations(&self) -> Result<(), ConfigStoreError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS monitors (
                name        TEXT PRIMARY KEY,
                config_json TEXT NOT NULL,
                version     INTEGER NOT NULL DEFAULT 1,
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        Ok(())
    }

    fn parse_stored_monitor(
        row: &sqlx::sqlite::SqliteRow,
    ) -> Result<StoredMonitor, ConfigStoreError> {
        let config_json: String = row
            .try_get("config_json")
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let monitor: Monitor = serde_json::from_str(&config_json)
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let version: i64 = row
            .try_get("version")
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let created_at_str: String = row
            .try_get("created_at")
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let updated_at_str: String = row
            .try_get("updated_at")
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(StoredMonitor {
            monitor,
            version,
            created_at,
            updated_at,
        })
    }
}

#[async_trait]
impl ConfigStore for SqliteConfigStore {
    async fn list_monitors(&self) -> Result<Vec<StoredMonitor>, ConfigStoreError> {
        let rows = sqlx::query(
            "SELECT name, config_json, version, created_at, updated_at FROM monitors ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        rows.iter().map(Self::parse_stored_monitor).collect()
    }

    async fn get_monitor(&self, name: &str) -> Result<StoredMonitor, ConfigStoreError> {
        let row = sqlx::query("SELECT name, config_json, version, created_at, updated_at FROM monitors WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        match row {
            Some(row) => Self::parse_stored_monitor(&row),
            None => Err(ConfigStoreError::NotFound(name.to_string())),
        }
    }

    async fn create_monitor(&self, monitor: &Monitor) -> Result<StoredMonitor, ConfigStoreError> {
        let config_json =
            serde_json::to_string(monitor).map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            "INSERT INTO monitors (name, config_json, version, created_at, updated_at) VALUES (?, ?, 1, ?, ?)",
        )
        .bind(&monitor.name)
        .bind(&config_json)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => {
                let created_at = chrono::DateTime::parse_from_rfc3339(&now)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                Ok(StoredMonitor {
                    monitor: monitor.clone(),
                    version: 1,
                    created_at,
                    updated_at: created_at,
                })
            }
            Err(e) => {
                // SQLite returns UNIQUE constraint violation for duplicate primary key
                if e.to_string().contains("UNIQUE constraint failed") {
                    Err(ConfigStoreError::AlreadyExists(monitor.name.clone()))
                } else {
                    Err(ConfigStoreError::Database(Box::new(e)))
                }
            }
        }
    }

    async fn update_monitor(
        &self,
        name: &str,
        monitor: &Monitor,
        expected_version: i64,
    ) -> Result<StoredMonitor, ConfigStoreError> {
        let config_json =
            serde_json::to_string(monitor).map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let now = Utc::now().to_rfc3339();
        let new_version = expected_version + 1;

        let result = sqlx::query(
            "UPDATE monitors SET config_json = ?, version = ?, updated_at = ? WHERE name = ? AND version = ?",
        )
        .bind(&config_json)
        .bind(new_version)
        .bind(&now)
        .bind(name)
        .bind(expected_version)
        .execute(&self.pool)
        .await
        .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        if result.rows_affected() == 0 {
            // Either not found or version conflict — check which
            let existing = sqlx::query("SELECT version FROM monitors WHERE name = ?")
                .bind(name)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

            match existing {
                None => Err(ConfigStoreError::NotFound(name.to_string())),
                Some(row) => {
                    let actual: i64 = row
                        .try_get("version")
                        .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
                    Err(ConfigStoreError::VersionConflict {
                        name: name.to_string(),
                        expected: expected_version,
                        actual,
                    })
                }
            }
        } else {
            let updated_at = chrono::DateTime::parse_from_rfc3339(&now)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok(StoredMonitor {
                monitor: monitor.clone(),
                version: new_version,
                created_at: updated_at, // Not ideal but we'd need another query for exact created_at
                updated_at,
            })
        }
    }

    async fn delete_monitor(&self, name: &str) -> Result<(), ConfigStoreError> {
        let result = sqlx::query("DELETE FROM monitors WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        if result.rows_affected() == 0 {
            Err(ConfigStoreError::NotFound(name.to_string()))
        } else {
            Ok(())
        }
    }

    async fn get_config_fingerprint(&self) -> Result<u64, ConfigStoreError> {
        let rows = sqlx::query("SELECT name, version FROM monitors ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        let mut hasher = DefaultHasher::new();
        for row in &rows {
            let name: String = row
                .try_get("name")
                .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
            let version: i64 = row
                .try_get("version")
                .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
            name.hash(&mut hasher);
            version.hash(&mut hasher);
        }
        Ok(hasher.finish())
    }
}

#[cfg(test)]
mod sqlite_tests {
    use super::*;
    use crate::config_store::tests as config_store_tests;

    async fn make_store() -> SqliteConfigStore {
        SqliteConfigStore::new_in_memory().await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get() {
        config_store_tests::test_create_and_get(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_create_duplicate_fails() {
        config_store_tests::test_create_duplicate_fails(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_list_monitors() {
        config_store_tests::test_list_monitors(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_list_empty() {
        config_store_tests::test_list_empty(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_update_increments_version() {
        config_store_tests::test_update_increments_version(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_update_version_conflict() {
        config_store_tests::test_update_version_conflict(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_update_nonexistent_fails() {
        config_store_tests::test_update_nonexistent_fails(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_delete_monitor() {
        config_store_tests::test_delete_monitor(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_delete_nonexistent_fails() {
        config_store_tests::test_delete_nonexistent_fails(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_fingerprint_changes_on_mutation() {
        config_store_tests::test_fingerprint_changes_on_mutation(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_json_roundtrip_single_step() {
        config_store_tests::test_json_roundtrip_single_step(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_json_roundtrip_multi_step() {
        config_store_tests::test_json_roundtrip_multi_step(&make_store().await).await;
    }

    #[tokio::test]
    async fn test_json_roundtrip_with_all_fields() {
        config_store_tests::test_json_roundtrip_with_all_fields(&make_store().await).await;
    }
}
