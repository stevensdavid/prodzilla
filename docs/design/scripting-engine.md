# Scripting Engine Design for Prodzilla

## 1. Language Recommendation: Rhai

**Rhai** is the recommended scripting language for Prodzilla's embedded scripting engine.

### Why Rhai over alternatives?

| Criteria | Rhai | Lua (mlua) | JS (Boa) | Starlark | WASM |
|----------|------|------------|----------|----------|------|
| Rust integration | Native, first-class | FFI-based, good | Heavy runtime | Good | Excellent isolation |
| Sandboxing | Built-in, secure by default | Manual | Manual | Built-in | Built-in |
| Syntax familiarity | Rust/JS-like hybrid | Lua-specific | JS (most familiar) | Python-like | N/A |
| Binary size impact | ~500KB | ~1MB+ (C lib) | ~5MB+ | ~1MB | ~2MB+ |
| Async Rust compat | Sync, but easy to bridge | Sync | Partial async | Sync | Complex |
| Memory/CPU limits | Built-in limits | Manual | Manual | Built-in | Built-in |
| Learning curve | Low (familiar syntax) | Medium | Low | Low | High |
| Crate maturity | Stable, actively maintained | Stable | Maturing | Stable | Complex toolchain |
| No-std / small footprint | Yes | No (C dependency) | No | No | Depends |

**Key reasons for Rhai:**

1. **Native Rust integration** — No FFI boundary. Register Rust functions directly. Types map naturally. Errors propagate cleanly. This is the biggest advantage for a Rust project.

2. **Secure sandboxing by default** — No filesystem access, no network access, no system calls. Scripts can only call functions you explicitly register. This is critical for a monitoring tool where configs may come from untrusted sources.

3. **Built-in resource limits** — Max operations count, max call stack depth, max string length, max array size. Prevents runaway scripts from consuming resources.

4. **Familiar syntax** — Looks like a mix of Rust and JavaScript. Anyone who knows JS, Python, or Rust can read/write it immediately:
   ```rhai
   let resp = http_get("https://api.example.com/health");
   if resp.status == 200 {
       let body = parse_json(resp.body);
       assert(body.status == "ok", "Health check failed");
   }
   ```

5. **Small binary impact** — ~500KB added to the binary. Prodzilla targets <15MB RAM; Rhai's footprint is minimal.

6. **Actively maintained** — Regular releases, good documentation, responsive maintainer.

**Why not the others?**

- **Lua**: Great language, but the FFI boundary adds complexity. `mlua` is good but Rhai's native Rust integration is smoother. Lua's 1-indexed arrays and metatables are less intuitive for the target audience.
- **JavaScript (Boa)**: Most familiar syntax, but Boa's runtime is significantly heavier (~5MB+). `deno_core` is even heavier and brings in V8. Overkill for monitor scripts.
- **Starlark**: Good sandboxing, but Python-like syntax with deliberate restrictions (no recursion, no mutation after freeze). Too restrictive for monitoring scripts that need loops and state.
- **WASM**: Polyglot is appealing, but the toolchain complexity (compile step, module management) is too high for what should be quick-to-write monitor scripts.

---

## 2. Functionality / API Surface

Scripts should have access to a curated set of functions exposed from Rust. These are grouped by category:

### 2.1 HTTP Operations

```rhai
// Basic requests — all return a Response object
let resp = http_get(url);
let resp = http_post(url, body);
let resp = http_put(url, body);
let resp = http_delete(url);

// Full control via request builder
let resp = http_request(#{
    method: "POST",
    url: "https://api.example.com/auth",
    headers: #{ "Content-Type": "application/json", "X-Api-Key": env("API_KEY") },
    body: `{"username": "test"}`,
    timeout_seconds: 5,
});

// Response object fields:
// resp.status     → integer (200, 404, etc.)
// resp.body       → string (raw body)
// resp.headers    → map of header name → value
// resp.duration_ms → integer (request duration)
```

All HTTP functions route through the existing `reqwest::Client` with OTel context propagation. Scripts never access the network directly.

### 2.2 Response Parsing

