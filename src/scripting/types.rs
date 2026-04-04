use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::monitor::model::StepResult;

pub struct ScriptContext {
    pub http_client: reqwest::Client,
    pub step_results: Mutex<Vec<StepResult>>,
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

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)] // variants are part of the public error API; not all are constructed yet
pub enum ScriptError {
    #[error("Script parse error: {0}")]
    ParseError(String),
    #[error("Script runtime error: {0}")]
    RuntimeError(String),
    #[error("Assertion failed: {0}")]
    AssertionFailed(String),
    #[error("Script timeout after {0:?}")]
    Timeout(Duration),
    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
}
