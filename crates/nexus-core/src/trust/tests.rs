use super::*;
use crate::types::AuthType;

fn bearer() -> AuthConfig {
    AuthConfig::new(AuthType::Bearer)
}

#[test]
fn origin_normalises_scheme_host_and_default_port() {
    assert_eq!(
        origin_of("HTTPS://Api.Example.com:443/x?y=1").as_deref(),
        Some("https://api.example.com")
    );
    assert_eq!(origin_of("not a url"), None);
    assert_ne!(origin_of("http://a.test/"), origin_of("https://a.test/"));
    assert_ne!(
        origin_of("http://a.test:81/"),
        origin_of("http://a.test:82/")
    );
}

#[test]
fn origin_change_drops_auth_with_warning() {
    let auth = bearer();
    let (kept, warn) = bind_auth_to_origin(Some(&auth), "https://a.test/x", "https://evil.test/x");
    assert!(kept.is_none());
    let msg = warn.unwrap().args[0].as_str().unwrap().to_string();
    assert!(msg.contains("Auth not sent"));
    assert!(msg.contains("https://a.test") && msg.contains("https://evil.test"));
}

#[test]
fn same_origin_keeps_auth() {
    let auth = bearer();
    let (kept, warn) =
        bind_auth_to_origin(Some(&auth), "https://a.test/x", "https://a.test:443/y?q=1");
    assert!(kept.is_some() && warn.is_none());
}

#[test]
fn secrets_are_filtered_and_scripts_stripped() {
    let v = |k: &str, t: VariableType| Variable {
        key: k.into(),
        value: "x".into(),
        var_type: t,
        enabled: true,
        description: None,
    };
    let vars = vec![v("a", VariableType::String), v("b", VariableType::Secret)];
    assert_eq!(without_secrets(&vars).len(), 1);
    assert_eq!(secret_count(&vars), 1);

    let mut c = crate::types::create_default_collection("c");
    c.pre_request_script = Some("1".into());
    let mut r = crate::types::create_default_request();
    r.post_response_script = Some("2".into());
    c.items.push(CollectionItem::Request(r));
    assert_eq!(script_count(&c), 2);
    assert_eq!(script_count(&strip_scripts(&c)), 0);
}

fn request_with_body(body: serde_json::Value) -> NexusRequest {
    serde_json::from_value(serde_json::json!({
        "id": "r1", "name": "R", "method": "POST", "url": "https://a.test/",
        "headers": [], "params": [], "auth": {"type": "none"},
        "body": body,
        "settings": {"rejectUnauthorized": false}
    }))
    .unwrap()
}

#[test]
fn untrusted_body_files_must_be_picked_in_app() {
    let form = request_with_body(serde_json::json!({
        "mode": "form-data",
        "formData": [{"key": "f", "value": "/home/u/.ssh/id_rsa", "type": "file", "enabled": true}]
    }));
    let err = check_body_files(&form, &HashSet::new()).unwrap_err();
    assert!(err.contains("/home/u/.ssh/id_rsa") && err.contains("re-pick"));
    let picked: HashSet<String> = ["/home/u/.ssh/id_rsa".to_string()].into();
    assert!(check_body_files(&form, &picked).is_ok());

    let bin = request_with_body(serde_json::json!({
        "mode": "binary", "binary": {"path": "/etc/passwd"}
    }));
    assert!(check_body_files(&bin, &HashSet::new()).is_err());
    let text = request_with_body(serde_json::json!({"mode": "json", "raw": "{}"}));
    assert!(check_body_files(&text, &HashSet::new()).is_ok());
}

#[test]
fn nexus_import_drops_tls_opt_out() {
    let r = request_with_body(serde_json::json!({"mode": "none"}));
    let col = serde_json::json!({
        "id": "c", "name": "C", "variables": [], "version": "1",
        "items": [{"type": "request", "request": serde_json::to_value(&r).unwrap()}]
    });
    let mut parsed: NexusCollection = match serde_json::from_value(col.clone()) {
        Ok(c) => c,
        Err(_) => serde_json::from_value(serde_json::json!({
            "id": "c", "name": "C", "variables": [], "version": "1",
            "items": [serde_json::to_value(&r).unwrap()]
        }))
        .unwrap(),
    };
    assert_eq!(strip_insecure_tls(&mut parsed), 1);
    assert_eq!(strip_insecure_tls(&mut parsed), 0);
    let CollectionItem::Request(req) = &parsed.items[0] else {
        panic!()
    };
    assert_eq!(req.settings.reject_unauthorized, None);
}
