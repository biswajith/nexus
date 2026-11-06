use std::process::Command;

use serde_json::json;

use super::*;
use crate::types::*;

fn request() -> NexusRequest {
    NexusRequest {
        id: "req-1".into(),
        name: "Test Request".into(),
        method: HttpMethod::Get,
        url: "https://api.example.com/data".into(),
        headers: vec![],
        params: vec![],
        body: RequestBody::default(),
        auth: AuthConfig::new(AuthType::None),
        description: None,
        pre_request_script: None,
        post_response_script: None,
        settings: RequestSettings::default(),
    }
}

fn post() -> NexusRequest {
    NexusRequest {
        method: HttpMethod::Post,
        ..request()
    }
}

fn kv(key: &str, value: &str, enabled: bool) -> KeyValuePair {
    KeyValuePair {
        key: key.into(),
        value: value.into(),
        enabled,
        description: None,
    }
}

fn auth(auth_type: AuthType, extra: serde_json::Value) -> AuthConfig {
    let serde_json::Value::Object(extra) = extra else {
        panic!("object expected")
    };
    AuthConfig { auth_type, extra }
}

fn raw_body(mode: BodyMode, raw: &str) -> RequestBody {
    RequestBody {
        mode,
        raw: Some(raw.into()),
        ..Default::default()
    }
}

fn curl(r: &NexusRequest) -> String {
    generate_curl(r).unwrap()
}

// ---- method and URL ----

#[test]
fn basic_get_has_no_method_flag() {
    let r = curl(&request());
    assert!(r.contains("curl"));
    assert!(r.contains("'https://api.example.com/data'"));
    assert!(!r.contains("-X"));
}

#[test]
fn includes_method_for_non_get() {
    assert!(curl(&post()).contains("-X POST"));
    assert!(curl(&NexusRequest {
        method: HttpMethod::Delete,
        ..request()
    })
    .contains("-X DELETE"));
}

// ---- query parameters ----

#[test]
fn appends_enabled_params() {
    let r = curl(&NexusRequest {
        params: vec![kv("page", "1", true), kv("limit", "10", true)],
        ..request()
    });
    assert!(r.contains("page=1"));
    assert!(r.contains("limit=10"));
}

#[test]
fn skips_disabled_params() {
    let r = curl(&NexusRequest {
        params: vec![kv("page", "1", true), kv("secret", "hidden", false)],
        ..request()
    });
    assert!(r.contains("page=1"));
    assert!(!r.contains("secret"));
}

#[test]
fn skips_params_with_empty_keys() {
    let r = curl(&NexusRequest {
        params: vec![kv("", "val", true)],
        ..request()
    });
    assert!(!r.contains("=val"));
}

#[test]
fn uses_ampersand_when_url_has_query() {
    let r = curl(&NexusRequest {
        url: "https://api.example.com/data?existing=1".into(),
        params: vec![kv("extra", "2", true)],
        ..request()
    });
    assert!(r.contains("existing=1&extra=2"));
}

#[test]
fn url_encodes_params() {
    let r = curl(&NexusRequest {
        params: vec![kv("q", "hello world", true)],
        ..request()
    });
    assert!(r.contains("hello%20world"));
}

#[test]
fn encode_uri_component_matches_js() {
    assert_eq!(
        encode_uri_component("a b&c=d/é!*'()~-_."),
        "a%20b%26c%3Dd%2F%C3%A9!*'()~-_."
    );
}

// ---- headers ----

#[test]
fn adds_enabled_headers_only() {
    let r = curl(&NexusRequest {
        headers: vec![kv("Accept", "application/json", true)],
        ..request()
    });
    assert!(r.contains("-H 'Accept: application/json'"));
    let r = curl(&NexusRequest {
        headers: vec![kv("Accept", "application/json", false)],
        ..request()
    });
    assert!(!r.contains("Accept"));
}

#[test]
fn skips_headers_with_empty_keys() {
    let r = curl(&NexusRequest {
        headers: vec![kv("", "val", true)],
        ..request()
    });
    assert!(!r.contains("-H '"));
}

