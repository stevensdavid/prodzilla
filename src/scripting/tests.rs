use std::sync::Mutex;
use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::scripting::types::{ScriptContext, ScriptError};
use crate::scripting::{engine, ScriptRunner};

fn make_ctx() -> ScriptContext {
    ScriptContext {
        http_client: reqwest::Client::new(),
        step_results: Mutex::new(vec![]),
        log_entries: Mutex::new(vec![]),
        monitor_name: "test".to_string(),
        timeout: Duration::from_secs(10),
    }
}

fn make_ctx_with_timeout(timeout: Duration) -> ScriptContext {
    ScriptContext {
        http_client: reqwest::Client::new(),
        step_results: Mutex::new(vec![]),
        log_entries: Mutex::new(vec![]),
        monitor_name: "test".to_string(),
        timeout,
    }
}

#[test]
fn test_engine_creation() {
    let engine = engine::create_engine();
    // Should compile and run a simple script without panic
    let result = engine.eval::<i64>("1 + 1");
    assert_eq!(result.unwrap(), 2);
}

#[test]
fn test_script_compilation_valid() {
    let runner = ScriptRunner::new("let x = 1;");
    assert!(runner.is_ok());
}

#[test]
fn test_script_compilation_invalid() {
    let runner = ScriptRunner::new("let x = !");
    assert!(runner.is_err());
    match runner.unwrap_err() {
        ScriptError::ParseError {
            diagnostics,
            message,
        } => {
            assert!(!message.is_empty());
            assert!(!diagnostics.is_empty());
        }
        other => panic!("Expected ParseError, got: {:?}", other),
    }
}

#[test]
fn test_validate_valid() {
    let result = ScriptRunner::validate("1 + 1");
    assert!(result.is_ok());
}

#[test]
fn test_validate_invalid() {
    // Missing closing brace is a genuine parse error
    let result = ScriptRunner::validate("fn foo() {");
    assert!(result.is_err());
}

#[test]
fn test_validate_returns_position_info() {
    let result = ScriptRunner::validate("let x = !");
    match result.unwrap_err() {
        ScriptError::ParseError { diagnostics, .. } => {
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].line.is_some(), "Should have line info");
            assert_eq!(diagnostics[0].line, Some(1));
            assert!(diagnostics[0].column.is_some(), "Should have column info");
        }
        other => panic!("Expected ParseError, got: {:?}", other),
    }
}

