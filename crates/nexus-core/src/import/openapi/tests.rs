use super::*;
use serde_json::json;

fn merge(mut base: Value, overrides: Value) -> Value {
    for (k, v) in overrides.as_object().unwrap() {
        if v.is_null() {
            base.as_object_mut().unwrap().remove(k);
        } else {
            base[k] = v.clone();
        }
    }
    base
}

/// `null` in `overrides` removes the key (JS `undefined`).
fn spec3(overrides: Value) -> String {
    merge(
        json!({
            "openapi": "3.0.3",
            "info": { "title": "Test API", "description": "A test API", "version": "1.0.0" },
            "servers": [{ "url": "https://api.example.com/v1" }],
            "paths": {}
        }),
        overrides,
    )
    .to_string()
}

fn spec2(overrides: Value) -> String {
    merge(
        json!({
            "swagger": "2.0",
            "info": { "title": "Swagger API", "version": "1.0.0" },
            "host": "api.example.com",
            "basePath": "/v1",
            "schemes": ["https"],
            "paths": {}
        }),
        overrides,
    )
    .to_string()
}

fn import3(overrides: Value) -> ImportResult {
    import(&spec3(overrides)).unwrap()
}

fn requests(items: &[CollectionItem]) -> Vec<&NexusRequest> {
    let mut out = vec![];
    for item in items {
        match item {
            CollectionItem::Request(r) => out.push(r),
            CollectionItem::Folder(f) => out.extend(requests(&f.items)),
        }
    }
    out
}

fn first(r: &ImportResult) -> &NexusRequest {
    requests(&r.collections[0].items)[0]
}

fn post_with_content(content: Value) -> ImportResult {
    import3(
        json!({ "paths": { "/t": { "post": { "summary": "T", "requestBody": { "content": content } } } } }),
    )
}

fn json_body(schema: Value) -> Value {
    let r = post_with_content(json!({ "application/json": { "schema": schema } }));
    serde_json::from_str(first(&r).body.raw.as_deref().unwrap()).unwrap()
}

#[test]
fn basic_import() {
    let r = import3(json!({}));
    assert_eq!(r.collections.len(), 1);
    assert_eq!(r.collections[0].name, "Test API");
    assert_eq!(r.collections[0].description.as_deref(), Some("A test API"));
    assert!(r.collections[0].id.starts_with("col_"));
    let vars = &r.collections[0].variables;
    assert_eq!(vars.len(), 1);
    assert_eq!(
        (vars[0].key.as_str(), vars[0].value.as_str()),
        ("base_url", "https://api.example.com/v1")
    );
}

#[test]
fn default_base_url_without_servers() {
    let r = import3(json!({ "servers": null }));
    assert_eq!(r.collections[0].variables[0].value, "http://localhost");
}

#[test]
fn environments_from_multiple_servers() {
    let r = import3(json!({ "servers": [
        { "url": "https://api.prod.com", "description": "Production" },
        { "url": "https://api.staging.com", "description": "Staging" },
        { "url": "https://api.dev.com" }
    ]}));
    assert_eq!(r.environments.len(), 3);
    assert_eq!(r.environments[0].name, "Production");
    assert_eq!(r.environments[0].variables[0].value, "https://api.prod.com");
    assert_eq!(r.environments[1].name, "Staging");
    assert_eq!(r.environments[2].name, "Server 3");

    assert!(import3(json!({})).environments.is_empty());
    assert!(import3(json!({ "servers": [] })).environments.is_empty());
}

#[test]
fn builds_a_request_per_method() {
    let r = import3(
        json!({ "paths": { "/users": { "get": { "summary": "List users" }, "post": { "summary": "Create user" } } } }),
    );
    assert!(r.collections[0].items.len() >= 2);
}

#[test]
fn url_uses_base_url_and_colon_params() {
    let r = import3(
        json!({ "paths": { "/users/{userId}/posts/{postId}": { "get": { "summary": "Get post" } } } }),
    );
    assert_eq!(first(&r).url, "{{base_url}}/users/:userId/posts/:postId");
}