```rhai
let data = parse_json(resp.body);   // Returns Rhai Dynamic (map/array/string/number)
let val = json_path(resp.body, "$.users[0].name");  // JSONPath extraction

// Direct field access after parse_json:
let user = data.users[0];
let name = user.name;
```

### 2.3 Assertions

```rhai
assert(condition, "failure message");           // Fail step if false
assert_eq(actual, expected, "message");         // Equality check
assert_ne(actual, expected, "message");         // Inequality check
assert_contains(haystack, needle, "message");   // String contains
assert_matches(string, regex_pattern, "msg");   // Regex match

// Soft assertions (record failure but continue executing)
soft_assert(condition, "message");
```

When an assertion fails, it records a step failure with the message and (by default) stops execution. Soft assertions record failures but let the script continue — useful for collecting multiple validation errors in one run.

### 2.4 Variables & State

```rhai
// Environment variables (read-only, from process env)
let key = env("API_KEY");

// Script-local state (persists across the script's lifetime, not across runs)
let counter = 0;
counter += 1;

// Generate values
let id = uuid();                    // UUID v4
let ts = timestamp();               // Current UTC timestamp as ISO 8601 string
let epoch = timestamp_epoch();      // Current UTC epoch seconds
```

### 2.5 Control Flow & Timing

```rhai
// Sleep/delay
sleep_ms(1000);   // Pause execution for 1 second

// Retry with backoff
let resp = retry(3, 2000, || {     // 3 attempts, 2s between
    let r = http_get(url);
    assert(r.status == 200, "not ready");
    r
});

// Poll until condition
let resp = poll(|| {
    let r = http_get(url);
    r.status == 200                 // Return true to stop polling
}, #{
    interval_ms: 1000,
    timeout_ms: 30000,
    message: "Service did not become ready",
});
```

### 2.6 Data Transformation

```rhai
// String ops (built-in to Rhai)
let lower = s.to_lower_case();
let trimmed = s.trim();
let parts = s.split(",");
let replaced = s.replace("old", "new");

// Encoding
let encoded = base64_encode(data);
let decoded = base64_decode(encoded);
let url_enc = url_encode(data);

// Hashing / crypto (for API auth signatures)
let hash = hmac_sha256(key, message);
let hash = sha256(data);

// JSON construction
let body = to_json(#{ user: "test", token: id });
```

### 2.7 Logging & Debugging

```rhai
log_info("Step completed successfully");
log_warn("Retrying due to 429");
log_debug("Response body: " + resp.body);

// Attach metadata to the monitor result
set_metadata("response_time_p99", value);
```

Log output integrates with Prodzilla's existing tracing infrastructure.

### 2.8 Step Recording

```rhai
// Explicitly record a step result (for structured output)
step("authenticate", || {
    let resp = http_post(auth_url, creds);
    assert(resp.status == 200, "Auth failed");
    parse_json(resp.body)
});

step("fetch-data", || {
    let resp = http_get(data_url);
    assert(resp.status == 200, "Fetch failed");
    parse_json(resp.body)
});
```

The `step()` function creates named step boundaries that map to `StepResult` entries in the `MonitorResult`. This preserves the existing result structure and gives visibility into which part of a scripted monitor failed.

---

## 3. Integration Architecture

### 3.1 New Monitor Type (Additive)

Scripted monitors are a **third monitor type** alongside single-step and multi-step:

```
Monitor
├── Single-step  (url + http_method at root level)
├── Multi-step   (steps array in YAML)
└── Scripted     (script file reference)       ← NEW
```

All three share the same `Monitor` struct, scheduling, result storage, alerting, and OTel integration. The `Monitorable` trait implementation branches on which variant is present.

### 3.2 Config Format

