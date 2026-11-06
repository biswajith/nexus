//! `ScriptSandbox`: runs one user script in a fresh QuickJS runtime (port of `sandbox.ts`).
//!
//! Security model (see also the source-scan tests in `sandbox_tests.rs`):
//! - a new `Runtime` + `Context` per call, dropped afterwards; no state is shared between runs;
//! - memory and native-stack limits, and a wall-clock deadline enforced by the interrupt handler
//!   (QuickJS raises an *uncatchable* exception, so a script cannot swallow it);
//! - the context has no filesystem/network/process APIs (no module loader, no `std`/`os`);
//! - the single host function is captured inside the runtime closure and never stored on the
//!   global; it takes and returns JSON strings only, so scripts never hold a host object;
//! - script errors are reported as plain messages; pending promise jobs are drained under the
//!   same deadline before results are collected.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use regex::Regex;
use rquickjs::function::This;
use rquickjs::{Context, Ctx, Function, Object, Runtime, Value as JsValue};
use serde::Serialize;
use serde_json::{json, Value};

use super::assertions::{match_schema, TestResult};
use crate::environments::{VariableScopes, VariableStore};
use crate::types::Variable;
use crate::visualizer::Visualizer;

/// Default wall-clock budget per script (`new ScriptSandbox(timeoutMs = 30_000)`).
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MEMORY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const STACK_LIMIT_BYTES: usize = 512 * 1024;

const RUNTIME_JS: &str = include_str!("sandbox_runtime.js");
const ASSERTIONS_JS: &str = include_str!("assertions.js");
const ASSERTIONS_MARKER: &str = "/*__ASSERTIONS__*/";

static REPLACE_IN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{([^\n\r\u{2028}\u{2029}]+?)\}\}").expect("valid regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableChangeAction {
    Set,
    Unset,
}

/// One recorded `set`/`unset` on a tracked scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableChange {
    pub scope: String,
    pub action: VariableChangeAction,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Variable scope that records set/unset operations (`TrackedScope` in the TS).
#[derive(Debug, Clone, Default)]
pub struct TrackedScope {
    name: String,
    data: IndexMap<String, String>,
    pub changes: Vec<VariableChange>,
}

impl TrackedScope {
    /// Seeds the scope from enabled variables only.
    pub fn new(name: &str, variables: &[Variable]) -> Self {
        let data = variables
            .iter()
            .filter(|v| v.enabled)
            .map(|v| (v.key.clone(), v.value.clone()))
            .collect();
        Self {
            name: name.to_string(),
            data,
            changes: Vec::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
        self.changes.push(VariableChange {
            scope: self.name.clone(),
            action: VariableChangeAction::Set,
            key: key.to_string(),
            value: Some(value.to_string()),
        });
    }

    pub fn unset(&mut self, key: &str) {
        self.data.shift_remove(key);
        self.changes.push(VariableChange {
            scope: self.name.clone(),
            action: VariableChangeAction::Unset,
            key: key.to_string(),
            value: None,
        });
    }

    /// Snapshot of the current variables.
    pub fn to_object(&self) -> IndexMap<String, String> {
        self.data.clone()
    }
}

/// The request as scripts see (and may mutate) it. `body: None` is both `null` and `undefined`.
#[derive(Debug, Clone, PartialEq)]
pub struct MutableRequest {
    pub url: String,
    pub method: String,
    pub headers: IndexMap<String, String>,
    pub body: Option<String>,
}

/// Response data exposed as `nx.response`.
#[derive(Debug, Clone)]
pub struct ResponseAccess {
    pub code: u16,
    pub status: String,
    /// Header values are strings or arrays of strings.
    pub headers: IndexMap<String, Value>,
    pub response_time: f64,
    pub body: String,
    /// What `nx.response.json()` yields (`Err` = it throws that message).
    pub json: Result<Value, String>,
}

impl ResponseAccess {
    /// `json()` parses `body` and throws on invalid JSON; `text()` is `body`.
    pub fn new(
        code: u16,
        status: &str,
        headers: IndexMap<String, Value>,
        response_time: f64,
        body: &str,
    ) -> Self {
        let json = serde_json::from_str::<Value>(body).map_err(|e| e.to_string());
        Self {
            code,
            status: status.to_string(),
            headers,
            response_time,
            body: body.to_string(),
            json,
        }
    }

