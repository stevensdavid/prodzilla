use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use std::sync::Arc;
use tracing::debug;

use crate::app_state::AppState;
use crate::monitor::manager::ConfigChangeEvent;
use crate::monitor::model::Monitor;

use super::model::{
    ApiError, MonitorApiResponse, MonitorListResponse, UpdateMonitorRequest, ValidatedJson,
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
    let _ = state
        .change_tx
        .send(ConfigChangeEvent::MonitorCreated(name.clone()));

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
    let _ = state
        .change_tx
        .send(ConfigChangeEvent::MonitorUpdated(name));

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
    let _ = state
        .change_tx
        .send(ConfigChangeEvent::MonitorDeleted(name));

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::config::Config;
    use crate::config_store::sqlite::SqliteConfigStore;
    use crate::monitor::model::{Monitor, ScheduleParameters};
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

    async fn setup() -> (axum::Router, broadcast::Receiver<ConfigChangeEvent>) {
        let store = Arc::new(SqliteConfigStore::new_in_memory().await.unwrap());
        let config = Config { monitors: vec![] };
        let (change_tx, change_rx) = broadcast::channel(16);
        let app_state = Arc::new(AppState::with_store(config, store, change_tx));

        let app = crate::web_server::api_v1::router().layer(Extension(app_state));

        (app, change_rx)
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
        let (app, _rx) = setup().await;

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
        let (app, mut rx) = setup().await;

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
        let (app, _rx) = setup().await;

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
        let (app, mut rx) = setup().await;

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
        let (app, _rx) = setup().await;

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
        let (app, _rx) = setup().await;

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
        let (app, mut rx) = setup().await;

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
        let (app, _rx) = setup().await;

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
        let (app, _rx) = setup().await;

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
}
