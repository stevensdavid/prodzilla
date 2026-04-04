use rhai::Engine;

pub fn register(engine: &mut Engine) {
    engine.register_fn("env", |name: String| -> String {
        std::env::var(&name).unwrap_or_default()
    });

    engine.register_fn("uuid", || -> String { uuid::Uuid::new_v4().to_string() });

    engine.register_fn("timestamp", || -> String {
        chrono::Utc::now().to_rfc3339()
    });

    engine.register_fn("timestamp_epoch", || -> i64 {
        chrono::Utc::now().timestamp()
    });
}
