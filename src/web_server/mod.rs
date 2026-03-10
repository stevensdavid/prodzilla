mod api_v1;

use axum::{Extension, Router};
use std::{env, sync::Arc};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

use crate::app_state::AppState;

mod prometheus_metrics;

pub async fn start_axum_server(app_state: Arc<AppState>) {
    let spa_fallback = ServeDir::new("frontend/dist")
        .fallback(ServeFile::new("frontend/dist/index.html"));

    let app = Router::new()
        .nest("/api/v1", api_v1::router())
        .layer(Extension(app_state.clone()))
        .fallback_service(spa_fallback);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

pub async fn start_prometheus_server(registry: Arc<prometheus::Registry>) {
    let host = match env::var("OTEL_EXPORTER_PROMETHEUS_HOST") {
        Ok(host) => host,
        Err(_) => "localhost".to_owned(),
    };
    let port = match env::var("OTEL_EXPORTER_PROMETHEUS_PORT") {
        Ok(port) => port,
        Err(_) => "9464".to_owned(),
    };
    let app = Router::new()
        .route("/metrics", axum::routing::get(prometheus_metrics::metrics_handler))
        .layer(Extension(registry));

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
        .await
        .unwrap();

    info!(
        "Serving Prometheus metrics on {}/metrics",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app).await.unwrap();
}
