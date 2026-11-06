use super::*;

fn imp(cmd: &str) -> NexusRequest {
    import(cmd).unwrap()
}

#[test]
fn simple_get() {
    let req = imp("curl https://api.example.com/users");
    assert_eq!(req.method, HttpMethod::Get);
    assert_eq!(req.url, "https://api.example.com/users");
    assert!(req.headers.is_empty());
    assert_eq!(req.body.mode, BodyMode::None);
}

#[test]
fn id_name_and_auth() {
    let req = imp("curl https://api.example.com/test");
    assert!(req.id.starts_with("req_"));
    assert_eq!(imp("curl https://api.example.com/users").name, "GET /users");
    assert_eq!(
        imp("curl https://api.example.com").auth,
        AuthConfig::new(AuthType::None)
    );
}

#[test]
fn url_without_protocol() {
    let req = imp("curl example.com/api");
    assert_eq!(req.url, "example.com/api");
    assert!(req.name.contains("/api"));
}

#[test]
fn request_flag() {
    assert_eq!(
        imp("curl -X POST https://api.example.com/users").method,
        HttpMethod::Post
    );
    assert_eq!(
        imp("curl --request PUT https://api.example.com/users/1").method,
        HttpMethod::Put
    );
    assert_eq!(
        imp("curl -X DELETE https://api.example.com/users/1").method,
        HttpMethod::Delete
    );
    assert_eq!(
        imp("curl -X patch https://api.example.com/users/1").method,
        HttpMethod::Patch
    );
}

#[test]
fn unsupported_method_is_an_error() {
    assert!(import("curl -X FROB https://api.example.com").is_err());
}

#[test]
fn headers() {
    let req = imp("curl -H \"Content-Type: application/json\" https://api.example.com");
    assert_eq!(req.headers.len(), 1);
    assert_eq!(
        req.headers[0],
        KeyValuePair {
            key: "Content-Type".into(),
            value: "application/json".into(),
            enabled: true,
            description: None
        }
    );

    let req = imp("curl -H \"Accept: application/json\" -H \"Authorization: Bearer tok123\" https://api.example.com");
    assert_eq!(req.headers.len(), 2);
    assert_eq!(req.headers[0].key, "Accept");
    assert_eq!(req.headers[1].key, "Authorization");

    let req = imp("curl --header \"X-Custom: value\" https://api.example.com");
    assert_eq!(
        (req.headers[0].key.as_str(), req.headers[0].value.as_str()),
        ("X-Custom", "value")
    );

    let req = imp("curl -H \"X-URL: https://example.com\" https://api.example.com");
    assert_eq!(req.headers[0].value, "https://example.com");

    assert!(imp("curl -H \"NoColon\" https://api.example.com")
        .headers
        .is_empty());
}

#[test]
fn data_flags() {
    let req = imp("curl -d '{\"name\":\"alice\"}' https://api.example.com/users");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.body.mode, BodyMode::Json);
    assert_eq!(req.body.raw.as_deref(), Some("{\"name\":\"alice\"}"));

    let req = imp("curl -H \"Content-Type: application/json\" -d 'data' https://api.example.com");
    assert_eq!(req.body.mode, BodyMode::Json);

    let req = imp("curl -d \"user=alice&pass=secret\" https://api.example.com/login");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.body.mode, BodyMode::XWwwFormUrlencoded);
    let enc = req.body.urlencoded.unwrap();
    assert_eq!(enc.len(), 2);
    assert_eq!(
        (enc[0].key.as_str(), enc[0].value.as_str()),
        ("user", "alice")
    );

    let req = imp("curl -H \"Content-Type: application/x-www-form-urlencoded\" -d \"a=1\" https://api.example.com");
    assert_eq!(req.body.mode, BodyMode::XWwwFormUrlencoded);

    let req =
        imp("curl -H \"Content-Type: text/plain\" -d \"hello world\" https://api.example.com");
    assert_eq!(req.body.mode, BodyMode::Text);
    assert_eq!(req.body.raw.as_deref(), Some("hello world"));

    assert_eq!(
        imp("curl -d '{\"key\":\"val\"}' https://api.example.com").method,
        HttpMethod::Post
    );
    assert_eq!(
        imp("curl -X PUT -d '{\"key\":\"val\"}' https://api.example.com").method,
        HttpMethod::Put
    );
    assert_eq!(
        imp("curl --data-raw '{\"a\":1}' https://api.example.com")
            .body
            .mode,
        BodyMode::Json
    );
    assert_eq!(
        imp("curl --data-binary '{\"b\":2}' https://api.example.com")
            .body
            .mode,
        BodyMode::Json
    );
}

#[test]
fn data_urlencode() {
    let req = imp("curl --data-urlencode \"name=Alice\" --data-urlencode \"city=New York\" https://api.example.com");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.body.mode, BodyMode::XWwwFormUrlencoded);
    let enc = req.body.urlencoded.unwrap();
    assert_eq!(enc.len(), 2);
    assert_eq!(
        (enc[0].key.as_str(), enc[0].value.as_str()),
        ("name", "Alice")
    );
    assert_eq!(enc[1].value, "New York");
}

