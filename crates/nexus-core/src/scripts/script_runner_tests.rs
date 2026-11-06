//! Port of `script-runner.test.ts`.

use indexmap::IndexMap;
use serde_json::{json, Value};

use super::*;
use crate::types::*;

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

fn make_request(f: impl FnOnce(&mut NexusRequest)) -> NexusRequest {
    let mut r = create_default_request();
    r.id = "req-1".into();
    r.name = "Test Request".into();
    r.method = HttpMethod::Get;
    r.url = "https://api.example.com/data".into();
    r.headers = vec![KeyValuePair {
        key: "Accept".into(),
        value: "application/json".into(),
        enabled: true,
        description: None,
    }];
    f(&mut r);
    r
}

fn make_folder(id: &str, f: impl FnOnce(&mut NexusFolder)) -> NexusFolder {
    let mut folder = NexusFolder {
        id: id.into(),
        name: id.into(),
        description: None,
        items: vec![],
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        item_order: None,
    };
    f(&mut folder);
    folder
}

fn make_collection(f: impl FnOnce(&mut NexusCollection)) -> NexusCollection {
    let mut c = NexusCollection {
        id: "coll-1".into(),
        name: "Test Collection".into(),
        description: None,
        version: None,
        variables: vec![],
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        items: vec![],
        item_order: None,
    };
    f(&mut c);
    c
}

fn make_response(f: impl FnOnce(&mut ResponseAccess)) -> ResponseAccess {
    let mut r = ResponseAccess::new(
        200,
        "OK",
        IndexMap::from([("content-type".to_string(), json!("application/json"))]),
        50.0,
        r#"{"result":"ok"}"#,
    );
    f(&mut r);
    r
}

fn pre(coll: &NexusCollection, req: &NexusRequest, folders: &[&NexusFolder]) -> ScriptChainResult {
    ScriptRunner::default()
        .run_pre_request_chain(coll, req, folders, &[], &[])
        .unwrap()
}

fn post(
    coll: &NexusCollection,
    req: &NexusRequest,
    folders: &[&NexusFolder],
    resp: &ResponseAccess,
) -> Vec<ScriptResult> {
    ScriptRunner::default()
        .run_post_response_chain(coll, req, folders, &[], &[], resp)
        .unwrap()
}

fn log(r: &ScriptResult, i: usize) -> &Vec<Value> {
    &r.logs[i].args
}

// ---- runPreRequestChain ----

#[test]
fn pre_returns_empty_results_when_no_scripts_exist() {
    let out = pre(&make_collection(|_| {}), &make_request(|_| {}), &[]);
    assert!(out.results.is_empty());
    assert_eq!(out.request.url, "https://api.example.com/data");
}

#[test]
fn pre_runs_collection_script() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(r#"console.log("collection pre");"#.into())
    });
    let out = pre(&coll, &make_request(|_| {}), &[]);
    assert_eq!(out.results.len(), 1);
    assert_eq!(log(&out.results[0], 0), &vec![json!("collection pre")]);
}

