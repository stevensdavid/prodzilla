use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::config_store::ConfigStore;
use crate::monitor::monitor_logic::Monitorable;
use crate::monitor::schedule::monitoring_loop;
use crate::AppState;

/// Signal sent when config changes occur.
#[derive(Debug, Clone)]
pub enum ConfigChangeEvent {
    MonitorCreated(String),
    MonitorUpdated(String),
    MonitorDeleted(String),
    FullReconcile,
}

/// Tracks a running monitor task.
struct RunningMonitor {
    version: i64,
    cancel_token: CancellationToken,
    join_handle: JoinHandle<()>,
}

/// Manages running monitor tasks, reconciling them against the config store.
pub struct MonitorManager {
    store: Arc<dyn ConfigStore>,
    app_state: Arc<AppState>,
    running: HashMap<String, RunningMonitor>,
    change_rx: broadcast::Receiver<ConfigChangeEvent>,
    last_fingerprint: u64,
    poll_interval: Duration,
}

impl MonitorManager {
    pub fn new(
        store: Arc<dyn ConfigStore>,
        app_state: Arc<AppState>,
        change_rx: broadcast::Receiver<ConfigChangeEvent>,
    ) -> Self {
        Self {
            store,
            app_state,
            running: HashMap::new(),
            change_rx,
            last_fingerprint: 0,
            poll_interval: Duration::from_secs(30),
        }
    }

