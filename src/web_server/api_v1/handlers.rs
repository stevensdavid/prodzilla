use axum::{
    extract::{Path, Query},
    http::{header, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use std::sync::Arc;
use tracing::{debug, warn};

use crate::app_state::AppState;
use crate::config_store::ConfigStoreError;
use crate::monitor::manager::ConfigChangeEvent;
use crate::monitor::model::Monitor;
use crate::monitor::monitor_logic::Monitorable;

use super::model::{
    ApiError, MonitorApiResponse, MonitorListResponse, MonitorQueryParams, MonitorSummary,
    UpdateMonitorRequest, ValidatedJson,
};

pub async fn list_monitors(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<MonitorListResponse>, ApiError> {
    debug!("API: list monitors");

    let stored = state.config_store.list_monitors().await?;
    let monitors = stored.into_iter().map(MonitorApiResponse::from).collect();

    Ok(Json(MonitorListResponse { monitors }))
}

pub async fn get_monitor(
    Path(name): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    debug!("API: get monitor {}", name);

    let stored = state.config_store.get_monitor(&name).await?;
    let version = stored.version.to_string();
    let response = MonitorApiResponse::from(stored);

    Ok(([(header::ETAG, version)], Json(response)))
}

pub async fn create_monitor(
    Extension(state): Extension<Arc<AppState>>,
    ValidatedJson(monitor): ValidatedJson<Monitor>,
) -> Result<impl IntoResponse, ApiError> {
    debug!("API: create monitor {}", monitor.name);

    let name = monitor.name.clone();
    let stored = state.config_store.create_monitor(&monitor).await?;
    if state
        .change_tx
        .send(ConfigChangeEvent::MonitorCreated(name.clone()))
        .is_err()
    {
        warn!("No receivers for config change event (monitor created: {name})");
    }

    let location = format!("/api/v1/monitors/{name}");
    let response = MonitorApiResponse::from(stored);

    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(response),
    ))
}

pub async fn update_monitor(
    Path(name): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    ValidatedJson(req): ValidatedJson<UpdateMonitorRequest>,
) -> Result<impl IntoResponse, ApiError> {
    debug!("API: update monitor {}", name);

    if req.monitor.name != name {
        return Err(ApiError {
            error: "validation_error".to_string(),
            message: format!(
                "Monitor name in body ({}) does not match URL ({})",
                req.monitor.name, name
            ),
            details: None,
        });
    }

    let stored = state
        .config_store
        .update_monitor(&name, &req.monitor, req.version)
        .await?;
    if state
        .change_tx
        .send(ConfigChangeEvent::MonitorUpdated(name.clone()))
        .is_err()
    {
        warn!("No receivers for config change event (monitor updated: {name})");
    }

    let version = stored.version.to_string();
    let response = MonitorApiResponse::from(stored);

    Ok(([(header::ETAG, version)], Json(response)))
}

pub async fn delete_monitor(
    Path(name): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<StatusCode, ApiError> {
    debug!("API: delete monitor {}", name);

    state.config_store.delete_monitor(&name).await?;
    if state
        .change_tx
        .send(ConfigChangeEvent::MonitorDeleted(name.clone()))
        .is_err()
    {
        warn!("No receivers for config change event (monitor deleted: {name})");
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Get a summary list of all monitors with their status
pub async fn monitor_summary(
    Extension(state): Extension<Arc<AppState>>,
) -> Json<Vec<MonitorSummary>> {
    debug!("API: monitor summary");

    let mut summaries: Vec<MonitorSummary> = vec![];

    let monitor_lock = state.monitor_results.read().unwrap();
    for (key, value) in monitor_lock.iter() {
        if let Some(last) = value.last() {
            let status = if last.success { "OK" } else { "FAILING" };
            summaries.push(MonitorSummary {
                name: key.clone(),
                status: status.to_owned(),
                last_probed: last.timestamp_started,
            });
        }
    }

    Json(summaries)
}

/// Get results for a specific monitor
pub async fn get_monitor_results(
    Path(name): Path<String>,
    Query(params): Query<MonitorQueryParams>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    debug!("API: get monitor results for {}", name);

    let show_response = params.show_response.unwrap_or(false);

    let monitor_lock = state.monitor_results.read().unwrap();
    if let Some(results) = monitor_lock.get(&name) {
        let mut cloned_results = results.clone();
        cloned_results.reverse();

        if !show_response {
            for result in &mut cloned_results {
                for step_result in &mut result.step_results {
                    step_result.response = None;
                }
            }
        }

        return Ok(Json(serde_json::to_value(cloned_results).unwrap()));
    }

    Err(ApiError {
        error: "not_found".to_string(),
        message: format!("Monitor not found: {name}"),
        details: None,
    })
}

/// Trigger a specific monitor to run immediately
pub async fn trigger_monitor(
    Path(name): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    debug!("API: trigger monitor {}", name);

    // Try config store first, fall back to static config
    let monitor = match state.config_store.get_monitor(&name).await {
        Ok(stored) => Some(stored.monitor),
        Err(ConfigStoreError::NotFound(_)) => state
            .config
            .monitors
            .iter()
            .find(|m| m.name == name)
            .cloned(),
        Err(e) => return Err(ApiError::from(e)),
    };

    if let Some(monitor) = monitor {
        monitor.probe_and_store_result(state.clone()).await;

        let lock = state.monitor_results.read().unwrap();
        if let Some(results) = lock.get(&name) {
            return Ok(Json(
                serde_json::to_value(results.last().unwrap().clone()).unwrap(),
            ));
        }
    }

    Err(ApiError {
        error: "not_found".to_string(),
        message: format!("Monitor not found: {name}"),
        details: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::config::Config;
    use crate::config_store::sqlite::SqliteConfigStore;
    use crate::monitor::model::{Monitor, MonitorResult, ScheduleParameters};
    use axum::body::Body;
    use axum::http::{self, Request, Response};
    use tokio::sync::broadcast;
    use tower::util::ServiceExt;

    fn test_monitor(name: &str) -> Monitor {
        Monitor {
            name: name.to_string(),
            url: Some("https://example.com".to_string()),
            http_method: Some("GET".to_string()),
            with: None,
            expectations: None,
            sensitive: false,
            steps: None,
            schedule: ScheduleParameters {
                initial_delay: 0,
                interval: 60,
            },
            alerts: None,
            tags: None,
        }
    }

    async fn setup() -> (
        axum::Router,
        broadcast::Receiver<ConfigChangeEvent>,
        Arc<AppState>,
    ) {
        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let config = Config { monitors: vec![] };
        let (change_tx, change_rx) = broadcast::channel(16);
        let app_state = Arc::new(AppState::with_store(config, store, change_tx));

        let app = crate::web_server::api_v1::router().layer(Extension(app_state.clone()));

        (app, change_rx, app_state)
    }

    async fn send(app: axum::Router, req: Request<Body>) -> Response<Body> {
        app.oneshot(req).await.unwrap()
    }

    async fn response_json<T: serde::de::DeserializeOwned>(resp: Response<Body>) -> T {
        let body = resp.into_body();
        let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn test_list_monitors_empty() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .uri("/monitors")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let list: MonitorListResponse = response_json(resp).await;
        assert!(list.monitors.is_empty());
    }

    #[tokio::test]
    async fn test_create_and_get_monitor() {
        let (app, mut rx, _state) = setup().await;

        let monitor = test_monitor("test-mon");
        let json_body = serde_json::to_string(&monitor).unwrap();

        // Create
        let resp = send(
            app.clone(),
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json_body))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::CREATED);
        assert!(resp.headers().get(header::LOCATION).is_some());

        let created: MonitorApiResponse = response_json(resp).await;
        assert_eq!(created.name, "test-mon");
        assert_eq!(created.version, 1);

        // Check change event
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, ConfigChangeEvent::MonitorCreated(ref name) if name == "test-mon"));

        // Get
        let resp = send(
            app,
            Request::builder()
                .uri("/monitors/test-mon")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::ETAG).unwrap().to_str().unwrap(),
            "1"
        );
    }

    #[tokio::test]
    async fn test_create_duplicate_returns_409() {
        let (app, _rx, _state) = setup().await;

        let monitor = test_monitor("dup-mon");
        let json_body = serde_json::to_string(&monitor).unwrap();

        // First create
        let resp = send(
            app.clone(),
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json_body.clone()))
                .unwrap(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Duplicate
        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json_body))
                .unwrap(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_update_monitor() {
        let (app, mut rx, _state) = setup().await;

        let monitor = test_monitor("upd-mon");
        let json_body = serde_json::to_string(&monitor).unwrap();

        // Create
        send(
            app.clone(),
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json_body))
                .unwrap(),
        )
        .await;
        let _ = rx.try_recv(); // consume create event

        // Update
        let mut updated_monitor = test_monitor("upd-mon");
        updated_monitor.url = Some("https://updated.example.com".to_string());
        let update_req = serde_json::json!({
            "monitor": updated_monitor,
            "version": 1
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::PUT)
                .uri("/monitors/upd-mon")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&update_req).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let updated: MonitorApiResponse = response_json(resp).await;
        assert_eq!(updated.version, 2);
        assert_eq!(
            updated.monitor.url.as_deref(),
            Some("https://updated.example.com")
        );

        let event = rx.try_recv().unwrap();
        assert!(matches!(event, ConfigChangeEvent::MonitorUpdated(ref name) if name == "upd-mon"));
    }

    #[tokio::test]
    async fn test_update_version_conflict() {
        let (app, _rx, _state) = setup().await;

        let monitor = test_monitor("conflict-mon");
        let json_body = serde_json::to_string(&monitor).unwrap();

        // Create
        send(
            app.clone(),
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json_body))
                .unwrap(),
        )
        .await;

        // Update with wrong version
        let update_req = serde_json::json!({
            "monitor": test_monitor("conflict-mon"),
            "version": 99
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::PUT)
                .uri("/monitors/conflict-mon")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&update_req).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_update_name_mismatch() {
        let (app, _rx, _state) = setup().await;

        let monitor = test_monitor("orig-mon");
        let json_body = serde_json::to_string(&monitor).unwrap();

        // Create
        send(
            app.clone(),
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json_body))
                .unwrap(),
        )
        .await;

        // Update with mismatched name
        let update_req = serde_json::json!({
            "monitor": test_monitor("different-name"),
            "version": 1
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::PUT)
                .uri("/monitors/orig-mon")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&update_req).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_delete_monitor() {
        let (app, mut rx, _state) = setup().await;

        let monitor = test_monitor("del-mon");
        let json_body = serde_json::to_string(&monitor).unwrap();

        // Create
        send(
            app.clone(),
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json_body))
                .unwrap(),
        )
        .await;
        let _ = rx.try_recv();

        // Delete
        let resp = send(
            app.clone(),
            Request::builder()
                .method(http::Method::DELETE)
                .uri("/monitors/del-mon")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        let event = rx.try_recv().unwrap();
        assert!(matches!(event, ConfigChangeEvent::MonitorDeleted(ref name) if name == "del-mon"));

        // Verify gone
        let resp = send(
            app,
            Request::builder()
                .uri("/monitors/del-mon")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_nonexistent_returns_404() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .uri("/monitors/no-such-monitor")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_invalid_json_returns_400() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{invalid json"))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_monitor_summary_empty() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .uri("/monitors/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let summaries: Vec<MonitorSummary> = response_json(resp).await;
        assert!(summaries.is_empty());
    }

    #[tokio::test]
    async fn test_monitor_summary_with_results() {
        let (app, _rx, state) = setup().await;

        let result = MonitorResult {
            monitor_name: "test-mon".to_string(),
            timestamp_started: chrono::Utc::now(),
            success: true,
            step_results: vec![],
        };
        state.add_monitor_result("test-mon".to_string(), result);

        let resp = send(
            app,
            Request::builder()
                .uri("/monitors/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let summaries: Vec<MonitorSummary> = response_json(resp).await;
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "test-mon");
        assert_eq!(summaries[0].status, "OK");
    }

    #[tokio::test]
    async fn test_get_monitor_results_not_found() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .uri("/monitors/nonexistent/results")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_monitor_results_with_data() {
        let (app, _rx, state) = setup().await;

        let result = MonitorResult {
            monitor_name: "test-mon".to_string(),
            timestamp_started: chrono::Utc::now(),
            success: true,
            step_results: vec![],
        };
        state.add_monitor_result("test-mon".to_string(), result);

        let resp = send(
            app,
            Request::builder()
                .uri("/monitors/test-mon/results")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let results: Vec<MonitorResult> = response_json(resp).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }

    #[tokio::test]
    async fn test_trigger_monitor_not_found() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors/nonexistent/trigger")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
