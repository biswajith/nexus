use super::handler::apply_auth_at;
use super::*;
use crate::http::SendBody;
use crate::types::{AuthConfig, AuthType};
use chrono::{DateTime, TimeZone, Utc};
use indexmap::IndexMap;
use serde_json::{json, Value};

fn config(auth_type: &str, extra: Value) -> AuthConfig {
    let mut v = json!({ "type": auth_type });
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn req() -> PreparedRequest {
    PreparedRequest {
        method: "GET".into(),
        url: "https://api.example.com/data?existing=1".into(),
        headers: IndexMap::new(),
        body: None,
    }
}

fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2024, 5, 6, 7, 8, 9).unwrap()
}

fn apply(c: &AuthConfig, r: &PreparedRequest) -> AuthResult {
    apply_auth(c, r).unwrap()
}

fn header_of(r: &AuthResult, name: &str) -> String {
    r.headers[name].clone()
}

fn single_header(name: &str, value: &str) -> IndexMap<String, String> {
    IndexMap::from([(name.to_string(), value.to_string())])
}

#[test]
fn none_inherit_and_hawk_are_empty() {
    for t in ["none", "inherit", "hawk"] {
        assert!(apply(&config(t, json!({})), &req()).is_empty(), "{t}");
    }
}

#[test]
fn bearer() {
    let r = apply(&config("bearer", json!({"token": "my-jwt-token"})), &req());
    assert_eq!(
        r.headers,
        single_header("Authorization", "Bearer my-jwt-token")
    );
    let r = apply(
        &config("bearer", json!({"token": "tok123", "prefix": "Token"})),
        &req(),
    );
    assert_eq!(r.headers, single_header("Authorization", "Token tok123"));
    assert!(apply(&config("bearer", json!({})), &req()).is_empty());
    assert!(apply(&config("bearer", json!({"token": ""})), &req()).is_empty());
}

#[test]
fn basic() {
    let r = apply(
        &config("basic", json!({"username": "user", "password": "pass"})),
        &req(),
    );
    assert_eq!(
        r.headers,
        single_header("Authorization", "Basic dXNlcjpwYXNz")
    );
    let r = apply(&config("basic", json!({})), &req());
    assert_eq!(r.headers, single_header("Authorization", "Basic Og=="));
    let r = apply(
        &config(
            "basic",
            json!({"username": "user@domain.com", "password": "p@ss:w0rd!"}),
        ),
        &req(),
    );
    assert_eq!(
        r.headers,
        single_header(
            "Authorization",
            "Basic dXNlckBkb21haW4uY29tOnBAc3M6dzByZCE="
        )
    );
}

#[test]
fn api_key() {
    let c = |extra| config("api-key", extra);
    let r = apply(
        &c(json!({"key": "X-Api-Key", "value": "secret123"})),
        &req(),
    );
    assert_eq!(r.headers, single_header("X-Api-Key", "secret123"));
    let r = apply(
        &c(json!({"key": "X-Api-Key", "value": "secret123", "addTo": "header"})),
        &req(),
    );
    assert_eq!(r.headers, single_header("X-Api-Key", "secret123"));
    let r = apply(
        &c(json!({"key": "api_key", "value": "abc", "addTo": "query"})),
        &req(),
    );
    assert_eq!(r.params, single_header("api_key", "abc"));
    assert!(r.headers.is_empty());
    assert!(apply(&c(json!({"value": "abc"})), &req()).is_empty());
    assert!(apply(&c(json!({"key": "X-Api-Key"})), &req()).is_empty());
}

#[test]
fn oauth2() {
    let c = |extra| config("oauth2", extra);
    assert_eq!(
        apply(&c(json!({"accessToken": "oauth-tok"})), &req()).headers,
        single_header("Authorization", "Bearer oauth-tok")
    );
    assert_eq!(
        apply(
            &c(json!({"accessToken": "tok", "tokenType": "MAC"})),
            &req()
        )
        .headers,
        single_header("Authorization", "MAC tok")
    );
    assert_eq!(
        apply(&c(json!({"accessToken": "tok", "addTo": "query"})), &req()).params,
        single_header("access_token", "tok")
    );
    assert!(apply(&c(json!({})), &req()).is_empty());
}

fn digest_config() -> Value {
    json!({
        "username": "admin", "password": "secret", "realm": "test@example.com",
        "nonce": "abc123nonce", "qop": "auth", "algorithm": "MD5", "opaque": "opaque-val"
    })
}

fn merged(base: Value, over: Value) -> Value {
    let mut b = base;
    b.as_object_mut()
        .unwrap()
        .extend(over.as_object().unwrap().clone());
    b
}

fn digest_req() -> PreparedRequest {
    PreparedRequest {
        url: "https://api.example.com/resource?q=1".into(),
        ..req()
    }
}

