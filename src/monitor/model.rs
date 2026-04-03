use chrono::{DateTime, Utc};

use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

// ============================================================================
// Configuration Types (used in YAML config)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputParameters {
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expectation {
    pub field: ExpectField,
    pub operation: ExpectOperation,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpectOperation {
    Equals,
    NotEquals,
    IsOneOf,
    Contains,
    NotContains,
    Matches,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpectField {
    Body,
    StatusCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleParameters {
    pub initial_delay: u32,
    pub interval: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub url: String,
    pub http_method: String,
    pub with: Option<InputParameters>,
    pub expectations: Option<Vec<Expectation>>,
    #[serde(default)] // default to false
    pub sensitive: bool,
}

/// Monitor represents a unified configuration that can be either a single-step test,
/// a multi-step test, or a scripted monitor. It must have exactly one of: `steps`,
/// root-level fields (url, http_method), or script/script_path.
#[derive(Debug, Clone, Serialize)]
pub struct Monitor {
    pub name: String,
    // Fields for single-step monitors
    pub url: Option<String>,
    pub http_method: Option<String>,
    pub with: Option<InputParameters>,
    pub expectations: Option<Vec<Expectation>>,
    #[serde(default)]
    pub sensitive: bool,
    // Fields for multi-step monitors
    pub steps: Option<Vec<Step>>,
    // Fields for scripted monitors
    pub script: Option<String>,
    #[serde(skip_serializing)]
    pub script_path: Option<String>,
    pub script_timeout_seconds: Option<u64>,
    // Common fields
    pub schedule: ScheduleParameters,
    pub alerts: Option<Vec<Alert>>,
    pub tags: Option<HashMap<String, String>>,
}

impl Monitor {
    /// Returns true if this monitor is a multi-step monitor
    pub fn is_multi_step(&self) -> bool {
        self.steps.is_some()
    }

    /// Returns true if this monitor uses inline Rhai script execution
    pub fn is_scripted(&self) -> bool {
        self.script.is_some()
    }

    /// Returns the steps to execute. For single-step monitors,
    /// returns a synthetic step from the root-level fields.
    /// Scripted monitors return an empty vec (they don't use Step-based execution).
    pub fn get_steps(&self) -> Vec<Step> {
        if let Some(steps) = &self.steps {
            steps.clone()
        } else if self.is_scripted() {
            vec![]
        } else {
            // Create synthetic step from single-step monitor fields
            vec![Step {
                name: self.name.clone(),
                url: self.url.clone().unwrap(),
                http_method: self.http_method.clone().unwrap(),
                with: self.with.clone(),
                expectations: self.expectations.clone(),
                sensitive: self.sensitive,
            }]
        }
    }

    /// Validates that step names are unique within this monitor
    fn validate_step_names(&self) -> Result<(), String> {
        if let Some(steps) = &self.steps {
            let mut seen = HashMap::new();
            for step in steps {
                if seen.insert(&step.name, ()).is_some() {
                    return Err(format!(
                        "Duplicate step name '{}' in monitor '{}'",
                        step.name, self.name
                    ));
                }
            }
        }
        Ok(())
    }
}

// Custom deserialization to validate the Monitor structure
impl<'de> Deserialize<'de> for Monitor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct MonitorHelper {
            name: String,
            url: Option<String>,
            http_method: Option<String>,
            with: Option<InputParameters>,
            expectations: Option<Vec<Expectation>>,
            #[serde(default)]
            sensitive: bool,
            steps: Option<Vec<Step>>,
            script: Option<String>,
            script_path: Option<String>,
            script_timeout_seconds: Option<u64>,
            schedule: ScheduleParameters,
            alerts: Option<Vec<Alert>>,
            tags: Option<HashMap<String, String>>,
        }

        let helper = MonitorHelper::deserialize(deserializer)?;

        let has_steps = helper.steps.is_some() && !helper.steps.as_ref().unwrap().is_empty();
        let has_monitor_fields = helper.url.is_some() || helper.http_method.is_some();
        let has_script = helper.script.is_some() || helper.script_path.is_some();

        // script and script_path are mutually exclusive
        if helper.script.is_some() && helper.script_path.is_some() {
            return Err(de::Error::custom(format!(
                "Monitor '{}' cannot have both 'script' and 'script_path'",
                helper.name
            )));
        }

        // Exactly one monitor type must be specified
        let type_count = [has_steps, has_monitor_fields, has_script]
            .iter()
            .filter(|&&x| x)
            .count();
        if type_count != 1 {
            return Err(de::Error::custom(format!(
                "Monitor '{}' must have exactly one of: steps, url+http_method, or script/script_path",
                helper.name
            )));
        }

        // For single-step monitors, require url and http_method
        if has_monitor_fields {
            if helper.url.is_none() {
                return Err(de::Error::custom(format!(
                    "Monitor '{}' is missing required field 'url'",
                    helper.name
                )));
            }
            if helper.http_method.is_none() {
                return Err(de::Error::custom(format!(
                    "Monitor '{}' is missing required field 'http_method'",
                    helper.name
                )));
            }
        }

        // For multi-step monitors, ensure steps is non-empty
        if has_steps && helper.steps.as_ref().unwrap().is_empty() {
            return Err(de::Error::custom(format!(
                "Monitor '{}' has 'steps' field but it is empty",
                helper.name
            )));
        }

        let monitor = Monitor {
            name: helper.name,
            url: helper.url,
            http_method: helper.http_method,
            with: helper.with,
            expectations: helper.expectations,
            sensitive: helper.sensitive,
            steps: helper.steps,
            script: helper.script,
            script_path: helper.script_path,
            script_timeout_seconds: helper.script_timeout_seconds,
            schedule: helper.schedule,
            alerts: helper.alerts,
            tags: helper.tags,
        };

        // Validate unique step names
        monitor.validate_step_names().map_err(de::Error::custom)?;

        Ok(monitor)
    }
}

// ============================================================================
// Result Types (runtime execution results)
// ============================================================================

/// Unified result type for monitor execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorResult {
    pub monitor_name: String,
    pub timestamp_started: DateTime<Utc>,
    pub success: bool,
    pub step_results: Vec<StepResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_name: String,
    pub timestamp_started: DateTime<Utc>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<EndpointResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
}

