use super::*;
use serde_json::json;

fn collection(overrides: Value) -> Value {
    let mut base = json!({
        "info": {
            "name": "Test Collection",
            "description": "A test",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": []
    });
    for (k, v) in overrides.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

fn request_item() -> Value {
    json!({ "name": "My Request", "request": { "method": "GET", "url": "https://api.example.com/users", "header": [] } })
}

fn import(overrides: Value) -> ImportResult {
    import_collection(&collection(overrides)).unwrap()
}

/// The single top-level item of an import as a request.
fn first_request(r: &ImportResult) -> &NexusRequest {
    match &r.collections[0].items[0] {
        CollectionItem::Request(req) => req,
        CollectionItem::Folder(_) => panic!("expected a request"),
    }
}

fn request_with(request: Value) -> ImportResult {
    import(json!({ "item": [{ "name": "R", "request": request }] }))
}

fn body_of(body: Value) -> RequestBody {
    first_request(&request_with(
        json!({ "method": "POST", "url": "https://e.com", "body": body }),
    ))
    .body
    .clone()
}

fn auth_of(auth: Value) -> AuthConfig {
    import(json!({ "auth": auth })).collections[0]
        .auth
        .clone()
        .unwrap()
}

fn auth_json(auth: Value) -> Value {
    serde_json::to_value(auth_of(auth)).unwrap()
}

#[test]
fn collection_basics() {
    let r = import(json!({}));
    assert_eq!(r.collections.len(), 1);
    assert_eq!(r.collections[0].name, "Test Collection");
    assert_eq!(r.collections[0].description.as_deref(), Some("A test"));
    assert!(r.environments.is_empty());
    assert!(r.collections[0].items.is_empty());
    let id = &r.collections[0].id;
    assert!(
        id.starts_with("col_") && id.len() == 12 && id[4..].chars().all(|c| c.is_ascii_hexdigit())
    );
}

#[test]
fn collection_variables() {
    let r = import(json!({ "variable": [
        { "key": "base_url", "value": "https://api.example.com" },
        { "key": "token", "value": "abc123", "type": "secret", "disabled": true, "description": "API token" }
    ]}));
    let vars = &r.collections[0].variables;
    assert_eq!(vars.len(), 2);
    assert_eq!(
        vars[0],
        Variable {
            key: "base_url".into(),
            value: "https://api.example.com".into(),
            var_type: VariableType::String,
            enabled: true,
            description: None
        }
    );
    assert_eq!(
        vars[1],
        Variable {
            key: "token".into(),
            value: "abc123".into(),
            var_type: VariableType::Secret,
            enabled: false,
            description: Some("API token".into())
        }
    );
}

#[test]
fn collection_level_auth_and_scripts() {
    let r = import(json!({
        "auth": { "type": "bearer", "bearer": [{ "key": "token", "value": "xyz" }] },
        "event": [
            { "listen": "prerequest", "script": { "exec": ["pm.environment.set(\"foo\", \"bar\");"] } },
            { "listen": "test", "script": { "exec": ["pm.test(\"ok\", function() {});"] } }
        ]
    }));
    let col = &r.collections[0];
    assert_eq!(
        serde_json::to_value(col.auth.as_ref().unwrap()).unwrap(),
        json!({ "type": "bearer", "token": "xyz" })
    );
    assert!(col
        .pre_request_script
        .as_deref()
        .unwrap()
        .contains("nx.environment.set"));
    assert!(col
        .post_response_script
        .as_deref()
        .unwrap()
        .contains("nx.test"));
}

#[test]
fn folders() {
    let r = import(json!({ "item": [{ "name": "Users", "item": [request_item()] }] }));
    let CollectionItem::Folder(folder) = &r.collections[0].items[0] else {
        panic!("folder")
    };
    assert_eq!(folder.name, "Users");
    assert!(folder.id.starts_with("folder_"));
    assert_eq!(folder.items.len(), 1);

    let r = import(
        json!({ "item": [{ "name": "API", "item": [{ "name": "v1", "item": [request_item()] }] }] }),
    );
    let CollectionItem::Folder(outer) = &r.collections[0].items[0] else {
        panic!()
    };
    assert_eq!(outer.items.len(), 1);
    let CollectionItem::Folder(inner) = &outer.items[0] else {
        panic!()
    };
    assert_eq!(inner.name, "v1");
    assert_eq!(inner.items.len(), 1);
}

#[test]
fn folder_auth_and_scripts() {
    let r = import(json!({ "item": [{
        "name": "Secured",
        "item": [],
        "auth": { "type": "basic", "basic": [{ "key": "username", "value": "admin" }, { "key": "password", "value": "pass" }] },
        "event": [{ "listen": "prerequest", "script": { "exec": ["console.log(\"hi\");"] } }]
    }]}));
    let CollectionItem::Folder(folder) = &r.collections[0].items[0] else {
        panic!()
    };
    assert_eq!(
        serde_json::to_value(folder.auth.as_ref().unwrap()).unwrap(),
        json!({ "type": "basic", "username": "admin", "password": "pass" })
    );
    assert!(folder
        .pre_request_script
        .as_deref()
        .unwrap()
        .contains("console.log"));
}

#[test]
fn basic_request() {
    let r = import(json!({ "item": [request_item()] }));
    let req = first_request(&r);
    assert_eq!(req.method, HttpMethod::Get);
    assert_eq!(req.url, "https://api.example.com/users");
    assert_eq!(req.name, "My Request");
    assert!(req.id.starts_with("req_"));
}

#[test]
fn method_handling() {
    assert_eq!(
        first_request(&request_with(json!({ "url": "https://example.com" }))).method,
        HttpMethod::Get
    );
    assert_eq!(
        first_request(&request_with(
            json!({ "method": "post", "url": "https://example.com" })
        ))
        .method,
        HttpMethod::Post
    );

    let r = request_with(json!({ "method": "PROPFIND", "url": "https://example.com" }));
    assert_eq!(first_request(&r).method, HttpMethod::Get);
    assert!(r.warnings.iter().any(|w| w.contains("PROPFIND")));
}

#[test]
fn url_forms() {
    let r = request_with(
        json!({ "method": "GET", "url": { "raw": "https://api.example.com/test", "host": ["api", "example", "com"], "path": ["test"] } }),
    );
    assert_eq!(first_request(&r).url, "https://api.example.com/test");
    assert_eq!(
        first_request(&request_with(json!({ "method": "GET" }))).url,
        ""
    );
}

#[test]
fn headers_enabled_state() {
    let r = request_with(
        json!({ "method": "GET", "url": "https://example.com", "header": [
            { "key": "Accept", "value": "application/json", "disabled": false },
            { "key": "X-Debug", "value": "true", "disabled": true, "description": "debug header" }
        ]}),
    );
    let h = &first_request(&r).headers;
    assert_eq!(h.len(), 2);
    assert_eq!(
        h[0],
        KeyValuePair {
            key: "Accept".into(),
            value: "application/json".into(),
            enabled: true,
            description: None
        }
    );
    assert_eq!(
        h[1],
        KeyValuePair {
            key: "X-Debug".into(),
            value: "true".into(),
            enabled: false,
            description: Some("debug header".into())
        }
    );
}

#[test]
fn query_params() {
    let r = request_with(
        json!({ "method": "GET", "url": "https://example.com/search?q=hello&page=1" }),
    );
    let p = &first_request(&r).params;
    assert_eq!(p.len(), 2);
    assert_eq!((p[0].key.as_str(), p[0].value.as_str()), ("q", "hello"));
    assert_eq!((p[1].key.as_str(), p[1].value.as_str()), ("page", "1"));

    let r = request_with(json!({ "method": "GET", "url": {
        "raw": "https://example.com/search?q=hello",
        "query": [
            { "key": "q", "value": "hello", "disabled": false },
            { "key": "limit", "value": "10", "disabled": true, "description": "page size" }
        ]
    }}));
    let p = &first_request(&r).params;
    assert_eq!(p.len(), 2);
    assert_eq!(
        p[0],
        KeyValuePair {
            key: "q".into(),
            value: "hello".into(),
            enabled: true,
            description: None
        }
    );
    assert_eq!(
        p[1],
        KeyValuePair {
            key: "limit".into(),
            value: "10".into(),
            enabled: false,
            description: Some("page size".into())
        }
    );

    assert!(first_request(&request_with(
        json!({ "method": "GET", "url": "https://example.com" })
    ))
    .params
    .is_empty());
}

#[test]
fn request_auth() {
    let r = import(json!({ "item": [request_item()] }));
    assert_eq!(first_request(&r).auth, AuthConfig::new(AuthType::Inherit));

    let r = request_with(
        json!({ "method": "GET", "url": "https://example.com", "auth": { "type": "bearer", "bearer": [{ "key": "token", "value": "my-token" }] } }),
    );
    assert_eq!(
        serde_json::to_value(&first_request(&r).auth).unwrap(),
        json!({ "type": "bearer", "token": "my-token" })
    );
}

#[test]
fn request_scripts() {
    let r = import(json!({ "item": [{
        "name": "R",
        "request": { "method": "GET", "url": "https://example.com" },
        "event": [
            { "listen": "prerequest", "script": { "exec": ["pm.environment.set(\"key\", \"val\");"] } },
            { "listen": "test", "script": { "exec": ["pm.test(\"status\", function() {", "  pm.response.to.have.status(200);", "});"] } }
        ]
    }]}));
    let req = first_request(&r);
    assert!(req
        .pre_request_script
        .as_deref()
        .unwrap()
        .contains("nx.environment.set"));
    assert!(req
        .post_response_script
        .as_deref()
        .unwrap()
        .contains("nx.test"));
}

#[test]
fn empty_scripts() {
    let r = import(
        json!({ "item": [{ "name": "R", "request": { "method": "GET", "url": "https://example.com" }, "event": [{ "listen": "prerequest", "script": {} }] }] }),
    );
    assert_eq!(first_request(&r).pre_request_script.as_deref(), Some(""));
    let r = import(
        json!({ "item": [{ "name": "R", "request": { "method": "GET", "url": "https://example.com" }, "event": [{ "listen": "prerequest", "script": { "exec": ["", "  "] } }] }] }),
    );
    assert_eq!(first_request(&r).pre_request_script.as_deref(), Some(""));
}

#[test]
fn body_none_cases() {
    let r = import(json!({ "item": [request_item()] }));
    assert_eq!(first_request(&r).body.mode, BodyMode::None);
    assert_eq!(body_of(json!({})).mode, BodyMode::None);
    assert_eq!(
        body_of(json!({ "mode": "something-new" })).mode,
        BodyMode::None
    );
}

#[test]
fn raw_bodies() {
    let b = body_of(
        json!({ "mode": "raw", "raw": "{\"key\":\"value\"}", "options": { "raw": { "language": "json" } } }),
    );
    assert_eq!(
        (b.mode, b.raw.as_deref()),
        (BodyMode::Json, Some("{\"key\":\"value\"}"))
    );
    assert_eq!(
        body_of(json!({ "mode": "raw", "raw": "{\"auto\": true}" })).mode,
        BodyMode::Json
    );
    assert_eq!(
        body_of(
            json!({ "mode": "raw", "raw": "<root/>", "options": { "raw": { "language": "xml" } } })
        )
        .mode,
        BodyMode::Xml
    );
    assert_eq!(body_of(json!({ "mode": "raw", "raw": "<html></html>", "options": { "raw": { "language": "html" } } })).mode, BodyMode::Html);
    let b = body_of(json!({ "mode": "raw", "raw": "plain text" }));
    assert_eq!(
        (b.mode, b.raw.as_deref()),
        (BodyMode::Text, Some("plain text"))
    );
}

#[test]
fn structured_bodies() {
    let b = body_of(json!({ "mode": "urlencoded", "urlencoded": [
        { "key": "user", "value": "alice", "disabled": false },
        { "key": "pass", "value": "secret", "disabled": true, "description": "password" }
    ]}));
    assert_eq!(b.mode, BodyMode::XWwwFormUrlencoded);
    let u = b.urlencoded.unwrap();
    assert_eq!(u.len(), 2);
    assert!(u[0].enabled && !u[1].enabled);

    let b = body_of(json!({ "mode": "formdata", "formdata": [
        { "key": "file", "src": "/path/to/file", "type": "file" },
        { "key": "name", "value": "doc", "type": "text" }
    ]}));
    assert_eq!(b.mode, BodyMode::FormData);
    let f = b.form_data.unwrap();
    assert_eq!(f.len(), 2);
    assert_eq!(f[0].value, "/path/to/file");
    assert_eq!(f[0].entry_type, FormDataType::File);
    assert_eq!(f[1].entry_type, FormDataType::Text);

    let b = body_of(
        json!({ "mode": "graphql", "graphql": { "query": "{ users { id } }", "variables": "{\"limit\":10}" } }),
    );
    assert_eq!(b.mode, BodyMode::Graphql);
    let g = b.graphql.unwrap();
    assert_eq!(g.query, "{ users { id } }");
    assert_eq!(g.variables.as_deref(), Some("{\"limit\":10}"));

    assert_eq!(body_of(json!({ "mode": "file" })).mode, BodyMode::Binary);
}

#[test]
fn auth_types() {
    assert_eq!(
        auth_json(json!({ "type": "bearer", "bearer": [{ "key": "token", "value": "abc" }] })),
        json!({ "type": "bearer", "token": "abc" })
    );
    assert_eq!(
        auth_json(
            json!({ "type": "basic", "basic": [{ "key": "username", "value": "user" }, { "key": "password", "value": "pass" }] })
        ),
        json!({ "type": "basic", "username": "user", "password": "pass" })
    );
    assert_eq!(
        auth_json(
            json!({ "type": "apikey", "apikey": [{ "key": "key", "value": "X-API-Key" }, { "key": "value", "value": "secret" }, { "key": "in", "value": "header" }] })
        ),
        json!({ "type": "api-key", "key": "X-API-Key", "value": "secret", "addTo": "header" })
    );
    assert_eq!(
        auth_json(
            json!({ "type": "apikey", "apikey": [{ "key": "key", "value": "api_key" }, { "key": "value", "value": "s" }, { "key": "in", "value": "query" }] })
        ),
        json!({ "type": "api-key", "key": "api_key", "value": "s", "addTo": "query" })
    );

    let a = auth_json(json!({ "type": "oauth2", "oauth2": [
        { "key": "accessToken", "value": "tok" }, { "key": "tokenType", "value": "Bearer" },
        { "key": "addTokenTo", "value": "header" }, { "key": "authUrl", "value": "https://auth.example.com" },
        { "key": "accessTokenUrl", "value": "https://token.example.com" }, { "key": "clientId", "value": "cid" },
        { "key": "clientSecret", "value": "cs" }, { "key": "scope", "value": "read" },
        { "key": "redirect_uri", "value": "https://cb.example.com" }
    ]}));
    assert_eq!(a["type"], "oauth2");
    assert_eq!(a["accessToken"], "tok");
    assert_eq!(a["clientId"], "cid");
    assert_eq!(a["tokenUrl"], "https://token.example.com");
    assert_eq!(a["redirectUri"], "https://cb.example.com");
    assert_eq!(a["addTo"], "header");

    assert_eq!(
        auth_json(json!({ "type": "digest", "digest": [
            { "key": "username", "value": "u" }, { "key": "password", "value": "p" }, { "key": "realm", "value": "r" },
            { "key": "nonce", "value": "n" }, { "key": "algorithm", "value": "SHA-256" }, { "key": "qop", "value": "auth" },
            { "key": "opaque", "value": "o" }
        ]})),
        json!({ "type": "digest", "username": "u", "password": "p", "realm": "r", "nonce": "n", "algorithm": "SHA-256", "qop": "auth", "opaque": "o" })
    );
    assert_eq!(
        auth_json(json!({ "type": "digest", "digest": [{ "key": "username", "value": "u" }] }))
            ["algorithm"],
        "MD5"
    );

    assert_eq!(
        auth_json(json!({ "type": "awsv4", "awsv4": [
            { "key": "accessKey", "value": "AK" }, { "key": "secretKey", "value": "SK" }, { "key": "region", "value": "us-east-1" },
            { "key": "service", "value": "s3" }, { "key": "sessionToken", "value": "ST" }
        ]})),
        json!({ "type": "aws-sig-v4", "accessKey": "AK", "secretKey": "SK", "region": "us-east-1", "service": "s3", "sessionToken": "ST" })
    );
    assert_eq!(
        auth_of(json!({ "type": "noauth" })),
        AuthConfig::new(AuthType::None)
    );
}

#[test]
fn hawk_and_unknown_auth_warn() {
    let r = import(json!({ "auth": { "type": "hawk", "hawk": [] } }));
    assert_eq!(r.collections[0].auth, Some(AuthConfig::new(AuthType::None)));
    assert!(r
        .warnings
        .contains(&"Hawk auth imported but not fully supported in Nexus.".to_string()));

    let r = import(json!({ "auth": { "type": "ntlm" } }));
    assert_eq!(r.collections[0].auth, Some(AuthConfig::new(AuthType::None)));
    assert!(r
        .warnings
        .iter()
        .any(|w| w.contains("Unknown auth type \"ntlm\"")));
}

#[test]
fn environment_import() {
    let r = import_environment(&json!({
        "name": "Production",
        "values": [
            { "key": "host", "value": "api.prod.com" },
            { "key": "secret", "value": "hidden", "type": "secret", "disabled": true }
        ]
    }))
    .unwrap();
    assert_eq!(r.environments.len(), 1);
    assert!(r.collections.is_empty());
    assert!(r.warnings.is_empty());
    let env = &r.environments[0];
    assert_eq!(env.name, "Production");
    assert_eq!(env.variables.len(), 2);
    assert_eq!(
        env.variables[0],
        Variable {
            key: "host".into(),
            value: "api.prod.com".into(),
            var_type: VariableType::String,
            enabled: true,
            description: None
        }
    );
    assert_eq!(
        env.variables[1],
        Variable {
            key: "secret".into(),
            value: "hidden".into(),
            var_type: VariableType::Secret,
            enabled: false,
            description: None
        }
    );

    let r = import_environment(&json!({ "name": "Empty", "values": [] })).unwrap();
    assert!(r.environments[0].variables.is_empty());
}

#[test]
fn script_warnings_are_prefixed() {
    let r = import(json!({ "item": [{
        "name": "R",
        "request": { "method": "GET", "url": "https://e.com" },
        "event": [{ "listen": "test", "script": { "exec": ["pm.sendRequest(\"https://x.com\", function() {});"] } }]
    }]}));
    assert!(r
        .warnings
        .iter()
        .any(|w| w.contains("[Script]") && w.contains("pm.sendRequest")));
}

#[test]
fn warnings_do_not_leak_between_imports() {
    import(json!({ "auth": { "type": "hawk", "hawk": [] } }));
    assert!(import(json!({})).warnings.is_empty());
}

#[test]
fn malformed_input_is_an_error_not_a_panic() {
    assert!(import_collection(&json!({})).is_err());
    assert!(import_collection(&json!({ "info": { "name": "x" } })).is_err());
    assert!(
        import_collection(&json!({ "info": { "name": "x" }, "item": [{ "name": "orphan" }] }))
            .is_err()
    );
    assert!(import_environment(&json!({ "name": "x" })).is_err());
}

#[test]
fn numeric_and_boolean_values_are_stringified() {
    let r = import_environment(&json!({ "name": "E", "values": [{ "key": "port", "value": 8080 }, { "key": "on", "value": true }] })).unwrap();
    assert_eq!(r.environments[0].variables[0].value, "8080");
    assert_eq!(r.environments[0].variables[1].value, "true");
}