    /// `json()` returns `null` instead of throwing on invalid JSON (what `http:send` does).
    pub fn json_null_on_error(mut self) -> Self {
        if self.json.is_err() {
            self.json = Ok(Value::Null);
        }
        self
    }
}

/// Everything a script can see; scopes and visualizer are mutated in place by `execute`, so the
/// same context can be reused across a chain of scripts.
#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub environment: TrackedScope,
    pub collection_variables: TrackedScope,
    pub globals: TrackedScope,
    pub request: MutableRequest,
    pub response: Option<ResponseAccess>,
    /// `Some` exposes `nx.visualizer.set`.
    pub visualizer: Option<Visualizer>,
}

impl ScriptContext {
    pub fn new(
        environment: TrackedScope,
        collection_variables: TrackedScope,
        globals: TrackedScope,
        request: MutableRequest,
    ) -> Self {
        Self {
            environment,
            collection_variables,
            globals,
            request,
            response: None,
            visualizer: None,
        }
    }
}

/// `nx.execution` state. `next_request`: `None` = never called, `Some(None)` = `setNextRequest(null)`
/// (stop), `Some(Some(name))`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionControl {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_request: Option<Option<String>>,
    pub skipped: bool,
}

/// One captured `console.*` call. `args` holds JSON values (`undefined` becomes `null`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// `log`, `warn`, `error` or `info`.
    pub level: String,
    pub args: Vec<Value>,
    /// Epoch milliseconds.
    pub timestamp: i64,
}

/// Request fields the script changed. `body: Some(None)` means set to `null`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMutations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Option<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptResult {
    pub logs: Vec<LogEntry>,
    pub test_results: Vec<TestResult>,
    /// Cumulative changes of the three scopes (env, collection, global, in that order).
    pub variable_changes: Vec<VariableChange>,
    pub execution_control: ExecutionControl,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_mutations: Option<RequestMutations>,
}

/// Runs Nexus scripts in QuickJS. Cheap to construct; holds only limits.
#[derive(Debug, Clone)]
pub struct ScriptSandbox {
    timeout: Duration,
}

impl Default for ScriptSandbox {
    fn default() -> Self {
        Self::new(DEFAULT_TIMEOUT_MS)
    }
}

static CLOCK_START: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Host-side state the `host(op, argsJson)` bridge mutates; moved out of the [`ScriptContext`]
/// for the duration of a run and moved back afterwards.
struct HostState {
    environment: TrackedScope,
    collection: TrackedScope,
    globals: TrackedScope,
    visualizer: Option<Visualizer>,
    result: ScriptResult,
}

impl ScriptSandbox {
    /// `timeout_ms`: wall-clock budget for the script run (and again for result collection).
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// Factory for tracked scopes, as in the TS.
    pub fn create_tracked_scope(&self, scope_name: &str, variables: &[Variable]) -> TrackedScope {
        TrackedScope::new(scope_name, variables)
    }

    /// Executes `script` (a function body), capturing logs, tests, variable deltas and request
    /// mutations. `result.error` is set when the script throws, doesn't compile, or hits a limit.
    pub fn execute(&self, script: &str, context: &mut ScriptContext) -> ScriptResult {
        let payload = build_payload(context);
        let state = Rc::new(RefCell::new(HostState {
            environment: context.environment.clone(),
            collection: context.collection_variables.clone(),
            globals: context.globals.clone(),
            visualizer: context.visualizer.take(),
            result: ScriptResult::default(),
        }));

        let final_request = self.run(script, &payload, &state);

        let mut st = state.borrow_mut();
        let mut result = std::mem::take(&mut st.result);
        context.environment = std::mem::take(&mut st.environment);
        context.collection_variables = std::mem::take(&mut st.collection);
        context.globals = std::mem::take(&mut st.globals);
        context.visualizer = st.visualizer.take();

        result.variable_changes = [
            &context.environment,
            &context.collection_variables,
            &context.globals,
        ]
        .iter()
        .flat_map(|s| s.changes.iter().cloned())
        .collect();

        if let Some(req) = final_request {
            let orig = &context.request;
            // Header order matters (the TS compared `JSON.stringify` of the objects).
            let headers_changed = !req.headers.iter().eq(orig.headers.iter());
            if req.url != orig.url
                || req.method != orig.method
                || headers_changed
                || req.body != orig.body
            {
                result.request_mutations = Some(RequestMutations {
                    url: Some(req.url),
                    method: Some(req.method),
                    headers: Some(req.headers),
                    body: Some(req.body),
                });
            }
        }
        result
    }

