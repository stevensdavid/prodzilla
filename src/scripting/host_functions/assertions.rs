use rhai::{Dynamic, Engine, EvalAltResult, Position};

pub fn register(engine: &mut Engine) {
    engine.register_fn(
        "assert",
        |cond: bool, msg: String| -> Result<(), Box<EvalAltResult>> {
            if !cond {
                Err(Box::new(EvalAltResult::ErrorRuntime(
                    msg.into(),
                    Position::NONE,
                )))
            } else {
                Ok(())
            }
        },
    );

    engine.register_fn(
        "assert_eq",
        |a: Dynamic, b: Dynamic, msg: String| -> Result<(), Box<EvalAltResult>> {
            let a_str = a.to_string();
            let b_str = b.to_string();
            if a_str != b_str {
                Err(Box::new(EvalAltResult::ErrorRuntime(
                    format!("{}: expected '{}' == '{}'", msg, a_str, b_str).into(),
                    Position::NONE,
                )))
            } else {
                Ok(())
            }
        },
    );
}