```yaml
monitors:
  # Existing single-step monitor (unchanged)
  - name: health-check
    url: https://api.example.com/health
    http_method: GET
    expectations:
      - field: StatusCode
        operation: Equals
        value: "200"
    schedule:
      initial_delay: 0
      interval: 30

  # Existing multi-step monitor (unchanged)
  - name: login-flow
    steps:
      - name: authenticate
        url: https://api.example.com/auth
        http_method: POST
        with:
          body: '{"user": "test", "pass": "test"}'
      - name: get-profile
        url: https://api.example.com/profile
        http_method: GET
        with:
          headers:
            Authorization: "Bearer ${{ steps.authenticate.response.body.token }}"
    schedule:
      initial_delay: 5
      interval: 60

  # NEW: Scripted monitor
  - name: checkout-flow
    script: ./monitors/checkout-flow.rhai
    schedule:
      initial_delay: 10
      interval: 120
    alerts:
      - url: https://hooks.slack.com/services/xxx
    tags:
      team: payments
      severity: critical
```

### 3.3 Validation Rules

The existing mutual-exclusivity check in `Monitor`'s custom Deserialize expands:

- `url` + `http_method` → single-step (no `steps`, no `script`)
- `steps` → multi-step (no `url`/`http_method`, no `script`)
- `script` or `script_path` → scripted (no `url`/`http_method`, no `steps`)
- Exactly one of the three must be present

The existing `MonitorHelper` struct in the custom `Deserialize` impl must be updated to include the new `script`, `script_path`, and `script_timeout_seconds` fields — otherwise they are silently dropped during deserialization. The existing empty-steps guard (`steps` present but empty → error) must also be preserved when expanding the validation logic.

### 3.4 Script File Resolution

Script paths are resolved relative to the config file's directory:

```
project/
├── prodzilla.yml           # config references ./monitors/checkout.rhai
└── monitors/
    └── checkout-flow.rhai   # resolved relative to prodzilla.yml location
```

Scripts are loaded at config load time and stored as `String` in the `Monitor` struct. This means:
- Parse errors are caught at startup, not at first execution
- No filesystem access needed at runtime
- Script content can be logged/inspected in debug mode

### 3.5 Execution Model

```
┌─────────────────────────────────────────────┐
│              Scheduling Loop                │
│         (same as existing monitors)         │
└───────────────┬─────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────┐
│         ScriptRunner::execute()             │
│  1. Create new Rhai Engine instance         │
│  2. Register all host functions             │
│  3. Set resource limits                     │
│  4. Create ScriptContext (holds results,    │
│     HTTP client ref, OTel context)          │
│  5. Execute script                          │
│  6. Collect StepResults from context        │
│  7. Return MonitorResult                    │
└───────────────┬─────────────────────────────┘
                │
        ┌───────┴────────┐
        │                │
        ▼                ▼
  ┌───────────┐   ┌────────────┐
  │ Result    │   │  Alerting  │
  │ Storage   │   │  (if fail) │
  └───────────┘   └────────────┘
```

**Key detail: Async bridging.** Rhai's engine is synchronous. HTTP calls from scripts need to go through async Rust. The solution uses `futures::executor::block_on` inside `tokio::task::spawn_blocking`:

```rust
// In the script runner, host functions use futures::executor::block_on
// to bridge from sync Rhai callbacks into async Rust.
// We cannot use tokio's Handle::block_on() here because spawn_blocking
// threads are still within the Tokio runtime context and would panic.

fn register_http_functions(engine: &mut Engine, ctx: Arc<ScriptContext>) {
    let ctx_clone = ctx.clone();
    engine.register_fn("http_get", move |url: &str| -> Dynamic {
        let client = ctx_clone.http_client.clone();

        // Bridge from sync Rhai → async Rust using a non-Tokio executor
        let result = futures::executor::block_on(async {
            client.get(url).send().await
        });

        // Convert to Rhai-friendly Response map
        response_to_dynamic(result)
    });
}
```

Script execution runs inside `tokio::task::spawn_blocking`, which offloads the CPU-bound Rhai interpreter off the async runtime's worker threads. The existing `probe_and_store_result` flow is async, so scripted monitors diverge here — they move into blocking context for the duration of script evaluation, then return a `MonitorResult` like any other monitor type.

### 3.6 Sandboxing & Resource Limits

