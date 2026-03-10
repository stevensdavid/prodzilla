use std::hash::{DefaultHasher, Hash, Hasher};

use async_trait::async_trait;
use chrono::Utc;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

use crate::config_store::{ConfigStore, ConfigStoreError, StoredMonitor};
use crate::monitor::model::Monitor;

pub struct PostgresConfigStore {
    pool: PgPool,
}

impl PostgresConfigStore {
    /// Create a new PostgreSQL config store.
    pub async fn new(database_url: &str) -> Result<Self, ConfigStoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
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
                config_json JSONB NOT NULL,
                version     INTEGER NOT NULL DEFAULT 1,
                created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        Ok(())
    }

    fn parse_stored_monitor(
        row: &sqlx::postgres::PgRow,
    ) -> Result<StoredMonitor, ConfigStoreError> {
        let config_json: String = row
            .try_get("config_json")
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let monitor: Monitor = serde_json::from_str(&config_json)
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let version: i32 = row
            .try_get("version")
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let created_at: chrono::DateTime<Utc> = row
            .try_get("created_at")
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
        let updated_at: chrono::DateTime<Utc> = row
            .try_get("updated_at")
            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        Ok(StoredMonitor {
            monitor,
            version: version as i64,
            created_at,
            updated_at,
        })
    }
}

#[async_trait]
impl ConfigStore for PostgresConfigStore {
    async fn list_monitors(&self) -> Result<Vec<StoredMonitor>, ConfigStoreError> {
        let rows = sqlx::query(
            "SELECT name, config_json::TEXT, version, created_at, updated_at FROM monitors ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        rows.iter().map(Self::parse_stored_monitor).collect()
    }

    async fn get_monitor(&self, name: &str) -> Result<StoredMonitor, ConfigStoreError> {
        let row = sqlx::query(
            "SELECT name, config_json::TEXT, version, created_at, updated_at FROM monitors WHERE name = $1",
        )
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

        let result = sqlx::query(
            "INSERT INTO monitors (name, config_json, version, created_at, updated_at) VALUES ($1, $2::JSONB, 1, NOW(), NOW())
             RETURNING name, config_json::TEXT, version, created_at, updated_at",
        )
        .bind(&monitor.name)
        .bind(&config_json)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(row) => Self::parse_stored_monitor(&row),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("duplicate key") || msg.contains("unique constraint") {
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
        let new_version = (expected_version + 1) as i32;

        let result = sqlx::query(
            "UPDATE monitors SET config_json = $1::JSONB, version = $2, updated_at = NOW()
             WHERE name = $3 AND version = $4
             RETURNING name, config_json::TEXT, version, created_at, updated_at",
        )
        .bind(&config_json)
        .bind(new_version)
        .bind(name)
        .bind(expected_version as i32)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

        match result {
            Some(row) => Self::parse_stored_monitor(&row),
            None => {
                // Check if not found or version conflict
                let existing = sqlx::query("SELECT version FROM monitors WHERE name = $1")
                    .bind(name)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;

                match existing {
                    None => Err(ConfigStoreError::NotFound(name.to_string())),
                    Some(row) => {
                        let actual: i32 = row
                            .try_get("version")
                            .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
                        Err(ConfigStoreError::VersionConflict {
                            name: name.to_string(),
                            expected: expected_version,
                            actual: actual as i64,
                        })
                    }
                }
            }
        }
    }

    async fn delete_monitor(&self, name: &str) -> Result<(), ConfigStoreError> {
        let result = sqlx::query("DELETE FROM monitors WHERE name = $1")
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
            let version: i32 = row
                .try_get("version")
                .map_err(|e| ConfigStoreError::Database(Box::new(e)))?;
            name.hash(&mut hasher);
            (version as i64).hash(&mut hasher);
        }
        Ok(hasher.finish())
    }
}
