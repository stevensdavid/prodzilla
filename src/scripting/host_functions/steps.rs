use std::sync::Arc;

use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext};

use crate::monitor::model::StepResult;
use crate::scripting::types::ScriptContext;

pub fn register(engine: &mut Engine, ctx: Arc<ScriptContext>) {
    let ctx_clone = Arc::clone(&ctx);
    engine.register_fn(
        "step",
        move |ncc: NativeCallContext,
              name: String,
              f: FnPtr|
              -> Result<Dynamic, Box<EvalAltResult>> {
            let start = chrono::Utc::now();
            let result = f.call_raw(&ncc, None, Vec::<rhai::Dynamic>::new());
            let success = result.is_ok();
            let error_msg = result.as_ref().err().map(|e| e.to_string());
            let step_result = StepResult {
                step_name: name,
                timestamp_started: start,
                success,
                error_message: error_msg,
                response: None,
                trace_id: None,
                span_id: None,
            };
            ctx_clone.step_results.lock().unwrap().push(step_result);
            result
        },
    );
}
