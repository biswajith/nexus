//! Port of `sandbox.test.ts` and the sandbox rules of `architecture.test.ts`.

use std::time::Instant;

use indexmap::IndexMap;
use serde_json::{json, Value};

use super::*;
use crate::types::{Variable, VariableType};
use crate::visualizer::Visualizer;

fn make_vars(entries: &[(&str, &str)]) -> Vec<Variable> {
    entries
        .iter()
        .map(|(k, v)| Variable {
            key: (*k).into(),
            value: (*v).into(),
            var_type: VariableType::String,
            enabled: true,
            description: None,
        })
        .collect()
}

fn make_context() -> ScriptContext {
    let sb = ScriptSandbox::default();
    ScriptContext::new(
        sb.create_tracked_scope("environment", &[]),
        sb.create_tracked_scope("collection", &[]),
        sb.create_tracked_scope("global", &[]),
        MutableRequest {
            url: "https://api.example.com/test".into(),
            method: "GET".into(),
            headers: IndexMap::from([("Content-Type".to_string(), "application/json".to_string())]),
            body: None,
        },
    )
}

fn make_response() -> ResponseAccess {
    ResponseAccess::new(
        200,
        "OK",
        IndexMap::from([("content-type".to_string(), json!("application/json"))]),
        42.0,
        r#"{"status":"ok"}"#,
    )
}

fn exec(script: &str) -> ScriptResult {
    ScriptSandbox::default().execute(script, &mut make_context())
}

fn exec_with(script: &str, ctx: &mut ScriptContext) -> ScriptResult {
    ScriptSandbox::default().execute(script, ctx)
}

fn args(r: &ScriptResult, i: usize) -> &Vec<Value> {
    &r.logs[i].args
}

// ---- console logging ----

