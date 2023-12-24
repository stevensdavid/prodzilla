mod config;
mod probe;
mod http_probe;
mod schedule;

use axum::{
    routing::{get},Router,
};
use schedule::schedule_probes;

use crate::config::load_config;

const PRODZILLA_YAML: &str = "prodzilla.yml";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    start_monitoring().await?;

    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(root));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn start_monitoring() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config(PRODZILLA_YAML).await?;
    schedule_probes(config.probes).await?;
    Ok(())
}

async fn root() -> &'static str {
    "Hello, World!"
}

// todo
// start calling the probe endpoints
// - do we need tracing?
// - shall we fix the capitalization of initialDelay