    /// Creates the runtime, runs the script, drains jobs, collects the final request.
    fn run(
        &self,
        script: &str,
        payload: &str,
        state: &Rc<RefCell<HostState>>,
    ) -> Option<MutableRequest> {
        let base = Instant::now();
        let deadline_ms = Arc::new(AtomicU64::new(u64::MAX));
        let timed_out = Arc::new(AtomicBool::new(false));
        let timeout_ms = self.timeout.as_millis().min(u128::from(u64::MAX / 2)) as u64;
        let arm = {
            let deadline_ms = deadline_ms.clone();
            move || {
                deadline_ms.store(
                    base.elapsed().as_millis() as u64 + timeout_ms,
                    Ordering::SeqCst,
                )
            }
        };
        let expired = {
            let deadline_ms = deadline_ms.clone();
            let timed_out = timed_out.clone();
            move || {
                let over = base.elapsed().as_millis() as u64 >= deadline_ms.load(Ordering::SeqCst);
                if over {
                    timed_out.store(true, Ordering::SeqCst);
                }
                over
            }
        };

        let fail = |msg: String| {
            let mut st = state.borrow_mut();
            if st.result.error.is_none() {
                st.result.error = Some(msg);
            }
        };

        let Ok(rt) = Runtime::new() else {
            fail("Script error".into());
            return None;
        };
        rt.set_memory_limit(MEMORY_LIMIT_BYTES);
        rt.set_max_stack_size(STACK_LIMIT_BYTES);
        rt.set_interrupt_handler(Some(Box::new(expired.clone())));
        let Ok(ctx) = Context::full(&rt) else {
            fail("Script error".into());
            return None;
        };

        let timeout_msg = format!(
            "Script execution timed out after {}ms",
            self.timeout.as_millis()
        );

        let mut collected: Option<String> = None;
        ctx.with(|ctx| {
            let host = {
                let state = state.clone();
                move |op: String, args_json: String| -> String {
                    host_call(&state, &op, &args_json)
                }
            };
            let Ok(host_fn) = Function::new(ctx.clone(), host) else {
                fail("Script error".into());
                return;
            };
            let runtime_src = RUNTIME_JS.replace(ASSERTIONS_MARKER, ASSERTIONS_JS);
            let handles: rquickjs::Result<(Function, Function)> = (|| {
                let install: Function = ctx.eval(runtime_src)?;
                let obj: Object = install.call((This(ctx.globals()), host_fn, payload))?;
                Ok((obj.get("run")?, obj.get("collect")?))
            })();
            let Ok((run_fn, collect_fn)) = handles else {
                fail("Script error".into());
                return;
            };

            // Run the script, then drain promise jobs, all under one deadline.
            arm();
            let ran: rquickjs::Result<()> = run_fn.call((script,));
            let engine_err = ran.err().map(|e| describe_engine_error(&ctx, e));
            if engine_err.is_none() {
                // A job that throws is just an unhandled rejection (ignored, like the TS).
                while ctx.execute_pending_job() {
                    if expired() {
                        break;
                    }
                }
            }
            if timed_out.load(Ordering::SeqCst) {
                fail(timeout_msg.clone());
                return;
            }
            if let Some(msg) = engine_err {
                fail(msg);
            }

            arm();
            match collect_fn.call::<_, String>(()) {
                Ok(s) => collected = Some(s),
                Err(e) => {
                    let msg = if timed_out.load(Ordering::SeqCst) {
                        timeout_msg.clone()
                    } else {
                        describe_engine_error(&ctx, e)
                    };
                    fail(msg);
                }
            }
        });

        let parsed: Value = serde_json::from_str(&collected?).ok()?;
        let headers = parsed
            .get("headers")?
            .as_object()?
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
            .collect();
        Some(MutableRequest {
            url: parsed.get("url")?.as_str()?.to_string(),
            method: parsed.get("method")?.as_str()?.to_string(),
            headers,
            body: parsed.get("body")?.as_str().map(str::to_string),
        })
    }
}

