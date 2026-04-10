pub mod engine;
pub mod host_functions;
pub mod types;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use chrono::Utc;

use crate::monitor::model::{MonitorResult, StepResult};
use types::{LogEntry, ScriptContext, ScriptDiagnostic, ScriptError};

/// Full output from script execution, including logs captured during the run.
pub struct ExecutionOutput {
    pub result: MonitorResult,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug)]
pub struct ScriptRunner {
    ast: rhai::AST,
}

fn parse_error_to_script_error(e: rhai::ParseError) -> ScriptError {
    let line = e.1.line();
    let column = e.1.position();
    let message = e.to_string();
    let diagnostic_message = e.0.to_string();
    ScriptError::ParseError {
        message,
        diagnostics: vec![ScriptDiagnostic {
            line,
            column,
            message: diagnostic_message,
        }],
    }
}

impl ScriptRunner {
    pub fn new(script_source: &str) -> Result<Self, ScriptError> {
        let engine = engine::create_engine();
        let ast = engine
            .compile(script_source)
            .map_err(parse_error_to_script_error)?;
        Ok(Self { ast })
    }

    pub fn validate(script_source: &str) -> Result<(), ScriptError> {
        let engine = engine::create_engine();
        engine
            .compile(script_source)
            .map_err(parse_error_to_script_error)?;
        Ok(())
    }

    pub async fn execute(&self, ctx: ScriptContext) -> ExecutionOutput {
        let monitor_name = ctx.monitor_name.clone();
        let timeout = ctx.timeout;
        let timestamp_started = Utc::now();

        let ast = self.ast.clone();
        let ctx_arc = Arc::new(ctx);

        let result = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking({
                let ctx_arc = Arc::clone(&ctx_arc);
                move || {
                    let mut engine = engine::create_engine();
                    host_functions::http::register(&mut engine, Arc::clone(&ctx_arc));
                    host_functions::assertions::register(&mut engine);
                    host_functions::parsing::register(&mut engine);
                    host_functions::utilities::register(&mut engine);
                    host_functions::logging::register(&mut engine, Arc::clone(&ctx_arc));
                    host_functions::steps::register(&mut engine, Arc::clone(&ctx_arc));

                    engine.run_ast(&ast)
                }
            }),
        )
        .await;

        let (success, error_message) = match result {
            Ok(Ok(Ok(()))) => (true, None),
            Ok(Ok(Err(e))) => (false, Some(e.to_string())),
            Ok(Err(join_err)) => (
                false,
                Some(format!("Script execution panicked: {}", join_err)),
            ),
            Err(_) => (false, Some(format!("Script timed out after {:?}", timeout))),
        };

        let mut step_results = ctx_arc
            .step_results
            .lock()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>();

        // If no step results were recorded, add a synthetic one representing the whole script
        if step_results.is_empty() {
            step_results.push(StepResult {
                step_name: monitor_name.clone(),
                timestamp_started,
                success,
                error_message: error_message.clone(),
                response: None,
                trace_id: None,
                span_id: None,
            });
        } else if !success {
            // Mark the last step as failed if the overall script failed
            if let Some(last) = step_results.last_mut() {
                if last.success {
                    last.success = false;
                    last.error_message = error_message.clone();
                }
                // Only set error_message if not already set (e.g., from within a step closure)
                // to avoid overwriting clean assertion messages with Rhai-wrapped versions
                if last.error_message.is_none() {
                    last.error_message = error_message.clone();
                }
            }
        }

        let logs = ctx_arc
            .log_entries
            .lock()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>();

        ExecutionOutput {
            result: MonitorResult {
                monitor_name,
                timestamp_started,
                success,
                step_results,
            },
            logs,
        }
    }
}
