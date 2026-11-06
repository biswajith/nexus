use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

async fn server() -> MockServer {
    let s = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .mount(&s)
        .await;
    s
}

fn req(id: &str, url: &str, extra: Value) -> Value {
    let mut r = json!({
        "id": id, "name": id, "method": "GET", "url": url, "headers": [], "params": [],
        "body": {"mode": "none"}, "auth": {"type": "inherit"}, "settings": {"timeout": 2000},
    });
    for (k, v) in extra.as_object().unwrap() {
        r[k] = v.clone();
    }
    r
}

fn folder(id: &str, items: Vec<Value>, extra: Value) -> Value {
    let mut f = json!({"id": id, "name": id, "items": items});
    for (k, v) in extra.as_object().unwrap() {
        f[k] = v.clone();
    }
    f
}

fn config(items: Vec<Value>, extra: Value) -> RunConfig {
    let mut c = json!({
        "collection": {"id": "c", "name": "c", "variables": [], "items": items},
        "iterations": 1, "delayMs": 0, "stopOnError": false, "persistVariables": false,
    });
    for (k, v) in extra.as_object().unwrap() {
        if k == "collectionExtra" {
            for (ck, cv) in v.as_object().unwrap() {
                c["collection"][ck] = cv.clone();
            }
        } else {
            c[k] = v.clone();
        }
    }
    serde_json::from_value(c).unwrap()
}

fn runner() -> CollectionRunner {
    CollectionRunner::new(2000, CancelToken::new())
}

type Events = Arc<Mutex<Vec<Value>>>;

async fn run_collect(r: &CollectionRunner, cfg: RunConfig) -> (RunSummary, Vec<Value>) {
    let events: Events = Arc::default();
    let sink = events.clone();
    let summary = r
        .run(cfg, move |e| {
            sink.lock().unwrap().push(serde_json::to_value(&e).unwrap())
        })
        .await;
    let events = events.lock().unwrap().clone();
    (summary, events)
}

async fn run(cfg: RunConfig) -> RunSummary {
    run_collect(&runner(), cfg).await.0
}

fn paths(s: &MockServer) -> Vec<String> {
    futures_paths(s)
}

fn futures_paths(s: &MockServer) -> Vec<String> {
    let reqs = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(s.received_requests())
    });
    reqs.unwrap()
        .iter()
        .map(|r| r.url.path().to_string())
        .collect()
}

fn types(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .map(|e| e["type"].as_str().unwrap().to_string())
        .collect()
}

// ---- basic execution