```rust
let mut engine = Engine::new();

// Execution limits
engine.set_max_operations(1_000_000);    // ~1M ops before abort
engine.set_max_call_levels(64);           // Prevent deep recursion
engine.set_max_string_size(1_048_576);    // 1MB max string
engine.set_max_array_size(10_000);        // 10K array elements
engine.set_max_map_size(10_000);          // 10K map entries

// Overall timeout (enforced by wrapping execution in tokio::time::timeout)
// Configured per-monitor in YAML, default 60 seconds
```

**What scripts CANNOT do (by design):**
- Access the filesystem (no `std::fs` registered)
- Open network connections (only through registered `http_*` functions)
- Spawn processes or threads
- Access Prodzilla internals beyond the registered API
- Run indefinitely (operation count + timeout limits)

---

## 4. Implementation Plan

### 4.1 New Dependencies

```toml
# Cargo.toml additions
rhai = { version = "1", features = ["sync", "metadata"] }
# "sync" — makes Engine Send+Sync for use across async tasks
# "metadata" — enables function metadata for script IDE support
futures = "0.3"
# Used for futures::executor::block_on to bridge sync Rhai → async Rust
# inside spawn_blocking (where Tokio's block_on would panic)
```

### 4.2 New Module Structure

```
src/
├── scripting/
│   ├── mod.rs              # Public API: ScriptRunner, ScriptContext
│   ├── engine.rs           # Engine creation, function registration
│   ├── host_functions/
│   │   ├── mod.rs
│   │   ├── http.rs         # http_get, http_post, http_request, etc.
│   │   ├── assertions.rs   # assert, assert_eq, soft_assert, etc.
│   │   ├── parsing.rs      # parse_json, json_path
│   │   ├── crypto.rs       # hmac_sha256, sha256, base64_encode/decode
│   │   ├── time.rs         # sleep_ms, timestamp, retry, poll
│   │   ├── env.rs          # env(), uuid()
│   │   └── logging.rs      # log_info, log_warn, log_debug
│   ├── types.rs            # Response, ScriptError, ScriptStepResult
│   └── tests.rs            # Unit tests with mock HTTP
├── monitor/
│   ├── model.rs            # Add `script: Option<String>` field
│   ├── monitor_logic.rs    # Branch on script presence in Monitorable
│   └── ...
└── config.rs               # Load script files, validate new variant
```

### 4.3 Key Types

```rust
// src/scripting/types.rs

/// Context shared between Rust host and Rhai script during execution
pub struct ScriptContext {
    pub http_client: reqwest::Client,             // Clone of the global lazy_static CLIENT
    pub otel_context: opentelemetry::Context,
    pub step_results: Mutex<Vec<StepResult>>,    // Collected during execution
    pub assertion_failures: Mutex<Vec<String>>,   // Soft assertion failures
    pub metadata: Mutex<HashMap<String, String>>, // User-set metadata
    pub monitor_name: String,
    pub timeout: Duration,
}

/// Rhai-visible response object (registered as custom type)
#[derive(Clone, Debug)]
pub struct ScriptResponse {
    pub status: i64,
    pub body: String,
    pub headers: HashMap<String, String>,
    pub duration_ms: i64,
}

/// Errors from script execution
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("Script parse error: {0}")]
    ParseError(String),
    #[error("Script runtime error: {0}")]
    RuntimeError(String),
    #[error("Assertion failed: {0}")]
    AssertionFailed(String),
    #[error("Script timeout after {0:?}")]
    Timeout(Duration),
    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
}
```

```rust
// src/scripting/mod.rs

pub struct ScriptRunner {
    ast: AST,  // Pre-compiled script
}

impl ScriptRunner {
    /// Create a new runner, compiling the script upfront
    pub fn new(script_source: &str) -> Result<Self, ScriptError> {
        let mut engine = create_engine();  // Sets limits, registers types
        let ast = engine.compile(script_source)
            .map_err(|e| ScriptError::ParseError(e.to_string()))?;
        Ok(Self { ast })
    }

    /// Validate that a script compiles without creating a runner
    pub fn validate(script_source: &str) -> Result<(), ScriptError> {
        let mut engine = create_engine();
        engine.compile(script_source)
            .map_err(|e| ScriptError::ParseError(e.to_string()))?;
        Ok(())
    }

    /// Execute the script, returning collected step results.
    /// Creates a fresh Engine per execution since host functions capture
    /// per-execution context (ScriptContext). This is cheap — Rhai engine
    /// creation is sub-millisecond.
    pub async fn execute(&self, ctx: ScriptContext) -> MonitorResult {
        let ctx = Arc::new(ctx);
        let timeout = ctx.timeout;
        let ast = self.ast.clone();
        let ctx_clone = ctx.clone();

        let result = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || {
                let mut engine = create_engine();
                register_host_functions(&mut engine, ctx_clone);
                engine.eval_ast(&ast)
            })
        ).await;

        // Convert result + collected step_results → MonitorResult
        build_monitor_result(ctx, result)
    }
}
```

