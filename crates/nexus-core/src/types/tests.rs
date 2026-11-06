use super::*;
use serde_json::json;

fn roundtrip<T>(value: Value) -> T
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let parsed: T = serde_json::from_value(value.clone()).expect("deserialize");
    let back = serde_json::to_value(&parsed).expect("serialize");
    assert_eq!(back, value);
    parsed
}

#[test]
fn http_method_strings() {
    for (m, s) in [
        (HttpMethod::Get, "GET"),
        (HttpMethod::Post, "POST"),
        (HttpMethod::Put, "PUT"),
        (HttpMethod::Patch, "PATCH"),
        (HttpMethod::Delete, "DELETE"),
        (HttpMethod::Head, "HEAD"),
        (HttpMethod::Options, "OPTIONS"),
    ] {
        assert_eq!(serde_json::to_value(m).unwrap(), json!(s));
        assert_eq!(serde_json::from_value::<HttpMethod>(json!(s)).unwrap(), m);
    }
}

#[test]
fn body_mode_and_auth_type_strings() {
    for (m, s) in [
        (BodyMode::None, "none"),
        (BodyMode::Json, "json"),
        (BodyMode::Xml, "xml"),
        (BodyMode::Text, "text"),
        (BodyMode::Html, "html"),
        (BodyMode::FormData, "form-data"),
        (BodyMode::XWwwFormUrlencoded, "x-www-form-urlencoded"),
        (BodyMode::Binary, "binary"),
        (BodyMode::Graphql, "graphql"),
    ] {
        assert_eq!(serde_json::to_value(m).unwrap(), json!(s));
        assert_eq!(serde_json::from_value::<BodyMode>(json!(s)).unwrap(), m);
    }
    for (a, s) in [
        (AuthType::None, "none"),
        (AuthType::Inherit, "inherit"),
        (AuthType::ApiKey, "api-key"),
        (AuthType::Bearer, "bearer"),
        (AuthType::Basic, "basic"),
        (AuthType::Oauth2, "oauth2"),
        (AuthType::Digest, "digest"),
        (AuthType::AwsSigV4, "aws-sig-v4"),
        (AuthType::Hawk, "hawk"),
    ] {
        assert_eq!(serde_json::to_value(a).unwrap(), json!(s));
        assert_eq!(serde_json::from_value::<AuthType>(json!(s)).unwrap(), a);
    }
}

#[test]
fn auth_config_keeps_extra_keys() {
    let a: AuthConfig =
        roundtrip(json!({"type": "bearer", "token": "abc", "nested": {"a": [1, 2]}}));
    assert_eq!(a.auth_type, AuthType::Bearer);
    assert_eq!(a.extra["token"], json!("abc"));
    assert_eq!(
        serde_json::to_value(AuthConfig::new(AuthType::Inherit)).unwrap(),
        json!({"type": "inherit"})
    );
}

#[test]
fn request_body_omits_optionals_and_uses_camel_case() {
    assert_eq!(
        serde_json::to_value(RequestBody::default()).unwrap(),
        json!({"mode": "none"})
    );
    roundtrip::<RequestBody>(json!({
        "mode": "form-data",
        "formData": [{"key": "f", "value": "/tmp/x", "type": "file", "enabled": true, "description": "d"}],
        "urlencoded": [{"key": "a", "value": "b", "enabled": false}],
        "binary": {"path": "/x", "contentType": "image/png"},
        "graphql": {"query": "{ a }", "variables": "{}"}
    }));
}

#[test]
fn default_request_matches_ts_shape() {
    let r = create_default_request();
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["name"], "New Request");
    assert_eq!(v["method"], "GET");
    assert_eq!(v["url"], "");
    assert_eq!(v["headers"], json!([]));
    assert_eq!(v["params"], json!([]));
    assert_eq!(v["body"], json!({"mode": "none"}));
    assert_eq!(v["auth"], json!({"type": "inherit"}));
    assert_eq!(v["settings"], json!({}));
    assert!(v.get("preRequestScript").is_none());
    assert!(uuid::Uuid::parse_str(&r.id).is_ok());
    assert_ne!(r.id, create_default_request().id);
}

