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
use crate::scripting::ScriptRunner;

use super::model::{
    ApiError, CompletionsResponse, DiagnosticDto, ExecuteScriptRequest, ExecuteScriptResponse,
    MonitorApiResponse, MonitorListResponse, MonitorQueryParams, MonitorSummary,
    ScriptingCompletionItem, UpdateMonitorRequest, ValidateScriptRequest, ValidateScriptResponse,
    ValidatedJson,
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

fn validate_script(monitor: &Monitor) -> Result<(), ApiError> {
    if monitor.script_path.is_some() {
        return Err(ApiError {
            error: "validation_error".to_string(),
            message:
                "script_path is only valid in YAML config files. Use 'script' with inline content."
                    .to_string(),
            details: None,
        });
    }
    if let Some(script) = &monitor.script {
        ScriptRunner::validate(script).map_err(|e| match &e {
            crate::scripting::types::ScriptError::ParseError { diagnostics, .. } => ApiError {
                error: "validation_error".to_string(),
                message: format!("Script compilation failed: {}", e),
                details: Some(serde_json::to_value(diagnostics).unwrap()),
            },
            _ => ApiError {
                error: "validation_error".to_string(),
                message: format!("Script error: {}", e),
                details: None,
            },
        })?;
    }
    Ok(())
}

pub async fn create_monitor(
    Extension(state): Extension<Arc<AppState>>,
    ValidatedJson(monitor): ValidatedJson<Monitor>,
) -> Result<impl IntoResponse, ApiError> {
    debug!("API: create monitor {}", monitor.name);

    validate_script(&monitor)?;

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

    validate_script(&req.monitor)?;

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
        let result = monitor.probe_and_store_result(state.clone()).await;
        return Ok(Json(serde_json::to_value(result).unwrap()));
    }

    Err(ApiError {
        error: "not_found".to_string(),
        message: format!("Monitor not found: {name}"),
        details: None,
    })
}

// --- Scripting endpoints ---