#[test]
fn pre_runs_scripts_in_order_collection_folder_request() {
    let coll = make_collection(|c| c.pre_request_script = Some(r#"console.log("coll");"#.into()));
    let folder = make_folder("folder-1", |f| {
        f.pre_request_script = Some(r#"console.log("folder");"#.into())
    });
    let req = make_request(|r| r.pre_request_script = Some(r#"console.log("request");"#.into()));
    let out = pre(&coll, &req, &[&folder]);
    let order: Vec<_> = out
        .results
        .iter()
        .map(|r| log(r, 0)[0].as_str().unwrap().to_string())
        .collect();
    assert_eq!(order, ["coll", "folder", "request"]);
}

#[test]
fn pre_runs_multiple_folder_scripts_in_nesting_order() {
    let outer = make_folder("outer", |f| {
        f.pre_request_script = Some(r#"console.log("outer");"#.into())
    });
    let inner = make_folder("inner", |f| {
        f.pre_request_script = Some(r#"console.log("inner");"#.into())
    });
    let req = make_request(|r| r.pre_request_script = Some(r#"console.log("req");"#.into()));
    let out = pre(&make_collection(|_| {}), &req, &[&outer, &inner]);
    let order: Vec<_> = out
        .results
        .iter()
        .map(|r| log(r, 0)[0].as_str().unwrap().to_string())
        .collect();
    assert_eq!(order, ["outer", "inner", "req"]);
}

#[test]
fn pre_applies_request_mutations() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(r#"nx.request.url = "https://mutated.com/api";"#.into())
    });
    let out = pre(&coll, &make_request(|_| {}), &[]);
    assert_eq!(out.request.url, "https://mutated.com/api");
}

#[test]
fn pre_chains_mutations_across_scripts() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(r#"nx.request.url = nx.request.url + "/v2";"#.into())
    });
    let req = make_request(|r| {
        r.url = "https://api.com".into();
        r.pre_request_script = Some(r#"nx.request.url = nx.request.url + "/final";"#.into());
    });
    assert_eq!(
        pre(&coll, &req, &[]).request.url,
        "https://api.com/v2/final"
    );
}

#[test]
fn pre_applies_method_header_and_body_mutations() {
    let req = make_request(|r| {
        r.pre_request_script = Some(
            r#"nx.request.method = "POST"; nx.request.headers["X-A"] = "1"; nx.request.body = "{}";"#.into(),
        )
    });
    let out = pre(&make_collection(|_| {}), &req, &[]);
    assert_eq!(out.request.method, "POST");
    assert_eq!(out.request.headers["X-A"], "1");
    assert_eq!(out.request.headers["Accept"], "application/json");
    assert_eq!(out.request.body.as_deref(), Some("{}"));
}

#[test]
fn pre_stops_chain_when_script_sets_execution_to_skipped() {
    let coll = make_collection(|c| {
        c.pre_request_script =
            Some(r#"console.log("coll runs"); nx.execution.skipRequest();"#.into())
    });
    let req =
        make_request(|r| r.pre_request_script = Some(r#"console.log("should not run");"#.into()));
    let out = pre(&coll, &req, &[]);
    assert_eq!(out.results.len(), 1);
    assert_eq!(log(&out.results[0], 0), &vec![json!("coll runs")]);
    assert!(out.results[0].execution_control.skipped);
}

#[test]
fn pre_propagates_variable_changes_across_scripts() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(r#"nx.environment.set("token", "abc");"#.into())
    });
    let req = make_request(|r| {
        r.pre_request_script = Some(r#"console.log(nx.environment.get("token"));"#.into())
    });
    let out = pre(&coll, &req, &[]);
    assert_eq!(out.results.len(), 2);
    assert_eq!(log(&out.results[1], 0), &vec![json!("abc")]);
}

#[test]
fn pre_populates_request_headers_from_enabled_headers_only() {
    let req = make_request(|r| {
        r.headers = vec![
            KeyValuePair {
                key: "Enabled".into(),
                value: "yes".into(),
                enabled: true,
                description: None,
            },
            KeyValuePair {
                key: "Disabled".into(),
                value: "no".into(),
                enabled: false,
                description: None,
            },
        ];
        r.pre_request_script =
            Some(r#"console.log(nx.request.headers["Enabled"]); console.log(nx.request.headers["Disabled"]);"#.into());
    });
    let out = pre(&make_collection(|_| {}), &req, &[]);
    assert_eq!(log(&out.results[0], 0), &vec![json!("yes")]);
    assert_eq!(log(&out.results[0], 1), &vec![Value::Null]);
}

#[test]
fn pre_handles_script_errors_without_crashing_the_chain() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(r#"throw new Error("coll error");"#.into())
    });
    let req = make_request(|r| r.pre_request_script = Some(r#"console.log("still runs");"#.into()));
    let out = pre(&coll, &req, &[]);
    assert_eq!(out.results.len(), 2);
    assert!(out.results[0]
        .error
        .as_ref()
        .unwrap()
        .contains("coll error"));
    assert_eq!(log(&out.results[1], 0), &vec![json!("still runs")]);
}

#[test]
fn pre_uses_collection_variables_for_the_collection_scope() {
    let coll = make_collection(|c| {
        c.variables = make_vars(&[("apiUrl", "https://api.com")]);
        c.pre_request_script = Some(r#"console.log(nx.collectionVariables.get("apiUrl"));"#.into());
    });
    let out = pre(&coll, &make_request(|_| {}), &[]);
    assert_eq!(log(&out.results[0], 0), &vec![json!("https://api.com")]);
}

#[test]
fn pre_sets_a_collection_variable_and_reads_it_in_a_later_script() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(r#"nx.collectionVariables.set("step", "init");"#.into())
    });
    let req = make_request(|r| {
        r.pre_request_script = Some(r#"console.log(nx.collectionVariables.get("step"));"#.into())
    });
    let out = pre(&coll, &req, &[]);
    assert_eq!(out.results.len(), 2);
    assert_eq!(log(&out.results[1], 0), &vec![json!("init")]);
}

#[test]
fn pre_records_collection_variable_changes() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(r#"nx.collectionVariables.set("token", "coll-token");"#.into())
    });
    let out = pre(&coll, &make_request(|_| {}), &[]);
    assert!(out.results[0].variable_changes.contains(&VariableChange {
        scope: "collection".into(),
        action: VariableChangeAction::Set,
        key: "token".into(),
        value: Some("coll-token".into()),
    }));
}

#[test]
fn pre_overwrites_a_collection_variable_across_scripts() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(r#"nx.collectionVariables.set("phase", "pre-coll");"#.into())
    });
    let req = make_request(|r| {
        r.pre_request_script =
            Some(r#"nx.collectionVariables.set("phase", "pre-req"); console.log(nx.collectionVariables.get("phase"));"#.into())
    });
    let out = pre(&coll, &req, &[]);
    assert_eq!(log(&out.results[1], 0), &vec![json!("pre-req")]);
}

#[test]
fn pre_uses_env_vars_and_global_vars_for_their_scopes() {
    let coll = make_collection(|c| {
        c.pre_request_script = Some(
            r#"console.log(nx.environment.get("env_key")); console.log(nx.globals.get("global_key"));"#.into(),
        )
    });
    let out = ScriptRunner::default()
        .run_pre_request_chain(
            &coll,
            &make_request(|_| {}),
            &[],
            &make_vars(&[("env_key", "env_val")]),
            &make_vars(&[("global_key", "global_val")]),
        )
        .unwrap();
    assert_eq!(log(&out.results[0], 0), &vec![json!("env_val")]);
    assert_eq!(log(&out.results[0], 1), &vec![json!("global_val")]);
}

#[test]
fn pre_skips_empty_scripts() {
    let coll = make_collection(|c| c.pre_request_script = Some(String::new()));
    assert!(pre(&coll, &make_request(|_| {}), &[]).results.is_empty());
}

#[test]
fn runner_respects_its_timeout() {
    let req = make_request(|r| r.pre_request_script = Some("while (true) {}".into()));
    let out = ScriptRunner::new(50)
        .run_pre_request_chain(&make_collection(|_| {}), &req, &[], &[], &[])
        .unwrap();
    assert!(out.results[0].error.as_ref().unwrap().contains("timed out"));
}

// ---- runPostResponseChain ----

#[test]
fn post_returns_empty_results_when_no_scripts_exist() {
    assert!(post(
        &make_collection(|_| {}),
        &make_request(|_| {}),
        &[],
        &make_response(|_| {})
    )
    .is_empty());
}

#[test]
fn post_runs_scripts_request_then_folders_inner_to_outer_then_collection() {
    let coll = make_collection(|c| c.post_response_script = Some(r#"console.log("coll");"#.into()));
    let outer = make_folder("outer", |f| {
        f.post_response_script = Some(r#"console.log("outer");"#.into())
    });
    let inner = make_folder("inner", |f| {
        f.post_response_script = Some(r#"console.log("inner");"#.into())
    });
    let req = make_request(|r| r.post_response_script = Some(r#"console.log("request");"#.into()));
    let results = post(&coll, &req, &[&outer, &inner], &make_response(|_| {}));
    let order: Vec<_> = results
        .iter()
        .map(|r| log(r, 0)[0].as_str().unwrap().to_string())
        .collect();
    assert_eq!(order, ["request", "inner", "outer", "coll"]);
}

#[test]
fn post_provides_response_data_to_scripts() {
    let response = make_response(|r| {
        r.code = 404;
        r.status = "Not Found".into();
    });
    let req = make_request(|r| {
        r.post_response_script =
            Some("console.log(nx.response.code); console.log(nx.response.status);".into())
    });
    let results = post(&make_collection(|_| {}), &req, &[], &response);
    assert_eq!(log(&results[0], 0), &vec![json!(404)]);
    assert_eq!(log(&results[0], 1), &vec![json!("Not Found")]);
}

#[test]
fn post_allows_test_assertions_against_response() {
    let req = make_request(|r| {
        r.post_response_script = Some(
            r#"nx.test("status is 200", () => { nx.expect(nx.response.code).to.equal(200); });"#
                .into(),
        )
    });
    let results = post(&make_collection(|_| {}), &req, &[], &make_response(|_| {}));
    assert_eq!(results[0].test_results.len(), 1);
    assert!(results[0].test_results[0].passed);
}

#[test]
fn post_propagates_variable_changes_across_scripts() {
    let req = make_request(|r| {
        r.post_response_script = Some(r#"nx.environment.set("extracted", "fromResp");"#.into())
    });
    let coll = make_collection(|c| {
        c.post_response_script = Some(r#"console.log(nx.environment.get("extracted"));"#.into())
    });
    let results = post(&coll, &req, &[], &make_response(|_| {}));
    assert_eq!(results.len(), 2);
    assert_eq!(log(&results[1], 0), &vec![json!("fromResp")]);
}

#[test]
fn post_sets_a_collection_variable_and_reads_it_in_a_later_script() {
    let req = make_request(|r| {
        r.post_response_script =
            Some(r#"nx.collectionVariables.set("lastStatus", String(nx.response.code));"#.into())
    });
    let coll = make_collection(|c| {
        c.post_response_script =
            Some(r#"console.log(nx.collectionVariables.get("lastStatus"));"#.into())
    });
    let results = post(&coll, &req, &[], &make_response(|r| r.code = 201));
    assert_eq!(results.len(), 2);
    assert_eq!(log(&results[1], 0), &vec![json!("201")]);
}

#[test]
fn post_parses_response_json_in_scripts() {
    let response = ResponseAccess::new(200, "OK", IndexMap::new(), 1.0, r#"{"items":[1,2,3]}"#);
    let req = make_request(|r| {
        r.post_response_script =
            Some("const data = nx.response.json(); console.log(data.items.length);".into())
    });
    let results = post(&make_collection(|_| {}), &req, &[], &response);
    assert_eq!(log(&results[0], 0), &vec![json!(3)]);
}

#[test]
fn post_handles_script_errors_without_crashing_the_chain() {
    let req =
        make_request(|r| r.post_response_script = Some(r#"throw new Error("req error");"#.into()));
    let coll = make_collection(|c| {
        c.post_response_script = Some(r#"console.log("coll still runs");"#.into())
    });
    let results = post(&coll, &req, &[], &make_response(|_| {}));
    assert_eq!(results.len(), 2);
    assert!(results[0].error.as_ref().unwrap().contains("req error"));
    assert_eq!(log(&results[1], 0), &vec![json!("coll still runs")]);
}

// ---- findParentFolders ----

fn req_item(id: &str) -> CollectionItem {
    CollectionItem::Request(make_request(|r| r.id = id.into()))
}

fn folder_item(id: &str, items: Vec<CollectionItem>) -> CollectionItem {
    CollectionItem::Folder(make_folder(id, |f| f.items = items))
}

fn ids(folders: &[&NexusFolder]) -> Vec<String> {
    folders.iter().map(|f| f.id.clone()).collect()
}

#[test]
fn find_parents_returns_empty_for_a_top_level_request() {
    let coll = make_collection(|c| c.items = vec![req_item("r1")]);
    assert!(ScriptRunner::default()
        .find_parent_folders(&coll, "r1")
        .is_empty());
}

#[test]
fn find_parents_returns_the_parent_folder_for_a_nested_request() {
    let coll = make_collection(|c| c.items = vec![folder_item("f1", vec![req_item("r1")])]);
    assert_eq!(
        ids(&ScriptRunner::default().find_parent_folders(&coll, "r1")),
        ["f1"]
    );
}

#[test]
fn find_parents_returns_folders_root_to_parent_for_deep_requests() {
    let coll = make_collection(|c| {
        c.items = vec![folder_item(
            "a",
            vec![folder_item(
                "b",
                vec![folder_item("c", vec![req_item("deep")])],
            )],
        )]
    });
    assert_eq!(
        ids(&ScriptRunner::default().find_parent_folders(&coll, "deep")),
        ["a", "b", "c"]
    );
}

#[test]
fn find_parents_returns_empty_when_request_id_is_not_found() {
    let coll = make_collection(|c| c.items = vec![folder_item("f1", vec![req_item("r1")])]);
    assert!(ScriptRunner::default()
        .find_parent_folders(&coll, "nope")
        .is_empty());
}

#[test]
fn find_parents_finds_requests_among_siblings_and_skips_other_branches() {
    let coll = make_collection(|c| {
        c.items = vec![
            folder_item("other", vec![req_item("x")]),
            folder_item("f1", vec![req_item("r1"), req_item("r2"), req_item("r3")]),
        ]
    });
    assert_eq!(
        ids(&ScriptRunner::default().find_parent_folders(&coll, "r2")),
        ["f1"]
    );
}

// ---- extractBody (via pre-request chain) ----

fn body_seen_by_script(body: RequestBody) -> Value {
    let req = make_request(|r| {
        r.body = body;
        r.pre_request_script = Some("console.log(nx.request.body);".into());
    });
    let out = pre(&make_collection(|_| {}), &req, &[]);
    out.results[0].logs[0].args[0].clone()
}

fn raw_body(mode: BodyMode, raw: Option<&str>) -> RequestBody {
    RequestBody {
        mode,
        raw: raw.map(Into::into),
        ..Default::default()
    }
}

#[test]
fn extracts_raw_body_for_json_xml_text_html() {
    assert_eq!(
        body_seen_by_script(raw_body(BodyMode::Json, Some(r#"{"a":1}"#))),
        json!(r#"{"a":1}"#)
    );
    assert_eq!(
        body_seen_by_script(raw_body(BodyMode::Xml, Some("<root/>"))),
        json!("<root/>")
    );
    assert_eq!(
        body_seen_by_script(raw_body(BodyMode::Text, Some("plain text"))),
        json!("plain text")
    );
    assert_eq!(
        body_seen_by_script(raw_body(BodyMode::Html, Some("<b>hi</b>"))),
        json!("<b>hi</b>")
    );
}

#[test]
fn encodes_urlencoded_body() {
    let kv = |k: &str, v: &str, enabled| KeyValuePair {
        key: k.into(),
        value: v.into(),
        enabled,
        description: None,
    };
    let body = RequestBody {
        mode: BodyMode::XWwwFormUrlencoded,
        urlencoded: Some(vec![
            kv("user", "alice", true),
            kv("pass", "a b", true),
            kv("disabled", "nope", false),
        ]),
        ..Default::default()
    };
    assert_eq!(body_seen_by_script(body), json!("user=alice&pass=a%20b"));
}

#[test]
fn urlencoded_matches_encode_uri_component_for_special_characters() {
    let body = RequestBody {
        mode: BodyMode::XWwwFormUrlencoded,
        urlencoded: Some(vec![KeyValuePair {
            key: "k&=".into(),
            value: "é!~*'()-_.".into(),
            enabled: true,
            description: None,
        }]),
        ..Default::default()
    };
    assert_eq!(body_seen_by_script(body), json!("k%26%3D=%C3%A9!~*'()-_."));
}

#[test]
fn serializes_graphql_body_with_variables() {
    let body = RequestBody {
        mode: BodyMode::Graphql,
        graphql: Some(GraphqlBody {
            query: "{ users { id } }".into(),
            variables: Some(r#"{"limit":10}"#.into()),
        }),
        ..Default::default()
    };
    let seen = body_seen_by_script(body);
    let parsed: Value = serde_json::from_str(seen.as_str().unwrap()).unwrap();
    assert_eq!(parsed["query"], "{ users { id } }");
    assert_eq!(parsed["variables"], json!({"limit": 10}));
}

#[test]
fn graphql_without_variables_omits_them_and_bad_variables_error() {
    let body = RequestBody {
        mode: BodyMode::Graphql,
        graphql: Some(GraphqlBody {
            query: "{ a }".into(),
            variables: None,
        }),
        ..Default::default()
    };
    assert_eq!(body_seen_by_script(body), json!(r#"{"query":"{ a }"}"#));

    let bad = make_request(|r| {
        r.body = RequestBody {
            mode: BodyMode::Graphql,
            graphql: Some(GraphqlBody {
                query: "{ a }".into(),
                variables: Some("{nope".into()),
            }),
            ..Default::default()
        }
    });
    assert!(ScriptRunner::default()
        .run_pre_request_chain(&make_collection(|_| {}), &bad, &[], &[], &[])
        .is_err());
}

#[test]
fn returns_null_for_mode_none_and_missing_raw() {
    assert_eq!(
        body_seen_by_script(raw_body(BodyMode::None, None)),
        Value::Null
    );
    assert_eq!(
        body_seen_by_script(raw_body(BodyMode::Json, None)),
        Value::Null
    );
}
