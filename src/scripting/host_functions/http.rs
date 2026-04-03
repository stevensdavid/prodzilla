use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rhai::{Dynamic, Engine, EvalAltResult, Map, Position};

use crate::scripting::types::{ScriptContext, ScriptResponse};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

pub fn register(engine: &mut Engine, ctx: Arc<ScriptContext>) {
    // Register ScriptResponse as a custom type with getters
    engine.register_type::<ScriptResponse>();
    engine.register_get("status", |resp: &mut ScriptResponse| resp.status);
    engine.register_get("body", |resp: &mut ScriptResponse| resp.body.clone());
    engine.register_get("duration_ms", |resp: &mut ScriptResponse| resp.duration_ms);
    engine.register_get("headers", |resp: &mut ScriptResponse| {
        let mut map = Map::new();
        for (k, v) in &resp.headers {
            map.insert(k.clone().into(), Dynamic::from(v.clone()));
        }
        map
    });

    // http_get
    {
        let ctx_clone = Arc::clone(&ctx);
        engine.register_fn(
            "http_get",
            move |url: String| -> Result<ScriptResponse, Box<EvalAltResult>> {
                let client = ctx_clone.http_client.clone();
                futures::executor::block_on(async move {
                    do_request(client, "GET", &url, None, HashMap::new()).await
                })
                .map_err(|e| Box::new(EvalAltResult::ErrorRuntime(e.into(), Position::NONE)))
            },
        );
    }

    // http_post
    {
        let ctx_clone = Arc::clone(&ctx);
        engine.register_fn(
            "http_post",
            move |url: String, body: String| -> Result<ScriptResponse, Box<EvalAltResult>> {
                let client = ctx_clone.http_client.clone();
                futures::executor::block_on(async move {
                    do_request(client, "POST", &url, Some(body), HashMap::new()).await
                })
                .map_err(|e| Box::new(EvalAltResult::ErrorRuntime(e.into(), Position::NONE)))
            },
        );
    }

    // http_put
    {
        let ctx_clone = Arc::clone(&ctx);
        engine.register_fn(
            "http_put",
            move |url: String, body: String| -> Result<ScriptResponse, Box<EvalAltResult>> {
                let client = ctx_clone.http_client.clone();
                futures::executor::block_on(async move {
                    do_request(client, "PUT", &url, Some(body), HashMap::new()).await
                })
                .map_err(|e| Box::new(EvalAltResult::ErrorRuntime(e.into(), Position::NONE)))
            },
        );
    }

    // http_delete
    {
        let ctx_clone = Arc::clone(&ctx);
        engine.register_fn(
            "http_delete",
            move |url: String| -> Result<ScriptResponse, Box<EvalAltResult>> {
                let client = ctx_clone.http_client.clone();
                futures::executor::block_on(async move {
                    do_request(client, "DELETE", &url, None, HashMap::new()).await
                })
                .map_err(|e| Box::new(EvalAltResult::ErrorRuntime(e.into(), Position::NONE)))
            },
        );
    }

    // http_request with options map
    {
        let ctx_clone = Arc::clone(&ctx);
        engine.register_fn(
            "http_request",
            move |options: Map| -> Result<ScriptResponse, Box<EvalAltResult>> {
                let url = options
                    .get("url")
                    .and_then(|v| v.clone().try_cast::<String>())
                    .ok_or_else(|| {
                        Box::new(EvalAltResult::ErrorRuntime(
                            "http_request: 'url' is required".into(),
                            Position::NONE,
                        ))
                    })?;

                let method = options
                    .get("method")
                    .and_then(|v| v.clone().try_cast::<String>())
                    .unwrap_or_else(|| "GET".to_string());

                let body = options
                    .get("body")
                    .and_then(|v| v.clone().try_cast::<String>());

                let headers: HashMap<String, String> = options
                    .get("headers")
                    .and_then(|v| v.clone().try_cast::<Map>())
                    .map(|map| {
                        map.into_iter()
                            .filter_map(|(k, v)| v.try_cast::<String>().map(|s| (k.to_string(), s)))
                            .collect()
                    })
                    .unwrap_or_default();

                let client = ctx_clone.http_client.clone();
                futures::executor::block_on(async move {
                    do_request(client, &method, &url, body, headers).await
                })
                .map_err(|e| Box::new(EvalAltResult::ErrorRuntime(e.into(), Position::NONE)))
            },
        );
    }
}

async fn do_request(
    client: reqwest::Client,
    method: &str,
    url: &str,
    body: Option<String>,
    headers: HashMap<String, String>,
) -> Result<ScriptResponse, String> {
    use std::str::FromStr;

    let method =
        reqwest::Method::from_str(method).map_err(|e| format!("Invalid HTTP method: {}", e))?;

    let mut builder = client
        .request(method, url)
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS));

    if let Some(b) = body {
        builder = builder.body(b);
    }

    for (key, value) in headers {
        builder = builder.header(&key, &value);
    }

    let start = Instant::now();
    let response = builder
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let duration_ms = start.elapsed().as_millis() as i64;
    let status = response.status().as_u16() as i64;

    let resp_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|v_str| (k.as_str().to_string(), v_str.to_string()))
        })
        .collect();

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    Ok(ScriptResponse {
        status,
        body: body_text,
        headers: resp_headers,
        duration_ms,
    })
}