// ---- authentication ----

#[test]
fn bearer_auth() {
    let r = curl(&NexusRequest {
        auth: auth(AuthType::Bearer, json!({"token": "my-token"})),
        ..request()
    });
    assert!(r.contains("-H 'Authorization: Bearer my-token'"));
}

#[test]
fn bearer_with_empty_token_adds_nothing() {
    let r = curl(&NexusRequest {
        auth: auth(AuthType::Bearer, json!({"token": ""})),
        ..request()
    });
    assert!(!r.contains("Authorization"));
}

#[test]
fn basic_auth_uses_u_flag() {
    let r = curl(&NexusRequest {
        auth: auth(
            AuthType::Basic,
            json!({"username": "user", "password": "pass"}),
        ),
        ..request()
    });
    assert!(r.contains("-u 'user:pass'"));
}

#[test]
fn api_key_header() {
    let r = curl(&NexusRequest {
        auth: auth(
            AuthType::ApiKey,
            json!({"key": "X-Api-Key", "value": "abc123", "addTo": "header"}),
        ),
        ..request()
    });
    assert!(r.contains("-H 'X-Api-Key: abc123'"));
}

#[test]
fn api_key_needs_key_and_value() {
    let r = curl(&NexusRequest {
        auth: auth(
            AuthType::ApiKey,
            json!({"key": "", "value": "abc123", "addTo": "header"}),
        ),
        ..request()
    });
    assert!(!r.contains("X-Api-Key"));
    assert!(!r.contains("abc123"));
}

#[test]
fn api_key_not_added_for_query_placement() {
    let r = curl(&NexusRequest {
        auth: auth(
            AuthType::ApiKey,
            json!({"key": "X-Api-Key", "value": "abc", "addTo": "query"}),
        ),
        ..request()
    });
    assert!(!r.contains("-H 'X-Api-Key"));
}

// ---- body modes ----