fn build_completions() -> CompletionsResponse {
    let items = vec![
        ScriptingCompletionItem {
            label: "http_get".to_string(),
            kind: "function".to_string(),
            detail: "http_get(url: String) -> ScriptResponse".to_string(),
            documentation: Some("Perform an HTTP GET request.\n\nReturns a `ScriptResponse` with `.status` (i64), `.body` (String), `.headers` (Map), `.duration_ms` (i64).".to_string()),
            insert_text: Some("http_get(\"${1:url}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "http_post".to_string(),
            kind: "function".to_string(),
            detail: "http_post(url: String, body: String) -> ScriptResponse".to_string(),
            documentation: Some("Perform an HTTP POST request with a body.\n\nReturns a `ScriptResponse` with `.status` (i64), `.body` (String), `.headers` (Map), `.duration_ms` (i64).".to_string()),
            insert_text: Some("http_post(\"${1:url}\", \"${2:body}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "http_put".to_string(),
            kind: "function".to_string(),
            detail: "http_put(url: String, body: String) -> ScriptResponse".to_string(),
            documentation: Some("Perform an HTTP PUT request with a body.\n\nReturns a `ScriptResponse` with `.status` (i64), `.body` (String), `.headers` (Map), `.duration_ms` (i64).".to_string()),
            insert_text: Some("http_put(\"${1:url}\", \"${2:body}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "http_delete".to_string(),
            kind: "function".to_string(),
            detail: "http_delete(url: String) -> ScriptResponse".to_string(),
            documentation: Some("Perform an HTTP DELETE request.\n\nReturns a `ScriptResponse` with `.status` (i64), `.body` (String), `.headers` (Map), `.duration_ms` (i64).".to_string()),
            insert_text: Some("http_delete(\"${1:url}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "http_request".to_string(),
            kind: "function".to_string(),
            detail: "http_request(options: Map) -> ScriptResponse".to_string(),
            documentation: Some("Perform a fully configurable HTTP request.\n\nOptions map keys: `url` (String, required), `method` (String, required), `body` (String, optional), `headers` (Map, optional).\n\nReturns a `ScriptResponse` with `.status` (i64), `.body` (String), `.headers` (Map), `.duration_ms` (i64).".to_string()),
            insert_text: Some("http_request(#{url: \"${1:url}\", method: \"${2:GET}\"})".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "assert".to_string(),
            kind: "function".to_string(),
            detail: "assert(condition: bool, message: String)".to_string(),
            documentation: Some("Assert that a condition is true. If false, the script fails with the given message.".to_string()),
            insert_text: Some("assert(${1:condition}, \"${2:message}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "assert_eq".to_string(),
            kind: "function".to_string(),
            detail: "assert_eq(actual: Dynamic, expected: Dynamic, message: String)".to_string(),
            documentation: Some("Assert that two values are equal. If not, the script fails with the given message.".to_string()),
            insert_text: Some("assert_eq(${1:actual}, ${2:expected}, \"${3:message}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "parse_json".to_string(),
            kind: "function".to_string(),
            detail: "parse_json(text: String) -> Dynamic".to_string(),
            documentation: Some("Parse a JSON string into a Rhai object (Map or Array).".to_string()),
            insert_text: Some("parse_json(${1:text})".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "to_json".to_string(),
            kind: "function".to_string(),
            detail: "to_json(value: Dynamic) -> String".to_string(),
            documentation: Some("Serialize a Rhai value to a JSON string.".to_string()),
            insert_text: Some("to_json(${1:value})".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "env".to_string(),
            kind: "function".to_string(),
            detail: "env(name: String) -> String".to_string(),
            documentation: Some("Read an environment variable. Returns an empty string if not set.".to_string()),
            insert_text: Some("env(\"${1:VAR_NAME}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "uuid".to_string(),
            kind: "function".to_string(),
            detail: "uuid() -> String".to_string(),
            documentation: Some("Generate a random UUID v4 string.".to_string()),
            insert_text: Some("uuid()".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "timestamp".to_string(),
            kind: "function".to_string(),
            detail: "timestamp() -> String".to_string(),
            documentation: Some("Get the current UTC time as an RFC 3339 / ISO 8601 string.".to_string()),
            insert_text: Some("timestamp()".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "timestamp_epoch".to_string(),
            kind: "function".to_string(),
            detail: "timestamp_epoch() -> i64".to_string(),
            documentation: Some("Get the current UTC time as a Unix epoch timestamp (seconds).".to_string()),
            insert_text: Some("timestamp_epoch()".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "log_info".to_string(),
            kind: "function".to_string(),
            detail: "log_info(message: String)".to_string(),
            documentation: Some("Log a message at INFO level.".to_string()),
            insert_text: Some("log_info(\"${1:message}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "log_warn".to_string(),
            kind: "function".to_string(),
            detail: "log_warn(message: String)".to_string(),
            documentation: Some("Log a message at WARN level.".to_string()),
            insert_text: Some("log_warn(\"${1:message}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "log_debug".to_string(),
            kind: "function".to_string(),
            detail: "log_debug(message: String)".to_string(),
            documentation: Some("Log a message at DEBUG level.".to_string()),
            insert_text: Some("log_debug(\"${1:message}\")".to_string()),
            insert_text_rules: Some(4),
        },
        ScriptingCompletionItem {
            label: "step".to_string(),
            kind: "function".to_string(),
            detail: "step(name: String, body: FnPtr) -> Dynamic".to_string(),
            documentation: Some("Execute a named step. The closure is executed and its result is tracked.\n\nStep results (pass/fail, error messages) are automatically recorded.\n\n```rhai\nstep(\"check-api\", || {\n    let resp = http_get(\"https://api.example.com/health\");\n    assert(resp.status == 200, \"health check failed\");\n});\n```".to_string()),
            insert_text: Some("step(\"${1:name}\", || {\n\t${2}\n})".to_string()),
            insert_text_rules: Some(4),
        },
        // ScriptResponse properties
        ScriptingCompletionItem {
            label: "status".to_string(),
            kind: "property".to_string(),
            detail: "ScriptResponse.status: i64".to_string(),
            documentation: Some("HTTP status code of the response (e.g. 200, 404, 500).".to_string()),
            insert_text: None,
            insert_text_rules: None,
        },
        ScriptingCompletionItem {
            label: "body".to_string(),
            kind: "property".to_string(),
            detail: "ScriptResponse.body: String".to_string(),
            documentation: Some("Response body as a string. Use `parse_json(resp.body)` to parse JSON responses.".to_string()),
            insert_text: None,
            insert_text_rules: None,
        },
        ScriptingCompletionItem {
            label: "headers".to_string(),
            kind: "property".to_string(),
            detail: "ScriptResponse.headers: Map".to_string(),
            documentation: Some("Response headers as a key-value map.".to_string()),
            insert_text: None,
            insert_text_rules: None,
        },
        ScriptingCompletionItem {
            label: "duration_ms".to_string(),
            kind: "property".to_string(),
            detail: "ScriptResponse.duration_ms: i64".to_string(),
            documentation: Some("Time taken for the HTTP request in milliseconds.".to_string()),
            insert_text: None,
            insert_text_rules: None,
        },
    ];
    CompletionsResponse { items }
}

static COMPLETIONS: std::sync::OnceLock<CompletionsResponse> = std::sync::OnceLock::new();

pub async fn scripting_completions() -> Json<CompletionsResponse> {
    let completions = COMPLETIONS.get_or_init(build_completions);
    Json(CompletionsResponse {
        items: completions.items.clone(),
    })
}

pub async fn scripting_validate(
    ValidatedJson(req): ValidatedJson<ValidateScriptRequest>,
) -> Json<ValidateScriptResponse> {
    match ScriptRunner::validate(&req.script) {
        Ok(()) => Json(ValidateScriptResponse {
            valid: true,
            diagnostics: vec![],
        }),
        Err(crate::scripting::types::ScriptError::ParseError { diagnostics, .. }) => {
            let dto_diagnostics = diagnostics
                .into_iter()
                .map(|d| {
                    let line = d.line.unwrap_or(1);
                    let col = d.column.unwrap_or(1);
                    DiagnosticDto {
                        start_line: line,
                        start_column: col,
                        end_line: line,
                        end_column: col + 1,
                        message: d.message,
                        severity: 8, // Monaco MarkerSeverity.Error
                    }
                })
                .collect();
            Json(ValidateScriptResponse {
                valid: false,
                diagnostics: dto_diagnostics,
            })
        }
        Err(e) => Json(ValidateScriptResponse {
            valid: false,
            diagnostics: vec![DiagnosticDto {
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 2,
                message: e.to_string(),
                severity: 8,
            }],
        }),
    }
}

pub async fn scripting_execute(
    ValidatedJson(req): ValidatedJson<ExecuteScriptRequest>,
) -> Result<Json<ExecuteScriptResponse>, ApiError> {
    use std::sync::Mutex;
    use std::time::Duration;

    use crate::monitor::http::get_client;
    use crate::scripting::types::ScriptContext;

    let timeout_secs = req.timeout_seconds.unwrap_or(30).min(120);

    let runner = ScriptRunner::new(&req.script).map_err(|e| ApiError {
        error: "validation_error".to_string(),
        message: format!("Script compilation failed: {}", e),
        details: None,
    })?;

    let ctx = ScriptContext {
        http_client: get_client().clone(),
        step_results: Mutex::new(Vec::new()),
        log_entries: Mutex::new(Vec::new()),
        monitor_name: "dry-run".to_string(),
        timeout: Duration::from_secs(timeout_secs),
    };

    let output = runner.execute(ctx).await;

    Ok(Json(ExecuteScriptResponse {
        result: output.result,
        logs: output.logs,
    }))
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
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
            script: None,
            script_path: None,
            script_timeout_seconds: None,
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

    // --- Scripting endpoint tests ---

    use crate::web_server::api_v1::model::{
        CompletionsResponse, ExecuteScriptResponse, ValidateScriptResponse,
    };

    #[tokio::test]
    async fn test_scripting_completions_returns_items() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .uri("/scripting/completions")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let completions: CompletionsResponse = response_json(resp).await;
        assert!(!completions.items.is_empty());

        let http_get_item = completions.items.iter().find(|i| i.label == "http_get");
        assert!(http_get_item.is_some(), "Should contain http_get");
        let item = http_get_item.unwrap();
        assert!(item.detail.contains("ScriptResponse"));
        assert!(item.insert_text.is_some());
    }

    #[tokio::test]
    async fn test_scripting_completions_includes_all_functions() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .uri("/scripting/completions")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        let completions: CompletionsResponse = response_json(resp).await;
        let labels: Vec<&str> = completions.items.iter().map(|i| i.label.as_str()).collect();

        // All 17 host functions
        for name in &[
            "http_get",
            "http_post",
            "http_put",
            "http_delete",
            "http_request",
            "assert",
            "assert_eq",
            "parse_json",
            "to_json",
            "env",
            "uuid",
            "timestamp",
            "timestamp_epoch",
            "log_info",
            "log_warn",
            "log_debug",
            "step",
        ] {
            assert!(labels.contains(name), "Missing completion for: {}", name);
        }

        // ScriptResponse properties
        for prop in &["status", "body", "headers", "duration_ms"] {
            assert!(
                labels.contains(prop),
                "Missing property completion for: {}",
                prop
            );
        }
    }

    #[tokio::test]
    async fn test_scripting_validate_valid_script() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"script":"let x = 1;"}"#))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ValidateScriptResponse = response_json(resp).await;
        assert!(result.valid);
        assert!(result.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn test_scripting_validate_invalid_script() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"script":"let x = !"}"#))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ValidateScriptResponse = response_json(resp).await;
        assert!(!result.valid);
        assert!(!result.diagnostics.is_empty());
        assert_eq!(result.diagnostics[0].start_line, 1);
    }

    #[tokio::test]
    async fn test_scripting_validate_multiline_error() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"script":"let x = 1;\nlet y = 2;\nlet z = !"}"#,
                ))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ValidateScriptResponse = response_json(resp).await;
        assert!(!result.valid);
        assert_eq!(result.diagnostics[0].start_line, 3);
    }

    #[tokio::test]
    async fn test_scripting_validate_empty_script() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"script":""}"#))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ValidateScriptResponse = response_json(resp).await;
        assert!(result.valid);
    }

    #[tokio::test]
    async fn test_scripting_execute_simple() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"script":"let x = 1 + 1;"}"#))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ExecuteScriptResponse = response_json(resp).await;
        assert!(result.result.success);
        assert!(result.logs.is_empty());
    }

    #[tokio::test]
    async fn test_scripting_execute_failing_assertion() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"script":"assert(false, \"boom\");"}"#))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ExecuteScriptResponse = response_json(resp).await;
        assert!(!result.result.success);
        let error = result.result.step_results[0]
            .error_message
            .as_ref()
            .unwrap();
        assert!(error.contains("boom"));
    }

    #[tokio::test]
    async fn test_scripting_execute_with_steps() {
        let (app, _rx, _state) = setup().await;

        let body = serde_json::json!({
            "script": "step(\"s1\", || { let x = 1; });"
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ExecuteScriptResponse = response_json(resp).await;
        assert!(result.result.success);
        assert_eq!(result.result.step_results[0].step_name, "s1");
    }

    #[tokio::test]
    async fn test_scripting_execute_with_log_capture() {
        let (app, _rx, _state) = setup().await;

        let body = serde_json::json!({
            "script": "log_info(\"hello\"); log_warn(\"uh oh\");"
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ExecuteScriptResponse = response_json(resp).await;
        assert!(result.result.success);
        assert_eq!(result.logs.len(), 2);
        assert_eq!(result.logs[0].level, "info");
        assert_eq!(result.logs[0].message, "hello");
        assert_eq!(result.logs[1].level, "warn");
        assert_eq!(result.logs[1].message, "uh oh");
    }

    #[tokio::test]
    async fn test_scripting_execute_compile_error() {
        let (app, _rx, _state) = setup().await;

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"script":"let x = !"}"#))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_scripting_execute_timeout() {
        let (app, _rx, _state) = setup().await;

        let body = serde_json::json!({
            "script": "loop {}",
            "timeout_seconds": 1
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ExecuteScriptResponse = response_json(resp).await;
        assert!(!result.result.success);
    }

    #[tokio::test]
    async fn test_scripting_execute_uses_shared_http_client() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("user-agent", "Prodzilla Probe/1.0"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let (app, _rx, _state) = setup().await;
        let script = format!(
            r#"let resp = http_get("{}"); assert(resp.status == 200, "expected 200");"#,
            mock_server.uri()
        );
        let body = serde_json::json!({
            "script": script,
            "timeout_seconds": 10
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/scripting/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let result: ExecuteScriptResponse = response_json(resp).await;
        assert!(
            result.result.success,
            "Script should succeed; step errors: {:?}",
            result
                .result
                .step_results
                .iter()
                .filter_map(|s| s.error_message.as_ref())
                .collect::<Vec<_>>()
        );
        // wiremock .expect(1) verifies the request had the correct user-agent
    }

    #[tokio::test]
    async fn test_create_monitor_with_script() {
        let (app, _rx, _state) = setup().await;

        let body = serde_json::json!({
            "name": "scripted-mon",
            "script": "let x = 1;",
            "script_timeout_seconds": 45,
            "schedule": { "initial_delay": 0, "interval": 60 }
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::CREATED);
        let created: MonitorApiResponse = response_json(resp).await;
        assert_eq!(created.monitor.script.as_deref(), Some("let x = 1;"));
        assert_eq!(created.monitor.script_timeout_seconds, Some(45));
    }

    #[tokio::test]
    async fn test_create_monitor_invalid_script_returns_diagnostics() {
        let (app, _rx, _state) = setup().await;

        let body = serde_json::json!({
            "name": "bad-script-mon",
            "script": "let x = !",
            "schedule": { "initial_delay": 0, "interval": 60 }
        });

        let resp = send(
            app,
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let error: ApiError = response_json(resp).await;
        assert_eq!(error.error, "validation_error");
        assert!(
            error.details.is_some(),
            "Should include diagnostics in details"
        );
        let details = error.details.unwrap();
        assert!(details.is_array());
        let diag = &details[0];
        assert!(diag.get("line").is_some());
    }

    #[tokio::test]
    async fn test_get_scripted_monitor_roundtrip() {
        let (app, _rx, _state) = setup().await;

        let body = serde_json::json!({
            "name": "roundtrip-script",
            "script": "let x = 1 + 1;",
            "script_timeout_seconds": 30,
            "schedule": { "initial_delay": 0, "interval": 120 }
        });

        // Create
        let resp = send(
            app.clone(),
            Request::builder()
                .method(http::Method::POST)
                .uri("/monitors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Get
        let resp = send(
            app,
            Request::builder()
                .uri("/monitors/roundtrip-script")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let fetched: MonitorApiResponse = response_json(resp).await;
        assert_eq!(fetched.monitor.script.as_deref(), Some("let x = 1 + 1;"));
        assert_eq!(fetched.monitor.script_timeout_seconds, Some(30));
        assert!(fetched.monitor.url.is_none());
        assert!(fetched.monitor.steps.is_none());
    }
}
