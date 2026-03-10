use tracing::{debug, info};

use crate::config::Config;
use crate::config_store::{ConfigStore, ConfigStoreError};

/// Seeds the database from a YAML config.
/// Only creates monitors that don't already exist in the database.
/// Returns the count of monitors seeded.
pub async fn seed_from_yaml(
    store: &dyn ConfigStore,
    config: &Config,
) -> Result<usize, ConfigStoreError> {
    let mut seeded = 0;
    for monitor in &config.monitors {
        match store.create_monitor(monitor).await {
            Ok(_) => {
                info!("Seeded monitor '{}' from YAML", monitor.name);
                seeded += 1;
            }
            Err(ConfigStoreError::AlreadyExists(_)) => {
                debug!(
                    "Monitor '{}' already exists in DB, skipping seed",
                    monitor.name
                );
            }
            Err(e) => return Err(e),
        }
    }
    Ok(seeded)
}

#[cfg(test)]
mod seed_tests {
    use super::*;
    use crate::config_store::sqlite::SqliteConfigStore;
    use crate::config_store::tests::{make_full_test_monitor, make_test_monitor};

    fn make_config(monitors: Vec<crate::monitor::model::Monitor>) -> Config {
        Config { monitors }
    }

    #[tokio::test]
    async fn test_seed_inserts_all_monitors() {
        let store = SqliteConfigStore::new_in_memory().await.unwrap();
        let config = make_config(vec![
            make_test_monitor("seed-1", "https://example.com/1"),
            make_test_monitor("seed-2", "https://example.com/2"),
            make_test_monitor("seed-3", "https://example.com/3"),
        ]);

        let count = seed_from_yaml(&store, &config).await.unwrap();
        assert_eq!(count, 3);

        let monitors = store.list_monitors().await.unwrap();
        assert_eq!(monitors.len(), 3);
    }

    #[tokio::test]
    async fn test_seed_skips_existing() {
        let store = SqliteConfigStore::new_in_memory().await.unwrap();

        // Pre-create one monitor
        let existing = make_test_monitor("seed-1", "https://example.com/existing");
        store.create_monitor(&existing).await.unwrap();

        let config = make_config(vec![
            make_test_monitor("seed-1", "https://example.com/1"),
            make_test_monitor("seed-2", "https://example.com/2"),
            make_test_monitor("seed-3", "https://example.com/3"),
        ]);

        let count = seed_from_yaml(&store, &config).await.unwrap();
        assert_eq!(count, 2);

        let monitors = store.list_monitors().await.unwrap();
        assert_eq!(monitors.len(), 3);
    }

    #[tokio::test]
    async fn test_seed_empty_config() {
        let store = SqliteConfigStore::new_in_memory().await.unwrap();
        let config = make_config(vec![]);

        let count = seed_from_yaml(&store, &config).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_seed_preserves_existing_config() {
        let store = SqliteConfigStore::new_in_memory().await.unwrap();

        // Pre-create with the original URL
        let existing = make_test_monitor("seed-1", "https://example.com/original");
        store.create_monitor(&existing).await.unwrap();

        // Seed with a different URL for the same name
        let config = make_config(vec![make_test_monitor(
            "seed-1",
            "https://example.com/from-yaml",
        )]);

        seed_from_yaml(&store, &config).await.unwrap();

        // Verify the original config was preserved
        let fetched = store.get_monitor("seed-1").await.unwrap();
        assert_eq!(
            fetched.monitor.url.as_deref(),
            Some("https://example.com/original")
        );
    }

    #[tokio::test]
    async fn test_seed_handles_multi_step_monitors() {
        let store = SqliteConfigStore::new_in_memory().await.unwrap();
        let config = make_config(vec![make_full_test_monitor("multi-seed")]);

        let count = seed_from_yaml(&store, &config).await.unwrap();
        assert_eq!(count, 1);

        let fetched = store.get_monitor("multi-seed").await.unwrap();
        assert!(fetched.monitor.is_multi_step());
    }
}
