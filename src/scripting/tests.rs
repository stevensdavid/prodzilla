use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::scripting::{engine, ScriptRunner};
use crate::scripting::types::{ScriptContext, ScriptError};

fn make_ctx() -> ScriptContext {
    ScriptContext {
        http_client: reqwest::Client::new(),
        step_results: Mutex::new(vec![]),
        assertion_failures: Mutex::new(vec![]),
        metadata: Mutex::new(HashMap::new()),
        monitor_name: "test".to_string(),
        timeout: Duration::from_secs(10),
    }
}

fn make_ctx_with_timeout(timeout: Duration) -> ScriptContext {
    ScriptContext {
        http_client: reqwest::Client::new(),
        step_results: Mutex::new(vec![]),
        assertion_failures: Mutex::new(vec![]),
        metadata: Mutex::new(HashMap::new()),
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
        ScriptError::ParseError(_) => {}
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
    let result = runner.execute(make_ctx()).await;
    assert!(result.success, "Script should succeed, got: {:?}", result.step_results);
}

#[tokio::test]
async fn test_assert_pass() {
    let runner = ScriptRunner::new(r#"assert(true, "should not fail");"#).unwrap();
    let result = runner.execute(make_ctx()).await;
    assert!(result.success);
}

#[tokio::test]
async fn test_assert_fail() {
    let runner = ScriptRunner::new(r#"assert(false, "intentional failure");"#).unwrap();
    let result = runner.execute(make_ctx()).await;
    assert!(!result.success);
}

#[tokio::test]
async fn test_assert_eq_pass() {
    let runner = ScriptRunner::new(r#"assert_eq(1, 1, "should be equal");"#).unwrap();
    let result = runner.execute(make_ctx()).await;
    assert!(result.success);
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
    let result = runner.execute(make_ctx()).await;
    assert!(result.success, "Script failed: {:?}", result.step_results);
}

#[tokio::test]
async fn test_env() {
    std::env::set_var("PRODZILLA_TEST_VAR_SCRIPTING", "hello_scripting");
    let script = r#"
        let val = env("PRODZILLA_TEST_VAR_SCRIPTING");
        assert(val == "hello_scripting", "env var should match");
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let result = runner.execute(make_ctx()).await;
    assert!(result.success, "Script failed: {:?}", result.step_results);
}

#[tokio::test]
async fn test_step_records_result() {
    let script = r#"
        step("my-step", || {
            let x = 1 + 1;
        });
    "#;
    let runner = ScriptRunner::new(script).unwrap();
    let result = runner.execute(make_ctx()).await;
    assert!(result.success, "Script should succeed: {:?}", result.step_results);
    // Should have exactly one step result named "my-step"
    assert_eq!(result.step_results.len(), 1, "Should have 1 step result");
    assert_eq!(result.step_results[0].step_name, "my-step");
    assert!(result.step_results[0].success);
}

#[tokio::test]
async fn test_full_execution() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/status"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"status": "ok"}"#),
        )
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
    let result = runner.execute(make_ctx()).await;
    assert!(result.success, "Full execution should succeed: {:?}", result.step_results);
    assert_eq!(result.step_results.len(), 1);
    assert_eq!(result.step_results[0].step_name, "check-api");
    assert!(result.step_results[0].success);
}

#[tokio::test]
async fn test_script_timeout() {
    // Rhai's max_operations will stop infinite loops before OS timeout,
    // but we use a short Duration as well to test both paths
    let runner = ScriptRunner::new("loop {}").unwrap();
    let ctx = make_ctx_with_timeout(Duration::from_millis(200));
    let result = runner.execute(ctx).await;
    // Script should fail due to resource limit or timeout
    assert!(!result.success, "Infinite loop should fail");
}