/// Response from an HTTP endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointResponse {
    pub timestamp_received: DateTime<Utc>,
    pub status_code: u32,
    pub body: String,
    pub sensitive: bool,
}

impl EndpointResponse {
    pub fn truncated_body(&self, n: usize) -> String {
        self.body.chars().take(n).collect()
    }
}

/// Internal result from calling an endpoint (before converting to StepResult)
pub struct EndpointResult {
    pub timestamp_request_started: DateTime<Utc>,
    pub timestamp_response_received: DateTime<Utc>,
    pub status_code: u32,
    pub body: String,
    pub trace_id: String,
    pub span_id: String,
    pub sensitive: bool,
}

impl EndpointResult {
    pub fn to_endpoint_response(&self) -> EndpointResponse {
        EndpointResponse {
            timestamp_received: self.timestamp_response_received,
            status_code: self.status_code,
            body: self.body.clone(),
            sensitive: self.sensitive,
        }
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;

    #[test]
    fn test_script_path_not_serialized() {
        // Regression: script_path is YAML-only and should not appear in API responses
        let monitor = Monitor {
            name: "test".to_string(),
            url: Some("http://example.com".to_string()),
            http_method: Some("GET".to_string()),
            with: None,
            expectations: None,
            sensitive: false,
            steps: None,
            script: None,
            script_path: Some("/path/to/script.rhai".to_string()),
            script_timeout_seconds: None,
            schedule: ScheduleParameters {
                initial_delay: 0,
                interval: 0,
            },
            alerts: None,
            tags: None,
        };
        let json = serde_json::to_value(&monitor).unwrap();
        assert!(
            json.get("script_path").is_none(),
            "script_path should not be serialized, but got: {}",
            json
        );
    }
}