#[test]
fn default_environment_matches_ts_shape() {
    let e = create_default_environment();
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["name"], "New Environment");
    assert_eq!(v["variables"], json!([]));
}

#[test]
fn collection_with_nested_folder_roundtrips() {
    let req = serde_json::to_value(create_default_request()).unwrap();
    let c: NexusCollection = roundtrip(json!({
        "id": "c1",
        "name": "Coll",
        "version": "1",
        "variables": [{"key": "k", "value": "v", "type": "secret", "enabled": true}],
        "auth": {"type": "basic", "username": "u", "password": "p"},
        "preRequestScript": "x",
        "items": [
            {"id": "f1", "name": "F", "items": [req.clone()], "itemOrder": ["a"]},
            req
        ],
        "itemOrder": ["f1"]
    }));
    assert!(is_folder(&c.items[0]));
    assert!(is_request(&c.items[1]));
    match &c.items[0] {
        CollectionItem::Folder(f) => assert!(f.items[0].is_request()),
        _ => unreachable!(),
    }
}

#[test]
fn request_settings_roundtrip() {
    roundtrip::<RequestSettings>(
        json!({"followRedirects": true, "maxRedirects": 5, "timeout": 3000, "rejectUnauthorized": false}),
    );
    roundtrip::<RequestSettings>(json!({}));
}

#[test]
fn response_hides_body_bytes_and_keeps_header_arrays() {
    let r: NexusResponse = serde_json::from_value(json!({
        "status": 200,
        "statusText": "OK",
        "headers": {"content-type": "application/json", "set-cookie": ["a=1", "b=2"]},
        "bodyText": "{\"a\":1}",
        "bodyJson": {"a": 1},
        "timing": {"dns": 0, "tcp": 0, "tls": 0, "ttfb": 12.5, "download": 1, "total": 13.5},
        "size": 7
    }))
    .unwrap();
    assert!(r.body.is_empty());
    let mut r2 = r.clone();
    r2.body = vec![1, 2, 3];
    let v = serde_json::to_value(&r2).unwrap();
    assert!(v.get("body").is_none());
    assert_eq!(v["headers"]["set-cookie"], json!(["a=1", "b=2"]));
    assert_eq!(v["bodyJson"], json!({"a": 1}));
    assert_eq!(v["statusText"], "OK");
    let r3 = NexusResponse::default();
    assert!(serde_json::to_value(&r3).unwrap().get("bodyJson").is_none());
}

#[test]
fn history_and_console_roundtrip() {
    roundtrip::<HistoryEntry>(json!({
        "id": "h", "timestamp": 1700000000000i64,
        "request": {"method": "GET", "url": "http://x", "headers": {"a": "b"}, "body": "z"},
        "response": {"status": 200, "statusText": "OK", "responseTime": 12.0, "size": 3},
        "collectionId": "c", "requestId": "r"
    }));
    roundtrip::<HistoryFilter>(
        json!({"startDate": 1, "method": "POST", "urlPattern": "x", "statusCode": 404, "limit": 10, "offset": 5}),
    );
    roundtrip::<ConsoleEntry>(json!({
        "id": "e", "timestamp": 1, "level": "warn", "source": "script", "requestId": "r", "message": "m", "data": {"x": [1]}
    }));
    roundtrip::<ConsoleFilter>(
        json!({"level": "debug", "source": "system", "requestId": "r", "search": "s"}),
    );
    for s in ["local", "environment", "collection", "global"] {
        roundtrip::<ScopeLevel>(json!(s));
    }
    for s in ["json", "xml", "html", "text", "image", "pdf", "binary"] {
        roundtrip::<ResponseFormat>(json!(s));
    }
}