#[test]
fn method_is_uppercased() {
    let r = import3(json!({ "paths": { "/test": { "delete": { "summary": "Delete" } } } }));
    assert_eq!(first(&r).method, HttpMethod::Delete);
}

#[test]
fn request_naming() {
    let r = import3(json!({ "paths": {
        "/a": { "get": { "summary": "Get A" } },
        "/b": { "get": { "operationId": "getB" } },
        "/c": { "get": {} }
    }}));
    let names: Vec<&str> = requests(&r.collections[0].items)
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert!(names.contains(&"Get A"));
    assert!(names.contains(&"getB"));
    assert!(names.contains(&"GET /c"));

    let r = import3(
        json!({ "paths": { "/old": { "get": { "summary": "Old endpoint", "deprecated": true } } } }),
    );
    assert!(first(&r).name.starts_with("[DEPRECATED]"));
}

#[test]
fn non_method_keys_are_skipped() {
    let r = import3(
        json!({ "paths": { "/x": { "summary": "not a method", "get": { "summary": "Real" } } } }),
    );
    assert_eq!(requests(&r.collections[0].items).len(), 1);
}

#[test]
fn operation_description_is_kept() {
    let r = import3(
        json!({ "paths": { "/x": { "get": { "summary": "S", "description": "Does things" } } } }),
    );
    assert_eq!(first(&r).description.as_deref(), Some("Does things"));
}

#[test]
fn query_parameters() {
    let r = import3(
        json!({ "paths": { "/search": { "get": { "summary": "Search", "parameters": [
            { "name": "q", "in": "query", "required": true, "description": "search term", "example": "hello" },
            { "name": "page", "in": "query", "required": false, "schema": { "default": 1 } }
        ]}}}}),
    );
    let p = &first(&r).params;
    assert_eq!(p.len(), 2);
    assert_eq!(
        (p[0].key.as_str(), p[0].value.as_str(), p[0].enabled),
        ("q", "hello", true)
    );
    assert_eq!(p[0].description.as_deref(), Some("search term"));
    assert_eq!(
        (p[1].key.as_str(), p[1].value.as_str(), p[1].enabled),
        ("page", "1", false)
    );
}

#[test]
fn header_parameters_and_path_level_merge() {
    let r = import3(
        json!({ "paths": { "/data": { "get": { "summary": "d", "parameters": [{ "name": "X-Request-Id", "in": "header", "example": "abc-123" }] } } } }),
    );
    assert!(first(&r).headers.iter().any(|h| h.key == "X-Request-Id"));

    let r = import3(json!({ "paths": { "/items": {
        "parameters": [{ "name": "X-Trace", "in": "header", "example": "trace-1" }],
        "get": { "summary": "List items", "parameters": [{ "name": "limit", "in": "query", "example": "10" }] }
    }}}));
    let req = first(&r);
    assert!(req.headers.iter().any(|h| h.key == "X-Trace"));
    assert!(req.params.iter().any(|p| p.key == "limit"));
}

#[test]
fn parameter_example_resolution() {
    let value = |param: Value| {
        let r = import3(
            json!({ "paths": { "/t": { "get": { "summary": "T", "parameters": [param] } } } }),
        );
        first(&r).params[0].value.clone()
    };
    assert_eq!(
        value(json!({ "name": "foo", "in": "query", "schema": { "example": "bar" } })),
        "bar"
    );
    assert_eq!(
        value(
            json!({ "name": "status", "in": "query", "schema": { "enum": ["active", "inactive"] } })
        ),
        "active"
    );
    assert_eq!(value(json!({ "name": "x", "in": "query" })), "");
}

#[test]
fn json_body_from_schema_and_example() {
    let body = json_body(
        json!({ "type": "object", "properties": { "name": { "type": "string", "example": "Alice" }, "age": { "type": "integer", "example": 30 } } }),
    );
    assert_eq!(body["name"], "Alice");
    assert_eq!(body["age"], 30);

    let r = post_with_content(json!({ "application/json": {
        "example": { "custom": "example" },
        "schema": { "type": "object", "properties": { "name": { "type": "string" } } }
    }}));
    let parsed: Value = serde_json::from_str(first(&r).body.raw.as_deref().unwrap()).unwrap();
    assert_eq!(parsed["custom"], "example");
    assert_eq!(first(&r).body.mode, BodyMode::Json);
}

