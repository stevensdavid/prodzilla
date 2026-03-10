use std::collections::HashMap;

use crate::config_store::{ConfigStore, ConfigStoreError};
use crate::monitor::model::{
    Alert, ExpectField, ExpectOperation, Expectation, InputParameters, Monitor, ScheduleParameters,
    Step,
};

/// Create a simple single-step test monitor with the given name and URL.
pub fn make_test_monitor(name: &str, url: &str) -> Monitor {
    Monitor {
        name: name.to_string(),
        url: Some(url.to_string()),
        http_method: Some("GET".to_string()),
        with: None,
        expectations: Some(vec![Expectation {
            field: ExpectField::StatusCode,
            operation: ExpectOperation::Equals,
            value: "200".to_string(),
        }]),
        schedule: ScheduleParameters {
            initial_delay: 0,
            interval: 60,
        },
        alerts: None,
        tags: None,
        sensitive: false,
        steps: None,
    }
}

/// Create a multi-step test monitor with full optional fields populated.
pub fn make_full_test_monitor(name: &str) -> Monitor {
    Monitor {
        name: name.to_string(),
        url: None,
        http_method: None,
        with: None,
        expectations: None,
        schedule: ScheduleParameters {
            initial_delay: 5,
            interval: 120,
        },
        alerts: Some(vec![Alert {
            url: "https://hooks.slack.com/test".to_string(),
        }]),
        tags: Some(HashMap::from([
            ("env".to_string(), "prod".to_string()),
            ("team".to_string(), "platform".to_string()),
        ])),
        sensitive: true,
        steps: Some(vec![
            Step {
                name: "get-token".to_string(),
                url: "https://auth.example.com/token".to_string(),
                http_method: "POST".to_string(),
                with: Some(InputParameters {
                    headers: Some(HashMap::from([(
                        "Content-Type".to_string(),
                        "application/json".to_string(),
                    )])),
                    body: Some(r#"{"grant_type": "client_credentials"}"#.to_string()),
                    timeout_seconds: Some(5),
                }),
                expectations: Some(vec![Expectation {
                    field: ExpectField::StatusCode,
                    operation: ExpectOperation::Equals,
                    value: "200".to_string(),
                }]),
                sensitive: true,
            },
            Step {
                name: "check-api".to_string(),
                url: "https://api.example.com/health".to_string(),
                http_method: "GET".to_string(),
                with: Some(InputParameters {
                    headers: Some(HashMap::from([(
                        "Authorization".to_string(),
                        "Bearer ${{steps.get-token.response.body.token}}".to_string(),
                    )])),
                    body: None,
                    timeout_seconds: Some(10),
                }),
                expectations: Some(vec![
                    Expectation {
                        field: ExpectField::StatusCode,
                        operation: ExpectOperation::Equals,
                        value: "200".to_string(),
                    },
                    Expectation {
                        field: ExpectField::Body,
                        operation: ExpectOperation::Contains,
                        value: "healthy".to_string(),
                    },
                ]),
                sensitive: false,
            },
        ]),
    }
}

// ============================================================================
// Reusable test functions — run against any ConfigStore implementation
// ============================================================================

pub async fn test_create_and_get(store: &dyn ConfigStore) {
    let monitor = make_test_monitor("test-monitor-1", "https://example.com/health");
    let stored = store.create_monitor(&monitor).await.unwrap();
    assert_eq!(stored.version, 1);
    assert_eq!(stored.monitor.name, "test-monitor-1");

    let fetched = store.get_monitor("test-monitor-1").await.unwrap();
    assert_eq!(fetched.monitor.name, "test-monitor-1");
    assert_eq!(fetched.version, 1);
    assert_eq!(
        fetched.monitor.url.as_deref(),
        Some("https://example.com/health")
    );
}

pub async fn test_create_duplicate_fails(store: &dyn ConfigStore) {
    let monitor = make_test_monitor("dup-monitor", "https://example.com");
    store.create_monitor(&monitor).await.unwrap();

    let result = store.create_monitor(&monitor).await;
    assert!(matches!(result, Err(ConfigStoreError::AlreadyExists(_))));
}

pub async fn test_list_monitors(store: &dyn ConfigStore) {
    let m1 = make_test_monitor("list-1", "https://example.com/1");
    let m2 = make_test_monitor("list-2", "https://example.com/2");
    let m3 = make_test_monitor("list-3", "https://example.com/3");

    store.create_monitor(&m1).await.unwrap();
    store.create_monitor(&m2).await.unwrap();
    store.create_monitor(&m3).await.unwrap();

    let monitors = store.list_monitors().await.unwrap();
    assert_eq!(monitors.len(), 3);

    let names: Vec<&str> = monitors.iter().map(|m| m.monitor.name.as_str()).collect();
    assert!(names.contains(&"list-1"));
    assert!(names.contains(&"list-2"));
    assert!(names.contains(&"list-3"));
}

pub async fn test_list_empty(store: &dyn ConfigStore) {
    let monitors = store.list_monitors().await.unwrap();
    assert!(monitors.is_empty());
}

pub async fn test_update_increments_version(store: &dyn ConfigStore) {
    let monitor = make_test_monitor("update-test", "https://example.com/old");
    store.create_monitor(&monitor).await.unwrap();

    let mut updated = monitor.clone();
    updated.schedule.interval = 120;
    updated.url = Some("https://example.com/new".to_string());

    let stored = store
        .update_monitor("update-test", &updated, 1)
        .await
        .unwrap();
    assert_eq!(stored.version, 2);
    assert_eq!(
        stored.monitor.url.as_deref(),
        Some("https://example.com/new")
    );
    assert_eq!(stored.monitor.schedule.interval, 120);
}

pub async fn test_update_version_conflict(store: &dyn ConfigStore) {
    let monitor = make_test_monitor("conflict-test", "https://example.com");
    store.create_monitor(&monitor).await.unwrap();

    let result = store.update_monitor("conflict-test", &monitor, 99).await;
    assert!(matches!(
        result,
        Err(ConfigStoreError::VersionConflict { .. })
    ));
}

pub async fn test_update_nonexistent_fails(store: &dyn ConfigStore) {
    let monitor = make_test_monitor("ghost", "https://example.com");
    let result = store.update_monitor("ghost", &monitor, 1).await;
    assert!(matches!(result, Err(ConfigStoreError::NotFound(_))));
}

pub async fn test_delete_monitor(store: &dyn ConfigStore) {
    let monitor = make_test_monitor("delete-me", "https://example.com");
    store.create_monitor(&monitor).await.unwrap();

    store.delete_monitor("delete-me").await.unwrap();

    let result = store.get_monitor("delete-me").await;
    assert!(matches!(result, Err(ConfigStoreError::NotFound(_))));
}

pub async fn test_delete_nonexistent_fails(store: &dyn ConfigStore) {
    let result = store.delete_monitor("nonexistent").await;
    assert!(matches!(result, Err(ConfigStoreError::NotFound(_))));
}

pub async fn test_fingerprint_changes_on_mutation(store: &dyn ConfigStore) {
    let fp_empty = store.get_config_fingerprint().await.unwrap();

    let monitor = make_test_monitor("fp-test", "https://example.com");
    store.create_monitor(&monitor).await.unwrap();
    let fp_after_create = store.get_config_fingerprint().await.unwrap();
    assert_ne!(fp_empty, fp_after_create);

    let updated = make_test_monitor("fp-test", "https://example.com/updated");
    store.update_monitor("fp-test", &updated, 1).await.unwrap();
    let fp_after_update = store.get_config_fingerprint().await.unwrap();
    assert_ne!(fp_after_create, fp_after_update);

    store.delete_monitor("fp-test").await.unwrap();
    let fp_after_delete = store.get_config_fingerprint().await.unwrap();
    assert_ne!(fp_after_update, fp_after_delete);
    assert_eq!(fp_empty, fp_after_delete);
}

pub async fn test_json_roundtrip_single_step(store: &dyn ConfigStore) {
    let monitor = make_test_monitor("roundtrip-single", "https://example.com/api");
    store.create_monitor(&monitor).await.unwrap();

    let fetched = store.get_monitor("roundtrip-single").await.unwrap();
    assert_eq!(fetched.monitor.name, "roundtrip-single");
    assert_eq!(
        fetched.monitor.url.as_deref(),
        Some("https://example.com/api")
    );
    assert_eq!(fetched.monitor.http_method.as_deref(), Some("GET"));
    assert!(fetched.monitor.steps.is_none());
    assert!(!fetched.monitor.is_multi_step());

    let expectations = fetched.monitor.expectations.as_ref().unwrap();
    assert_eq!(expectations.len(), 1);
    assert!(matches!(expectations[0].field, ExpectField::StatusCode));
    assert!(matches!(expectations[0].operation, ExpectOperation::Equals));
}

pub async fn test_json_roundtrip_multi_step(store: &dyn ConfigStore) {
    let monitor = make_full_test_monitor("roundtrip-multi");
    store.create_monitor(&monitor).await.unwrap();

    let fetched = store.get_monitor("roundtrip-multi").await.unwrap();
    assert_eq!(fetched.monitor.name, "roundtrip-multi");
    assert!(fetched.monitor.is_multi_step());
    assert!(fetched.monitor.sensitive);

    let steps = fetched.monitor.steps.as_ref().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].name, "get-token");
    assert_eq!(steps[1].name, "check-api");
    assert!(steps[0].sensitive);
    assert!(!steps[1].sensitive);

    // Verify headers survived roundtrip
    let step0_headers = steps[0].with.as_ref().unwrap().headers.as_ref().unwrap();
    assert_eq!(
        step0_headers.get("Content-Type").unwrap(),
        "application/json"
    );

    let step1_headers = steps[1].with.as_ref().unwrap().headers.as_ref().unwrap();
    assert!(step1_headers
        .get("Authorization")
        .unwrap()
        .contains("steps.get-token"));

    // Verify alerts and tags
    let alerts = fetched.monitor.alerts.as_ref().unwrap();
    assert_eq!(alerts.len(), 1);
    assert!(alerts[0].url.contains("slack"));

    let tags = fetched.monitor.tags.as_ref().unwrap();
    assert_eq!(tags.get("env").unwrap(), "prod");
    assert_eq!(tags.get("team").unwrap(), "platform");

    // Verify schedule
    assert_eq!(fetched.monitor.schedule.initial_delay, 5);
    assert_eq!(fetched.monitor.schedule.interval, 120);
}

