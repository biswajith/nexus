use super::*;
use chrono::Utc;

fn make_token(expires_at: i64) -> StoredToken {
    StoredToken {
        access_token: "access-123".into(),
        token_type: "Bearer".into(),
        expires_in: None,
        expires_at,
        refresh_token: Some("refresh-456".into()),
        scope: Some("read write".into()),
    }
}

fn in_ms(ms: i64) -> i64 {
    Utc::now().timestamp_millis() + ms
}

#[test]
fn returns_none_for_a_key_that_was_never_saved() {
    assert_eq!(TokenStore::new(None).get_token("missing"), None);
}

#[test]
fn saves_and_retrieves_a_token() {
    let mut store = TokenStore::new(None);
    let token = make_token(in_ms(3_600_000));
    store.save_token("key1", token.clone()).unwrap();
    assert_eq!(store.get_token("key1"), Some(token));
}

#[test]
fn overwrites_a_token_with_the_same_key() {
    let mut store = TokenStore::new(None);
    let mut first = make_token(0);
    first.access_token = "first".into();
    let mut second = make_token(0);
    second.access_token = "second".into();
    store.save_token("k", first).unwrap();
    store.save_token("k", second).unwrap();
    assert_eq!(store.get_token("k").unwrap().access_token, "second");
}

#[test]
fn removes_a_token() {
    let mut store = TokenStore::new(None);
    store.save_token("k", make_token(0)).unwrap();
    store.remove_token("k").unwrap();
    assert_eq!(store.get_token("k"), None);
}

#[test]
fn removing_a_missing_key_does_not_fail() {
    assert!(TokenStore::new(None).remove_token("nope").is_ok());
}

#[test]
fn clears_all_tokens() {
    let mut store = TokenStore::new(None);
    store.save_token("a", make_token(0)).unwrap();
    store.save_token("b", make_token(0)).unwrap();
    store.clear().unwrap();
    assert_eq!(store.get_token("a"), None);
    assert_eq!(store.get_token("b"), None);
}

#[test]
fn is_expired_cases() {
    let store = TokenStore::new(None);
    assert!(
        !store.is_expired(&make_token(in_ms(3_600_000))),
        "well in the future"
    );
    assert!(
        store.is_expired(&make_token(in_ms(-1000))),
        "already expired"
    );
    assert!(
        store.is_expired(&make_token(in_ms(20_000))),
        "within the 30s buffer"
    );
    assert!(
        !store.is_expired(&make_token(in_ms(60_000))),
        "30s+ before expiry"
    );
}

fn tmp_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tokens.json");
    (dir, path)
}

#[test]
fn persists_tokens_to_disk_on_save() {
    let (_dir, path) = tmp_path();
    let mut store = TokenStore::new(Some(path.clone()));
    let token = make_token(in_ms(3_600_000));
    store.save_token("key1", token.clone()).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(data["key1"], serde_json::to_value(&token).unwrap());
    assert_eq!(data["key1"]["accessToken"], "access-123");
}

#[test]
fn loads_tokens_from_an_existing_file() {
    let (_dir, path) = tmp_path();
    let token = make_token(in_ms(3_600_000));
    std::fs::write(&path, serde_json::json!({ "mykey": token }).to_string()).unwrap();
    let mut store = TokenStore::new(Some(path));
    store.load();
    assert_eq!(store.get_token("mykey"), Some(token));
}

#[test]
fn starts_empty_when_the_file_does_not_exist() {
    let (_dir, path) = tmp_path();
    let mut store = TokenStore::new(Some(path));
    store.load();
    assert_eq!(store.get_token("any"), None);
}

#[test]
fn starts_empty_when_the_file_contains_invalid_json() {
    let (_dir, path) = tmp_path();
    std::fs::write(&path, "not valid json!!!").unwrap();
    let mut store = TokenStore::new(Some(path));
    store.load();
    assert_eq!(store.get_token("any"), None);
}

#[test]
fn updates_the_file_when_a_token_is_removed() {
    let (_dir, path) = tmp_path();
    let mut store = TokenStore::new(Some(path.clone()));
    store.save_token("a", make_token(0)).unwrap();
    store.save_token("b", make_token(0)).unwrap();
    store.remove_token("a").unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(data.get("a").is_none());
    assert!(data.get("b").is_some());
}

#[test]
fn writes_an_empty_json_object_on_clear() {
    let (_dir, path) = tmp_path();
    let mut store = TokenStore::new(Some(path.clone()));
    store.save_token("a", make_token(0)).unwrap();
    store.clear().unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(data, serde_json::json!({}));
}

#[test]
fn load_is_a_no_op_without_a_file_path() {
    let mut store = TokenStore::new(None);
    store.load();
}
