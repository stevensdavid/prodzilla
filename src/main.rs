mod alerts;
mod app_state;
mod config;
mod config_store;
mod errors;
mod monitor;
mod otel;
mod web_server;

use clap::Parser;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;
use web_server::start_axum_server;
use web_server::start_prometheus_server;

use crate::{
    app_state::AppState, config::load_config, config_store::seed::seed_from_yaml,
    config_store::sqlite::SqliteConfigStore, monitor::manager::MonitorManager,
};

const PRODZILLA_YAML: &str = "prodzilla.yml";
const DEFAULT_DB_PATH: &str = "prodzilla.db";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// YAML config file to seed monitors from
    #[arg(short, long, default_value = PRODZILLA_YAML)]
    file: String,

    /// SQLite database path for persistent config storage
    #[arg(long, default_value = DEFAULT_DB_PATH)]
    db_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let otel_state = otel::init();
    if let Some(registry) = &otel_state.metrics.registry {
        tokio::spawn(start_prometheus_server(registry.clone()));
    }

    // Load YAML config (for seeding)
    let config = load_config(args.file).await?;

    // Initialize config store
    let store = Arc::new(SqliteConfigStore::new(&args.db_url).await?);

    // Seed from YAML
    let seeded = seed_from_yaml(store.as_ref(), &config).await?;
    if seeded > 0 {
        info!("Seeded {} monitors from YAML into database", seeded);
    }

    // Create broadcast channel for config change events
    let (change_tx, change_rx) = broadcast::channel(64);

    // Create shared app state
    let app_state = Arc::new(AppState::with_store(config, store.clone(), change_tx));

    // Spawn monitor manager (replaces schedule_monitors)
    let manager = MonitorManager::new(store, app_state.clone(), change_rx);
    tokio::spawn(manager.run());

    // Start web server
    start_axum_server(app_state.clone()).await;

    Ok(())
}

#[cfg(test)]
mod test_utils;