#[test]
fn captures_console_log_calls() {
    let r = exec(r#"console.log("hello", 42);"#);
    assert_eq!(r.logs.len(), 1);
    assert_eq!(r.logs[0].level, "log");
    assert_eq!(r.logs[0].args, vec![json!("hello"), json!(42)]);
}

#[test]
fn captures_warn_error_info() {
    let r = exec(r#"console.warn("warning"); console.error("error"); console.info("info");"#);
    let levels: Vec<_> = r.logs.iter().map(|l| l.level.as_str()).collect();
    assert_eq!(levels, ["warn", "error", "info"]);
}

#[test]
fn captures_multiple_arguments_per_log_call() {
    let r = exec(r#"console.log("a", "b", { c: 1 });"#);
    assert_eq!(
        r.logs[0].args,
        vec![json!("a"), json!("b"), json!({"c": 1})]
    );
}

#[test]
fn captures_log_timestamps() {
    let before = chrono::Utc::now().timestamp_millis();
    let r = exec(r#"console.log("test");"#);
    let after = chrono::Utc::now().timestamp_millis();
    assert!(r.logs[0].timestamp >= before && r.logs[0].timestamp <= after);
}

#[test]
fn unserializable_log_args_degrade_to_null_or_string() {
    let r = exec("console.log(undefined, () => 1, 1n, Symbol('s').description); const c = {}; c.c = c; console.log(c);");
    assert_eq!(r.error, None);
    assert_eq!(r.logs[0].args[0], Value::Null);
    assert_eq!(r.logs[1].args.len(), 1);
}

// ---- nx.test and assertions ----

#[test]
fn records_passing_tests() {
    let r = exec(r#"nx.test("passes", () => { nx.expect(200).to.equal(200); });"#);
    assert_eq!(r.test_results.len(), 1);
    assert!(r.test_results[0].passed);
    assert_eq!(r.test_results[0].name, "passes");
}

#[test]
fn records_failing_tests_without_halting_execution() {
    let r = exec(
        r#"nx.test("fails", () => { nx.expect(200).to.equal(404); });
           nx.test("still runs", () => { nx.expect(true).to.be.true; });"#,
    );
    assert_eq!(r.test_results.len(), 2);
    assert!(!r.test_results[0].passed);
    assert!(r.test_results[0].error.is_some());
    assert!(r.test_results[1].passed);
}

#[test]
fn supports_chained_assertion_methods() {
    let mut ctx = make_context();
    ctx.response = Some(make_response());
    let r = exec_with(
        r#"nx.test("status is 200", () => { nx.expect(nx.response.code).to.equal(200); });
           nx.test("response time below 100", () => { nx.expect(nx.response.responseTime).to.be.below(100); });"#,
        &mut ctx,
    );
    assert!(r.test_results.iter().all(|t| t.passed));
    assert_eq!(r.test_results.len(), 2);
}

#[test]
fn records_test_duration() {
    let r = exec(r#"nx.test("quick", () => { nx.expect(1).to.equal(1); });"#);
    assert!(r.test_results[0].duration >= 0.0);
}

#[test]
fn test_duration_has_sub_millisecond_resolution() {
    let r = exec(r#"nx.test("quick", () => { nx.expect(1).to.equal(1); });"#);
    let d = r.test_results[0].duration;
    // Whole-millisecond `Date.now()` made every fast test read exactly 0.
    assert!(d > 0.0 && d < 50.0, "duration {d}");
}

// ---- variable access ----

fn ctx_with(env: &[(&str, &str)], coll: &[(&str, &str)], glob: &[(&str, &str)]) -> ScriptContext {
    let mut ctx = make_context();
    let sb = ScriptSandbox::default();
    ctx.environment = sb.create_tracked_scope("environment", &make_vars(env));
    ctx.collection_variables = sb.create_tracked_scope("collection", &make_vars(coll));
    ctx.globals = sb.create_tracked_scope("global", &make_vars(glob));
    ctx
}

fn change(
    scope: &str,
    action: VariableChangeAction,
    key: &str,
    value: Option<&str>,
) -> VariableChange {
    VariableChange {
        scope: scope.into(),
        action,
        key: key.into(),
        value: value.map(Into::into),
    }
}

#[test]
fn reads_environment_variables() {
    let mut ctx = ctx_with(&[("host", "localhost")], &[], &[]);
    let r = exec_with(r#"console.log(nx.environment.get("host"));"#, &mut ctx);
    assert_eq!(args(&r, 0), &vec![json!("localhost")]);
}

#[test]
fn sets_and_reads_environment_variables() {
    let mut ctx = make_context();
    let r = exec_with(
        r#"nx.environment.set("token", "abc123"); console.log(nx.environment.get("token"));"#,
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!("abc123")]);
    assert!(r.variable_changes.contains(&change(
        "environment",
        VariableChangeAction::Set,
        "token",
        Some("abc123")
    )));
    assert_eq!(ctx.environment.get("token"), Some("abc123"));
}

#[test]
fn unsets_variables_and_records_changes() {
    let mut ctx = ctx_with(&[("temp", "val")], &[], &[]);
    let r = exec_with(r#"nx.environment.unset("temp");"#, &mut ctx);
    assert!(r.variable_changes.contains(&change(
        "environment",
        VariableChangeAction::Unset,
        "temp",
        None
    )));
    assert_eq!(ctx.environment.get("temp"), None);
}

#[test]
fn reads_collection_variables() {
    let mut ctx = ctx_with(&[], &[("baseUrl", "https://api.com")], &[]);
    let r = exec_with(
        r#"console.log(nx.collectionVariables.get("baseUrl"));"#,
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!("https://api.com")]);
}

#[test]
fn sets_collection_variables_and_records_changes() {
    let mut ctx = make_context();
    let r = exec_with(
        r#"nx.collectionVariables.set("version", "v2"); console.log(nx.collectionVariables.get("version"));"#,
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!("v2")]);
    assert!(r.variable_changes.contains(&change(
        "collection",
        VariableChangeAction::Set,
        "version",
        Some("v2")
    )));
}

#[test]
fn unsets_collection_variables_and_records_changes() {
    let mut ctx = ctx_with(&[], &[("deprecated", "old")], &[]);
    let r = exec_with(r#"nx.collectionVariables.unset("deprecated");"#, &mut ctx);
    assert!(r.variable_changes.contains(&change(
        "collection",
        VariableChangeAction::Unset,
        "deprecated",
        None
    )));
}

#[test]
fn collection_to_object_returns_current_variables() {
    let mut ctx = ctx_with(&[], &[("host", "api.com"), ("version", "v1")], &[]);
    let r = exec_with(
        r#"console.log(JSON.stringify(nx.collectionVariables.toObject()));"#,
        &mut ctx,
    );
    let parsed: Value = serde_json::from_str(r.logs[0].args[0].as_str().unwrap()).unwrap();
    assert_eq!(parsed, json!({"host": "api.com", "version": "v1"}));
}

#[test]
fn reads_global_variables() {
    let mut ctx = ctx_with(&[], &[], &[("apiKey", "key123")]);
    let r = exec_with(r#"console.log(nx.globals.get("apiKey"));"#, &mut ctx);
    assert_eq!(args(&r, 0), &vec![json!("key123")]);
}

#[test]
fn variables_get_resolves_env_then_collection_then_global() {
    let mut ctx = ctx_with(
        &[("shared", "from-env")],
        &[("shared", "from-coll"), ("extra", "coll-only")],
        &[("fallback", "global-val")],
    );
    let r = exec_with(
        r#"console.log(nx.variables.get("shared"));
           console.log(nx.variables.get("extra"));
           console.log(nx.variables.get("fallback"));
           console.log(nx.variables.get("missing"));"#,
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!("from-env")]);
    assert_eq!(args(&r, 1), &vec![json!("coll-only")]);
    assert_eq!(args(&r, 2), &vec![json!("global-val")]);
    assert_eq!(args(&r, 3), &vec![Value::Null]);
}

#[test]
fn variables_set_writes_the_environment_scope() {
    let mut ctx = make_context();
    exec_with(r#"nx.variables.set("k", "v");"#, &mut ctx);
    assert_eq!(ctx.environment.get("k"), Some("v"));
}

#[test]
fn variables_replace_in_resolves_scopes_and_dynamic_tokens() {
    let mut ctx = ctx_with(&[("a", "1")], &[("b", "2")], &[("c", "3")]);
    let r = exec_with(
        r#"console.log(nx.variables.replaceIn("{{a}}-{{ b }}-{{c}}-{{nope}}"));
           console.log(nx.variables.replaceIn("{{$guid}}").length);
           console.log(nx.variables.replaceIn("{{$notADynamicVar}}"));"#,
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!("1-2-3-{{nope}}")]);
    assert_eq!(args(&r, 1), &vec![json!(36)]);
    assert_eq!(args(&r, 2), &vec![json!("{{$notADynamicVar}}")]);
}

#[test]
fn environment_to_object_returns_current_variables() {
    let mut ctx = ctx_with(&[("a", "1"), ("b", "2")], &[], &[]);
    let r = exec_with(
        r#"console.log(JSON.stringify(nx.environment.toObject()));"#,
        &mut ctx,
    );
    let parsed: Value = serde_json::from_str(r.logs[0].args[0].as_str().unwrap()).unwrap();
    assert_eq!(parsed, json!({"a": "1", "b": "2"}));
}

#[test]
fn disabled_variables_are_not_visible() {
    let mut vars = make_vars(&[("on", "1"), ("off", "2")]);
    vars[1].enabled = false;
    let scope = TrackedScope::new("env", &vars);
    assert_eq!(scope.get("on"), Some("1"));
    assert_eq!(scope.get("off"), None);
}

// ---- request mutation ----

#[test]
fn mutates_request_url() {
    let r = exec(r#"nx.request.url = "https://new-host.com/v2";"#);
    assert_eq!(
        r.request_mutations.unwrap().url.as_deref(),
        Some("https://new-host.com/v2")
    );
}

#[test]
fn mutates_request_method() {
    let r = exec(r#"nx.request.method = "POST";"#);
    assert_eq!(r.request_mutations.unwrap().method.as_deref(), Some("POST"));
}

#[test]
fn mutates_request_headers() {
    let r = exec(r#"nx.request.headers = { ...nx.request.headers, "X-Custom": "value" };"#);
    assert_eq!(
        r.request_mutations.unwrap().headers.unwrap()["X-Custom"],
        "value"
    );
}

#[test]
fn mutates_request_body() {
    let r = exec(r#"nx.request.body = '{"new":true}';"#);
    assert_eq!(
        r.request_mutations.unwrap().body,
        Some(Some(r#"{"new":true}"#.to_string()))
    );
}

#[test]
fn does_not_set_request_mutations_when_request_is_unchanged() {
    assert!(exec(r#"console.log("no change");"#)
        .request_mutations
        .is_none());
}

#[test]
fn reports_in_place_header_edits_as_request_mutations() {
    let r = exec(r#"nx.request.headers["X-Trace"] = "1";"#);
    let headers = r.request_mutations.unwrap().headers.unwrap();
    assert_eq!(
        headers,
        IndexMap::from([
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Trace".to_string(), "1".to_string()),
        ])
    );
}

#[test]
fn request_values_are_coerced_to_strings_on_collect() {
    let r = exec("nx.request.url = 5; nx.request.headers = { a: 1, b: null };");
    let m = r.request_mutations.unwrap();
    assert_eq!(m.url.as_deref(), Some("5"));
    assert_eq!(
        m.headers.unwrap(),
        IndexMap::from([
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "null".to_string())
        ])
    );
}

#[test]
fn scripts_see_the_existing_request() {
    let mut ctx = make_context();
    ctx.request.body = Some("payload".into());
    let r = exec_with("console.log(nx.request.url, nx.request.method, nx.request.body, nx.request.headers['Content-Type']);", &mut ctx);
    assert_eq!(
        args(&r, 0),
        &vec![
            json!("https://api.example.com/test"),
            json!("GET"),
            json!("payload"),
            json!("application/json")
        ]
    );
}

// ---- response access ----

#[test]
fn reads_response_code_and_status() {
    let mut ctx = make_context();
    let mut resp = make_response();
    resp.code = 201;
    resp.status = "Created".into();
    ctx.response = Some(resp);
    let r = exec_with(
        "console.log(nx.response.code); console.log(nx.response.status);",
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!(201)]);
    assert_eq!(args(&r, 1), &vec![json!("Created")]);
}

#[test]
fn reads_response_headers() {
    let mut ctx = make_context();
    let mut resp = make_response();
    resp.headers = IndexMap::from([
        ("x-request-id".to_string(), json!("abc")),
        ("set-cookie".to_string(), json!(["a", "b"])),
    ]);
    ctx.response = Some(resp);
    let r = exec_with(
        r#"console.log(nx.response.headers["x-request-id"], nx.response.headers["set-cookie"].length);"#,
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!("abc"), json!(2)]);
}

#[test]
fn parses_response_json() {
    let mut ctx = make_context();
    ctx.response = Some(ResponseAccess::new(
        200,
        "OK",
        IndexMap::new(),
        1.0,
        r#"{"items":[1,2,3]}"#,
    ));
    let r = exec_with(
        "const data = nx.response.json(); console.log(data.items.length);",
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!(3)]);
}

#[test]
fn response_json_throws_on_invalid_body_unless_null_mode() {
    let mut ctx = make_context();
    ctx.response = Some(ResponseAccess::new(
        200,
        "OK",
        IndexMap::new(),
        1.0,
        "not json",
    ));
    let r = exec_with("nx.response.json();", &mut ctx);
    assert!(r.error.is_some());

    ctx.response =
        Some(ResponseAccess::new(200, "OK", IndexMap::new(), 1.0, "not json").json_null_on_error());
    let r = exec_with("console.log(nx.response.json());", &mut ctx);
    assert_eq!(r.error, None);
    assert_eq!(args(&r, 0), &vec![Value::Null]);
}

#[test]
fn reads_response_text_and_time() {
    let mut ctx = make_context();
    ctx.response = Some(ResponseAccess::new(
        200,
        "OK",
        IndexMap::new(),
        150.0,
        "plain text",
    ));
    let r = exec_with(
        "console.log(nx.response.text()); console.log(nx.response.responseTime);",
        &mut ctx,
    );
    assert_eq!(args(&r, 0), &vec![json!("plain text")]);
    assert_eq!(args(&r, 1), &vec![json!(150)]);
}

#[test]
fn response_is_absent_without_a_response() {
    let r = exec("console.log(typeof nx.response, typeof nx.visualizer);");
    assert_eq!(args(&r, 0), &vec![json!("undefined"), json!("undefined")]);
}

// ---- execution control ----

#[test]
fn sets_next_request() {
    let r = exec(r#"nx.execution.setNextRequest("Login");"#);
    assert_eq!(
        r.execution_control.next_request,
        Some(Some("Login".to_string()))
    );
}

#[test]
fn sets_next_request_to_null_to_stop() {
    let r = exec("nx.execution.setNextRequest(null);");
    assert_eq!(r.execution_control.next_request, Some(None));
}

#[test]
fn skips_request() {
    assert!(
        exec("nx.execution.skipRequest();")
            .execution_control
            .skipped
    );
}

#[test]
fn defaults_to_no_skip_and_undefined_next_request() {
    let r = exec("// noop");
    assert!(!r.execution_control.skipped);
    assert_eq!(r.execution_control.next_request, None);
}

// ---- error handling ----

#[test]
fn captures_syntax_errors() {
    // QuickJS words these differently from V8 ("Unexpected token"); only the fact matters.
    let r = exec("function {");
    assert!(r.error.is_some_and(|e| !e.is_empty()));
}

#[test]
fn captures_runtime_errors() {
    let r = exec("undefinedFunction();");
    assert!(r.error.unwrap().contains("undefinedFunction"));
}

#[test]
fn captures_thrown_strings() {
    assert_eq!(
        exec(r#"throw "custom error";"#).error.as_deref(),
        Some("custom error")
    );
}

#[test]
fn still_returns_logs_and_test_results_on_error() {
    let r = exec(
        r#"console.log("before");
           nx.test("early test", () => { nx.expect(1).to.equal(1); });
           throw new Error("boom");"#,
    );
    assert!(r.error.unwrap().contains("boom"));
    assert_eq!(r.logs.len(), 1);
    assert_eq!(r.test_results.len(), 1);
    assert!(r.test_results[0].passed);
}

#[test]
fn unhandled_promise_rejections_do_not_fail_the_script() {
    let r = exec("Promise.reject(new Error('late')); console.log('ok');");
    assert_eq!(r.error, None);
    assert_eq!(r.logs.len(), 1);
}

// ---- sandbox restrictions ----

#[test]
fn prevents_access_to_require() {
    assert!(exec(r#"const fs = require("fs");"#).error.is_some());
}

#[test]
fn prevents_access_to_process() {
    assert!(exec("console.log(process.env);").error.is_some());
}

#[test]
fn prevents_access_to_global_and_global_this() {
    assert!(exec("console.log(globalThis.constructor);").error.is_some());
    assert!(exec("console.log(global.constructor);").error.is_some());
}

#[test]
fn no_io_or_host_apis_exist() {
    let r = exec(
        "console.log([typeof std, typeof os, typeof require, typeof module, typeof process, typeof fetch,
           typeof XMLHttpRequest, typeof WebSocket, typeof setTimeout, typeof setInterval, typeof Deno,
           typeof Buffer, typeof import_, typeof scriptArgs, typeof print].join(','));",
    );
    assert_eq!(r.error, None);
    let seen = r.logs[0].args[0].as_str().unwrap();
    assert!(seen.split(',').all(|t| t == "undefined"), "{seen}");
}

#[test]
fn dynamic_import_cannot_load_anything() {
    let r = exec("import('fs').then(() => console.log('loaded'), () => console.log('refused'));");
    assert!(r.logs.iter().all(|l| l.args != vec![json!("loaded")]));
}

#[test]
fn nx_and_console_are_not_replaceable() {
    let r = exec("'use strict'; try { nx = 1; } catch (e) { console.log('blocked'); }");
    assert_eq!(args(&r, 0), &vec![json!("blocked")]);
}

#[test]
fn state_does_not_leak_between_executions() {
    let sb = ScriptSandbox::default();
    let mut ctx = make_context();
    sb.execute(
        "this.leak = 1; Object.prototype.polluted = 1; Array.prototype.push = null;",
        &mut ctx,
    );
    let r = sb.execute(
        "console.log(typeof leak, typeof ({}).polluted, typeof [].push);",
        &mut ctx,
    );
    assert_eq!(
        args(&r, 0),
        &vec![json!("undefined"), json!("undefined"), json!("function")]
    );
}

#[test]
fn tampering_with_intrinsics_cannot_break_the_runtime_bridge() {
    let r = exec(
        r#"JSON.parse = () => { throw new Error('tampered'); };
           JSON.stringify = () => 'x';
           String = () => 'x';
           Array.isArray = () => false;
           Object.keys = () => [];
           nx.environment.set("k", "v");
           console.log(nx.environment.get("k"));
           nx.request.url = "https://changed";"#,
    );
    assert_eq!(r.error, None, "{:?}", r.error);
    assert_eq!(r.logs[0].args, vec![json!("v")]);
    assert_eq!(
        r.request_mutations.unwrap().url.as_deref(),
        Some("https://changed")
    );
}

// ---- realm isolation ----

/// Each probe tries to reach a `Function` constructor and run code that would touch the host.
/// There is no host object in the context, so `process` must never resolve.
fn probe(expr: &str) -> String {
    format!(
        "let escaped = false;
         try {{
           const F = ({expr});
           const p = F('return process')();
           escaped = typeof p === 'object' && p !== null && typeof p.pid === 'number';
         }} catch (e) {{ /* blocked */ }}
         console.log(escaped ? 'ESCAPED' : 'blocked');"
    )
}

#[test]
fn blocks_host_function_via_every_reachable_path() {
    let paths = [
        ("nx scope function", "nx.environment.get.constructor"),
        (
            "nx.variables function",
            "nx.variables.replaceIn.constructor",
        ),
        ("console method", "console.log.constructor"),
        ("expectation object", "nx.expect(1).constructor.constructor"),
        (
            "expectation type checker",
            "nx.expect(1).to.be.a.constructor",
        ),
        (
            "request headers",
            "nx.request.headers.constructor.constructor",
        ),
        (
            "response headers",
            "nx.response.headers.constructor.constructor",
        ),
        (
            "response json",
            "nx.response.json().constructor.constructor",
        ),
        (
            "environment snapshot",
            "nx.environment.toObject().constructor.constructor",
        ),
        ("global this", "this.constructor.constructor"),
        ("script function", "(function(){}).constructor"),
    ];
    for (label, expr) in paths {
        let mut ctx = make_context();
        ctx.response = Some(make_response());
        let r = exec_with(&probe(expr), &mut ctx);
        assert_eq!(
            r.logs.first().map(|l| &l.args),
            Some(&vec![json!("blocked")]),
            "path: {label}"
        );
    }
}

#[test]
fn blocks_host_function_via_errors_raised_by_host_calls() {
    let script = probe(
        "(() => {
          try { nx.environment.set({ toString() { return 1n; } }, 'x'); } catch (e) { return e.constructor.constructor; }
          try { nx.expect({}).to.matchSchema({ type: 'nope' }); } catch (e) { return e.constructor.constructor; }
          return null;
        })()",
    );
    let r = exec(&script);
    assert_eq!(args(&r, 0), &vec![json!("blocked")]);
}

#[test]
fn blocks_host_function_via_errors_caught_inside_nx_test() {
    let script = format!(
        "let ctor;
         nx.test('t', () => {{ try {{ nx.expect(1).to.equal(2); }} catch (e) {{ ctor = e.constructor.constructor; throw e; }} }});
         {}",
        probe("ctor")
    );
    let r = exec(&script);
    assert_eq!(
        r.logs.last().map(|l| &l.args),
        Some(&vec![json!("blocked")])
    );
    assert!(!r.test_results[0].passed);
}

#[test]
fn blocks_host_function_via_stack_overflow_around_host_calls() {
    let script = format!(
        "let ctor = null;
         function dive() {{
           try {{ dive(); }} catch (e) {{ if (!ctor) ctor = e.constructor.constructor; }}
           try {{ nx.environment.get('k'); }} catch (e) {{ if (!ctor) ctor = e.constructor.constructor; }}
         }}
         dive();
         {}",
        probe("ctor")
    );
    let r = exec(&script);
    assert_eq!(
        r.logs.last().map(|l| &l.args),
        Some(&vec![json!("blocked")])
    );
}

#[test]
fn walks_every_reachable_nx_and_console_function_and_finds_no_host_constructor() {
    let script = "
        const seen = new Set();
        const hits = [];
        const walk = (v, path) => {
          if (v === null || (typeof v !== 'object' && typeof v !== 'function') || seen.has(v)) return;
          seen.add(v);
          for (const target of [v, Object.getPrototypeOf(v)]) {
            try {
              const p = target.constructor.constructor('return process')();
              if (p && typeof p.pid === 'number') hits.push(path);
            } catch (e) { /* blocked */ }
          }
          for (const k of Object.getOwnPropertyNames(v)) {
            let child;
            try { child = v[k]; } catch (e) { continue; }
            walk(child, path + '.' + k);
          }
        };
        walk(nx, 'nx');
        walk(console, 'console');
        console.log(hits.length);
    ";
    let mut ctx = make_context();
    ctx.response = Some(make_response());
    ctx.visualizer = Some(Visualizer::new());
    let r = exec_with(script, &mut ctx);
    assert_eq!(r.error, None);
    assert_eq!(args(&r, 0), &vec![json!(0)]);
}

#[test]
fn the_host_bridge_is_not_reachable_from_the_global_scope() {
    // The bridge is a closure variable of the runtime, never a global; walking every global
    // property must not find a function that answers the `{"ok":...}` protocol.
    let r = exec(
        "const found = [];
         for (const k of Object.getOwnPropertyNames(Object.getPrototypeOf(Object.getPrototypeOf(this)) === null ? this : this)) {
           let v; try { v = this[k]; } catch (e) { continue; }
           if (typeof v === 'function') {
             try { const out = v('environment.get', '[\"x\"]'); if (typeof out === 'string' && out.includes('\"ok\"')) found.push(k); } catch (e) {}
           }
         }
         console.log(found.length);",
    );
    assert_eq!(r.error, None);
    assert_eq!(args(&r, 0), &vec![json!(0)]);
}

// ---- limits ----

#[test]
fn times_out_on_infinite_loops() {
    let start = Instant::now();
    let r = ScriptSandbox::new(50).execute("while(true) {}", &mut make_context());
    assert!(r.error.unwrap().contains("timed out"));
    assert!(start.elapsed().as_secs() < 5);
}

#[test]
fn timeout_cannot_be_swallowed_by_the_script() {
    let r = ScriptSandbox::new(50).execute(
        "for (;;) { try { while (true) {} } catch (e) {} finally { } }",
        &mut make_context(),
    );
    assert!(r.error.unwrap().contains("timed out"));
}

#[test]
fn does_not_let_thrown_accessors_run_outside_the_timeout() {
    let r = ScriptSandbox::new(50).execute(
        "throw { get message() { while (true) {} }, toString() { while (true) {} } };",
        &mut make_context(),
    );
    assert!(r.error.is_some());
}

#[test]
fn applies_the_timeout_to_promise_callbacks() {
    let r = ScriptSandbox::new(50).execute(
        "Promise.resolve().then(() => { while (true) {} });",
        &mut make_context(),
    );
    assert!(r.error.unwrap().contains("timed out"));
}

#[test]
fn applies_the_timeout_to_endless_microtask_chains() {
    let r = ScriptSandbox::new(100).execute(
        "const f = () => Promise.resolve().then(f); f();",
        &mut make_context(),
    );
    assert!(r.error.unwrap().contains("timed out"));
}

#[test]
fn promise_callbacks_are_drained_before_results_are_collected() {
    let r = exec(
        "Promise.resolve().then(() => { nx.environment.set('late', 'yes'); nx.request.url = 'https://late'; console.log('then'); });
         (async () => { await null; console.log('async'); })();
         console.log('sync');",
    );
    let order: Vec<_> = r.logs.iter().map(|l| l.args[0].as_str().unwrap()).collect();
    assert_eq!(order, ["sync", "then", "async"]);
    assert_eq!(
        r.request_mutations.unwrap().url.as_deref(),
        Some("https://late")
    );
    assert_eq!(r.variable_changes.len(), 1);
}

#[test]
fn memory_hogs_are_stopped() {
    let start = Instant::now();
    let r = ScriptSandbox::default().execute(
        "const a = []; while (true) { a.push(new Array(100000).fill(1)); }",
        &mut make_context(),
    );
    assert!(r.error.is_some());
    assert!(start.elapsed().as_secs() < 20);
}

#[test]
fn huge_string_growth_is_stopped() {
    let r =
        ScriptSandbox::default().execute("let s = 'x'; for (;;) { s += s; }", &mut make_context());
    assert!(r.error.is_some());
}

#[test]
fn runaway_recursion_is_an_error_not_a_crash() {
    let r = exec("function f() { return f() + 1; } f();");
    assert!(r.error.is_some());
}

// ---- visualizer ----

#[test]
fn calls_visualizer_set_when_provided() {
    let mut ctx = make_context();
    ctx.visualizer = Some(Visualizer::new());
    exec_with(
        r#"nx.visualizer.set("<h1>{{title}}</h1>", { title: "Test" });"#,
        &mut ctx,
    );
    let config = ctx.visualizer.as_ref().unwrap().config().unwrap();
    assert_eq!(config.template, "<h1>{{title}}</h1>");
    assert_eq!(config.data, json!({"title": "Test"}));
}

#[test]
fn visualizer_set_without_data_uses_null() {
    let mut ctx = make_context();
    ctx.visualizer = Some(Visualizer::new());
    exec_with(r#"nx.visualizer.set("<p>hi</p>");"#, &mut ctx);
    assert_eq!(ctx.visualizer.unwrap().config().unwrap().data, Value::Null);
}

// ---- createTrackedScope ----

#[test]
fn tracked_scope_creates_with_enabled_variables_only() {
    let mut vars = make_vars(&[("a", "1"), ("b", "2"), ("c", "3")]);
    vars[1].enabled = false;
    let scope = ScriptSandbox::default().create_tracked_scope("test", &vars);
    assert_eq!(scope.get("a"), Some("1"));
    assert_eq!(scope.get("b"), None);
    assert_eq!(scope.get("c"), Some("3"));
}

#[test]
fn tracked_scope_tracks_set_and_unset() {
    let mut scope = TrackedScope::new("env", &[]);
    scope.set("key", "value");
    assert_eq!(scope.get("key"), Some("value"));
    assert_eq!(
        scope.changes,
        vec![change(
            "env",
            VariableChangeAction::Set,
            "key",
            Some("value")
        )]
    );

    let mut scope = TrackedScope::new("env", &make_vars(&[("x", "1")]));
    scope.unset("x");
    assert_eq!(scope.get("x"), None);
    assert_eq!(
        scope.changes,
        vec![change("env", VariableChangeAction::Unset, "x", None)]
    );
}

#[test]
fn tracked_scope_to_object_is_a_plain_map() {
    let scope = TrackedScope::new("env", &make_vars(&[("a", "1"), ("b", "2")]));
    assert_eq!(
        scope.to_object(),
        IndexMap::from([
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "2".to_string())
        ])
    );
}

// ---- empty script ----

#[test]
fn returns_clean_result_for_empty_script() {
    let r = exec("");
    assert_eq!(r.error, None);
    assert!(r.logs.is_empty() && r.test_results.is_empty() && r.variable_changes.is_empty());
}

#[test]
fn result_serializes_to_the_ts_shape() {
    let r =
        exec(r#"console.log("x"); nx.environment.set("k","v"); nx.execution.setNextRequest("n");"#);
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["logs"][0]["level"], "log");
    assert_eq!(
        v["variableChanges"][0],
        json!({"scope":"environment","action":"set","key":"k","value":"v"})
    );
    assert_eq!(
        v["executionControl"],
        json!({"nextRequest":"n","skipped":false})
    );
    assert!(v.get("error").is_none() && v.get("requestMutations").is_none());
}

// ---- architecture rules (architecture.test.ts, "node:vm sandbox escape") ----

fn src(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/scripts")
            .join(name),
    )
    .unwrap()
}

fn all_rust_sources() -> Vec<(std::path::PathBuf, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let s = std::fs::read_to_string(&p).unwrap();
                out.push((p, s));
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut out,
    );
    out
}

#[test]
fn only_scripts_sandbox_may_use_the_js_engine() {
    for (path, code) in all_rust_sources() {
        let allowed =
            path.ends_with("scripts/sandbox.rs") || path.ends_with("scripts/sandbox_tests.rs");
        let uses = code
            .lines()
            .any(|l| !l.trim_start().starts_with("//") && l.contains("rquickjs"));
        assert!(allowed || !uses, "{} must not use rquickjs", path.display());
    }
}

#[test]
fn the_runtime_js_is_self_contained() {
    let runtime = src("sandbox_runtime.js");
    let assertions = src("assertions.js");
    let module_syntax = regex::Regex::new(r"(?m)^\s*(import|export)\s").unwrap();
    for (name, code) in [
        ("sandbox_runtime.js", &runtime),
        ("assertions.js", &assertions),
    ] {
        assert!(!module_syntax.is_match(code), "{name}: no module syntax");
        for banned in [
            "require(",
            "import(",
            "process",
            "eval(",
            "fetch(",
            "XMLHttpRequest",
        ] {
            assert!(!code.contains(banned), "{name} must not mention {banned}");
        }
    }
    // The only host link is the `host(op, argsJson)` parameter.
    assert!(runtime.contains("(function (host, payloadJson)"));
    let code_only: String = runtime
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(code_only.matches("host(").count(), 1);
}

#[test]
fn every_execution_gets_a_fresh_limited_runtime() {
    let code = src("sandbox.rs");
    for needle in [
        "Runtime::new()",
        "set_memory_limit(",
        "set_max_stack_size(",
        "set_interrupt_handler(",
        "execute_pending_job()",
    ] {
        assert!(code.contains(needle), "sandbox.rs must use {needle}");
    }
    // Exactly one host function is ever registered, and it is passed as an argument, not a global.
    assert_eq!(code.matches("Function::new(").count(), 1);
    assert!(
        !code.contains(".globals().set("),
        "never set host values on the global object"
    );
}

#[test]
fn no_filesystem_network_or_loader_features_are_enabled() {
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .unwrap();
    let line = |name: &str| {
        manifest
            .lines()
            .find(|l| l.starts_with(&format!("{name} ")))
            .unwrap()
            .to_string()
    };
    // rquickjs: default features only (no `loader`/`dyn-load`/`full`/`futures`...).
    assert!(
        !line("rquickjs").contains("features"),
        "{}",
        line("rquickjs")
    );
    // jsonschema: no `resolve-http`/`resolve-file`, so `$ref` can never read files or hit the network.
    assert!(line("jsonschema").contains("default-features = false"));
    assert!(!line("jsonschema").contains("resolve-"));
}

#[test]
fn log_flood_is_truncated_with_bounded_buffer() {
    let r = ScriptSandbox::new(5_000).execute(
        "const s = 'x'.repeat(100000); for (let i = 0; i < 2000; i++) console.log(s);",
        &mut make_context(),
    );
    let bytes: usize = r
        .logs
        .iter()
        .map(|l| l.args.iter().map(|a| a.to_string().len()).sum::<usize>())
        .sum();
    assert!(bytes <= 4 * 1024 * 1024 + 1024, "{bytes}");
    assert!(r.logs.len() <= 1001);
    assert_eq!(r.logs.last().unwrap().args[0], json!("[log truncated]"));
}

#[test]
fn huge_string_allocation_fails_with_memory_error() {
    let r = exec("let s = 'x'.repeat(1 << 20); const a = []; while (true) a.push(s + a.length);");
    assert!(r.error.is_some(), "expected memory error");
}

#[test]
fn test_results_are_capped() {
    let r = ScriptSandbox::new(10_000).execute(
        "const n = 'n'.repeat(100000); for (let i = 0; i < 2500; i++) nx.test(n, () => {});",
        &mut make_context(),
    );
    assert_eq!(r.test_results.len(), 2000);
    assert!(r.test_results[0].name.len() <= 8 * 1024);
}

#[test]
fn variable_sets_are_capped_per_scope() {
    let r = exec("for (let i = 0; i < 2000; i++) nx.environment.set('k' + i, 'v');");
    assert!(r.error.is_some(), "expected cap error");
    let r = exec(
        "const v = 'x'.repeat(1000000); for (let i = 0; i < 6; i++) nx.globals.set('k' + i, v);",
    );
    assert!(r.error.is_some(), "expected byte cap error");
    let r = exec("for (let i = 0; i < 100; i++) nx.environment.set('k' + i, 'v');");
    assert!(r.error.is_none());
}
