use super::*;
use crate::types::HeaderValue;
use indexmap::IndexMap;
use tempfile::TempDir;

const EX: &str = "https://example.com/";

fn jar() -> NexusCookieJar {
    NexusCookieJar::new(None)
}

fn tmp_path() -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("cookies.json");
    (dir, p)
}

#[test]
fn stores_and_retrieves_a_cookie_for_a_url() {
    let mut j = jar();
    j.set_cookie("session=abc123; Path=/", EX).unwrap();
    let cookies = j.get_cookies(EX).unwrap();
    assert_eq!(cookies.len(), 1);
    assert_eq!(
        (cookies[0].key.as_str(), cookies[0].value.as_str()),
        ("session", "abc123")
    );
}

#[test]
fn stores_multiple_cookies_for_the_same_domain() {
    let mut j = jar();
    j.set_cookie("a=1; Path=/", EX).unwrap();
    j.set_cookie("b=2; Path=/", EX).unwrap();
    assert_eq!(j.get_cookies(EX).unwrap().len(), 2);
}

#[test]
fn does_not_return_cookies_for_a_different_domain() {
    let mut j = jar();
    j.set_cookie("a=1; Path=/", EX).unwrap();
    assert_eq!(j.get_cookies("https://other.com/").unwrap().len(), 0);
}

#[test]
fn cookie_string_is_semicolon_separated_or_empty() {
    let mut j = jar();
    assert_eq!(j.get_cookie_string(EX).unwrap(), "");
    j.set_cookie("a=1; Path=/", EX).unwrap();
    j.set_cookie("b=2; Path=/", EX).unwrap();
    let s = j.get_cookie_string(EX).unwrap();
    assert!(s.contains("a=1") && s.contains("b=2") && s.contains("; "));
}

#[test]
fn get_all_cookies_spans_domains_and_is_empty_for_a_fresh_jar() {
    let mut j = jar();
    assert!(j.get_all_cookies().is_empty());
    j.set_cookie("a=1; Path=/", EX).unwrap();
    j.set_cookie("b=2; Path=/", "https://other.com/").unwrap();
    assert_eq!(j.get_all_cookies().len(), 2);
}

#[test]
fn removes_a_specific_cookie_by_domain_path_and_key() {
    let mut j = jar();
    j.set_cookie("a=1; Path=/", EX).unwrap();
    j.set_cookie("b=2; Path=/", EX).unwrap();
    j.remove_cookie("example.com", "/", "a").unwrap();
    let cookies = j.get_cookies(EX).unwrap();
    assert_eq!(cookies.len(), 1);
    assert_eq!(cookies[0].key, "b");
}

#[test]
fn remove_cookie_leaves_other_domains_untouched() {
    let mut j = jar();
    j.set_cookie("a=1; Path=/", EX).unwrap();
    j.set_cookie("b=2; Path=/", "https://other.com/").unwrap();
    j.remove_cookie("example.com", "/", "a").unwrap();
    let rest = j.get_all_cookies();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].key, "b");
}

#[test]
fn remove_all_cookies_clears_the_jar() {
    let mut j = jar();
    j.set_cookie("a=1; Path=/", EX).unwrap();
    j.set_cookie("b=2; Path=/", "https://other.com/").unwrap();
    j.remove_all_cookies().unwrap();
    assert!(j.get_all_cookies().is_empty());
}

#[test]
fn apply_to_request_sets_cookie_header_only_when_cookies_match() {
    let mut j = jar();
    j.set_cookie("token=xyz; Path=/", "https://api.example.com/")
        .unwrap();
    let mut headers: IndexMap<String, String> = IndexMap::new();
    headers.insert("Accept".into(), "application/json".into());
    j.apply_to_request("https://api.example.com/data", &mut headers)
        .unwrap();
    assert!(headers["cookie"].contains("token=xyz"));
    assert_eq!(headers["Accept"], "application/json");

    let mut none = IndexMap::new();
    jar()
        .apply_to_request("https://api.example.com/", &mut none)
        .unwrap();
    assert!(!none.contains_key("cookie"));
}

#[test]
fn process_response_handles_string_array_and_absent_set_cookie() {
    let mut j = jar();
    let mut h: IndexMap<String, HeaderValue> = IndexMap::new();
    h.insert(
        "set-cookie".into(),
        HeaderValue::One("session=abc; Path=/".into()),
    );
    j.process_response(EX, &h).unwrap();
    assert_eq!(j.get_cookies(EX).unwrap()[0].key, "session");

    let mut j = jar();
    h.insert(
        "set-cookie".into(),
        HeaderValue::Many(vec!["a=1; Path=/".into(), "b=2; Path=/".into()]),
    );
    j.process_response(EX, &h).unwrap();
    assert_eq!(j.get_cookies(EX).unwrap().len(), 2);

    let mut j = jar();
    let mut h = IndexMap::new();
    h.insert(
        "content-type".to_string(),
        HeaderValue::One("text/html".into()),
    );
    j.process_response(EX, &h).unwrap();
    assert!(j.get_all_cookies().is_empty());
}

#[test]
fn persists_cookies_to_disk_on_set_cookie() {
    let (_d, p) = tmp_path();
    let mut j = NexusCookieJar::new(Some(p.clone()));
    j.set_cookie("a=1; Path=/", EX).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    assert!(!parsed.as_array().unwrap().is_empty());
}

#[test]
fn loads_cookies_from_disk() {
    let (_d, p) = tmp_path();
    NexusCookieJar::new(Some(p.clone()))
        .set_cookie("token=xyz; Path=/", EX)
        .unwrap();
    let mut j2 = NexusCookieJar::new(Some(p));
    j2.load();
    let cookies = j2.get_cookies(EX).unwrap();
    assert_eq!(cookies.len(), 1);
    assert_eq!(cookies[0].value, "xyz");
}

#[test]
fn starts_empty_when_persist_file_is_missing_or_invalid() {
    let (_d, p) = tmp_path();
    let mut j = NexusCookieJar::new(Some(p.clone()));
    j.load();
    assert!(j.get_all_cookies().is_empty());
    std::fs::write(&p, "not json!").unwrap();
    j.load();
    assert!(j.get_all_cookies().is_empty());
}

#[test]
fn persist_file_follows_remove_all_and_remove_cookie() {
    let (_d, p) = tmp_path();
    let mut j = NexusCookieJar::new(Some(p.clone()));
    j.set_cookie("a=1; Path=/", EX).unwrap();
    j.set_cookie("b=2; Path=/", EX).unwrap();
    j.remove_cookie("example.com", "/", "a").unwrap();
    let mut j2 = NexusCookieJar::new(Some(p.clone()));
    j2.load();
    let c = j2.get_cookies(EX).unwrap();
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].key, "b");

    j.remove_all_cookies().unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    assert!(parsed.as_array().unwrap().is_empty());
}

#[test]
fn load_is_a_no_op_without_persist_path() {
    let mut j = jar();
    j.set_cookie("a=1; Path=/", EX).unwrap();
    j.load();
    assert_eq!(j.get_all_cookies().len(), 1);
}

#[cfg(unix)]
#[test]
fn persisted_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let (_d, p) = tmp_path();
    NexusCookieJar::new(Some(p.clone()))
        .set_cookie("a=1; Path=/", EX)
        .unwrap();
    assert_eq!(
        std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