### 4.4 Model Changes

```rust
// In Monitor struct (model.rs), add:
pub struct Monitor {
    pub name: String,
    // Existing fields...
    pub url: Option<String>,
    pub http_method: Option<String>,
    pub steps: Option<Vec<Step>>,
    // NEW — see Section 7.1 for dual representation details
    pub script: Option<String>,              // Rhai source code (DB/API)
    pub script_path: Option<String>,         // File path (YAML only, resolved at load time)
    pub script_timeout_seconds: Option<u64>, // Default: 60
    // ...existing fields...
}
```

Update the custom `Deserialize` to enforce mutual exclusivity:
- Exactly one of: (`url` + `http_method`), `steps`, `script`, or `script_path`
- `script` and `script_path` cannot both be present

### 4.5 Config Loading Changes

```rust
// Called after YAML parse, BEFORE seeding into database.
// Converts script_path → script content so the DB stores source code.
fn resolve_script_paths(config: &mut Config, config_dir: &Path) -> Result<()> {
    for monitor in &mut config.monitors {
        if let Some(script_path) = monitor.script_path.take() {
            let full_path = config_dir.join(&script_path);
            let content = std::fs::read_to_string(&full_path)
                .map_err(|e| format!("Failed to load script {}: {}", full_path.display(), e))?;

            // Validate the script compiles
            ScriptRunner::validate(&content)?;

            // Store content in script field; script_path is now None
            monitor.script = Some(content);
        }
    }
    Ok(())
}
```

See Section 7.2 for the full startup sequence with database seeding.

### 4.6 Monitor Logic Integration

```rust
// In monitor_logic.rs, within probe_and_store_result():

if let Some(script_content) = &monitor.script {
    let ctx = ScriptContext {
        http_client: get_http_client(),  // Clone of the global lazy_static CLIENT
        otel_context: current_otel_context,
        step_results: Mutex::new(Vec::new()),
        assertion_failures: Mutex::new(Vec::new()),
        metadata: Mutex::new(HashMap::new()),
        monitor_name: monitor.name.clone(),
        timeout: Duration::from_secs(
            monitor.script_timeout_seconds.unwrap_or(60)
        ),
    };

    let runner = ScriptRunner::new(script_content).unwrap(); // Pre-validated at load
    let monitor_result = runner.execute(ctx).await;

    // Store result (same path as existing monitors)
    app_state.add_monitor_result(monitor.name.clone(), monitor_result);
}
```

---

## 5. Example Script

File: `monitors/checkout-flow.rhai`