#[test]
fn urlencoded_body() {
    let r = post_with_content(json!({ "application/x-www-form-urlencoded": { "schema": {
        "type": "object",
        "properties": { "username": { "type": "string", "example": "admin" }, "password": { "type": "string" } },
        "required": ["username"]
    }}}));
    let body = &first(&r).body;
    assert_eq!(body.mode, BodyMode::XWwwFormUrlencoded);
    let u = body.urlencoded.as_ref().unwrap();
    assert_eq!(u.len(), 2);
    assert_eq!(u[0].key, "username");
    assert!(u[0].enabled);
    assert!(!u[1].enabled);
}

#[test]
fn multipart_body() {
    let r = post_with_content(json!({ "multipart/form-data": { "schema": {
        "type": "object",
        "properties": { "file": { "type": "string", "format": "binary" }, "desc": { "type": "string", "example": "My file" } }
    }}}));
    let body = &first(&r).body;
    assert_eq!(body.mode, BodyMode::FormData);
    let f = body.form_data.as_ref().unwrap();
    assert_eq!(f.len(), 2);
    assert_eq!(f[0].entry_type, FormDataType::Text);
}

#[test]
fn xml_text_and_none_bodies() {
    let r = post_with_content(json!({ "application/xml": { "schema": { "type": "object" } } }));
    assert_eq!(first(&r).body.mode, BodyMode::Xml);
    let r = post_with_content(json!({ "application/octet-stream": {} }));
    assert_eq!(first(&r).body.mode, BodyMode::Text);
    let r = import3(json!({ "paths": { "/test": { "get": { "summary": "Test" } } } }));
    assert_eq!(first(&r).body.mode, BodyMode::None);
}

#[test]
fn json_bodies_get_a_content_type_header() {
    let r = post_with_content(json!({ "application/json": { "schema": { "type": "object" } } }));
    assert!(first(&r).headers.iter().any(|h| h.key == "Content-Type"));
}

