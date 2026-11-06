//! The `http_send` pipeline (used by the egui app).
//!
//! Flow (same order as the TS): resolve `{{vars}}` in url/headers/body -> [pre-request script
//! hook] -> apply auth -> send -> [post-response script hook] -> build the response.
//! The TS handler does not touch the cookie jar or the history log (the renderer logs history
//! itself via `history_log`), so neither does this.

use crate::environments::{VariableScopes, VariableStore};
use crate::http::{send_authenticated_body, SendBody};
use crate::scripts::{
    execute_blocking, LogEntry, MutableRequest, ResponseAccess, ScriptContext, ScriptResult,
    ScriptSandbox, TestResult, TrackedScope,
};
use crate::types::{
    AuthConfig, HeaderValue, HttpMethod, RequestSettings, ResponseFormat, ResponseTiming, Variable,
};
use crate::visualizer::Visualizer;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The renderer's `SendRequestPayload`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpSendPayload {
    pub method: HttpMethod,
    pub url: String,
    #[serde(default)]
    pub headers: IndexMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    /// Binary body (multipart forms, file uploads); sent instead of `body`, no variable
    /// resolution, invisible to scripts (a script that sets `nx.request.body` replaces it).
    #[serde(default)]
    pub body_bytes: Option<Vec<u8>>,
    #[serde(default)]
    pub settings: Option<RequestSettings>,
    #[serde(default)]
    pub auth: Option<AuthConfig>,
    #[serde(default)]
    pub pre_request_script: Option<String>,
    #[serde(default)]
    pub post_response_script: Option<String>,
    #[serde(default)]
    pub environment_vars: Vec<Variable>,
    #[serde(default)]
    pub collection_vars: Vec<Variable>,
    #[serde(default)]
    pub global_vars: Vec<Variable>,
    /// The request comes from an untrusted (imported / pulled) collection: no scripts run and
    /// Secret environment/global variables are withheld.
    #[serde(default)]
    pub untrusted: bool,
}

/// What `http:send` returns to the renderer (not `NexusResponse`: no raw body bytes, plus script output).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpSendResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: IndexMap<String, HeaderValue>,
    pub body_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_json: Option<Value>,
    pub timing: ResponseTiming,
    pub size: u64,
    pub test_results: Vec<TestResult>,
    pub script_logs: Vec<LogEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visualizer_html: Option<String>,
    /// Raw body bytes, kept only for image/pdf/binary responses (where `body_text` is lossy).
    /// Not serialized.
    #[serde(skip)]
    pub body_bytes: Option<Vec<u8>>,
}

async fn run_script(
    sandbox: &ScriptSandbox,
    script: &str,
    ctx: ScriptContext,
) -> Result<(ScriptResult, ScriptContext), String> {
    execute_blocking(sandbox, script, ctx)
        .await
        .map_err(|e| e.to_string())
}

fn parse_method(m: &str) -> Result<HttpMethod, String> {
    crate::http::parse_method(m).map_err(|e| e.to_string())
}