```rhai
// Checkout flow: authenticate → add to cart → checkout → verify order

// Step 1: Authenticate
let auth = step("authenticate", || {
    let resp = http_post(
        "https://api.store.com/auth",
        to_json(#{ username: "test-user", password: env("TEST_PASSWORD") })
    );
    assert_eq(resp.status, 200, "Authentication failed");
    let body = parse_json(resp.body);
    assert(body.token != "", "No token in response");
    body
});

let token = auth.token;
let headers = #{ "Authorization": "Bearer " + token };

// Step 2: Add item to cart
let cart = step("add-to-cart", || {
    let resp = http_request(#{
        method: "POST",
        url: "https://api.store.com/cart",
        headers: headers,
        body: to_json(#{ product_id: "SKU-1234", quantity: 1 }),
    });
    assert(resp.status == 200 || resp.status == 201, "Add to cart failed: " + resp.status);
    parse_json(resp.body)
});

// Step 3: Checkout
let order = step("checkout", || {
    let resp = http_request(#{
        method: "POST",
        url: "https://api.store.com/checkout",
        headers: headers,
        body: to_json(#{ cart_id: cart.cart_id, payment_method: "test" }),
    });
    assert_eq(resp.status, 200, "Checkout failed");
    let body = parse_json(resp.body);
    assert(body.order_id != "", "No order ID returned");
    log_info("Order created: " + body.order_id);
    body
});

// Step 4: Poll for order completion
step("verify-order", || {
    poll(|| {
        let resp = http_request(#{
            method: "GET",
            url: "https://api.store.com/orders/" + order.order_id,
            headers: headers,
        });
        let body = parse_json(resp.body);
        body.status == "completed"
    }, #{
        interval_ms: 2000,
        timeout_ms: 30000,
        message: "Order did not complete within 30 seconds",
    });
});
```

---

## 6. Risks & Mitigations

### Risk 1: Async ↔ Sync Bridge Complexity
**Risk:** Using `block_on` inside `spawn_blocking` can be tricky. Nesting Tokio runtimes panics.