#[test]
fn tag_folders() {
    let r = import3(json!({
        "paths": {
            "/users": { "get": { "summary": "List users", "tags": ["Users"] } },
            "/users/{id}": { "get": { "summary": "Get user", "tags": ["Users"] } },
            "/posts": { "get": { "summary": "List posts", "tags": ["Posts"] } }
        },
        "tags": [{ "name": "Users", "description": "User management" }, { "name": "Posts", "description": "Blog posts" }]
    }));
    let folders: Vec<&NexusFolder> = r.collections[0]
        .items
        .iter()
        .filter_map(|i| {
            if let CollectionItem::Folder(f) = i {
                Some(f)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(folders.len(), 2);
    let users = folders.iter().find(|f| f.name == "Users").unwrap();
    assert_eq!(users.items.len(), 2);
    assert_eq!(users.description.as_deref(), Some("User management"));
}

#[test]
fn untagged_operations_stay_top_level() {
    let r = import3(json!({ "paths": {
        "/health": { "get": { "summary": "Health check" } },
        "/users": { "get": { "summary": "List users", "tags": ["Users"] } }
    }}));
    let top: Vec<&NexusRequest> = r.collections[0]
        .items
        .iter()
        .filter_map(|i| {
            if let CollectionItem::Request(r) = i {
                Some(r)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].name, "Health check");
}

#[test]
fn string_format_examples() {
    let body = json_body(json!({ "type": "object", "properties": {
        "email": { "type": "string", "format": "email" },
        "createdAt": { "type": "string", "format": "date-time" },
        "website": { "type": "string", "format": "uri" },
        "uid": { "type": "string", "format": "uuid" },
        "birthday": { "type": "string", "format": "date" }
    }}));
    assert_eq!(body["email"], "user@example.com");
    assert_eq!(body["createdAt"], "2026-01-01T00:00:00Z");
    assert_eq!(body["website"], "https://example.com");
    assert_eq!(body["uid"], "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(body["birthday"], "2026-01-01");
}

#[test]
fn primitive_examples() {
    let body = json_body(json!({ "type": "object", "properties": {
        "count": { "type": "integer" }, "ratio": { "type": "number" },
        "active": { "type": "boolean" }, "label": { "type": "string" }
    }}));
    assert_eq!(body["count"], 0);
    assert_eq!(body["ratio"], 0);
    assert_eq!(body["active"], true);
    assert_eq!(body["label"], "string");
}

#[test]
fn enum_array_and_default_examples() {
    let body = json_body(
        json!({ "type": "object", "properties": { "status": { "type": "string", "enum": ["active", "inactive"] } } }),
    );
    assert_eq!(body["status"], "active");

    let body = json_body(
        json!({ "type": "object", "properties": { "tags": { "type": "array", "items": { "type": "string" } }, "empty": { "type": "array" } } }),
    );
    assert_eq!(body["tags"], json!(["string"]));
    assert_eq!(body["empty"], json!([]));

    let body = json_body(json!({ "type": "object", "properties": {
        "count": { "type": "integer", "default": 42 }, "name": { "type": "string", "default": "default-name" }
    }}));
    assert_eq!(body["count"], 42);
    assert_eq!(body["name"], "default-name");
}

#[test]
fn unresolved_ref_warns() {
    let r = post_with_content(
        json!({ "application/json": { "schema": { "$ref": "#/components/schemas/User" } } }),
    );
    assert!(r.warnings.iter().any(|w| w.contains("$ref")));
    assert_eq!(first(&r).body.raw.as_deref(), Some("{}"));
}

#[test]
fn self_referencing_schema_does_not_recurse() {
    let body = json_body(
        json!({ "type": "object", "properties": { "child": { "$ref": "#/components/schemas/Node" } } }),
    );
    assert_eq!(body, json!({ "child": {} }));
}

#[test]
fn swagger2_base_url() {
    let r = import(&spec2(json!({}))).unwrap();
    assert_eq!(
        r.collections[0].variables[0].value,
        "https://api.example.com/v1"
    );
    let r = import(&spec2(
        json!({ "host": null, "schemes": null, "basePath": null }),
    ))
    .unwrap();
    assert_eq!(r.collections[0].variables[0].value, "https://localhost");
}

#[test]
fn swagger2_bodies() {
    let r = import(&spec2(json!({ "paths": { "/users": { "post": { "summary": "Create user", "parameters": [{
        "name": "body", "in": "body",
        "schema": { "type": "object", "properties": { "name": { "type": "string", "example": "Bob" } } }
    }]}}}}))).unwrap();
    assert_eq!(first(&r).body.mode, BodyMode::Json);
    let body: Value = serde_json::from_str(first(&r).body.raw.as_deref().unwrap()).unwrap();
    assert_eq!(body["name"], "Bob");

    let r = import(&spec2(
        json!({ "paths": { "/login": { "post": { "summary": "Login", "parameters": [
            { "name": "username", "in": "formData", "type": "string", "example": "admin" },
            { "name": "password", "in": "formData", "type": "string" }
        ]}}}}),
    ))
    .unwrap();
    assert_eq!(first(&r).body.mode, BodyMode::XWwwFormUrlencoded);
    assert_eq!(
        first(&r).body.urlencoded.as_ref().unwrap()[0].key,
        "username"
    );

    let r = import(&spec2(json!({ "paths": { "/upload": { "post": { "summary": "Upload", "consumes": ["multipart/form-data"], "parameters": [
        { "name": "file", "in": "formData", "type": "file" },
        { "name": "desc", "in": "formData", "type": "string", "example": "My file" }
    ]}}}}))).unwrap();
    assert_eq!(first(&r).body.mode, BodyMode::FormData);
    let f = first(&r).body.form_data.as_ref().unwrap();
    assert_eq!(f[0].entry_type, FormDataType::File);
    assert_eq!(f[1].entry_type, FormDataType::Text);

    let r = import(&spec2(
        json!({ "paths": { "/test": { "get": { "summary": "Test" } } } }),
    ))
    .unwrap();
    assert_eq!(first(&r).body.mode, BodyMode::None);
}

#[test]
fn yaml_spec() {
    let yaml = "
openapi: \"3.0.0\"
info:
  title: YAML API
  version: \"1.0\"
paths:
  /ping:
    get:
      summary: Ping
";
    let r = import(yaml).unwrap();
    assert_eq!(r.collections[0].name, "YAML API");
    assert_eq!(first(&r).name, "Ping");
}

#[test]
fn empty_and_missing_paths() {
    assert!(import3(json!({ "paths": {} })).collections[0]
        .items
        .is_empty());
    assert!(import3(json!({ "paths": null })).collections[0]
        .items
        .is_empty());
}

#[test]
fn all_requests_inherit_auth() {
    let r = import3(json!({ "paths": { "/test": { "get": { "summary": "Test" } } } }));
    assert_eq!(first(&r).auth, AuthConfig::new(AuthType::Inherit));
}

#[test]
fn invalid_documents_are_errors() {
    assert!(import("not: [valid").is_err());
    assert!(import("just a string").is_err());
    assert!(import("{\"openapi\":\"3.0.0\"}").is_err());
    assert!(import("{").is_err());
}

#[test]
fn path_param_conversion() {
    assert_eq!(convert_path_params("/a/{b}/c/{d}"), "/a/:b/c/:d");
    assert_eq!(convert_path_params("/a/{}/c"), "/a/{}/c");
    assert_eq!(convert_path_params("/a/{open"), "/a/{open");
}

#[test]
fn yaml_alias_fanout_is_rejected_quickly() {
    // One 500-item anchor referenced 4000 times: ~2M nodes from ~30 KB of text, which passes
    // serde_yaml's own repetition check.
    let items = vec!["x"; 500].join(", ");
    let mut y =
        format!("openapi: 3.0.0\ninfo: {{title: T, version: '1'}}\nbase: &a [{items}]\nrefs:\n");
    for _ in 0..4000 {
        y.push_str("  - *a\n");
    }
    let start = std::time::Instant::now();
    let err = import(&y).unwrap_err().to_string();
    assert!(err.contains("too many nodes"), "{err}");
    assert!(start.elapsed().as_secs() < 5);
}

#[test]
fn yaml_with_aliases_still_imports() {
    let y = "openapi: 3.0.0\ninfo: {title: T, version: '1'}\nx: &a [1, 2]\ny: *a\npaths: {}\n";
    assert_eq!(import(y).unwrap().collections[0].name, "T");
}

#[test]
fn repeated_operation_tags_do_not_duplicate_requests() {
    let spec = spec3(json!({"paths": {"/a": {"get": {"tags": ["t","t","t"]}}}}));
    let r = import(&spec).unwrap();
    let CollectionItem::Folder(f) = &r.collections[0].items[0] else {
        panic!("expected folder")
    };
    assert_eq!(f.items.len(), 1);
}

#[test]
fn operation_tags_keep_order_and_are_capped() {
    let many: Vec<String> = (0..100).map(|i| format!("t{i}")).collect();
    let spec = spec3(json!({"paths": {
        "/a": {"get": {"tags": ["users", "admin", "users"]}},
        "/b": {"get": {"tags": many}},
    }}));
    let r = import(&spec).unwrap();
    let names: Vec<&str> = r.collections[0]
        .items
        .iter()
        .filter_map(|i| match i {
            CollectionItem::Folder(f) => Some(f.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(&names[..2], ["users", "admin"]);
    assert_eq!(names.len(), 2 + 32);
}

#[test]
fn large_but_plain_yaml_still_imports_under_the_node_cap() {
    let items = vec!["1"; 1_000_000].join(", ");
    let y =
        format!("openapi: 3.0.0\ninfo: {{title: Big, version: '1'}}\nx: [{items}]\npaths: {{}}\n");
    assert_eq!(import(&y).unwrap().collections[0].name, "Big");
}
