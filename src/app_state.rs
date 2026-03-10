use std::{collections::HashMap, sync::Arc, sync::RwLock};

use tokio::sync::broadcast;

use crate::{
    config::Config, config_store::ConfigStore, monitor::manager::ConfigChangeEvent,
    monitor::model::MonitorResult, otel::metrics::Metrics,
};

// Limits the number of results we store per monitor. Once we go over this amount we remove the earliest.
const RESULT_LIMIT: usize = 100;

pub struct AppState {
    pub monitor_results: RwLock<HashMap<String, Vec<MonitorResult>>>,
    pub config_store: Arc<dyn ConfigStore>,
    pub change_tx: broadcast::Sender<ConfigChangeEvent>,
    pub config: Config,
    pub metrics: Metrics,
}

impl AppState {
    pub fn new(config: Config) -> AppState {
        let (change_tx, _) = broadcast::channel(64);
        // Default in-memory store for backward compatibility
        AppState {
            monitor_results: RwLock::new(HashMap::new()),
            config_store: Arc::new(crate::config_store::NoopConfigStore),
            change_tx,
            config,
            metrics: Metrics::new(),
        }
    }

    pub fn with_store(
        config: Config,
        store: Arc<dyn ConfigStore>,
        change_tx: broadcast::Sender<ConfigChangeEvent>,
    ) -> AppState {
        AppState {
            monitor_results: RwLock::new(HashMap::new()),
            config_store: store,
            change_tx,
            config,
            metrics: Metrics::new(),
        }
    }

    pub fn add_monitor_result(&self, monitor_name: String, result: MonitorResult) {
        let mut write_lock = self.monitor_results.write().unwrap();

        let results = write_lock.entry(monitor_name).or_default();
        results.push(result);

        // Ensure only the latest 100 elements are kept
        while results.len() > RESULT_LIMIT {
            results.remove(0);
        }
    }
}