#[test]
fn test_validate_multiline_error_position() {
    let script = "let x = 1;\nlet y = 2;\nlet z = !";
    let result = ScriptRunner::validate(script);
    match result.unwrap_err() {
        ScriptError::ParseError { diagnostics, .. } => {
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].line, Some(3), "Error should be on line 3");
        }
        other => panic!("Expected ParseError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_http_get() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&mock_server)
        .await;

    let url = format!("{}/health", mock_server.uri());
    let script = format!(
        r#"
        let resp = http_get("{url}");
        assert(resp.status == 200, "Expected 200");
        "#
    );

    let runner = ScriptRunner::new(&script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(
        output.result.success,
        "Script should succeed, got: {:?}",
        output.result.step_results
    );
}

#[tokio::test]
async fn test_assert_pass() {
    let runner = ScriptRunner::new(r#"assert(true, "should not fail");"#).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(output.result.success);
}

#[tokio::test]
async fn test_assert_fail() {
    let runner = ScriptRunner::new(r#"assert(false, "intentional failure");"#).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(!output.result.success);
}

#[tokio::test]
async fn test_assert_eq_pass() {
    let runner = ScriptRunner::new(r#"assert_eq(1, 1, "should be equal");"#).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(output.result.success);
}

#[tokio::test]
async fn test_parse_json() {
    let script = r#"
        let json_str = "{\"name\": \"alice\", \"age\": 30}";
        let obj = parse_json(json_str);
        assert(obj.name == "alice", "name should be alice");
        assert(obj.age == 30, "age should be 30");
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(
        output.result.success,
        "Script failed: {:?}",
        output.result.step_results
    );
}

#[tokio::test]
async fn test_env() {
    std::env::set_var("PRODZILLA_TEST_VAR_SCRIPTING", "hello_scripting");
    let script = r#"
        let val = env("PRODZILLA_TEST_VAR_SCRIPTING");
        assert(val == "hello_scripting", "env var should match");
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(
        output.result.success,
        "Script failed: {:?}",
        output.result.step_results
    );
}

#[tokio::test]
async fn test_step_records_result() {
    let script = r#"
        step("my-step", || {
            let x = 1 + 1;
        });
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(
        output.result.success,
        "Script should succeed: {:?}",
        output.result.step_results
    );
    // Should have exactly one step result named "my-step"
    assert_eq!(
        output.result.step_results.len(),
        1,
        "Should have 1 step result"
    );
    assert_eq!(output.result.step_results[0].step_name, "my-step");
    assert!(output.result.step_results[0].success);
}

#[tokio::test]
async fn test_full_execution() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/status"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status": "ok"}"#))
        .mount(&mock_server)
        .await;

    let url = format!("{}/api/status", mock_server.uri());
    let script = format!(
        r#"
        step("check-api", || {{
            let resp = http_get("{url}");
            assert(resp.status == 200, "API should return 200");
            let data = parse_json(resp.body);
            assert(data.status == "ok", "status should be ok");
        }});
        "#
    );

    let runner = ScriptRunner::new(&script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(
        output.result.success,
        "Full execution should succeed: {:?}",
        output.result.step_results
    );
    assert_eq!(output.result.step_results.len(), 1);
    assert_eq!(output.result.step_results[0].step_name, "check-api");
    assert!(output.result.step_results[0].success);
}

#[tokio::test]
async fn test_script_timeout() {
    // Rhai's max_operations will stop infinite loops before OS timeout,
    // but we use a short Duration as well to test both paths
    let runner = ScriptRunner::new("loop {}").unwrap();
    let ctx = make_ctx_with_timeout(Duration::from_millis(200));
    let output = runner.execute(ctx).await;
    // Script should fail due to resource limit or timeout
    assert!(!output.result.success, "Infinite loop should fail");
}

#[tokio::test]
async fn test_step_assertion_failure_preserves_clean_error_message() {
    // Regression: assertion failures inside step() closures should keep
    // the clean message, not be overwritten with Rhai-wrapped noise like
    // "Runtime error: health check failed (line 3, position 13)"
    let script = r#"
        step("check", || {
            assert(false, "health check failed");
        });
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(!output.result.success);
    assert_eq!(output.result.step_results.len(), 1);
    let error = output.result.step_results[0]
        .error_message
        .as_ref()
        .unwrap();
    assert!(
        error.contains("health check failed"),
        "Error should contain the assertion message, got: {}",
        error
    );
    assert!(
        !error.contains("position"),
        "Error should not contain Rhai position noise, got: {}",
        error
    );
}

#[tokio::test]
async fn test_step_failure_outside_step_sets_error_on_last_step() {
    // When a script fails after a successful step(), the last step should
    // get the error message since it didn't have one of its own
    let script = r#"
        step("setup", || {
            let x = 1;
        });
        assert(false, "post-step failure");
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(!output.result.success);
    let last = output.result.step_results.last().unwrap();
    assert!(!last.success);
    assert!(
        last.error_message.is_some(),
        "Last step should have an error message"
    );
}

#[tokio::test]
async fn test_log_capture_info() {
    let runner = ScriptRunner::new(r#"log_info("hello");"#).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(output.result.success);
    assert_eq!(output.logs.len(), 1);
    assert_eq!(output.logs[0].level, "info");
    assert_eq!(output.logs[0].message, "hello");
}

#[tokio::test]
async fn test_log_capture_multiple_levels() {
    let script = r#"
        log_info("info msg");
        log_warn("warn msg");
        log_debug("debug msg");
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(output.result.success);
    assert_eq!(output.logs.len(), 3);
    assert_eq!(output.logs[0].level, "info");
    assert_eq!(output.logs[0].message, "info msg");
    assert_eq!(output.logs[1].level, "warn");
    assert_eq!(output.logs[1].message, "warn msg");
    assert_eq!(output.logs[2].level, "debug");
    assert_eq!(output.logs[2].message, "debug msg");
}

#[tokio::test]
async fn test_log_capture_during_steps() {
    let script = r#"
        step("s1", || {
            log_info("inside step");
        });
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(output.result.success);
    assert_eq!(output.result.step_results.len(), 1);
    assert_eq!(output.logs.len(), 1);
    assert_eq!(output.logs[0].message, "inside step");
}

#[tokio::test]
async fn test_step_with_own_error_not_overwritten_by_script_error() {
    // A step that fails with its own error, followed by another failure.
    // The step's original error message should be preserved.
    let script = r#"
        step("check", || {
            assert(false, "step-level failure");
        });
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let output = runner.execute(make_ctx()).await;
    assert!(!output.result.success);
    let last = output.result.step_results.last().unwrap();
    assert!(!last.success);
    let error = last.error_message.as_ref().unwrap();
    assert_eq!(
        error, "step-level failure",
        "Step's own error should be preserved, got: {}",
        error
    );
}
