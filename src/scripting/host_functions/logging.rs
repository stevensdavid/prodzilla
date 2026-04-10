use std::sync::Arc;

use chrono::Utc;
use rhai::Engine;

use crate::scripting::types::{LogEntry, ScriptContext};

pub fn register(engine: &mut Engine, ctx: Arc<ScriptContext>) {
    let ctx_info = Arc::clone(&ctx);
    engine.register_fn("log_info", move |msg: String| {
        tracing::info!("{}", msg);
        ctx_info.log_entries.lock().unwrap().push(LogEntry {
            level: "info".to_string(),
            message: msg,
            timestamp: Utc::now(),
        });
    });

    let ctx_warn = Arc::clone(&ctx);
    engine.register_fn("log_warn", move |msg: String| {
        tracing::warn!("{}", msg);
        ctx_warn.log_entries.lock().unwrap().push(LogEntry {
            level: "warn".to_string(),
            message: msg,
            timestamp: Utc::now(),
        });
    });

    let ctx_debug = Arc::clone(&ctx);
    engine.register_fn("log_debug", move |msg: String| {
        tracing::debug!("{}", msg);
        ctx_debug.log_entries.lock().unwrap().push(LogEntry {
            level: "debug".to_string(),
            message: msg,
            timestamp: Utc::now(),
        });
    });
}
