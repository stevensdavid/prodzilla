use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::monitor::model::StepResult;

pub struct ScriptContext {
    pub http_client: reqwest::Client,
    pub step_results: Mutex<Vec<StepResult>>,
    pub log_entries: Mutex<Vec<LogEntry>>,
    pub monitor_name: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct ScriptResponse {
    pub status: i64,
    pub body: String,
    pub headers: HashMap<String, String>,
    pub duration_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptDiagnostic {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)] // variants are part of the public error API; not all are constructed yet
pub enum ScriptError {
    #[error("Script parse error: {message}")]
    ParseError {
        message: String,
        diagnostics: Vec<ScriptDiagnostic>,
    },
    #[error("Script runtime error: {0}")]
    RuntimeError(String),
    #[error("Assertion failed: {0}")]
    AssertionFailed(String),
    #[error("Script timeout after {0:?}")]
    Timeout(Duration),
    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
}