/// Message for an exception that escaped the in-realm `run` wrapper (limits / engine errors only,
/// because script throws are caught in JS). Never invokes script-defined code beyond reading the
/// `message` of an engine-created error.
fn describe_engine_error(ctx: &Ctx<'_>, err: rquickjs::Error) -> String {
    if let rquickjs::Error::Exception = err {
        let v: JsValue = ctx.catch();
        if let Some(s) = v.as_string().and_then(|s| s.to_string().ok()) {
            return s;
        }
        if let Some(ex) = v.as_exception() {
            if let Some(m) = ex.message() {
                return m;
            }
        }
    }
    "Script error".to_string()
}

fn build_payload(context: &ScriptContext) -> String {
    let req = &context.request;
    let response = context.response.as_ref().map(|r| {
        let json = match &r.json {
            Ok(v) => json!({"ok": true, "v": v}),
            Err(e) => json!({"ok": false, "e": e}),
        };
        json!({
            "code": r.code,
            "status": r.status,
            "headers": r.headers,
            "responseTime": r.response_time,
            "text": r.body,
            "json": json,
        })
    });
    json!({
        "request": {
            "url": req.url,
            "method": req.method,
            "headers": req.headers,
            "body": req.body,
            "bodyUndefined": false,
        },
        "response": response,
        "visualizer": context.visualizer.is_some(),
    })
    .to_string()
}

/// The `host(op, argsJson)` bridge: always returns `{"ok":true,"v":...}` or `{"ok":false,"e":"..."}`.
fn host_call(state: &Rc<RefCell<HostState>>, op: &str, args_json: &str) -> String {
    let outcome = serde_json::from_str::<Value>(args_json)
        .map_err(|e| e.to_string())
        .and_then(|v| match v {
            Value::Array(a) => Ok(a),
            _ => Err("invalid host call".to_string()),
        })
        .and_then(|args| handle(&mut state.borrow_mut(), op, &args));
    match outcome {
        Ok(Some(v)) => json!({"ok": true, "v": v}),
        Ok(None) => json!({"ok": true}),
        Err(e) => json!({"ok": false, "e": e}),
    }
    .to_string()
}

fn str_arg<'a>(op: &str, args: &'a [Value], i: usize) -> Result<&'a str, String> {
    args.get(i)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("invalid argument to {op}"))
}