pub async fn test_json_roundtrip_with_all_fields(store: &dyn ConfigStore) {
    // Single-step monitor with all optional fields populated
    let monitor = Monitor {
        name: "roundtrip-all-fields".to_string(),
        url: Some("https://api.example.com/v1/data".to_string()),
        http_method: Some("POST".to_string()),
        with: Some(InputParameters {
            headers: Some(HashMap::from([
                ("Content-Type".to_string(), "application/json".to_string()),
                ("X-Custom-Header".to_string(), "value".to_string()),
            ])),
            body: Some(r#"{"key": "value"}"#.to_string()),
            timeout_seconds: Some(30),
        }),
        expectations: Some(vec![
            Expectation {
                field: ExpectField::StatusCode,
                operation: ExpectOperation::Equals,
                value: "200".to_string(),
            },
            Expectation {
                field: ExpectField::Body,
                operation: ExpectOperation::Contains,
                value: "success".to_string(),
            },
            Expectation {
                field: ExpectField::Body,
                operation: ExpectOperation::Matches,
                value: r#"^\{"result":.*\}$"#.to_string(),
            },
        ]),
        schedule: ScheduleParameters {
            initial_delay: 10,
            interval: 300,
        },
        alerts: Some(vec![
            Alert {
                url: "https://hooks.slack.com/test".to_string(),
            },
            Alert {
                url: "https://webhook.example.com/alert".to_string(),
            },
        ]),
        tags: Some(HashMap::from([
            ("env".to_string(), "staging".to_string()),
            ("priority".to_string(), "high".to_string()),
        ])),
        sensitive: false,
        steps: None,
    };

    store.create_monitor(&monitor).await.unwrap();
    let fetched = store.get_monitor("roundtrip-all-fields").await.unwrap();

    // Verify input parameters
    let with = fetched.monitor.with.as_ref().unwrap();
    assert_eq!(with.timeout_seconds, Some(30));
    assert_eq!(with.body.as_deref(), Some(r#"{"key": "value"}"#));
    let headers = with.headers.as_ref().unwrap();
    assert_eq!(headers.len(), 2);

    // Verify expectations
    let expectations = fetched.monitor.expectations.as_ref().unwrap();
    assert_eq!(expectations.len(), 3);
    assert!(matches!(
        expectations[2].operation,
        ExpectOperation::Matches
    ));

    // Verify alerts
    let alerts = fetched.monitor.alerts.as_ref().unwrap();
    assert_eq!(alerts.len(), 2);
}
