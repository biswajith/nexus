use super::*;
use crate::types::{HeaderValue, HttpMethod, RequestSettings};
use indexmap::IndexMap;
use wiremock::{matchers::any, Mock, MockServer, Request, ResponseTemplate};

fn hdrs(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn opts(method: HttpMethod, url: String, headers: IndexMap<String, String>) -> SendOptions {
    SendOptions {
        method,
        url,
        headers,
        body: None,
        settings: RequestSettings::default(),
    }
}

/// `/<code>/same` redirects within the server, `/<code>/cross` to `target`, anything else is 200.
async fn redirector(target: &MockServer) -> MockServer {
    let target_origin = target.uri();
    let source = MockServer::start().await;
    Mock::given(any())
        .respond_with(move |req: &Request| {
            let segs: Vec<&str> = req.url.path().split('/').collect();
            let (code, place) = (
                segs.get(1).copied().unwrap_or(""),
                segs.get(2).copied().unwrap_or(""),
            );
            let Ok(code) = code.parse::<u16>() else {
                return ResponseTemplate::new(200);
            };
            match place {
                "same" => ResponseTemplate::new(code).insert_header("location", "/final"),
                "cross" => ResponseTemplate::new(code)
                    .insert_header("location", format!("{target_origin}/final").as_str()),
                _ => ResponseTemplate::new(200),
            }
        })
        .mount(&source)
        .await;
    source
}

async fn ok_server() -> MockServer {
    let s = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&s)
        .await;
    s
}

fn credentials() -> IndexMap<String, String> {
    hdrs(&[
        ("X-Api-Key", "secret-key"),
        ("x-amz-security-token", "session-token"),
        ("Authorization", "Bearer t"),
        ("Cookie", "sid=1"),
        ("Accept", "application/json"),
    ])
}

async fn last(server: &MockServer) -> Request {
    server.received_requests().await.unwrap().pop().unwrap()
}

#[tokio::test]
async fn drops_credential_headers_when_a_redirect_changes_origin() {
    let target = ok_server().await;
    let source = redirector(&target).await;
    let res = send_http_request(opts(
        HttpMethod::Get,
        format!("{}/302/cross", source.uri()),
        credentials(),
    ))
    .await
    .unwrap();
    assert_eq!(res.status, 200);
    let seen = last(&target).await;
    assert_eq!(seen.url.path(), "/final");
    for h in [
        "x-api-key",
        "x-amz-security-token",
        "authorization",
        "cookie",
    ] {
        assert!(seen.headers.get(h).is_none(), "{h} leaked");
    }
    assert_eq!(
        seen.headers.get("accept").unwrap().to_str().unwrap(),
        "application/json"
    );
}

#[tokio::test]
async fn keeps_headers_on_a_same_origin_redirect() {
    let target = ok_server().await;
    let source = redirector(&target).await;
    send_http_request(opts(
        HttpMethod::Get,
        format!("{}/301/same", source.uri()),
        credentials(),
    ))
    .await
    .unwrap();
    let seen = last(&source).await;
    assert_eq!(seen.url.path(), "/final");
    assert_eq!(
        seen.headers.get("x-api-key").unwrap().to_str().unwrap(),
        "secret-key"
    );
    assert_eq!(
        seen.headers
            .get("x-amz-security-token")
            .unwrap()
            .to_str()
            .unwrap(),
        "session-token"
    );
}