fn handle(st: &mut HostState, op: &str, args: &[Value]) -> Result<Option<Value>, String> {
    if let Some((scope_name, method)) = op.split_once('.') {
        let scope = match scope_name {
            "environment" => Some(&mut st.environment),
            "collectionVariables" => Some(&mut st.collection),
            "globals" => Some(&mut st.globals),
            _ => None,
        };
        if let Some(scope) = scope {
            match method {
                "get" => {
                    return Ok(scope
                        .get(str_arg(op, args, 0)?)
                        .map(|s| Value::String(s.to_string())))
                }
                "set" => {
                    scope.set(str_arg(op, args, 0)?, str_arg(op, args, 1)?);
                    return Ok(None);
                }
                "unset" => {
                    scope.unset(str_arg(op, args, 0)?);
                    return Ok(None);
                }
                "toObject" => {
                    let s = serde_json::to_string(&scope.to_object()).map_err(|e| e.to_string())?;
                    return Ok(Some(Value::String(s)));
                }
                _ => {}
            }
        }
    }
    match op {
        "variables.get" => {
            let key = str_arg(op, args, 0)?;
            let v = st
                .environment
                .get(key)
                .or_else(|| st.collection.get(key))
                .or_else(|| st.globals.get(key));
            Ok(v.map(|s| Value::String(s.to_string())))
        }
        "replaceIn" => Ok(Some(Value::String(replace_in(st, str_arg(op, args, 0)?)))),
        "setNextRequest" => {
            st.result.execution_control.next_request = Some(match args.first() {
                Some(Value::Null) => None,
                _ => Some(str_arg(op, args, 0)?.to_string()),
            });
            Ok(None)
        }
        "skipRequest" => {
            st.result.execution_control.skipped = true;
            Ok(None)
        }
        // Sub-millisecond monotonic clock for test durations (the TS used `performance.now()`;
        // QuickJS' `Date.now()` is whole milliseconds, so fast tests all read 0).
        "clock" => Ok(Some(json!(CLOCK_START.elapsed().as_secs_f64() * 1000.0))),
        "log" => {
            let level = str_arg(op, args, 0)?;
            if !matches!(level, "log" | "warn" | "error" | "info") {
                return Err("invalid log level".into());
            }
            let encoded: Vec<Value> =
                serde_json::from_str(str_arg(op, args, 1)?).map_err(|e| e.to_string())?;
            let decoded = encoded
                .iter()
                .map(|a| {
                    if a.get("u").is_some() {
                        Value::Null
                    } else if let Some(j) = a.get("j").and_then(Value::as_str) {
                        serde_json::from_str(j).unwrap_or(Value::Null)
                    } else {
                        Value::String(
                            a.get("s")
                                .and_then(Value::as_str)
                                .unwrap_or("undefined")
                                .to_string(),
                        )
                    }
                })
                .collect();
            let timestamp = args
                .get(2)
                .and_then(Value::as_f64)
                .map(|t| t as i64)
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            st.result.logs.push(LogEntry {
                level: level.to_string(),
                args: decoded,
                timestamp,
            });
            Ok(None)
        }
        "test" => {
            st.result.test_results.push(TestResult {
                name: str_arg(op, args, 0)?.to_string(),
                passed: args.get(1) == Some(&Value::Bool(true)),
                error: args.get(2).and_then(Value::as_str).map(str::to_string),
                duration: args.get(3).and_then(Value::as_f64).unwrap_or(0.0),
            });
            Ok(None)
        }
        "matchSchema" => {
            let wrapped: Value =
                serde_json::from_str(str_arg(op, args, 0)?).map_err(|e| e.to_string())?;
            let schema: Value =
                serde_json::from_str(str_arg(op, args, 1)?).map_err(|e| e.to_string())?;
            let value = wrapped.get("v").cloned().unwrap_or(Value::Null);
            Ok(Some(match match_schema(&value, &schema) {
                Ok(()) => json!({"pass": true, "errors": ""}),
                Err(errors) => json!({"pass": false, "errors": errors}),
            }))
        }
        "visualizer" => {
            let viz = st.visualizer.as_mut().ok_or("visualizer unavailable")?;
            let data = match args.get(1) {
                Some(Value::Null) | None => Value::Null,
                _ => serde_json::from_str(str_arg(op, args, 1)?).map_err(|e| e.to_string())?,
            };
            viz.set(str_arg(op, args, 0)?, data);
            Ok(None)
        }
        "error" => {
            st.result.error = Some(str_arg(op, args, 0)?.to_string());
            Ok(None)
        }
        _ => Err(format!("unknown operation {op}")),
    }
}

/// `nx.variables.replaceIn`: `{{var}}` from env > collection > global, `{{$dynamic}}` generated.
fn replace_in(st: &HostState, template: &str) -> String {
    let dynamic = VariableStore::new(&VariableScopes::default());
    REPLACE_IN_RE
        .replace_all(template, |caps: &regex::Captures| {
            let whole = &caps[0];
            let key = caps[1].trim();
            if key.starts_with('$') {
                let token = format!("{{{{{key}}}}}");
                let resolved = dynamic.resolve(&token);
                return if resolved == token {
                    whole.to_string()
                } else {
                    resolved
                };
            }
            st.environment
                .get(key)
                .or_else(|| st.collection.get(key))
                .or_else(|| st.globals.get(key))
                .map_or_else(|| whole.to_string(), str::to_string)
        })
        .into_owned()
}