**Mitigation:** Use `tokio::task::spawn_blocking` to move script execution off the async runtime. Inside the blocking closure, use `futures::executor::block_on` (not Tokio's `Handle::block_on()` or `Runtime::block_on()`) to bridge back into async for HTTP calls. This works because `futures::executor::block_on` is a lightweight, standalone executor that doesn't conflict with the Tokio runtime context still present on `spawn_blocking` threads. Note: Tokio's `Handle::block_on()` will panic from `spawn_blocking` threads because they are still within the Tokio runtime context.

### Risk 2: Resource Consumption from Scripts
**Risk:** A script with aggressive polling or many HTTP requests could overwhelm the target system.

**Mitigation:**
- Operation count limits in Rhai engine
- Overall timeout per script execution
- Rate limiting option on HTTP functions (e.g., max N requests per script execution)
- `sleep_ms` contributes to the overall timeout
- Document best practices for script authors

### Risk 3: Script Debugging Difficulty
**Risk:** When a script fails, users need good error messages to debug.

**Mitigation:**
- Rhai provides line/column numbers in parse and runtime errors
- Assertion failures include user-provided messages and actual vs expected values
- `step()` boundaries give named failure points
- `log_debug()` outputs go to monitor results or traces
- Pre-compilation at config load catches syntax errors before first run

### Risk 4: Secret Leakage
**Risk:** Scripts might log or expose secrets from `env()` calls.

**Mitigation:**
- Response bodies from steps marked `sensitive` (future: add to script API) are not included in traces
- `log_*` output should sanitize known secret env var names
- Document that secrets should use `env()` (not hardcoded) and that log output may be stored in results

### Risk 5: Breaking Changes to Script API
**Risk:** As the API evolves, existing scripts may break.

**Mitigation:**
- Version the script API (start at v1)
- Use Rhai's metadata feature to document all registered functions
- Maintain backwards compatibility within a major version
- Consider an optional `#api_version = 1` pragma at the top of scripts

---

## 7. Integration with Database Config Store (v1.0)

The v1.0 branch introduced a database-backed configuration system with a CRUD REST API, hot-reloading MonitorManager, and YAML seeding. This affects the scripting engine design in several important ways.

### 7.1 Dual Representation: File Path vs Content

The `script` field has two meanings depending on context:

| Context | `script` field contains | Example |
|---------|------------------------|---------|
| YAML config file | Relative file path | `./monitors/checkout.rhai` |
| Database (`config_json`) | Rhai source code | `let resp = http_get(...);` |
| CRUD API request/response | Rhai source code | (same as DB) |

**Resolution:** Add a separate `script_path` field for YAML, keep `script` for content.

```rust
pub struct Monitor {
    // ...existing fields...

    /// Rhai script source code. Used in database storage and API.
    /// Mutually exclusive with url/http_method and steps.
    #[serde(default)]
    pub script: Option<String>,

    /// Path to a .rhai script file (YAML config only).
    /// Resolved relative to config file directory during seeding.
    /// Loaded into `script` field before database insertion.
    #[serde(default)]
    pub script_path: Option<String>,

    /// Max script execution time in seconds (default: 60).
    #[serde(default)]
    pub script_timeout_seconds: Option<u64>,
}
```

**Validation rules expand to:**
- Exactly one of: (`url` + `http_method`), `steps`, `script`, or `script_path`
- `script` and `script_path` are mutually exclusive
- `script_path` is only valid in YAML (the seeding process converts it to `script`)

### 7.2 YAML Seeding Flow

The existing `seed_from_yaml` in `src/config_store/seed.rs` inserts monitors into the database. For scripted monitors, the script file must be loaded before seeding:

```rust
// In config loading, BEFORE seeding into database:
fn resolve_script_paths(config: &mut Config, config_dir: &Path) -> Result<()> {
    for monitor in &mut config.monitors {
        if let Some(script_path) = monitor.script_path.take() {
            let full_path = config_dir.join(&script_path);
            let content = std::fs::read_to_string(&full_path)
                .map_err(|e| format!("Failed to load script {}: {}", full_path.display(), e))?;

            // Validate script compiles
            ScriptRunner::validate(&content)?;

            // Convert path → content for database storage
            monitor.script = Some(content);
            // script_path is now None (we called .take())
        }
    }
    Ok(())
}

// Startup sequence:
// 1. load_config("prodzilla.yml")       → Config with script_path fields
// 2. resolve_script_paths(&mut config)  → Convert paths to content
// 3. seed_from_yaml(store, &config)     → Insert into DB (script content in config_json)
// 4. MonitorManager::run()              → Reconcile from DB, start monitors
```

After seeding, the database contains the full script content. The original `.rhai` files are no longer referenced at runtime.

### 7.3 CRUD API for Scripted Monitors

Creating a scripted monitor via the API requires sending script content inline:

```bash
# Create a scripted monitor via API
curl -X POST http://localhost:3000/api/v1/monitors \
  -H "Content-Type: application/json" \
  -d '{
    "name": "checkout-flow",
    "script": "let resp = http_get(\"https://api.example.com/health\");\nassert_eq(resp.status, 200, \"Health check failed\");",
    "schedule": { "initial_delay": 10, "interval": 120 },
    "tags": { "team": "payments" }
  }'

# Update script content
curl -X PUT http://localhost:3000/api/v1/monitors/checkout-flow \
  -H "Content-Type: application/json" \
  -d '{
    "monitor": {
      "name": "checkout-flow",
      "script": "// updated script\nlet resp = http_get(\"https://api.example.com/v2/health\");\nassert_eq(resp.status, 200, \"V2 Health check failed\");",
      "schedule": { "initial_delay": 10, "interval": 120 }
    },
    "version": 1
  }'
```

**API validation additions:**
- When `script` field is present in a create/update request, validate the script compiles with `ScriptRunner::validate()` before storing
- Return `400 Bad Request` with parse error details if script is invalid
- This catches syntax errors at write time, not at first execution

```rust
// In create_monitor handler (or a shared validation layer):
if let Some(script) = &monitor.script {
    ScriptRunner::validate(script).map_err(|e| ApiError {
        error: "validation_error".to_string(),
        message: format!("Script compilation failed: {}", e),
        details: None,
    })?;
}
```

### 7.4 MonitorManager Hot-Reloading

The existing MonitorManager reconciliation loop works for scripted monitors with **no changes to the manager itself**. Here's why:

1. **Create via API** → `ConfigChangeEvent::MonitorCreated` → `reconcile_single()` → loads `Monitor` from DB (includes `script` content) → `start_monitor()` → `monitoring_loop()` → `probe_and_store_result()` branches on `script.is_some()`

2. **Update via API** → `ConfigChangeEvent::MonitorUpdated` → version mismatch detected → `stop_monitor()` + `start_monitor()` with new script content

3. **Delete via API** → `ConfigChangeEvent::MonitorDeleted` → `stop_monitor()` cancels the tokio task

4. **Poll fallback** → fingerprint changes when any monitor version bumps → full reconcile picks up script changes

The `start_monitor()` method passes `monitor.clone()` into the tokio task, which includes the `script` field. The script content flows through the existing pipeline without any manager changes.

### 7.5 ConfigStore: No Schema Migration Needed

Since the database stores monitors as serialized JSON (`config_json TEXT`), adding `script`, `script_path`, and `script_timeout_seconds` to the `Monitor` struct requires **zero schema changes**. Serde handles the new optional fields:

- Existing monitors in DB: `script` field absent → deserializes as `None`
- New scripted monitors: `script` field present → stored in JSON, round-trips correctly

This is a major advantage of the JSON storage approach.

### 7.6 ScriptRunner Caching (Optimization)

Since script compilation (parsing + AST generation) has a cost, and the MonitorManager may restart monitors, consider caching compiled scripts:

```rust
/// Pre-compiled script cache, keyed by script content hash.
/// Avoids re-compiling the same script on every monitor restart.
struct ScriptCache {
    cache: HashMap<u64, Arc<AST>>,  // hash(content) → compiled AST
}
```

This is an optimization for Phase 2 — the MVP can compile on each execution since Rhai compilation is fast (sub-millisecond for typical scripts).

### 7.7 Updated Architecture Diagram

```
                    ┌──────────────────┐
                    │   YAML Config    │
                    │  (script_path)   │
                    └────────┬─────────┘
                             │ resolve_script_paths()
                             ▼
                    ┌──────────────────┐
                    │   Config with    │
                    │ script content   │
                    └────────┬─────────┘
                             │ seed_from_yaml()
                             ▼
┌──────────────┐    ┌──────────────────┐    ┌──────────────┐
│  CRUD API    │───▶│    Database      │◀───│   Polling    │
│  (script     │    │  (config_json    │    │  (fingerprint│
│   content)   │    │   with script)   │    │   check)     │
└──────────────┘    └────────┬─────────┘    └──────────────┘
                             │
                    ┌────────┴─────────┐
                    │  MonitorManager  │
                    │  (reconcile)     │
                    └────────┬─────────┘
                             │ start_monitor()
                             ▼
              ┌──────────────────────────────┐
              │  monitoring_loop (tokio task) │
              │  ┌─────────────────────────┐ │
              │  │ if script.is_some():    │ │
              │  │   ScriptRunner::execute │ │
              │  │ elif steps.is_some():   │ │
              │  │   multi-step logic      │ │
              │  │ else:                   │ │
              │  │   single-step logic     │ │
              │  └─────────────────────────┘ │
              └──────────────────────────────┘
```

---

## 8. Implementation Phases

### Phase 1: Core Engine (MVP)
- Add `rhai` dependency to Cargo.toml
- `src/scripting/` module: `ScriptRunner`, `ScriptContext`, host function registration
- Core host functions: `http_get`, `http_post`, `http_request`, `parse_json`, `assert`, `assert_eq`, `step()`
- Utility functions: `env()`, `uuid()`, `log_info/warn/debug`
- Model changes: `script`, `script_path`, `script_timeout_seconds` fields on `Monitor`
- Deserialization: three-way mutual exclusivity validation
- Config loading: `resolve_script_paths()` to load `.rhai` files into content
- YAML seeding: script content flows into database via existing `seed_from_yaml`
- `probe_and_store_result()` branching for scripted monitors
- CRUD API: script compilation validation on create/update
- MonitorResult integration (no manager changes needed)
- Unit tests with wiremock

### Phase 2: Advanced Features
- `retry()`, `poll()`, `sleep_ms()`
- `soft_assert()`, `assert_contains`, `assert_matches`
- `base64_encode/decode`, `url_encode`
- `hmac_sha256`, `sha256` (for API auth)
- `json_path()` for complex extraction
- `set_metadata()` for custom result data
- ScriptRunner AST caching (avoid recompilation on monitor restart)

### Phase 3: Developer Experience
- Script validation CLI command (`prodzilla validate monitors/`)
- Better error reporting with context
- Script API documentation generation from Rhai metadata
- Example script library
- Optional: hot-reload scripts on file change (watch mode for YAML + script files)