#[test]
fn digest_header_has_expected_fields() {
    let r = apply(&config("digest", digest_config()), &digest_req());
    let h = header_of(&r, "Authorization");
    for needle in [
        "Digest",
        "username=\"admin\"",
        "realm=\"test@example.com\"",
        "nonce=\"abc123nonce\"",
        "uri=\"/resource?q=1\"",
        "algorithm=MD5",
        "qop=auth",
        "nc=00000001",
        "opaque=\"opaque-val\"",
    ] {
        assert!(h.contains(needle), "{needle} missing in {h}");
    }
    let re = regex::Regex::new(r#"response="[a-f0-9]+"[\s\S]*cnonce="[a-f0-9]{16}""#).unwrap();
    assert!(re.is_match(&h), "{h}");
}

#[test]
fn digest_known_vectors_computed_with_the_ts_algorithm() {
    let cnonce = "0123456789abcdef";
    let r = apply_auth_at(
        &config("digest", digest_config()),
        &digest_req(),
        fixed_now(),
        cnonce,
    )
    .unwrap();
    assert!(
        header_of(&r, "Authorization").contains("response=\"76286cfee99598fa522951647602f518\"")
    );

    let sha = merged(digest_config(), json!({"algorithm": "SHA-256"}));
    let r = apply_auth_at(&config("digest", sha), &digest_req(), fixed_now(), cnonce).unwrap();
    let h = header_of(&r, "Authorization");
    assert!(
        h.contains("response=\"5fbac89ea43b207c9a7a7f5a33e1ad939a23799010814c60c9efff328ed7bb52\"")
    );
    assert!(h.contains("algorithm=SHA-256"));

    let no_qop = merged(digest_config(), json!({"qop": ""}));
    let r = apply_auth_at(
        &config("digest", no_qop),
        &digest_req(),
        fixed_now(),
        cnonce,
    )
    .unwrap();
    let h = header_of(&r, "Authorization");
    assert!(h.contains("response=\"8fef50cf2a0670d182b9b36b50dd7b5c\""));
    assert!(!h.contains("qop=") && !h.contains("cnonce="));
}

#[test]
fn digest_requires_username_and_nonce_and_omits_empty_opaque() {
    assert!(apply(&config("digest", json!({"nonce": "n"})), &req()).is_empty());
    assert!(apply(&config("digest", json!({"username": "admin"})), &req()).is_empty());
    let c = config("digest", merged(digest_config(), json!({"opaque": ""})));
    assert!(!header_of(&apply(&c, &digest_req()), "Authorization").contains("opaque="));
}

fn aws_config() -> Value {
    json!({
        "accessKey": "AKIAIOSFODNN7EXAMPLE",
        "secretKey": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "region": "us-east-1", "service": "s3"
    })
}

#[test]
fn sig_v4_header_shape() {
    let r = apply(
        &config("aws-sig-v4", aws_config()),
        &PreparedRequest {
            url: "https://s3.amazonaws.com/mybucket".into(),
            ..req()
        },
    );
    let h = header_of(&r, "Authorization");
    assert!(h.contains("AWS4-HMAC-SHA256"));
    assert!(h.contains("Credential=AKIAIOSFODNN7EXAMPLE/"));
    assert!(h.contains("/us-east-1/s3/aws4_request"));
    assert!(h.contains("SignedHeaders=host;x-amz-date"));
    assert!(h.contains("Signature="));
    let re = regex::Regex::new(r"^\d{8}T\d{6}Z$").unwrap();
    assert!(re.is_match(&r.headers["x-amz-date"]));
    assert_eq!(r.headers["host"], "s3.amazonaws.com");
}

#[test]
fn sig_v4_session_token_and_defaults() {
    let c = config(
        "aws-sig-v4",
        merged(aws_config(), json!({"sessionToken": "my-session-token"})),
    );
    let r = apply(&c, &req());
    assert_eq!(r.headers["x-amz-security-token"], "my-session-token");
    assert!(header_of(&r, "Authorization")
        .contains("SignedHeaders=host;x-amz-date;x-amz-security-token"));
    assert!(!apply(&config("aws-sig-v4", aws_config()), &req())
        .headers
        .contains_key("x-amz-security-token"));

    let r = apply(
        &config("aws-sig-v4", json!({"accessKey": "AK", "secretKey": "SK"})),
        &req(),
    );
    assert!(header_of(&r, "Authorization").contains("/us-east-1/execute-api/aws4_request"));
}

#[test]
fn sig_v4_requires_both_keys() {
    assert!(apply(
        &config("aws-sig-v4", merged(aws_config(), json!({"accessKey": ""}))),
        &req()
    )
    .is_empty());
    assert!(apply(
        &config("aws-sig-v4", merged(aws_config(), json!({"secretKey": ""}))),
        &req()
    )
    .is_empty());
}

#[test]
fn sig_v4_hashes_the_body() {
    let c = config("aws-sig-v4", aws_config());
    let with = apply(
        &c,
        &PreparedRequest {
            body: Some(SendBody::Text("{\"key\":\"value\"}".into())),
            ..req()
        },
    );
    let without = apply(&c, &req());
    assert_ne!(
        header_of(&with, "Authorization"),
        header_of(&without, "Authorization")
    );
}

#[test]
fn sig_v4_known_vectors_computed_with_the_ts_algorithm() {
    // Query kept in order and form-encoded (`hello world` -> `hello+world`), lowercase method accepted.
    let r = PreparedRequest {
        method: "get".into(),
        url: "https://s3.amazonaws.com/mybucket?b=2&a=hello world&b=3".into(),
        headers: IndexMap::new(),
        body: Some(SendBody::Text("{\"key\":\"value\"}".into())),
    };
    let out = apply_auth_at(&config("aws-sig-v4", aws_config()), &r, fixed_now(), "x").unwrap();
    assert_eq!(out.headers["x-amz-date"], "20240506T070809Z");
    assert!(header_of(&out, "Authorization")
        .ends_with("Signature=a3d71d98eb840e916745d58168e55236a628e528e0b65838685656111b4a2787"));
    assert!(header_of(&out, "Authorization")
        .contains("Credential=AKIAIOSFODNN7EXAMPLE/20240506/us-east-1/s3/aws4_request"));

    // Port is part of the signed host.
    let r = PreparedRequest {
        method: "POST".into(),
        url: "https://api.example.com:8443".into(),
        headers: IndexMap::new(),
        body: None,
    };
    let c = config(
        "aws-sig-v4",
        merged(aws_config(), json!({"sessionToken": "tok"})),
    );
    let out = apply_auth_at(&c, &r, fixed_now(), "x").unwrap();
    assert_eq!(out.headers["host"], "api.example.com:8443");
    assert!(header_of(&out, "Authorization")
        .ends_with("Signature=878c605c0b3c90bd8e4d943ee06b469955b672d1f9f54a376427927fb3d0439b"));
}

#[test]
fn resolve_auth_leaves_none_and_inherit_unchanged() {
    for t in [AuthType::None, AuthType::Inherit] {
        let r = req();
        assert_eq!(resolve_auth(&AuthConfig::new(t), &r).unwrap(), r);
    }
}

#[test]
fn resolve_auth_merges_headers_without_mutating_the_input() {
    let mut r = req();
    r.headers.insert("X-Custom".into(), "value".into());
    let out = resolve_auth(&config("bearer", json!({"token": "tok"})), &r).unwrap();
    assert_eq!(out.headers["Authorization"], "Bearer tok");
    assert_eq!(out.headers["X-Custom"], "value");
    assert!(!r.headers.contains_key("Authorization"));
}

#[test]
fn resolve_auth_appends_and_preserves_query_params() {
    let c = config(
        "api-key",
        json!({"key": "api_key", "value": "secret", "addTo": "query"}),
    );
    let out = resolve_auth(
        &c,
        &PreparedRequest {
            url: "https://api.example.com/data".into(),
            ..req()
        },
    )
    .unwrap();
    assert_eq!(out.url, "https://api.example.com/data?api_key=secret");
    let out = resolve_auth(
        &c,
        &PreparedRequest {
            url: "https://api.example.com/data?page=1".into(),
            ..req()
        },
    )
    .unwrap();
    assert_eq!(
        out.url,
        "https://api.example.com/data?page=1&api_key=secret"
    );
}

#[test]
fn resolve_auth_set_replaces_existing_param_of_the_same_name() {
    let c = config(
        "api-key",
        json!({"key": "k", "value": "new", "addTo": "query"}),
    );
    let out = resolve_auth(
        &c,
        &PreparedRequest {
            url: "https://a.com/?k=old&x=1&k=dup".into(),
            ..req()
        },
    )
    .unwrap();
    assert_eq!(out.url, "https://a.com/?k=new&x=1");
}

#[test]
fn effective_auth_resolves_request_then_folder_then_collection() {
    let own = config("bearer", json!({"token": "own"}));
    let folder = config("basic", json!({}));
    let coll = config("bearer", json!({"token": "coll"}));
    let inherit = AuthConfig::new(AuthType::Inherit);

    assert_eq!(effective_auth(&own, &[Some(&folder), Some(&coll)]), own);
    assert_eq!(
        effective_auth(&inherit, &[Some(&folder), Some(&coll)]),
        folder
    );
    // An inheriting or missing folder defers to the next ancestor.
    assert_eq!(
        effective_auth(&inherit, &[Some(&inherit), None, Some(&coll)]),
        coll
    );
    assert_eq!(effective_auth(&inherit, &[None]).auth_type, AuthType::None);
    assert_eq!(effective_auth(&inherit, &[]).auth_type, AuthType::None);
    // Explicit `none` on a request does not inherit.
    assert_eq!(
        effective_auth(&AuthConfig::new(AuthType::None), &[Some(&coll)]).auth_type,
        AuthType::None
    );
}

#[test]
fn invalid_url_errors_for_digest_and_sig_v4() {
    let bad = PreparedRequest {
        url: "not a url".into(),
        ..req()
    };
    assert!(apply_auth(&config("digest", digest_config()), &bad).is_err());
    assert!(apply_auth(&config("aws-sig-v4", aws_config()), &bad).is_err());
}