#[test]
fn json_body_with_content_type() {
    let r = curl(&NexusRequest {
        body: raw_body(BodyMode::Json, r#"{"key":"value"}"#),
        ..post()
    });
    assert!(r.contains("-H 'Content-Type: application/json'"));
    assert!(r.contains(r#"--data-raw '{"key":"value"}'"#));
}

#[test]
fn xml_html_text_content_types() {
    let r = curl(&NexusRequest {
        body: raw_body(BodyMode::Xml, "<root/>"),
        ..post()
    });
    assert!(r.contains("-H 'Content-Type: application/xml'"));
    assert!(r.contains("--data-raw '<root/>'"));
    let r = curl(&NexusRequest {
        body: raw_body(BodyMode::Html, "<h1>Hello</h1>"),
        ..post()
    });
    assert!(r.contains("-H 'Content-Type: text/html'"));
    let r = curl(&NexusRequest {
        body: raw_body(BodyMode::Text, "hello"),
        ..post()
    });
    assert!(r.contains("-H 'Content-Type: text/plain'"));
    assert!(r.contains("--data-raw 'hello'"));
}

#[test]
fn does_not_duplicate_content_type() {
    let r = curl(&NexusRequest {
        headers: vec![kv("Content-Type", "application/json; charset=utf-8", true)],
        body: raw_body(BodyMode::Json, "{}"),
        ..post()
    });
    assert_eq!(r.matches("Content-Type").count(), 1);
}

#[test]
fn empty_raw_body_sends_nothing() {
    let r = curl(&NexusRequest {
        body: raw_body(BodyMode::Json, ""),
        ..post()
    });
    assert!(!r.contains("--data-raw"));
}

#[test]
fn escapes_single_quotes_in_body() {
    let r = curl(&NexusRequest {
        body: raw_body(BodyMode::Json, "it's a test"),
        ..post()
    });
    assert!(r.contains("it'\\''s a test"));
}

#[test]
fn urlencoded_entries() {
    let r = curl(&NexusRequest {
        body: RequestBody {
            mode: BodyMode::XWwwFormUrlencoded,
            urlencoded: Some(vec![
                kv("name", "John", true),
                kv("disabled", "skip", false),
            ]),
            ..Default::default()
        },
        ..post()
    });
    assert!(r.contains("--data-urlencode 'name=John'"));
    assert!(!r.contains("disabled"));
}

fn form_entry(key: &str, value: &str, entry_type: FormDataType, enabled: bool) -> FormDataEntry {
    FormDataEntry {
        key: key.into(),
        value: value.into(),
        entry_type,
        enabled,
        description: None,
    }
}

fn form_body(entries: Vec<FormDataEntry>) -> RequestBody {
    RequestBody {
        mode: BodyMode::FormData,
        form_data: Some(entries),
        ..Default::default()
    }
}

#[test]
fn form_data_text_entries() {
    let r = curl(&NexusRequest {
        body: form_body(vec![form_entry("name", "John", FormDataType::Text, true)]),
        ..post()
    });
    assert!(r.contains("--form-string 'name=John'"));
}

#[test]
fn form_data_file_entries() {
    let r = curl(&NexusRequest {
        body: form_body(vec![form_entry(
            "avatar",
            "/path/to/file.png",
            FormDataType::File,
            true,
        )]),
        ..post()
    });
    assert!(r.contains("-F 'avatar=@/path/to/file.png'"));
}

#[test]
fn skips_disabled_form_data() {
    let r = curl(&NexusRequest {
        body: form_body(vec![form_entry("skip", "me", FormDataType::Text, false)]),
        ..post()
    });
    assert!(!r.contains("-F"));
    assert!(!r.contains("--form-string"));
}

#[test]
fn binary_body() {
    let body = RequestBody {
        mode: BodyMode::Binary,
        binary: Some(BinaryBody {
            path: "/tmp/data.bin".into(),
            content_type: None,
        }),
        ..Default::default()
    };
    assert!(curl(&NexusRequest { body, ..post() }).contains("--data-binary '@/tmp/data.bin'"));
}

#[test]
fn binary_without_path_sends_nothing() {
    let body = RequestBody {
        mode: BodyMode::Binary,
        ..Default::default()
    };
    assert!(!curl(&NexusRequest { body, ..post() }).contains("--data-binary"));
}

fn graphql_body(query: &str, variables: Option<&str>) -> RequestBody {
    RequestBody {
        mode: BodyMode::Graphql,
        graphql: Some(GraphqlBody {
            query: query.into(),
            variables: variables.map(str::to_string),
        }),
        ..Default::default()
    }
}

#[test]
fn graphql_body_as_json() {
    let r = curl(&NexusRequest {
        body: graphql_body("{ users { id name } }", None),
        ..post()
    });
    assert!(r.contains("-H 'Content-Type: application/json'"));
    assert!(r.contains("--data-raw "));
    assert!(r.contains("users"));
}

#[test]
fn graphql_includes_variables() {
    let r = curl(&NexusRequest {
        body: graphql_body(
            "query($id: ID!) { user(id: $id) { name } }",
            Some(r#"{"id":"123"}"#),
        ),
        ..post()
    });
    assert!(r.contains("\"variables\""));
    assert!(r.contains("\"id\""));
}

#[test]
fn graphql_empty_query_sends_nothing() {
    assert!(!curl(&NexusRequest {
        body: graphql_body("", None),
        ..post()
    })
    .contains("--data-raw"));
}

#[test]
fn graphql_invalid_variables_is_an_error() {
    assert!(generate_curl(&NexusRequest {
        body: graphql_body("{ a }", Some("{nope")),
        ..post()
    })
    .is_err());
}

// ---- settings ----

#[test]
fn follow_redirects_default_adds_l() {
    assert!(curl(&request()).contains("-L"));
}

#[test]
fn follow_redirects_false_omits_l() {
    let settings = RequestSettings {
        follow_redirects: Some(false),
        ..Default::default()
    };
    assert!(!curl(&NexusRequest {
        settings,
        ..request()
    })
    .contains("-L"));
}

#[test]
fn reject_unauthorized_false_adds_k() {
    let settings = RequestSettings {
        reject_unauthorized: Some(false),
        ..Default::default()
    };
    assert!(curl(&NexusRequest {
        settings,
        ..request()
    })
    .contains("-k"));
    assert!(!curl(&request()).contains("-k"));
}

#[test]
fn timeout_converted_and_rounded_up() {
    let s = |t| RequestSettings {
        timeout: Some(t),
        ..Default::default()
    };
    assert!(curl(&NexusRequest {
        settings: s(5000),
        ..request()
    })
    .contains("--max-time 5"));
    assert!(curl(&NexusRequest {
        settings: s(1500),
        ..request()
    })
    .contains("--max-time 2"));
    assert!(!curl(&NexusRequest {
        settings: s(0),
        ..request()
    })
    .contains("--max-time"));
}

#[test]
fn max_redirects() {
    let settings = RequestSettings {
        max_redirects: Some(3),
        ..Default::default()
    };
    assert!(curl(&NexusRequest {
        settings,
        ..request()
    })
    .contains("--max-redirs 3"));
}

#[test]
fn parts_are_joined_with_line_continuations() {
    assert!(curl(&post()).contains(" \\\n  "));
}

// ---- shell safety ----

/// Runs the generated command in a real POSIX shell with `curl` replaced by a function that prints
/// its arguments, so we compare exactly what curl would receive.
fn shell_args(command: &str) -> Vec<String> {
    let script = format!("curl() {{ printf '%s\\0' \"$@\"; }}\n{command}");
    let out = Command::new("sh")
        .arg("-c")
        .arg(script)
        .env_clear()
        .output()
        .expect("run sh");
    assert!(
        out.status.success(),
        "sh failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let mut args: Vec<String> = text.split('\0').map(str::to_string).collect();
    args.pop();
    args
}

const PAYLOAD: &str = "x'; echo INJECTED; '$(echo INJECTED)`echo INJECTED`";

#[test]
fn shell_quote_round_trips_through_sh() {
    assert_eq!(
        shell_args(&format!("curl {}", shell_quote(PAYLOAD))),
        [PAYLOAD]
    );
}

#[test]
fn payloads_cannot_break_out_of_their_argument() {
    let cases: Vec<(&str, NexusRequest)> = vec![
        (
            "url",
            NexusRequest {
                url: format!("https://h.test/{PAYLOAD}"),
                ..request()
            },
        ),
        (
            "header",
            NexusRequest {
                headers: vec![kv("X-A", PAYLOAD, true)],
                ..request()
            },
        ),
        (
            "bearer token",
            NexusRequest {
                auth: auth(AuthType::Bearer, json!({"token": PAYLOAD})),
                ..request()
            },
        ),
        (
            "basic auth",
            NexusRequest {
                auth: auth(
                    AuthType::Basic,
                    json!({"username": PAYLOAD, "password": PAYLOAD}),
                ),
                ..request()
            },
        ),
        (
            "api key",
            NexusRequest {
                auth: auth(
                    AuthType::ApiKey,
                    json!({"addTo": "header", "key": "K", "value": PAYLOAD}),
                ),
                ..request()
            },
        ),
        (
            "raw body",
            NexusRequest {
                body: raw_body(BodyMode::Text, PAYLOAD),
                ..post()
            },
        ),
        (
            "urlencoded",
            NexusRequest {
                body: RequestBody {
                    mode: BodyMode::XWwwFormUrlencoded,
                    urlencoded: Some(vec![kv(PAYLOAD, PAYLOAD, true)]),
                    ..Default::default()
                },
                ..post()
            },
        ),
        (
            "form-data text",
            NexusRequest {
                body: form_body(vec![form_entry(PAYLOAD, PAYLOAD, FormDataType::Text, true)]),
                ..post()
            },
        ),
        (
            "form-data file",
            NexusRequest {
                body: form_body(vec![form_entry("f", PAYLOAD, FormDataType::File, true)]),
                ..post()
            },
        ),
        (
            "binary path",
            NexusRequest {
                body: RequestBody {
                    mode: BodyMode::Binary,
                    binary: Some(BinaryBody {
                        path: PAYLOAD.into(),
                        content_type: None,
                    }),
                    ..Default::default()
                },
                ..post()
            },
        ),
        (
            "graphql",
            NexusRequest {
                body: graphql_body(PAYLOAD, None),
                ..post()
            },
        ),
    ];
    for (label, req) in cases {
        let args = shell_args(&curl(&req));
        assert!(!args.iter().any(|a| a == "INJECTED"), "{label}: {args:?}");
        assert!(
            args.iter().any(|a| a.contains("x'; echo INJECTED;")),
            "{label}: payload not intact in {args:?}"
        );
    }
}

#[test]
fn method_is_always_plain_letters() {
    for m in [
        HttpMethod::Get,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
        HttpMethod::Delete,
        HttpMethod::Head,
        HttpMethod::Options,
    ] {
        assert!(m.as_str().bytes().all(|b| b.is_ascii_uppercase()));
    }
    // Anything else is rejected when the request is deserialized.
    assert!(serde_json::from_value::<HttpMethod>(json!(PAYLOAD)).is_err());
}

#[test]
fn numeric_settings_reject_injected_strings() {
    let settings =
        serde_json::from_value::<RequestSettings>(json!({"timeout": "1; echo INJECTED"}));
    assert!(settings.is_err());
    let settings =
        serde_json::from_value::<RequestSettings>(json!({"maxRedirects": "1; echo INJECTED"}));
    assert!(settings.is_err());
}

#[test]
fn raw_bodies_use_data_raw() {
    let r = curl(&NexusRequest {
        body: raw_body(BodyMode::Text, "@/etc/passwd"),
        ..post()
    });
    assert!(r.contains("--data-raw '@/etc/passwd'"));
    assert!(!r.split_whitespace().any(|w| w == "-d"));
}

#[test]
fn form_text_values_use_form_string() {
    let r = curl(&NexusRequest {
        body: form_body(vec![form_entry(
            "a",
            "@/etc/passwd",
            FormDataType::Text,
            true,
        )]),
        ..post()
    });
    assert!(r.contains("--form-string 'a=@/etc/passwd'"));
    assert!(!r.contains("-F "));
}

#[test]
fn urlencoded_names_are_encoded() {
    let r = curl(&NexusRequest {
        body: RequestBody {
            mode: BodyMode::XWwwFormUrlencoded,
            urlencoded: Some(vec![kv("a@/etc/passwd", "v", true)]),
            ..Default::default()
        },
        ..post()
    });
    assert!(r.contains("--data-urlencode 'a%40%2Fetc%2Fpasswd=v'"));
}

// ---- guard rules (ported from architecture.test.ts) ----

/// Source of the generator without comments, `shell_quote` and the test module.
fn generator_source() -> String {
    let src = include_str!("mod.rs");
    let src = src.split("#[cfg(test)]").next().unwrap();
    let no_comments = regex::Regex::new(r"(?m)//.*$")
        .unwrap()
        .replace_all(src, "")
        .into_owned();
    regex::Regex::new(r"(?s)pub fn shell_quote.*?\n\}")
        .unwrap()
        .replace_all(&no_comments, "")
        .into_owned()
}

#[test]
fn guard_codegen_never_interpolates_inside_hand_written_single_quotes() {
    let src = generator_source();
    assert!(
        !regex::Regex::new(r"'\{").unwrap().is_match(&src),
        "quote values with shell_quote() instead of '{{...}}'"
    );
}

#[test]
fn guard_codegen_uses_flags_that_never_read_local_files_from_a_value() {
    let src = generator_source();
    assert!(
        !src.contains("\"-d "),
        "use --data-raw: -d @file reads local files"
    );
    for flag in regex::Regex::new(r#""-F [^\n]*"#).unwrap().find_iter(&src) {
        assert!(
            flag.as_str().contains("=@"),
            "-F is only for real file uploads: {}",
            flag.as_str()
        );
    }
}