pub async fn execute(
    payload: HttpSendPayload,
    sandbox: ScriptSandbox,
) -> Result<HttpSendResponse, String> {
    let mut payload = payload;
    let mut script_logs: Vec<LogEntry> = Vec::new();
    if payload.untrusted {
        let withheld = crate::trust::secret_count(&payload.environment_vars)
            + crate::trust::secret_count(&payload.global_vars);
        payload.environment_vars = crate::trust::without_secrets(&payload.environment_vars);
        payload.global_vars = crate::trust::without_secrets(&payload.global_vars);
        payload.pre_request_script = None;
        payload.post_response_script = None;
        if let Some(st) = payload.settings.as_mut() {
            if crate::trust::restore_tls_verification(st) {
                script_logs.push(crate::trust::warning(
                    "Untrusted collection: the request's \"skip TLS verification\" setting was ignored.",
                ));
            }
        }
        script_logs.push(crate::trust::warning(format!(
            "Untrusted collection: scripts skipped, {withheld} secret variable(s) withheld (unresolved {{{{vars}}}} stay as-is). Trust the collection to enable them."
        )));
    }
    let variable_store = VariableStore::new(&VariableScopes {
        global: payload.global_vars.clone(),
        collection: payload.collection_vars.clone(),
        environment: payload.environment_vars.clone(),
    });

    let mut request_url = variable_store.resolve(&payload.url);
    let mut request_headers = variable_store.resolve_headers(&payload.headers);
    let mut request_body = payload
        .body
        .as_deref()
        .filter(|b| !b.is_empty())
        .map(|b| variable_store.resolve(b));
    let mut request_bytes = payload.body_bytes.clone();
    let mut request_method = payload.method;

    // Each script gets fresh tracked scopes seeded from the payload; their changes are not
    // returned to the renderer (the TS handler didn't either).
    let scope_ctx = |request: MutableRequest| {
        ScriptContext::new(
            TrackedScope::new("environment", &payload.environment_vars),
            TrackedScope::new("collection", &payload.collection_vars),
            TrackedScope::new("global", &payload.global_vars),
            request,
        )
    };

    // Auth is bound to the origin as configured (variables resolved, scripts not yet run).
    let configured_url = request_url.clone();

    if let Some(script) = payload
        .pre_request_script
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        let ctx = scope_ctx(MutableRequest {
            url: request_url.clone(),
            method: request_method.as_str().to_string(),
            headers: request_headers.clone(),
            body: request_body.clone(),
        });
        let (result, _) = run_script(&sandbox, script, ctx).await?;
        script_logs.extend(result.logs);

        if let Some(m) = result.request_mutations {
            if let Some(url) = m.url.filter(|u| !u.is_empty()) {
                request_url = url;
            }
            if let Some(method) = m.method.filter(|m| !m.is_empty()) {
                request_method = parse_method(&method)?;
            }
            if let Some(headers) = m.headers {
                request_headers = headers;
            }
            if let Some(body) = m.body {
                request_bytes = None;
                request_body = body;
            }
        }

        if let Some(error) = result.error {
            return Ok(HttpSendResponse {
                status: 0,
                status_text: format!("Pre-request script error: {error}"),
                headers: IndexMap::new(),
                body_text: error,
                body_json: None,
                timing: ResponseTiming::default(),
                size: 0,
                test_results: Vec::new(),
                script_logs,
                visualizer_html: None,
                body_bytes: None,
            });
        }
    }

    // `none`/`inherit` add nothing: inheritance is resolved by the caller (runner), not here.
    let send_body = match request_bytes {
        Some(b) => Some(SendBody::Bytes(b)),
        None => request_body.clone().map(SendBody::Text),
    };
    if payload
        .settings
        .as_ref()
        .is_some_and(|s| s.reject_unauthorized == Some(false))
    {
        script_logs.push(crate::trust::warning(
            "TLS certificate verification is DISABLED for this request.",
        ));
    }
    let (auth, origin_warning) =
        crate::trust::bind_auth_to_origin(payload.auth.as_ref(), &configured_url, &request_url);
    script_logs.extend(origin_warning);
    let sent = send_authenticated_body(
        auth,
        request_method,
        request_url,
        request_headers,
        send_body,
        payload.settings.clone().unwrap_or_default(),
    )
    .await
    .map_err(|e| e.to_string())?;
    let (response, request_url, request_headers) = (sent.response, sent.url, sent.headers);

    let mut test_results: Vec<TestResult> = Vec::new();
    let mut visualizer_html: Option<String> = None;

    // Post-response script errors are not surfaced (as in the TS): logs and tests still are.
    if let Some(script) = payload
        .post_response_script
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        let headers = response
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or(Value::Null)))
            .collect();
        let mut ctx = scope_ctx(MutableRequest {
            url: request_url.clone(),
            method: request_method.as_str().to_string(),
            headers: request_headers.clone(),
            body: request_body.clone(),
        });
        ctx.response = Some(
            ResponseAccess::new(
                response.status,
                &response.status_text,
                headers,
                response.timing.total,
                &response.body_text,
            )
            .json_null_on_error(),
        );
        ctx.visualizer = Some(Visualizer::new());

        let (result, ctx) = run_script(&sandbox, script, ctx).await?;
        script_logs.extend(result.logs);
        test_results = result.test_results;
        visualizer_html = ctx.visualizer.and_then(|v| v.render_with_wrapper());
    }

    let keep_bytes = matches!(
        crate::response::detect_content_type(&response.headers),
        ResponseFormat::Image | ResponseFormat::Pdf | ResponseFormat::Binary
    );
    let body_bytes = keep_bytes.then_some(response.body);
    Ok(HttpSendResponse {
        status: response.status,
        status_text: response.status_text,
        headers: response.headers,
        body_text: response.body_text,
        body_json: response.body_json,
        timing: response.timing,
        size: response.size,
        test_results,
        script_logs,
        visualizer_html,
        body_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::any;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn server() -> MockServer {
        let s = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
            .mount(&s)
            .await;
        s
    }

    fn payload(v: Value) -> HttpSendPayload {
        serde_json::from_value(v).unwrap()
    }

    async fn run(p: HttpSendPayload) -> Result<HttpSendResponse, String> {
        execute(p, ScriptSandbox::default()).await
    }

    fn var(k: &str, v: &str) -> Value {
        json!({"key": k, "value": v, "type": "string", "enabled": true})
    }

    #[tokio::test]
    async fn resolves_variables_across_scopes_applies_auth_and_returns_the_ui_shape() {
        let s = server().await;
        let res = run(payload(json!({
            "method": "POST",
            "url": format!("{}/{{{{path}}}}", s.uri()),
            "headers": {"X-Who": "{{who}}"},
            "body": "{\"n\":\"{{n}}\"}",
            "auth": {"type": "api-key", "key": "k", "value": "v", "addTo": "query"},
            "environmentVars": [var("path", "things")],
            "collectionVars": [var("who", "me")],
            "globalVars": [var("n", "1")],
        })))
        .await
        .unwrap();

        let req = s.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(req.method.as_str(), "POST");
        assert_eq!(req.url.path(), "/things");
        assert_eq!(req.url.query(), Some("k=v"));
        assert_eq!(req.headers.get("x-who").unwrap().to_str().unwrap(), "me");
        assert_eq!(req.body, br#"{"n":"1"}"#);

        let v = serde_json::to_value(&res).unwrap();
        assert_eq!(v["status"], 200);
        assert_eq!(v["statusText"], "OK");
        assert_eq!(v["bodyText"], r#"{"ok":true}"#);
        assert_eq!(v["bodyJson"], json!({"ok": true}));
        assert_eq!(v["size"], 11);
        assert_eq!(v["testResults"], json!([]));
        assert_eq!(v["scriptLogs"], json!([]));
        assert!(v.get("visualizerHtml").is_none());
        assert_eq!(v["timing"]["dns"], 0.0);
        assert!(v["headers"]["content-length"].is_string());
    }

    /// Payload whose URL a pre-request script sends to `evil`, with api-key auth configured.
    fn redirecting(good: &MockServer, evil: &MockServer, script: String) -> Value {
        let _ = evil;
        json!({
            "method": "GET",
            "url": format!("{}/a", good.uri()),
            "auth": {"type": "api-key", "key": "k", "value": "v", "addTo": "query"},
            "preRequestScript": script,
        })
    }

    #[tokio::test]
    async fn origin_change_by_script_drops_auth_and_warns() {
        let (good, evil) = (server().await, server().await);
        let script = format!("nx.request.url = '{}/steal';", evil.uri());
        let res = run(payload(redirecting(&good, &evil, script)))
            .await
            .unwrap();
        let req = evil.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(req.url.path(), "/steal");
        assert_eq!(req.url.query(), None, "auth must not follow the script");
        let logs = serde_json::to_string(&res.script_logs).unwrap();
        assert!(logs.contains("Auth not sent"), "{logs}");
    }

    #[tokio::test]
    async fn same_origin_script_change_keeps_auth() {
        let (good, evil) = (server().await, server().await);
        let script = format!("nx.request.url = '{}/other';", good.uri());
        let res = run(payload(redirecting(&good, &evil, script)))
            .await
            .unwrap();
        let req = good.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(req.url.path(), "/other");
        assert_eq!(req.url.query(), Some("k=v"));
        assert!(res.script_logs.is_empty());
    }

    fn secret_payload(s: &MockServer, untrusted: bool) -> HttpSendPayload {
        payload(json!({
            "method": "GET",
            "url": format!("{}/a", s.uri()),
            "headers": {"X-Tok": "{{tok}}", "X-N": "{{n}}"},
            "preRequestScript": "nx.request.headers['X-Script'] = 'ran';",
            "environmentVars": [
                {"key": "tok", "value": "S3CRET", "type": "secret", "enabled": true},
                var("n", "1"),
            ],
            "untrusted": untrusted,
        }))
    }

    #[tokio::test]
    async fn untrusted_skips_scripts_and_withholds_secrets() {
        let s = server().await;
        let res = run(secret_payload(&s, true)).await.unwrap();
        let req = s.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(
            req.headers.get("x-tok").unwrap().to_str().unwrap(),
            "{{tok}}"
        );
        assert_eq!(req.headers.get("x-n").unwrap().to_str().unwrap(), "1");
        assert!(req.headers.get("x-script").is_none());
        assert!(serde_json::to_string(&res.script_logs)
            .unwrap()
            .contains("Untrusted"));
    }

    #[tokio::test]
    async fn trusted_runs_scripts_and_resolves_secrets() {
        let s = server().await;
        run(secret_payload(&s, false)).await.unwrap();
        let req = s.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(
            req.headers.get("x-tok").unwrap().to_str().unwrap(),
            "S3CRET"
        );
        assert_eq!(
            req.headers.get("x-script").unwrap().to_str().unwrap(),
            "ran"
        );
    }

    #[tokio::test]
    async fn raw_bytes_are_kept_for_binary_responses_only() {
        let s = MockServer::start().await;
        Mock::given(wiremock::matchers::path("/img"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(vec![0u8, 159, 255, 1], "image/png"),
            )
            .mount(&s)
            .await;
        Mock::given(wiremock::matchers::path("/json"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(b"{}".to_vec(), "application/json"),
            )
            .mount(&s)
            .await;
        let img = run(payload(
            json!({"method": "GET", "url": format!("{}/img", s.uri())}),
        ))
        .await
        .unwrap();
        assert_eq!(img.body_bytes, Some(vec![0u8, 159, 255, 1]));
        let js = run(payload(
            json!({"method": "GET", "url": format!("{}/json", s.uri())}),
        ))
        .await
        .unwrap();
        assert_eq!(js.body_bytes, None);
        assert!(serde_json::to_value(&img)
            .unwrap()
            .get("bodyBytes")
            .is_none());
    }

    #[tokio::test]
    async fn body_bytes_are_sent_verbatim_and_a_script_body_replaces_them() {
        let s = server().await;
        run(payload(json!({
            "method": "POST",
            "url": s.uri(),
            "bodyBytes": [0, 159, 255, 10],
            "body": "ignored text",
        })))
        .await
        .unwrap();
        let req = s.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(req.body, vec![0u8, 159, 255, 10]);

        run(payload(json!({
            "method": "POST",
            "url": s.uri(),
            "bodyBytes": [1, 2, 3],
            "preRequestScript": "nx.request.body = 'from script';",
        })))
        .await
        .unwrap();
        let req = s.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(req.body, b"from script");
    }

    #[tokio::test]
    async fn inherit_and_none_auth_add_nothing_and_defaults_apply() {
        let s = server().await;
        run(payload(
            json!({"method": "GET", "url": s.uri(), "auth": {"type": "inherit"}}),
        ))
        .await
        .unwrap();
        let req = s.received_requests().await.unwrap().pop().unwrap();
        assert!(req.headers.get("authorization").is_none());
    }

    #[tokio::test]
    async fn send_failure_becomes_an_error_string() {
        let err = run(payload(
            json!({"method": "GET", "url": "http://127.0.0.1:1/"}),
        ))
        .await
        .unwrap_err();
        assert!(err.contains("error sending request"));
    }

    #[test]
    fn rejects_unknown_methods_at_deserialization() {
        assert!(
            serde_json::from_value::<HttpSendPayload>(json!({"method": "NOPE", "url": "x"}))
                .is_err()
        );
    }

    #[tokio::test]
    async fn pre_request_script_mutates_the_request_and_logs() {
        let s = server().await;
        let res = run(payload(json!({
            "method": "GET",
            "url": format!("{}/old", s.uri()),
            "headers": {"X-Keep": "1"},
            "environmentVars": [var("tok", "abc")],
            "preRequestScript": r#"
                console.log("pre", nx.environment.get("tok"));
                nx.request.url = nx.request.url.replace("/old", "/new");
                nx.request.method = "post";
                nx.request.headers["X-Tok"] = nx.environment.get("tok");
                nx.request.body = "{\"a\":1}";
            "#,
        })))
        .await
        .unwrap();

        let req = s.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(req.method.as_str(), "POST");
        assert_eq!(req.url.path(), "/new");
        assert_eq!(req.headers.get("x-tok").unwrap().to_str().unwrap(), "abc");
        assert_eq!(req.headers.get("x-keep").unwrap().to_str().unwrap(), "1");
        assert_eq!(req.body, br#"{"a":1}"#);
        assert_eq!(res.status, 200);
        assert_eq!(res.script_logs.len(), 1);
        assert_eq!(res.script_logs[0].level, "log");
        assert_eq!(res.script_logs[0].args, vec![json!("pre"), json!("abc")]);
        assert!(res.script_logs[0].timestamp > 0);
    }

    #[tokio::test]
    async fn pre_request_script_error_returns_the_error_response_without_sending() {
        let s = server().await;
        let res = run(payload(json!({
            "method": "GET",
            "url": s.uri(),
            "preRequestScript": "console.log('before'); throw new Error('nope');",
        })))
        .await
        .unwrap();
        assert!(s.received_requests().await.unwrap().is_empty());
        let v = serde_json::to_value(&res).unwrap();
        assert_eq!(v["status"], 0);
        assert_eq!(v["statusText"], "Pre-request script error: nope");
        assert_eq!(v["bodyText"], "nope");
        assert_eq!(v["headers"], json!({}));
        assert_eq!(v["size"], 0);
        assert_eq!(v["timing"]["total"], 0.0);
        assert_eq!(v["testResults"], json!([]));
        assert_eq!(v["scriptLogs"][0]["args"], json!(["before"]));
        assert!(v.get("visualizerHtml").is_none());
    }

    #[tokio::test]
    async fn pre_request_script_timeout_is_reported_as_a_script_error() {
        let s = server().await;
        let res = execute(
            payload(
                json!({"method": "GET", "url": s.uri(), "preRequestScript": "while (true) {}"}),
            ),
            ScriptSandbox::new(50),
        )
        .await
        .unwrap();
        assert_eq!(res.status, 0);
        assert!(
            res.status_text.starts_with("Pre-request script error: ")
                && res.status_text.contains("timed out")
        );
        assert!(s.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn pre_request_script_with_an_invalid_method_is_an_error() {
        let s = server().await;
        let err = run(payload(json!({"method": "GET", "url": s.uri(), "preRequestScript": "nx.request.method = 'NOPE';"})))
            .await
            .unwrap_err();
        assert!(err.contains("Invalid HTTP method"));
    }

    #[tokio::test]
    async fn empty_mutations_are_ignored_like_the_ts_truthy_checks() {
        let s = server().await;
        run(payload(json!({
            "method": "GET",
            "url": format!("{}/keep", s.uri()),
            "preRequestScript": "nx.request.url = ''; nx.request.method = '';",
        })))
        .await
        .unwrap();
        let req = s.received_requests().await.unwrap().pop().unwrap();
        assert_eq!(req.method.as_str(), "GET");
        assert_eq!(req.url.path(), "/keep");
    }

    #[tokio::test]
    async fn post_response_script_returns_tests_logs_and_visualizer() {
        let s = server().await;
        let res = run(payload(json!({
            "method": "GET",
            "url": s.uri(),
            "preRequestScript": "console.log('pre');",
            "postResponseScript": r#"
                console.log("post", nx.response.code, nx.response.status, nx.request.method);
                nx.test("status is 200", () => nx.expect(nx.response.code).to.equal(200));
                nx.test("ok flag", () => nx.expect(nx.response.json().ok).to.be.true);
                nx.test("fails", () => nx.expect(nx.response.text()).to.equal("x"));
                nx.visualizer.set("<b>{{ok}}</b>", nx.response.json());
            "#,
        })))
        .await
        .unwrap();

        let v = serde_json::to_value(&res).unwrap();
        assert_eq!(v["scriptLogs"][0]["args"], json!(["pre"]));
        assert_eq!(
            v["scriptLogs"][1]["args"],
            json!(["post", 200, "OK", "GET"])
        );
        assert_eq!(v["testResults"][0]["name"], "status is 200");
        assert_eq!(v["testResults"][0]["passed"], true);
        assert!(v["testResults"][0].get("error").is_none());
        assert_eq!(v["testResults"][1]["passed"], true);
        assert_eq!(v["testResults"][2]["passed"], false);
        assert!(v["testResults"][2]["error"]
            .as_str()
            .unwrap()
            .contains("to equal"));
        assert!(v["testResults"][2]["duration"].is_number());
        let html = v["visualizerHtml"].as_str().unwrap();
        assert!(
            html.contains("<b>true</b>") && html.starts_with("<!DOCTYPE html>"),
            "{html}"
        );
    }

    #[tokio::test]
    async fn post_response_script_sees_the_final_request_headers_and_response_headers() {
        let s = server().await;
        let res = run(payload(json!({
            "method": "POST",
            "url": s.uri(),
            "headers": {"X-A": "1"},
            "body": "hello",
            "auth": {"type": "bearer", "token": "t0k"},
            "postResponseScript": r#"
                console.log(nx.request.headers["X-A"], nx.request.headers["Authorization"], nx.request.body);
                console.log(nx.response.headers["content-length"], typeof nx.response.headers["content-type"]);
            "#,
        })))
        .await
        .unwrap();
        assert_eq!(
            res.script_logs[0].args,
            vec![json!("1"), json!("Bearer t0k"), json!("hello")]
        );
        assert_eq!(res.script_logs[1].args[0], json!("11"));
    }

    #[tokio::test]
    async fn post_response_json_is_null_for_non_json_bodies_and_errors_are_not_surfaced() {
        let s = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_body_string("plain"))
            .mount(&s)
            .await;
        let res = run(payload(json!({
            "method": "GET",
            "url": s.uri(),
            "postResponseScript": "console.log(nx.response.json()); nx.test('t', () => {}); throw new Error('late failure');",
        })))
        .await
        .unwrap();
        assert_eq!(res.status, 200);
        assert_eq!(res.script_logs[0].args, vec![Value::Null]);
        assert_eq!(res.test_results.len(), 1);
        assert!(res.visualizer_html.is_none());
    }

    #[tokio::test]
    async fn scripts_do_not_block_other_async_work() {
        // A busy script runs on a blocking thread, so a single-threaded executor stays responsive.
        let s = server().await;
        let slow = execute(
            payload(
                json!({"method": "GET", "url": s.uri(), "preRequestScript": "while (true) {}"}),
            ),
            ScriptSandbox::new(300),
        );
        let ticker = async {
            let start = std::time::Instant::now();
            for _ in 0..3 {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            start.elapsed()
        };
        let (res, elapsed) = tokio::join!(slow, ticker);
        assert_eq!(res.unwrap().status, 0);
        assert!(
            elapsed < std::time::Duration::from_millis(250),
            "{elapsed:?}"
        );
    }
}