#[test]
fn multipart_form() {
    let req = imp("curl -F \"file=@/path/to/file\" -F \"name=doc\" https://api.example.com/upload");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.body.mode, BodyMode::FormData);
    let fd = req.body.form_data.unwrap();
    assert_eq!(fd.len(), 2);
    assert_eq!(fd[0].key, "file");
    assert_eq!(fd[0].value, "@/path/to/file");
    assert_eq!(fd[0].entry_type, FormDataType::Text);

    assert_eq!(
        imp("curl --form \"key=val\" https://api.example.com")
            .body
            .mode,
        BodyMode::FormData
    );
}

#[test]
fn basic_auth_flag() {
    for cmd in [
        "curl -u admin:password https://api.example.com",
        "curl --user admin:password https://api.example.com",
    ] {
        let req = imp(cmd);
        let h = req
            .headers
            .iter()
            .find(|h| h.key == "Authorization")
            .unwrap();
        assert!(h.value.starts_with("Basic "));
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(h.value.trim_start_matches("Basic "))
            .unwrap();
        assert_eq!(decoded, b"admin:password");
    }
}

#[test]
fn query_params() {
    let req = imp("curl \"https://api.example.com/search?q=hello&page=2\"");
    assert_eq!(req.params.len(), 2);
    assert_eq!(
        (req.params[0].key.as_str(), req.params[0].value.as_str()),
        ("q", "hello")
    );
    assert_eq!(
        (req.params[1].key.as_str(), req.params[1].value.as_str()),
        ("page", "2")
    );
    assert_eq!(req.url, "https://api.example.com/search");

    assert_eq!(
        imp("curl \"https://api.example.com/search?q=hello%20world\"").params[0].value,
        "hello world"
    );
    assert!(imp("curl https://api.example.com/data").params.is_empty());
}

#[test]
fn ignored_flags() {
    for flag in ["-k", "-L", "-v", "-s", "--compressed"] {
        let req = imp(&format!("curl {flag} https://api.example.com"));
        assert_eq!(req.url, "https://api.example.com");
        assert_eq!(req.method, HttpMethod::Get);
    }
}

#[test]
fn continuations_and_quoting() {
    let req = imp("curl \\\n  -X POST \\\n  -H \"Content-Type: application/json\" \\\n  https://api.example.com");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.headers[0].key, "Content-Type");

    assert_eq!(
        imp("curl \\\r\n  -X POST \\\r\n  https://api.example.com").method,
        HttpMethod::Post
    );

    let req = imp("curl -H 'Accept: text/plain' https://api.example.com");
    assert_eq!(
        (req.headers[0].key.as_str(), req.headers[0].value.as_str()),
        ("Accept", "text/plain")
    );
    assert_eq!(
        imp("curl -H \"Accept: text/plain\" https://api.example.com").headers[0].key,
        "Accept"
    );
    assert_eq!(
        imp("curl 'https://api.example.com/test'").url,
        "https://api.example.com/test"
    );
}

#[test]
fn chrome_devtools_format() {
    let req = imp("curl 'https://api.example.com/users' -H $'Content-Type: application/json' --data-raw $'{\"name\":\"test\"}'");
    assert_eq!(req.headers[0].key, "Content-Type");
    assert_eq!(req.headers[0].value, "application/json");
    assert_eq!(req.body.mode, BodyMode::Json);
    assert_eq!(req.body.raw.as_deref(), Some("{\"name\":\"test\"}"));

    let req = imp("curl -d $'line1\\nline2' https://api.example.com");
    assert_eq!(req.body.raw.as_deref(), Some("line1\nline2"));
}

#[test]
fn real_world_examples() {
    let req = imp(
        "curl -X POST \\\n  -H \"Content-Type: application/json\" \\\n  -H \"Accept: application/json\" \\\n  -u admin:secret \\\n  -d '{\"name\":\"New User\",\"email\":\"new@example.com\"}' \\\n  https://api.example.com/users",
    );
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.url, "https://api.example.com/users");
    assert!(req.headers.len() >= 3);
    assert_eq!(req.body.mode, BodyMode::Json);
    let body: serde_json::Value = serde_json::from_str(req.body.raw.as_deref().unwrap()).unwrap();
    assert_eq!(body["name"], "New User");

    let req = imp("curl -s -L -k -X GET -H \"Authorization: Bearer token123\" \"https://api.example.com/data?format=json&limit=100\"");
    assert_eq!(req.method, HttpMethod::Get);
    assert_eq!(req.params.len(), 2);
    assert!(req.headers.iter().any(|h| h.key == "Authorization"));
}

#[test]
fn oversized_input_is_rejected() {
    let huge = format!(
        "curl https://x.com/{}",
        "a".repeat(super::super::MAX_IMPORT_BYTES)
    );
    assert!(import(&huge).is_err());
}