#[tokio::test]
async fn does_not_follow_redirects_when_follow_redirects_is_false() {
    let target = ok_server().await;
    let source = redirector(&target).await;
    let mut o = opts(
        HttpMethod::Get,
        format!("{}/302/cross", source.uri()),
        credentials(),
    );
    o.settings.follow_redirects = Some(false);
    let res = send_http_request(o).await.unwrap();
    assert_eq!(res.status, 302);
    assert_eq!(source.received_requests().await.unwrap().len(), 1);
    assert!(target.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn turns_post_into_get_on_303_and_keeps_the_method_on_307() {
    let target = ok_server().await;
    let source = redirector(&target).await;
    let mut o = opts(
        HttpMethod::Post,
        format!("{}/303/same", source.uri()),
        hdrs(&[("Content-Type", "text/plain")]),
    );
    o.body = Some(SendBody::Text("x".into()));
    send_http_request(o.clone()).await.unwrap();
    let seen = last(&source).await;
    assert_eq!(seen.method.as_str(), "GET");
    assert!(seen.headers.get("content-type").is_none());
    assert!(seen.body.is_empty());

    o.url = format!("{}/307/same", source.uri());
    send_http_request(o).await.unwrap();
    let seen = last(&source).await;
    assert_eq!(seen.method.as_str(), "POST");
    assert_eq!(seen.body, b"x");
}

#[tokio::test]
async fn stops_after_max_redirects() {
    let target = ok_server().await;
    let source = redirector(&target).await;
    let mut o = opts(
        HttpMethod::Get,
        format!("{}/302/cross", source.uri()),
        IndexMap::new(),
    );
    o.settings.max_redirects = Some(0);
    assert_eq!(send_http_request(o).await.unwrap().status, 302);
}

#[tokio::test]
async fn parses_body_json_text_size_headers_and_status_text() {
    let s = MockServer::start().await;
    Mock::given(any())
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_string(r#"{"a":1}"#)
                .insert_header("X-Thing", "1")
                .append_header("Set-Cookie", "a=1")
                .append_header("Set-Cookie", "b=2"),
        )
        .mount(&s)
        .await;
    let res = send_http_request(opts(HttpMethod::Get, s.uri(), IndexMap::new()))
        .await
        .unwrap();
    assert_eq!(res.status, 201);
    assert_eq!(res.status_text, "Created");
    assert_eq!(res.body_text, r#"{"a":1}"#);
    assert_eq!(res.body_json, Some(serde_json::json!({"a": 1})));
    assert_eq!(res.size, 7);
    assert_eq!(res.headers["x-thing"], HeaderValue::One("1".into()));
    assert_eq!(
        res.headers["set-cookie"],
        HeaderValue::Many(vec!["a=1".into(), "b=2".into()])
    );
    assert_eq!(
        (res.timing.dns, res.timing.tcp, res.timing.tls),
        (0.0, 0.0, 0.0)
    );
    assert!(res.timing.total >= res.timing.ttfb);
}

#[tokio::test]
async fn non_json_body_has_no_body_json_and_unmapped_status_has_empty_text() {
    let s = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(418).set_body_string("tea"))
        .mount(&s)
        .await;
    let res = send_http_request(opts(HttpMethod::Get, s.uri(), IndexMap::new()))
        .await
        .unwrap();
    assert_eq!(res.body_json, None);
    assert_eq!(res.status_text, "");
}

#[tokio::test]
async fn sends_method_headers_and_body() {
    let s = ok_server().await;
    let mut o = opts(
        HttpMethod::Put,
        format!("{}/x?y=1", s.uri()),
        hdrs(&[("X-A", "b")]),
    );
    o.body = Some(SendBody::Bytes(vec![1, 2, 3]));
    send_http_request(o).await.unwrap();
    let seen = last(&s).await;
    assert_eq!(seen.method.as_str(), "PUT");
    assert_eq!(seen.url.query(), Some("y=1"));
    assert_eq!(seen.headers.get("x-a").unwrap().to_str().unwrap(), "b");
    assert_eq!(seen.body, vec![1, 2, 3]);
}

#[tokio::test]
async fn times_out() {
    let s = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(500)))
        .mount(&s)
        .await;
    let mut o = opts(HttpMethod::Get, s.uri(), IndexMap::new());
    o.settings.timeout = Some(50);
    let err = send_http_request(o).await.unwrap_err();
    assert!(err.to_string().contains("timed out"));
}

#[tokio::test]
async fn invalid_url_and_header_are_errors() {
    assert!(
        send_http_request(opts(HttpMethod::Get, "not a url".into(), IndexMap::new()))
            .await
            .is_err()
    );
    let s = ok_server().await;
    assert!(
        send_http_request(opts(HttpMethod::Get, s.uri(), hdrs(&[("bad header", "x")])))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn connection_refused_is_an_error() {
    let err = send_http_request(opts(
        HttpMethod::Get,
        "http://127.0.0.1:1/".into(),
        IndexMap::new(),
    ))
    .await
    .unwrap_err();
    assert!(err.to_string().contains("error sending request"));
}

#[test]
fn strip_cross_origin_headers_keeps_only_allowlisted_case_insensitively() {
    let kept = strip_cross_origin_headers(&hdrs(&[
        ("Content-Type", "application/json"),
        ("USER-AGENT", "nexus"),
        ("X-Custom-Token", "t"),
        ("Proxy-Authorization", "p"),
    ]));
    assert_eq!(
        kept,
        hdrs(&[
            ("Content-Type", "application/json"),
            ("USER-AGENT", "nexus")
        ])
    );
}
