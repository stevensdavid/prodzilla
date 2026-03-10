use axum::{extract::FromRequest, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::config_store::{ConfigStoreError, StoredMonitor};
use crate::monitor::model::Monitor;

// --- Response types ---

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitorApiResponse {
    pub name: String,
    pub monitor: Monitor,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<StoredMonitor> for MonitorApiResponse {
    fn from(stored: StoredMonitor) -> Self {
        Self {
            name: stored.monitor.name.clone(),
            monitor: stored.monitor,
            version: stored.version,
            created_at: stored.created_at,
            updated_at: stored.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitorListResponse {
    pub monitors: Vec<MonitorApiResponse>,
}

// --- Request types ---

#[derive(Debug, Deserialize)]
pub struct UpdateMonitorRequest {
    pub monitor: Monitor,
    pub version: i64,
}

// --- Error type ---

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    fn with_status(self, status: StatusCode) -> (StatusCode, Json<ApiError>) {
        (status, Json(self))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.error.as_str() {
            "not_found" => StatusCode::NOT_FOUND,
            "already_exists" | "version_conflict" => StatusCode::CONFLICT,
            "validation_error" => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        self.with_status(status).into_response()
    }
}

impl From<ConfigStoreError> for ApiError {
    fn from(err: ConfigStoreError) -> Self {
        match err {
            ConfigStoreError::NotFound(name) => ApiError {
                error: "not_found".to_string(),
                message: format!("Monitor not found: {name}"),
                details: None,
            },
            ConfigStoreError::AlreadyExists(name) => ApiError {
                error: "already_exists".to_string(),
                message: format!("Monitor already exists: {name}"),
                details: None,
            },
            ConfigStoreError::VersionConflict {
                name,
                expected,
                actual,
            } => ApiError {
                error: "version_conflict".to_string(),
                message: format!(
                    "Version conflict for monitor {name}: expected {expected}, found {actual}"
                ),
                details: Some(serde_json::json!({
                    "expected_version": expected,
                    "actual_version": actual,
                })),
            },
            ConfigStoreError::Database(err) => {
                tracing::error!("Config store database error: {err}");
                ApiError {
                    error: "internal_error".to_string(),
                    message: "An internal error occurred".to_string(),
                    details: None,
                }
            }
        }
    }
}

// --- Custom JSON extractor for consistent 400 errors ---

pub struct ValidatedJson<T>(pub T);

#[axum::async_trait]
impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(ValidatedJson(value)),
            Err(rejection) => Err(ApiError {
                error: "validation_error".to_string(),
                message: rejection.body_text(),
                details: None,
            }),
        }
    }
}
