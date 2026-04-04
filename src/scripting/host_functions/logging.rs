use rhai::Engine;

pub fn register(engine: &mut Engine) {
    engine.register_fn("log_info", |msg: String| {
        tracing::info!("{}", msg);
    });

    engine.register_fn("log_warn", |msg: String| {
        tracing::warn!("{}", msg);
    });

    engine.register_fn("log_debug", |msg: String| {
        tracing::debug!("{}", msg);
    });
}