#[tokio::test(flavor = "multi_thread")]
async fn runs_a_single_request_and_returns_a_summary() {
    let s = server().await;
    let summary = run(config(
        vec![req("r1", &format!("{}/a", s.uri()), json!({}))],
        json!({}),
    ))
    .await;
    assert_eq!(summary.total_requests, 1);
    assert_eq!(summary.total_iterations, 1);
    assert_eq!(summary.results[0].status, 200);
    assert_eq!(summary.results[0].status_text, "OK");
    assert!(!summary.aborted);
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_multiple_requests_in_order() {
    let s = server().await;
    let u = s.uri();
    run(config(
        vec![
            req("a", &format!("{u}/1"), json!({})),
            req("b", &format!("{u}/2"), json!({})),
            req("c", &format!("{u}/3"), json!({})),
        ],
        json!({}),
    ))
    .await;
    assert_eq!(paths(&s), ["/1", "/2", "/3"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn flattens_nested_folders_depth_first() {
    let s = server().await;
    let u = s.uri();
    let items = vec![
        req("a", &format!("{u}/a"), json!({})),
        folder(
            "f1",
            vec![
                req("b", &format!("{u}/b"), json!({})),
                folder(
                    "f2",
                    vec![req("c", &format!("{u}/c"), json!({}))],
                    json!({}),
                ),
                req("d", &format!("{u}/d"), json!({})),
            ],
            json!({}),
        ),
        req("e", &format!("{u}/e"), json!({})),
    ];
    run(config(items, json!({}))).await;
    assert_eq!(paths(&s), ["/a", "/b", "/c", "/d", "/e"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn filters_by_request_ids() {
    let s = server().await;
    let u = s.uri();
    let items = vec![
        req("a", &format!("{u}/a"), json!({})),
        req("b", &format!("{u}/b"), json!({})),
        req("c", &format!("{u}/c"), json!({})),
    ];
    let summary = run(config(items, json!({"requestIds": ["c", "a"]}))).await;
    assert_eq!(summary.total_requests, 2);
    assert_eq!(paths(&s), ["/a", "/c"]);
}

// ---- iterations

#[tokio::test(flavor = "multi_thread")]
async fn repeats_all_requests_in_each_iteration() {
    let s = server().await;
    let u = s.uri();
    let items = vec![
        req("a", &format!("{u}/a"), json!({})),
        req("b", &format!("{u}/b"), json!({})),
    ];
    let summary = run(config(items, json!({"iterations": 3}))).await;
    assert_eq!(summary.total_requests, 6);
    assert_eq!(summary.total_iterations, 3);
    assert_eq!(paths(&s), ["/a", "/b", "/a", "/b", "/a", "/b"]);
}

// ---- events

#[tokio::test(flavor = "multi_thread")]
async fn emits_events_in_order_with_the_ts_shapes() {
    let s = server().await;
    let items = vec![req(
        "r1",
        &format!("{}/x", s.uri()),
        json!({"postResponseScript": "nx.test('ok', () => {}); nx.test('bad', () => { throw new Error('boom'); });"}),
    )];
    let (summary, events) = run_collect(&runner(), config(items, json!({"iterations": 2}))).await;
    assert_eq!(
        types(&events),
        [
            "iteration-start",
            "request-start",
            "request-complete",
            "test-result",
            "test-result",
            "iteration-complete",
            "iteration-start",
            "request-start",
            "request-complete",
            "test-result",
            "test-result",
            "iteration-complete",
            "run-complete",
        ]
    );
    assert_eq!(events[0]["data"], json!({"iteration": 0, "total": 2}));
    assert_eq!(
        events[1]["data"],
        json!({"requestId": "r1", "name": "r1", "iteration": 0})
    );
    let done = &events[2]["data"];
    assert_eq!(done["requestId"], "r1");
    assert_eq!(done["requestName"], "r1");
    assert_eq!(done["method"], "GET");
    assert_eq!(done["status"], 200);
    assert!(done["responseTime"].is_number() && done["size"].as_u64().unwrap() > 0);
    assert!(done.get("error").is_none());
    assert_eq!(done["scriptLogs"], json!([]));
    assert_eq!(events[3]["data"]["name"], "ok");
    assert_eq!(events[4]["data"]["passed"], false);
    assert_eq!(events[5]["data"], json!({"iteration": 0}));
    let last = events.last().unwrap();
    assert_eq!(last["data"]["totalRequests"], 2);
    assert_eq!(last["data"]["totalTests"], 4);
    assert_eq!(last["data"]["passedTests"], 2);
    assert_eq!(last["data"]["failedTests"], 2);
    assert_eq!(last["data"]["aborted"], false);
    assert_eq!(summary.total_tests, 4);
    assert_eq!(serde_json::to_value(&summary).unwrap(), last["data"]);
}

#[tokio::test]
async fn error_event_shape() {
    let e = serde_json::to_value(RunEvent::Error {
        message: "m".into(),
        request_id: "r".into(),
    })
    .unwrap();
    assert_eq!(
        e,
        json!({"type": "error", "data": {"message": "m", "requestId": "r"}})
    );
}

// ---- delay

#[tokio::test(flavor = "multi_thread")]
async fn waits_delay_ms_between_requests_but_not_after_the_last() {
    let s = server().await;
    let u = s.uri();
    let items = vec![
        req("a", &format!("{u}/a"), json!({})),
        req("b", &format!("{u}/b"), json!({})),
    ];
    let start = Instant::now();
    run(config(items.clone(), json!({"delayMs": 150}))).await;
    let one_gap = start.elapsed();
    assert!(one_gap >= Duration::from_millis(150), "{one_gap:?}");
    assert!(one_gap < Duration::from_millis(290), "{one_gap:?}");
}

// ---- cancel

#[tokio::test(flavor = "multi_thread")]
async fn cancel_stops_before_the_next_iteration() {
    let s = server().await;
    let r = runner();
    let token = r.cancel_token().clone();
    let events: Events = Arc::default();
    let sink = events.clone();
    let cfg = config(
        vec![req("a", &format!("{}/a", s.uri()), json!({}))],
        json!({"iterations": 5}),
    );
    let summary = r
        .run(cfg, move |e| {
            if let RunEvent::IterationComplete { iteration: 1 } = e {
                token.cancel();
            }
            sink.lock().unwrap().push(serde_json::to_value(&e).unwrap());
        })
        .await;
    assert_eq!(summary.total_requests, 2);
    assert!(summary.aborted);
    assert_eq!(
        events.lock().unwrap().last().unwrap()["data"]["aborted"],
        true
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_stops_between_requests() {
    let s = server().await;
    let u = s.uri();
    let r = runner();
    let token = r.cancel_token().clone();
    let items = vec![
        req("a", &format!("{u}/a"), json!({})),
        req("b", &format!("{u}/b"), json!({})),
        req("c", &format!("{u}/c"), json!({})),
    ];
    let summary = r
        .run(config(items, json!({})), move |e| {
            if let RunEvent::RequestComplete(res) = &e {
                if res.request_id == "a" {
                    token.cancel();
                }
            }
        })
        .await;
    assert_eq!(summary.total_requests, 1);
    assert!(summary.aborted);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_interrupts_the_delay() {
    let s = server().await;
    let u = s.uri();
    let r = runner();
    let token = r.cancel_token().clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        token.cancel();
    });
    let items = vec![
        req("a", &format!("{u}/a"), json!({})),
        req("b", &format!("{u}/b"), json!({})),
    ];
    let start = Instant::now();
    let summary = r.run(config(items, json!({"delayMs": 5000})), |_| {}).await;
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(summary.total_requests, 1);
    assert!(summary.aborted);
}

#[tokio::test(flavor = "multi_thread")]
async fn aborted_state_is_reset_for_the_next_run() {
    let s = server().await;
    let r = runner();
    r.cancel_token().cancel();
    let cfg = config(
        vec![req("a", &format!("{}/a", s.uri()), json!({}))],
        json!({}),
    );
    let summary = r.run(cfg, |_| {}).await;
    assert_eq!(summary.total_requests, 1);
    assert!(!summary.aborted);
}

// ---- error handling

#[tokio::test(flavor = "multi_thread")]
async fn request_errors_are_reported_and_the_run_continues() {
    let s = server().await;
    let items = vec![
        req("bad", "http://127.0.0.1:1/nope", json!({})),
        req("good", &format!("{}/g", s.uri()), json!({})),
    ];
    let summary = run(config(items, json!({}))).await;
    assert_eq!(summary.total_requests, 2);
    let bad = &summary.results[0];
    assert_eq!(bad.status, 0);
    assert_eq!(bad.status_text, "Request failed");
    assert!(bad.error.is_some());
    assert_eq!(bad.url, "http://127.0.0.1:1/nope");
    assert_eq!(summary.results[1].status, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_on_error_stops_the_iteration_and_emits_an_error_event() {
    let s = server().await;
    let items = vec![
        req("bad", "http://127.0.0.1:1/nope", json!({})),
        req("good", &format!("{}/g", s.uri()), json!({})),
    ];
    let (summary, events) =
        run_collect(&runner(), config(items, json!({"stopOnError": true}))).await;
    assert_eq!(summary.total_requests, 1);
    let err = events.iter().find(|e| e["type"] == "error").unwrap();
    assert_eq!(err["data"]["requestId"], "bad");
    assert!(err["data"]["message"].is_string());
    assert!(paths(&s).is_empty());
    // stopOnError only ends the iteration, the next one still starts (TS parity)
    let again = run(config(
        vec![req("bad", "http://127.0.0.1:1/x", json!({}))],
        json!({"stopOnError": true, "iterations": 2}),
    ))
    .await;
    assert_eq!(again.total_requests, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_on_error_also_triggers_on_failed_tests() {
    let s = server().await;
    let u = s.uri();
    let items = vec![
        req(
            "a",
            &format!("{u}/a"),
            json!({"postResponseScript": "nx.test('x', () => { throw new Error('no'); });"}),
        ),
        req("b", &format!("{u}/b"), json!({})),
    ];
    let summary = run(config(items.clone(), json!({"stopOnError": true}))).await;
    assert_eq!(summary.total_requests, 1);
    assert_eq!(summary.failed_tests, 1);
    let summary = run(config(items, json!({"stopOnError": false}))).await;
    assert_eq!(summary.total_requests, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_graphql_variables_become_a_request_error() {
    let items = vec![req(
        "g",
        "http://127.0.0.1:1/",
        json!({"method": "POST", "body": {"mode": "graphql", "graphql": {"query": "{a}", "variables": "{nope"}}, "preRequestScript": "// x"}),
    )];
    let summary = run(config(items, json!({}))).await;
    assert_eq!(summary.results[0].status_text, "Request failed");
    assert!(summary.results[0].error.is_some());
}

// ---- summary

#[tokio::test(flavor = "multi_thread")]
async fn summary_counts_time_and_zero_tests() {
    let s = server().await;
    let summary = run(config(
        vec![req("a", &format!("{}/a", s.uri()), json!({}))],
        json!({}),
    ))
    .await;
    assert_eq!(
        (
            summary.total_tests,
            summary.passed_tests,
            summary.failed_tests
        ),
        (0, 0, 0)
    );
    assert!(summary.total_time < 5000);
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_collection_still_emits_iteration_events() {
    let (summary, events) = run_collect(&runner(), config(vec![], json!({"iterations": 2}))).await;
    assert_eq!(summary.total_requests, 0);
    assert!(summary.results.is_empty());
    assert_eq!(
        types(&events),
        [
            "iteration-start",
            "iteration-complete",
            "iteration-start",
            "iteration-complete",
            "run-complete"
        ]
    );
}

// ---- data rows

#[tokio::test(flavor = "multi_thread")]
async fn cycles_data_rows_across_iterations() {
    let s = server().await;
    let rows = json!([{"id": "1"}, {"id": "2"}]);
    run(config(
        vec![req("a", &format!("{}/u/{{{{id}}}}", s.uri()), json!({}))],
        json!({"iterations": 3, "dataRows": rows}),
    ))
    .await;
    assert_eq!(paths(&s), ["/u/1", "/u/2", "/u/1"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn data_variables_win_over_environment_and_empty_rows_mean_one_empty_row() {
    let s = server().await;
    let env = json!([{"key": "id", "value": "env", "type": "string", "enabled": true}]);
    let items = vec![req("a", &format!("{}/u/{{{{id}}}}", s.uri()), json!({}))];
    run(config(
        items.clone(),
        json!({"environmentVars": env, "dataRows": [{"id": "row"}]}),
    ))
    .await;
    run(config(
        items,
        json!({"environmentVars": env, "dataRows": [], "iterations": 2}),
    ))
    .await;
    assert_eq!(paths(&s), ["/u/row", "/u/env", "/u/env"]);
}

#[test]
fn parses_csv_data_files() {
    let rows = parse_csv("\u{feff}id,name\r\n1,\"Smith, Jo\"\r\n2,\"say \"\"hi\"\"\"\r\n\r\n3\r\n")
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["id"], "1");
    assert_eq!(rows[0]["name"], "Smith, Jo");
    assert_eq!(rows[1]["name"], "say \"hi\"");
    assert_eq!(rows[2]["id"], "3");
    assert_eq!(rows[2]["name"], "");
    assert!(parse_csv("only,header\n").unwrap().is_empty());
}

#[test]
fn parses_json_data_files() {
    let rows = parse_json(r#"[{"a": "x", "n": 5, "t": true, "z": null, "o": {"k": 1}}]"#).unwrap();
    assert_eq!(rows[0]["a"], "x");
    assert_eq!(rows[0]["n"], "5");
    assert_eq!(rows[0]["t"], "true");
    assert_eq!(rows[0]["z"], "");
    assert_eq!(rows[0]["o"], r#"{"k":1}"#);
    assert_eq!(parse_json(r#"{"a":"1"}"#).unwrap().len(), 1);
    assert!(parse_json("[1,2]").is_err());
    assert!(parse_json("nope").is_err());
}

#[test]
fn data_file_type_comes_from_the_extension() {
    assert_eq!(parse_data_file("D.CSV", "a\n1\n").unwrap()[0]["a"], "1");
    assert_eq!(
        parse_data_file("d.json", r#"[{"a":"1"}]"#).unwrap()[0]["a"],
        "1"
    );
    assert!(parse_data_file("d.txt", "").is_err());
}

// ---- result shape / scripts / flow control

#[tokio::test(flavor = "multi_thread")]
async fn script_logs_and_pre_request_mutations_are_reported() {
    let s = server().await;
    let items = vec![req(
        "a",
        &format!("{}/old", s.uri()),
        json!({
            "preRequestScript": format!("console.log('pre'); nx.request.url = '{}/new'; nx.request.headers['X-T'] = '1';", s.uri()),
            "postResponseScript": "console.log('post', nx.response.code);",
        }),
    )];
    let summary = run(config(items, json!({}))).await;
    assert_eq!(paths(&s), ["/new"]);
    let r = &summary.results[0];
    assert!(r.url.ends_with("/new"));
    assert_eq!(r.script_logs.len(), 2);
    assert_eq!(r.script_logs[1].args, vec![json!("post"), json!(200)]);
    let got = s.received_requests().await.unwrap();
    assert_eq!(got[0].headers.get("x-t").unwrap().to_str().unwrap(), "1");
}

#[tokio::test(flavor = "multi_thread")]
async fn pre_request_script_errors_skip_the_send() {
    let s = server().await;
    let items = vec![req(
        "a",
        &format!("{}/a", s.uri()),
        json!({"preRequestScript": "throw new Error('bad pre');"}),
    )];
    let summary = run(config(items, json!({}))).await;
    let r = &summary.results[0];
    assert_eq!(r.status, 0);
    assert_eq!(r.status_text, "Pre-request script error");
    assert_eq!(r.error.as_deref(), Some("bad pre"));
    assert!(paths(&s).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn script_timeouts_are_small_and_reported() {
    let s = server().await;
    let r = CollectionRunner::new(50, CancelToken::new());
    let items = vec![req(
        "a",
        &format!("{}/a", s.uri()),
        json!({"preRequestScript": "while (true) {}"}),
    )];
    let (summary, _) = run_collect(&r, config(items, json!({}))).await;
    assert!(summary.results[0]
        .error
        .as_deref()
        .unwrap()
        .contains("timed out"));
}

#[tokio::test(flavor = "multi_thread")]
async fn collection_folder_and_request_scripts_run_in_the_documented_order() {
    let s = server().await;
    let items = vec![folder(
        "f",
        vec![req(
            "a",
            &format!("{}/a", s.uri()),
            json!({"preRequestScript": "console.log('req-pre');", "postResponseScript": "console.log('req-post');"}),
        )],
        json!({"preRequestScript": "console.log('folder-pre');", "postResponseScript": "console.log('folder-post');"}),
    )];
    let cfg = config(
        items,
        json!({"collectionExtra": {"preRequestScript": "console.log('coll-pre');", "postResponseScript": "console.log('coll-post');"}}),
    );
    let summary = run(cfg).await;
    let order: Vec<_> = summary.results[0]
        .script_logs
        .iter()
        .map(|l| l.args[0].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        order,
        [
            "coll-pre",
            "folder-pre",
            "req-pre",
            "req-post",
            "folder-post",
            "coll-post"
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn variable_changes_accumulate_across_requests_and_iterations() {
    let s = server().await;
    let u = s.uri();
    let items = vec![
        req(
            "a",
            &format!("{u}/a"),
            json!({"postResponseScript": "nx.environment.set('tok', 'abc'); nx.collectionVariables.set('n', '1'); nx.globals.set('g', 'G');"}),
        ),
        req(
            "b",
            &format!("{u}/b/{{{{tok}}}}/{{{{n}}}}/{{{{g}}}}"),
            json!({"postResponseScript": "nx.environment.set('tok', nx.environment.get('tok') + '!'); nx.globals.unset('g');"}),
        ),
        req("c", &format!("{u}/c/{{{{tok}}}}/{{{{g}}}}"), json!({})),
    ];
    run(config(items, json!({"iterations": 2}))).await;
    let got = paths(&s);
    assert_eq!(got[..3], ["/a", "/b/abc/1/G", "/c/abc!/%7B%7Bg%7D%7D"]);
    assert_eq!(got[3..], ["/a", "/b/abc/1/G", "/c/abc!/%7B%7Bg%7D%7D"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn set_next_request_jumps_by_name_and_null_stops() {
    let s = server().await;
    let u = s.uri();
    let jump = vec![
        req(
            "first",
            &format!("{u}/1"),
            json!({"postResponseScript": "nx.execution.setNextRequest('third');"}),
        ),
        req("second", &format!("{u}/2"), json!({})),
        req("third", &format!("{u}/3"), json!({})),
    ];
    run(config(jump, json!({}))).await;
    assert_eq!(paths(&s), ["/1", "/3"]);

    let s2 = server().await;
    let u2 = s2.uri();
    let stop = vec![
        req(
            "first",
            &format!("{u2}/1"),
            json!({"postResponseScript": "nx.execution.setNextRequest(null);"}),
        ),
        req("second", &format!("{u2}/2"), json!({})),
    ];
    let summary = run(config(stop, json!({"iterations": 2}))).await;
    // null ends only the current iteration
    assert_eq!(paths(&s2), ["/1", "/1"]);
    assert_eq!(summary.total_requests, 2);

    let s3 = server().await;
    let unknown = vec![
        req(
            "first",
            &format!("{}/1", s3.uri()),
            json!({"postResponseScript": "nx.execution.setNextRequest('nope');"}),
        ),
        req("second", &format!("{}/2", s3.uri()), json!({})),
    ];
    run(config(unknown, json!({}))).await;
    assert_eq!(paths(&s3), ["/1"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn skip_request_does_not_send_and_reports_skipped() {
    let s = server().await;
    let u = s.uri();
    let items = vec![
        req(
            "a",
            &format!("{u}/a"),
            json!({"preRequestScript": "nx.execution.skipRequest();"}),
        ),
        req("b", &format!("{u}/b"), json!({})),
    ];
    let summary = run(config(items, json!({}))).await;
    assert_eq!(paths(&s), ["/b"]);
    assert_eq!(summary.total_requests, 2);
    assert_eq!(summary.results[0].status_text, "Skipped");
    assert!(summary.results[0].error.is_none());
}

// ---- auth inheritance

async fn auth_header_of(items: Vec<Value>, extra: Value, s: &MockServer) -> Vec<Option<String>> {
    run(config(items, extra)).await;
    s.received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| {
            r.headers
                .get("authorization")
                .map(|v| v.to_str().unwrap().to_string())
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn inherits_auth_from_folder_then_collection() {
    let s = server().await;
    let u = s.uri();
    let items = vec![
        req("a", &format!("{u}/a"), json!({})),
        folder(
            "f",
            vec![
                req("b", &format!("{u}/b"), json!({})),
                req("c", &format!("{u}/c"), json!({"auth": {"type": "none"}})),
                req(
                    "d",
                    &format!("{u}/d"),
                    json!({"auth": {"type": "bearer", "token": "own"}}),
                ),
            ],
            json!({"auth": {"type": "bearer", "token": "folder"}}),
        ),
    ];
    let got = auth_header_of(
        items,
        json!({"collectionExtra": {"auth": {"type": "bearer", "token": "coll"}}}),
        &s,
    )
    .await;
    assert_eq!(
        got,
        [
            Some("Bearer coll".to_string()),
            Some("Bearer folder".to_string()),
            None,
            Some("Bearer own".to_string())
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn enabled_headers_and_raw_body_are_resolved_and_sent() {
    let s = server().await;
    let env = json!([{"key": "who", "value": "me", "type": "string", "enabled": true}]);
    let items = vec![req(
        "a",
        &format!("{}/a", s.uri()),
        json!({
            "method": "POST",
            "headers": [
                {"key": "X-Who", "value": "{{who}}", "enabled": true},
                {"key": "X-Off", "value": "no", "enabled": false}
            ],
            "body": {"mode": "json", "raw": "{\"u\":\"{{who}}\"}"},
        }),
    )];
    run(config(items, json!({"environmentVars": env}))).await;
    let got = &s.received_requests().await.unwrap()[0];
    assert_eq!(got.headers.get("x-who").unwrap().to_str().unwrap(), "me");
    assert!(got.headers.get("x-off").is_none());
    assert_eq!(got.body, br#"{"u":"me"}"#);
}

// ---- trust / auth origin binding

fn auth_query() -> Value {
    json!({"type": "api-key", "key": "k", "value": "v", "addTo": "query"})
}

#[tokio::test(flavor = "multi_thread")]
async fn origin_change_by_script_drops_auth_in_the_runner() {
    let (good, evil) = (server().await, server().await);
    let script = format!("nx.request.url = '{}/steal';", evil.uri());
    let summary = run(config(
        vec![req(
            "r1",
            &format!("{}/a", good.uri()),
            json!({"auth": auth_query(), "preRequestScript": script}),
        )],
        json!({}),
    ))
    .await;
    let reqs = evil.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].url.query(), None);
    assert!(summary.results[0]
        .script_logs
        .iter()
        .any(|l| l.args[0].as_str().unwrap().contains("Auth not sent")));
}

#[tokio::test(flavor = "multi_thread")]
async fn same_origin_keeps_auth_in_the_runner() {
    let good = server().await;
    let script = format!("nx.request.url = '{}/b';", good.uri());
    run(config(
        vec![req(
            "r1",
            &format!("{}/a", good.uri()),
            json!({"auth": auth_query(), "preRequestScript": script}),
        )],
        json!({}),
    ))
    .await;
    let reqs = good.received_requests().await.unwrap();
    assert_eq!(reqs[0].url.path(), "/b");
    assert_eq!(reqs[0].url.query(), Some("k=v"));
}

fn trust_config(s: &MockServer, untrusted: bool) -> RunConfig {
    config(
        vec![req(
            "r1",
            &format!("{}/a", s.uri()),
            json!({
                "headers": [{"key": "X-Tok", "value": "{{tok}}", "enabled": true}],
                "preRequestScript": "nx.request.headers['X-Script'] = 'ran';",
                "postResponseScript": "nx.test('t', () => {});",
            }),
        )],
        json!({
            "untrusted": untrusted,
            "environmentVars": [{"key": "tok", "value": "S3CRET", "type": "secret", "enabled": true}],
            "collectionExtra": {"preRequestScript": "nx.request.headers['X-Coll'] = 'ran';"},
        }),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn untrusted_collection_skips_scripts_and_withholds_secrets_in_the_runner() {
    let s = server().await;
    let summary = run(trust_config(&s, true)).await;
    let r = &s.received_requests().await.unwrap()[0];
    assert_eq!(r.headers.get("x-tok").unwrap().to_str().unwrap(), "{{tok}}");
    assert!(r.headers.get("x-script").is_none() && r.headers.get("x-coll").is_none());
    assert!(summary.results[0].test_results.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn trusted_collection_runs_scripts_and_resolves_secrets_in_the_runner() {
    let s = server().await;
    let summary = run(trust_config(&s, false)).await;
    let r = &s.received_requests().await.unwrap()[0];
    assert_eq!(r.headers.get("x-tok").unwrap().to_str().unwrap(), "S3CRET");
    assert!(r.headers.get("x-script").is_some() && r.headers.get("x-coll").is_some());
    assert_eq!(summary.results[0].test_results.len(), 1);
}
