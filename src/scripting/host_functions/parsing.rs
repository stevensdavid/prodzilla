use rhai::{Array, Dynamic, Engine, EvalAltResult, Map, Position};
use serde_json::Value;

pub fn register(engine: &mut Engine) {
    engine.register_fn(
        "parse_json",
        |s: String| -> Result<Dynamic, Box<EvalAltResult>> {
            match serde_json::from_str::<Value>(&s) {
                Ok(value) => Ok(json_value_to_dynamic(value)),
                Err(e) => Err(Box::new(EvalAltResult::ErrorRuntime(
                    format!("JSON parse error: {}", e).into(),
                    Position::NONE,
                ))),
            }
        },
    );

    engine.register_fn("to_json", |val: Dynamic| -> String {
        match dynamic_to_json_value(val) {
            Ok(json_val) => serde_json::to_string(&json_val).unwrap_or_else(|_| "{}".to_string()),
            Err(_) => "{}".to_string(),
        }
    });
}

fn json_value_to_dynamic(value: Value) -> Dynamic {
    match value {
        Value::Object(map) => {
            let mut rhai_map = Map::new();
            for (k, v) in map {
                rhai_map.insert(k.into(), json_value_to_dynamic(v));
            }
            Dynamic::from(rhai_map)
        }
        Value::Array(arr) => {
            let rhai_arr: Array = arr.into_iter().map(json_value_to_dynamic).collect();
            Dynamic::from(rhai_arr)
        }
        Value::String(s) => Dynamic::from(s),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::from(n.to_string())
            }
        }
        Value::Bool(b) => Dynamic::from(b),
        Value::Null => Dynamic::UNIT,
    }
}

fn dynamic_to_json_value(val: Dynamic) -> Result<Value, ()> {
    if val.is_unit() {
        return Ok(Value::Null);
    }
    if val.is::<bool>() {
        return Ok(Value::Bool(val.cast::<bool>()));
    }
    // Try integer first (i64), then float
    if let Some(i) = val.clone().try_cast::<i64>() {
        return Ok(Value::Number(i.into()));
    }
    if let Some(f) = val.clone().try_cast::<f64>() {
        let n = serde_json::Number::from_f64(f).unwrap_or_else(|| 0.into());
        return Ok(Value::Number(n));
    }
    if let Some(s) = val.clone().try_cast::<String>() {
        return Ok(Value::String(s));
    }
    if let Some(arr) = val.clone().try_cast::<Array>() {
        let json_arr: Result<Vec<Value>, ()> =
            arr.into_iter().map(dynamic_to_json_value).collect();
        return Ok(Value::Array(json_arr?));
    }
    if let Some(map) = val.try_cast::<Map>() {
        let mut json_obj = serde_json::Map::new();
        for (k, v) in map {
            json_obj.insert(k.to_string(), dynamic_to_json_value(v)?);
        }
        return Ok(Value::Object(json_obj));
    }
    Err(())
}