    /// Create a MonitorManager with a custom poll interval (useful for tests).
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Run the manager event loop. This should be spawned as a tokio task.
    pub async fn run(mut self) {
        // Initial full reconcile
        self.reconcile().await;

        loop {
            tokio::select! {
                event = self.change_rx.recv() => {
                    match event {
                        Ok(ConfigChangeEvent::FullReconcile) => {
                            self.reconcile().await;
                        }
                        Ok(ConfigChangeEvent::MonitorCreated(name)) |
                        Ok(ConfigChangeEvent::MonitorUpdated(name)) => {
                            self.reconcile_single(&name).await;
                        }
                        Ok(ConfigChangeEvent::MonitorDeleted(name)) => {
                            self.stop_monitor(&name).await;
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Missed {} config change events, doing full reconcile", n);
                            self.reconcile().await;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Config change channel closed, stopping manager");
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(self.poll_interval) => {
                    match self.store.get_config_fingerprint().await {
                        Ok(fp) if fp != self.last_fingerprint => {
                            info!("Config fingerprint changed, reconciling");
                            self.reconcile().await;
                        }
                        Ok(_) => {} // No change
                        Err(e) => {
                            error!("Failed to get config fingerprint: {}", e);
                        }
                    }
                }
            }
        }

        // Graceful shutdown: cancel all running monitors
        self.stop_all().await;
    }

    /// Full reconciliation: diff running tasks against DB state.
    async fn reconcile(&mut self) {
        let db_monitors = match self.store.list_monitors().await {
            Ok(monitors) => monitors,
            Err(e) => {
                error!("Failed to list monitors from store: {}", e);
                return;
            }
        };

        let db_map: HashMap<String, _> = db_monitors
            .into_iter()
            .map(|sm| (sm.monitor.name.clone(), sm))
            .collect();

        // Stop removed monitors
        let running_names: Vec<String> = self.running.keys().cloned().collect();
        for name in &running_names {
            if !db_map.contains_key(name) {
                info!("Monitor '{}' removed from config, stopping", name);
                self.stop_monitor(name).await;
            }
        }

        // Start new or update changed monitors
        for (name, stored) in &db_map {
            match self.running.get(name) {
                None => {
                    // New monitor
                    info!("Starting new monitor '{}'", name);
                    self.start_monitor(stored.monitor.clone(), stored.version);
                }
                Some(running) if running.version != stored.version => {
                    // Changed monitor: stop old, start new
                    info!(
                        "Monitor '{}' changed (v{} -> v{}), restarting",
                        name, running.version, stored.version
                    );
                    self.stop_monitor(name).await;
                    self.start_monitor(stored.monitor.clone(), stored.version);
                }
                Some(_) => {
                    // No change
                }
            }
        }

        // Update fingerprint
        match self.store.get_config_fingerprint().await {
            Ok(fp) => self.last_fingerprint = fp,
            Err(e) => error!("Failed to update fingerprint: {}", e),
        }
    }

    /// Reconcile a single monitor by name.
    async fn reconcile_single(&mut self, name: &str) {
        match self.store.get_monitor(name).await {
            Ok(stored) => {
                let should_restart = self
                    .running
                    .get(name)
                    .map_or(true, |r| r.version != stored.version);

                if should_restart {
                    if self.running.contains_key(name) {
                        self.stop_monitor(name).await;
                    }
                    self.start_monitor(stored.monitor.clone(), stored.version);
                }
            }
            Err(crate::config_store::ConfigStoreError::NotFound(_)) => {
                // Monitor was deleted
                self.stop_monitor(name).await;
            }
            Err(e) => {
                error!("Failed to get monitor '{}': {}", name, e);
            }
        }

        // Update fingerprint
        if let Ok(fp) = self.store.get_config_fingerprint().await {
            self.last_fingerprint = fp;
        }
    }

    fn start_monitor(&mut self, monitor: crate::monitor::model::Monitor, version: i64) {
        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();
        let app_state = self.app_state.clone();
        let name = monitor.name.clone();

        let join_handle = tokio::spawn(async move {
            monitoring_loop(&monitor, app_state, token_clone).await;
        });

        self.running.insert(
            name,
            RunningMonitor {
                version,
                cancel_token,
                join_handle,
            },
        );
    }

    async fn stop_monitor(&mut self, name: &str) {
        if let Some(running) = self.running.remove(name) {
            running.cancel_token.cancel();
            // Wait for the task to finish with a timeout
            match tokio::time::timeout(Duration::from_secs(5), running.join_handle).await {
                Ok(Ok(())) => {
                    info!("Monitor '{}' stopped gracefully", name);
                }
                Ok(Err(e)) => {
                    warn!("Monitor '{}' task panicked: {}", name, e);
                }
                Err(_) => {
                    warn!("Monitor '{}' did not stop within timeout", name);
                }
            }
        }
    }

    async fn stop_all(&mut self) {
        let names: Vec<String> = self.running.keys().cloned().collect();
        for name in names {
            self.stop_monitor(&name).await;
        }
    }
}

#[cfg(test)]
mod manager_tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::config::Config;
    use crate::config_store::sqlite::SqliteConfigStore;
    use crate::config_store::tests::make_test_monitor;
    use crate::config_store::ConfigStore;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_app_state() -> Arc<AppState> {
        Arc::new(AppState::new(Config { monitors: vec![] }))
    }

    #[tokio::test]
    async fn test_reconcile_starts_new_monitors() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1..)
            .mount(&mock_server)
            .await;

        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let monitor = make_test_monitor("start-test", &format!("{}/health", mock_server.uri()));
        store.create_monitor(&monitor).await.unwrap();

        let (tx, rx) = broadcast::channel(16);
        let app_state = make_app_state();

        let manager = MonitorManager::new(store.clone(), app_state.clone(), rx)
            .with_poll_interval(Duration::from_secs(300)); // Long poll, rely on initial reconcile

        let manager_handle = tokio::spawn(manager.run());

        // Wait for monitor to execute at least once
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Check results were stored
        let results = app_state.monitor_results.read().unwrap();
        assert!(
            results.contains_key("start-test"),
            "Monitor should have produced results"
        );

        // Shutdown
        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), manager_handle).await;
    }

    #[tokio::test]
    async fn test_reconcile_stops_removed_monitors() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let monitor = make_test_monitor("stop-test", &format!("{}/health", mock_server.uri()));
        store.create_monitor(&monitor).await.unwrap();

        let (tx, rx) = broadcast::channel(16);
        let app_state = make_app_state();

        let manager = MonitorManager::new(store.clone(), app_state.clone(), rx)
            .with_poll_interval(Duration::from_secs(300));

        let manager_handle = tokio::spawn(manager.run());

        // Wait for initial reconcile and at least one probe
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Delete monitor from store and trigger reconcile
        store.delete_monitor("stop-test").await.unwrap();
        tx.send(ConfigChangeEvent::MonitorDeleted("stop-test".to_string()))
            .unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        // Record how many results we have now
        let count_before = {
            let results = app_state.monitor_results.read().unwrap();
            results.get("stop-test").map_or(0, |v| v.len())
        };

        // Wait and check no new results appear
        tokio::time::sleep(Duration::from_secs(2)).await;
        let count_after = {
            let results = app_state.monitor_results.read().unwrap();
            results.get("stop-test").map_or(0, |v| v.len())
        };

        assert_eq!(
            count_before, count_after,
            "No new results should appear after monitor is stopped"
        );

        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), manager_handle).await;
    }

    #[tokio::test]
    async fn test_reconcile_updates_changed_monitors() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let monitor = make_test_monitor("update-test", &format!("{}/health", mock_server.uri()));
        store.create_monitor(&monitor).await.unwrap();

        let (tx, rx) = broadcast::channel(16);
        let app_state = make_app_state();

        let manager = MonitorManager::new(store.clone(), app_state.clone(), rx)
            .with_poll_interval(Duration::from_secs(300));

        let manager_handle = tokio::spawn(manager.run());

        // Wait for initial reconcile
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Update monitor in store
        let mut updated = monitor.clone();
        updated.schedule.interval = 999;
        store
            .update_monitor("update-test", &updated, 1)
            .await
            .unwrap();

        tx.send(ConfigChangeEvent::MonitorUpdated("update-test".to_string()))
            .unwrap();

        // Wait for update to take effect
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Verify monitor is still running (produces results)
        let results = app_state.monitor_results.read().unwrap();
        assert!(
            results.contains_key("update-test"),
            "Updated monitor should still be running"
        );

        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), manager_handle).await;
    }

    #[tokio::test]
    async fn test_event_driven_create() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let (tx, rx) = broadcast::channel(16);
        let app_state = make_app_state();

        let manager = MonitorManager::new(store.clone(), app_state.clone(), rx)
            .with_poll_interval(Duration::from_secs(300)); // Long poll

        let manager_handle = tokio::spawn(manager.run());

        // Wait for initial reconcile (empty DB)
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Add monitor and notify
        let monitor = make_test_monitor("event-create", &format!("{}/health", mock_server.uri()));
        store.create_monitor(&monitor).await.unwrap();
        tx.send(ConfigChangeEvent::MonitorCreated(
            "event-create".to_string(),
        ))
        .unwrap();

        // Wait for monitor to execute
        tokio::time::sleep(Duration::from_secs(2)).await;

        let results = app_state.monitor_results.read().unwrap();
        assert!(
            results.contains_key("event-create"),
            "Event-created monitor should have produced results"
        );

        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), manager_handle).await;
    }

    #[tokio::test]
    async fn test_event_driven_delete() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let monitor = make_test_monitor("event-delete", &format!("{}/health", mock_server.uri()));
        store.create_monitor(&monitor).await.unwrap();

        let (tx, rx) = broadcast::channel(16);
        let app_state = make_app_state();

        let manager = MonitorManager::new(store.clone(), app_state.clone(), rx)
            .with_poll_interval(Duration::from_secs(300));

        let manager_handle = tokio::spawn(manager.run());

        // Wait for initial reconcile + one probe
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Delete and notify
        store.delete_monitor("event-delete").await.unwrap();
        tx.send(ConfigChangeEvent::MonitorDeleted(
            "event-delete".to_string(),
        ))
        .unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        let count_before = {
            let r = app_state.monitor_results.read().unwrap();
            r.get("event-delete").map_or(0, |v| v.len())
        };

        tokio::time::sleep(Duration::from_secs(2)).await;

        let count_after = {
            let r = app_state.monitor_results.read().unwrap();
            r.get("event-delete").map_or(0, |v| v.len())
        };

        assert_eq!(
            count_before, count_after,
            "Deleted monitor should stop producing results"
        );

        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), manager_handle).await;
    }

    #[tokio::test]
    async fn test_poll_detects_external_changes() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let (_tx, rx) = broadcast::channel(16);
        let app_state = make_app_state();

        let manager = MonitorManager::new(store.clone(), app_state.clone(), rx)
            .with_poll_interval(Duration::from_secs(1)); // Short poll for test

        let manager_handle = tokio::spawn(manager.run());

        // Wait for initial reconcile (empty DB)
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Add monitor directly to DB (no event)
        let monitor = make_test_monitor("poll-detect", &format!("{}/health", mock_server.uri()));
        store.create_monitor(&monitor).await.unwrap();

        // Wait for poll to detect change + monitor to execute
        tokio::time::sleep(Duration::from_secs(4)).await;

        let results = app_state.monitor_results.read().unwrap();
        assert!(
            results.contains_key("poll-detect"),
            "Poll should have detected the new monitor and started it"
        );

        drop(_tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), manager_handle).await;
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let monitor = make_test_monitor("shutdown-test", &format!("{}/health", mock_server.uri()));
        store.create_monitor(&monitor).await.unwrap();

        let (tx, rx) = broadcast::channel(16);
        let app_state = make_app_state();

        let manager = MonitorManager::new(store.clone(), app_state.clone(), rx)
            .with_poll_interval(Duration::from_secs(300));

        let manager_handle = tokio::spawn(manager.run());

        // Wait for monitor to start
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Drop sender to close channel, triggering shutdown
        drop(tx);

        // Manager should exit gracefully
        let result = tokio::time::timeout(Duration::from_secs(10), manager_handle).await;
        assert!(result.is_ok(), "Manager should shut down within timeout");
    }
}
