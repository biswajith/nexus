use super::*;
use serde_json::json;

fn request(overrides: Value) -> Value {
    let mut base = json!({
        "id": "req_abc", "name": "Test Request", "method": "GET", "url": "https://api.example.com/users",
        "headers": [], "params": [], "body": { "mode": "none" }, "auth": { "type": "inherit" }, "settings": {}
    });
    for (k, v) in overrides.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

fn folder(overrides: Value) -> Value {
    let mut base = json!({ "id": "folder_abc", "name": "Test Folder", "items": [] });
    for (k, v) in overrides.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

fn collection(overrides: Value) -> NexusCollection {
    let mut base = json!({ "id": "col_abc", "name": "Test Collection", "description": "A collection", "variables": [], "items": [] });
    for (k, v) in overrides.as_object().unwrap() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).unwrap()
}

fn postman(overrides: Value) -> Value {
    serde_json::from_str(&export_as_postman_v21(&collection(overrides)).unwrap()).unwrap()
}

fn req_export(overrides: Value) -> Value {
    postman(json!({ "items": [request(overrides)] }))["item"][0].clone()
}

#[test]
fn nexus_json_export() {
    let col = collection(json!({
        "variables": [{ "key": "host", "value": "example.com", "type": "string", "enabled": true }],
        "items": [request(json!({}))]
    }));
    let json = export_as_nexus_json(&col).unwrap();
    assert!(json.contains('\n') && json.contains("  "));
    let parsed: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["name"], "Test Collection");
    assert_eq!(parsed["id"], "col_abc");
    assert_eq!(parsed["variables"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["items"].as_array().unwrap().len(), 1);
    // And it re-imports unchanged.
    assert_eq!(serde_json::from_str::<NexusCollection>(&json).unwrap(), col);
}

#[test]
fn postman_structure() {
    let p = postman(json!({}));
    assert_eq!(p["info"]["name"], "Test Collection");
    assert_eq!(
        p["info"]["schema"],
        "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
    );
    assert_eq!(p["info"]["_postman_id"], "col_abc");
    assert_eq!(p["item"], json!([]));
    assert_eq!(
        postman(json!({ "description": "My API" }))["info"]["description"],
        "My API"
    );
    assert!(postman(json!({}))["event"].is_null());
}

#[test]
fn postman_variables() {
    let p = postman(json!({ "variables": [
        { "key": "base_url", "value": "https://api.com", "type": "string", "enabled": true, "description": "Base URL" },
        { "key": "token", "value": "hidden", "type": "secret", "enabled": false }
    ]}));
    assert_eq!(p["variable"].as_array().unwrap().len(), 2);
    assert_eq!(
        p["variable"][0],
        json!({ "key": "base_url", "value": "https://api.com", "disabled": false, "description": "Base URL", "type": "string" })
    );
    assert_eq!(p["variable"][1]["disabled"], true);
    assert_eq!(p["variable"][1]["type"], "secret");
}

#[test]
fn postman_collection_auth() {
    let p = postman(json!({ "auth": { "type": "bearer", "token": "abc" } }));
    assert_eq!(p["auth"]["type"], "bearer");
    assert_eq!(
        p["auth"]["bearer"],
        json!([{ "key": "token", "value": "abc" }])
    );
    assert!(postman(json!({ "auth": { "type": "none" } }))["auth"].is_null());
    assert!(postman(json!({ "auth": { "type": "inherit" } }))["auth"].is_null());
}

#[test]
fn postman_collection_scripts() {
    let p = postman(
        json!({ "preRequestScript": "console.log(\"pre\");", "postResponseScript": "console.log(\"test\");" }),
    );
    assert_eq!(p["event"].as_array().unwrap().len(), 2);
    assert_eq!(p["event"][0]["listen"], "prerequest");
    assert_eq!(p["event"][1]["listen"], "test");
}

#[test]
fn basic_request_export() {
    let item =
        req_export(json!({ "name": "Get Users", "method": "GET", "url": "https://api.com/users" }));
    assert_eq!(item["name"], "Get Users");
    assert_eq!(item["request"]["method"], "GET");
    assert_eq!(item["request"]["url"]["raw"], "https://api.com/users");
}

#[test]
fn headers_disabled_flag() {
    let item = req_export(json!({ "headers": [
        { "key": "Accept", "value": "application/json", "enabled": true },
        { "key": "X-Debug", "value": "true", "enabled": false, "description": "debug" }
    ]}));
    let h = item["request"]["header"].as_array().unwrap();
    assert_eq!(h.len(), 2);
    assert_eq!(h[0]["disabled"], false);
    assert_eq!(h[1]["disabled"], true);
    assert_eq!(h[1]["description"], "debug");
}

#[test]
fn query_params_in_url_and_array() {
    let item = req_export(json!({ "url": "https://api.com/search", "params": [
        { "key": "q", "value": "test", "enabled": true }, { "key": "page", "value": "1", "enabled": false }
    ]}));
    assert_eq!(
        item["request"]["url"]["raw"],
        "https://api.com/search?q=test"
    );
    let q = item["request"]["url"]["query"].as_array().unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(q[0]["disabled"], false);
    assert_eq!(q[1]["disabled"], true);

    let item = req_export(
        json!({ "url": "https://api.com/test", "params": [{ "key": "x", "value": "y", "enabled": false }] }),
    );
    assert_eq!(item["request"]["url"]["raw"], "https://api.com/test");
}

#[test]
fn query_string_is_percent_encoded() {
    let item = req_export(
        json!({ "url": "https://a.com", "params": [{ "key": "a b", "value": "c&d=é", "enabled": true }] }),
    );
    assert_eq!(
        item["request"]["url"]["raw"],
        "https://a.com?a%20b=c%26d%3D%C3%A9"
    );
}

#[test]
fn raw_bodies() {
    for (mode, lang, raw) in [
        ("json", "json", "{\"key\":\"val\"}"),
        ("xml", "xml", "<root/>"),
        ("html", "html", "<p>hi</p>"),
        ("text", "text", "hello"),
    ] {
        let item = req_export(json!({ "body": { "mode": mode, "raw": raw } }));
        let body = &item["request"]["body"];
        assert_eq!(body["mode"], "raw");
        assert_eq!(body["raw"], raw);
        assert_eq!(body["options"]["raw"]["language"], lang);
    }
}

#[test]
fn structured_bodies() {
    let item = req_export(
        json!({ "body": { "mode": "x-www-form-urlencoded", "urlencoded": [
            { "key": "user", "value": "alice", "enabled": true },
            { "key": "pass", "value": "secret", "enabled": false, "description": "pw" }
        ]}}),
    );
    let body = &item["request"]["body"];
    assert_eq!(body["mode"], "urlencoded");
    assert_eq!(body["urlencoded"].as_array().unwrap().len(), 2);
    assert_eq!(body["urlencoded"][0]["disabled"], false);
    assert_eq!(body["urlencoded"][1]["disabled"], true);

    let item = req_export(json!({ "body": { "mode": "form-data", "formData": [
        { "key": "file", "value": "/path", "type": "file", "enabled": true },
        { "key": "name", "value": "doc", "type": "text", "enabled": true }
    ]}}));
    let body = &item["request"]["body"];
    assert_eq!(body["mode"], "formdata");
    assert_eq!(body["formdata"].as_array().unwrap().len(), 2);
    assert_eq!(body["formdata"][0]["type"], "file");

    let item = req_export(
        json!({ "body": { "mode": "graphql", "graphql": { "query": "{ users { id } }", "variables": "{}" } } }),
    );
    assert_eq!(item["request"]["body"]["mode"], "graphql");
    assert_eq!(
        item["request"]["body"]["graphql"]["query"],
        "{ users { id } }"
    );

    assert!(req_export(json!({}))["request"]["body"].is_null());
}

#[test]
fn request_auth() {
    let item =
        req_export(json!({ "auth": { "type": "basic", "username": "admin", "password": "pass" } }));
    assert_eq!(item["request"]["auth"]["type"], "basic");
    assert_eq!(
        item["request"]["auth"]["basic"],
        json!([{ "key": "username", "value": "admin" }, { "key": "password", "value": "pass" }])
    );
    assert!(req_export(json!({ "auth": { "type": "inherit" } }))["request"]["auth"].is_null());
    assert!(req_export(json!({ "auth": { "type": "none" } }))["request"]["auth"].is_null());
}

#[test]
fn request_scripts_become_pm_events() {
    let item = req_export(
        json!({ "preRequestScript": "nx.environment.set(\"k\", \"v\");", "postResponseScript": "nx.test(\"ok\", () => {});" }),
    );
    let events = item["event"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["listen"], "prerequest");
    assert_eq!(events[0]["script"]["type"], "text/javascript");
    let exec = |e: &Value| {
        e["script"]["exec"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(exec(&events[0]).contains("pm.environment.set"));
    assert_eq!(events[1]["listen"], "test");
    assert!(exec(&events[1]).contains("pm.test"));
}

#[test]
fn folders() {
    let p = postman(
        json!({ "items": [folder(json!({ "name": "Users", "items": [request(json!({ "name": "Get Users" }))] }))] }),
    );
    assert_eq!(p["item"][0]["name"], "Users");
    assert_eq!(p["item"][0]["item"].as_array().unwrap().len(), 1);
    assert_eq!(p["item"][0]["item"][0]["name"], "Get Users");

    assert_eq!(
        postman(
            json!({ "items": [folder(json!({ "auth": { "type": "bearer", "token": "tok" } }))] })
        )["item"][0]["auth"]["type"],
        "bearer"
    );
    assert!(
        postman(json!({ "items": [folder(json!({ "auth": { "type": "inherit" } }))] }))["item"][0]
            ["auth"]
            .is_null()
    );

    let p = postman(
        json!({ "items": [folder(json!({ "preRequestScript": "console.log(\"pre\");", "postResponseScript": "console.log(\"test\");" }))] }),
    );
    assert_eq!(p["item"][0]["event"].as_array().unwrap().len(), 2);

    let p = postman(
        json!({ "items": [folder(json!({ "name": "API", "items": [folder(json!({ "name": "v1", "items": [request(json!({ "name": "Ping" }))] }))] }))] }),
    );
    assert_eq!(p["item"][0]["item"][0]["name"], "v1");
    assert_eq!(p["item"][0]["item"][0]["item"][0]["name"], "Ping");
}

fn export_auth(auth: Value) -> Value {
    postman(json!({ "auth": auth }))["auth"].clone()
}

fn find(list: &Value, key: &str) -> Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|kv| kv["key"] == key)
        .unwrap()["value"]
        .clone()
}

#[test]
fn auth_type_conversion() {
    let a = export_auth(
        json!({ "type": "api-key", "key": "X-Key", "value": "secret", "addTo": "header" }),
    );
    assert_eq!(a["type"], "apikey");
    assert_eq!(
        a["apikey"],
        json!([{ "key": "key", "value": "X-Key" }, { "key": "value", "value": "secret" }, { "key": "in", "value": "header" }])
    );
    let a =
        export_auth(json!({ "type": "api-key", "key": "api_key", "value": "s", "addTo": "query" }));
    assert_eq!(find(&a["apikey"], "in"), "query");

    let a = export_auth(
        json!({ "type": "oauth2", "accessToken": "tok", "tokenType": "Bearer", "addTo": "header" }),
    );
    assert_eq!(a["type"], "oauth2");
    assert_eq!(find(&a["oauth2"], "accessToken"), "tok");
    let a = export_auth(json!({ "type": "oauth2", "accessToken": "t", "addTo": "query" }));
    assert_eq!(find(&a["oauth2"], "addTokenTo"), "queryParams");
    assert_eq!(find(&a["oauth2"], "tokenType"), "Bearer");

    let a = export_auth(
        json!({ "type": "digest", "username": "u", "password": "p", "realm": "r", "nonce": "n" }),
    );
    assert_eq!(a["type"], "digest");
    assert_eq!(find(&a["digest"], "username"), "u");

    let a = export_auth(
        json!({ "type": "aws-sig-v4", "accessKey": "AK", "secretKey": "SK", "region": "us-east-1", "service": "s3" }),
    );
    assert_eq!(a["type"], "awsv4");
    assert_eq!(find(&a["awsv4"], "region"), "us-east-1");

    assert_eq!(export_auth(json!({ "type": "hawk" }))["type"], "noauth");
}

#[test]
fn environment_export() {
    let env: NexusEnvironment = serde_json::from_value(json!({
        "id": "env_1", "name": "Production",
        "variables": [
            { "key": "host", "value": "api.prod.com", "type": "string", "enabled": true },
            { "key": "token", "value": "secret-val", "type": "secret", "enabled": true }
        ]
    }))
    .unwrap();
    let parsed: Value =
        serde_json::from_str(&export_environment_as_postman(&env).unwrap()).unwrap();
    assert_eq!(parsed["name"], "Production");
    assert_eq!(parsed["_postman_variable_scope"], "environment");
    assert_eq!(parsed["values"].as_array().unwrap().len(), 2);
    assert_eq!(
        parsed["values"][0],
        json!({ "key": "host", "value": "api.prod.com", "enabled": true, "type": "default" })
    );
    // Secrets are cleared.
    assert_eq!(parsed["values"][1]["value"], "");
    assert_eq!(parsed["values"][1]["type"], "secret");
}

#[test]
fn environment_disabled_and_empty() {
    let env: NexusEnvironment = serde_json::from_value(json!({
        "id": "e", "name": "Env", "variables": [{ "key": "off", "value": "val", "type": "string", "enabled": false }]
    }))
    .unwrap();
    let parsed: Value =
        serde_json::from_str(&export_environment_as_postman(&env).unwrap()).unwrap();
    assert_eq!(parsed["values"][0]["enabled"], false);

    let env: NexusEnvironment =
        serde_json::from_value(json!({ "id": "e", "name": "Empty", "variables": [] })).unwrap();
    let parsed: Value =
        serde_json::from_str(&export_environment_as_postman(&env).unwrap()).unwrap();
    assert_eq!(parsed["values"].as_array().unwrap().len(), 0);
}

#[test]
fn postman_export_round_trips_through_the_importer() {
    let col = collection(json!({
        "items": [folder(json!({ "items": [request(json!({ "method": "POST", "body": { "mode": "json", "raw": "{}" }, "preRequestScript": "nx.environment.set(\"a\",\"b\")" }))] }))]
    }));
    let exported = export_as_postman_v21(&col).unwrap();
    let imported = super::super::import_data(&exported, None).unwrap();
    assert_eq!(imported.collections[0].name, "Test Collection");
    let CollectionItem::Folder(f) = &imported.collections[0].items[0] else {
        panic!()
    };
    let CollectionItem::Request(r) = &f.items[0] else {
        panic!()
    };
    assert_eq!(r.method, HttpMethod::Post);
    assert_eq!(
        r.pre_request_script.as_deref(),
        Some("nx.environment.set(\"a\",\"b\")")
    );
}
