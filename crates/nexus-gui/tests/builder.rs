//! The request builder driven through accessible role + label queries, with real sends to a
//! local mock server whose received requests are asserted.

mod common;

use common::*;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use nexus_core::types::{AuthType, BodyMode, FormDataType};

fn fresh() -> (tempfile::TempDir, H, Server) {
    let dir = tempfile::tempdir().unwrap();
    let h = harness(dir.path());
    (dir, h, Server::start())
}

fn set_url(h: &mut H, url: &str) {
    type_into(h, "Request URL", url);
}

fn body_tab(h: &mut H, mode: &str) {
    click(h, Role::Tab, "Body");
    click(h, Role::RadioButton, mode);
}

fn auth_tab(h: &mut H, ty: &str) {
    click(h, Role::Tab, "Auth");
    pick(h, "Auth type", ty);
}

// ---- URL <-> params ----

#[test]
fn params_and_url_stay_in_sync_both_ways() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    set_url(&mut h, "http://x.test/p?a=1&b=2");
    click(&mut h, Role::Tab, "Params (2)");
    assert_eq!(value_of(&h, "Parameter 1 key"), "a");
    assert_eq!(value_of(&h, "Parameter 2 value"), "2");
    // a trailing blank row is always there to type into
    assert!(has(&h, Role::TextInput, "Parameter 3 key"));

    // typing in the blank row appends to the URL
    type_into(&mut h, "Parameter 3 key", "c d");
    type_into(&mut h, "Parameter 3 value", "3");
    assert_eq!(
        h.state().tabs.active().unwrap().request.url,
        "http://x.test/p?a=1&b=2&c%20d=3"
    );
    assert!(has(&h, Role::TextInput, "Parameter 4 key"));

    // disabling a row removes it from the URL but keeps the row
    click(&mut h, Role::CheckBox, "Parameter 1 enabled");
    assert_eq!(
        h.state().tabs.active().unwrap().request.url,
        "http://x.test/p?b=2&c%20d=3"
    );
    assert_eq!(value_of(&h, "Parameter 1 key"), "a");

    // removing a row updates the URL
    click(&mut h, Role::Button, "Remove Parameter 2");
    assert_eq!(
        h.state().tabs.active().unwrap().request.url,
        "http://x.test/p?c%20d=3"
    );

    // editing the URL updates the enabled rows and keeps the disabled one
    type_into(&mut h, "Request URL", "&z=9");
    let r = &h.state().tabs.active().unwrap().request;
    assert_eq!(r.url, "http://x.test/p?c%20d=3&z=9");
    let keys: Vec<_> = r
        .params
        .iter()
        .map(|p| (p.key.as_str(), p.enabled))
        .collect();
    assert_eq!(keys, vec![("a", false), ("c d", true), ("z", true)]);
    assert!(h.state().tabs.active().unwrap().dirty);
}

#[test]
fn variables_in_params_are_not_encoded_and_resolve_at_send() {
    let (dir, h, s) = fresh();
    let env = nexus_core::types::NexusEnvironment {
        id: "e1".into(),
        name: "Local".into(),
        variables: vec![nexus_core::types::Variable {
            key: "id".into(),
            value: "42".into(),
            var_type: nexus_core::types::VariableType::String,
            enabled: true,
            description: None,
        }],
    };
    h.state().app.environment_save(&env).unwrap();
    drop(h);
    let mut h = harness(dir.path());
    pick(&mut h, "Select environment", "Local");
    set_url(&mut h, &format!("{}/items", s.url()));
    click(&mut h, Role::Tab, "Params");
    type_into(&mut h, "Parameter 1 key", "id");
    type_into(&mut h, "Parameter 1 value", "{{id}}");
    assert!(h
        .state()
        .tabs
        .active()
        .unwrap()
        .request
        .url
        .ends_with("/items?id={{id}}"));
    send(&mut h);
    assert_eq!(s.last().url.query(), Some("id=42"));
}

// ---- key/value tables ----

#[test]
fn header_rows_add_remove_and_reach_the_server() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    click(&mut h, Role::Tab, "Headers");
    type_into(&mut h, "Header 1 key", "X-One");
    type_into(&mut h, "Header 1 value", "1");
    // the table grew a blank row by itself
    type_into(&mut h, "Header 2 key", "X-Two");
    type_into(&mut h, "Header 2 value", "2");
    click(&mut h, Role::Button, "Add Header");
    type_into(&mut h, "Header 3 key", "X-Off");
    type_into(&mut h, "Header 3 value", "off");
    click(&mut h, Role::CheckBox, "Header 3 enabled");
    assert!(has(&h, Role::Tab, "Headers (2)"));
    click(&mut h, Role::Button, "Remove Header 1");
    assert_eq!(value_of(&h, "Header 1 key"), "X-Two");
    send(&mut h);
    let r = s.last();
    assert_eq!(header(&r, "x-two").as_deref(), Some("2"));
    assert!(header(&r, "x-one").is_none());
    assert!(header(&r, "x-off").is_none());
}

// ---- bodies ----

#[test]
fn raw_bodies_json_xml_text_html() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    click(&mut h, Role::Button, "Send"); // GET with no body works
    wait_response(&mut h);
    assert!(s.last().body.is_empty());

    pick(&mut h, "HTTP method", "POST");
    for (mode, text, ct) in [
        ("JSON", "{\"a\":1}", "application/json"),
        ("XML", "<a>1</a>", "application/xml"),
        ("Text", "plain words", "text/plain"),
        ("HTML", "<p>hi</p>", "text/html"),
    ] {
        body_tab(&mut h, mode);
        // the editor keeps the text across modes; replace it
        h.state_mut().tabs.active_mut().unwrap().request.body.raw = Some(String::new());
        settle(&mut h);
        type_into_editor(&mut h, "Request body", text);
        h.state_mut().tabs.active_mut().unwrap().response = None;
        send(&mut h);
        let r = s.last();
        assert_eq!(r.method.as_str(), "POST", "{mode}");
        assert_eq!(String::from_utf8(r.body.clone()).unwrap(), text, "{mode}");
        assert_eq!(header(&r, "content-type").as_deref(), Some(ct), "{mode}");
    }
}

#[test]
fn urlencoded_body() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    pick(&mut h, "HTTP method", "POST");
    body_tab(&mut h, "URL Encoded");
    type_into(&mut h, "Field 1 key", "name");
    type_into(&mut h, "Field 1 value", "a b&c");
    type_into(&mut h, "Field 2 key", "x");
    type_into(&mut h, "Field 2 value", "{{nope}}");
    send(&mut h);
    let r = s.last();
    assert_eq!(
        String::from_utf8(r.body.clone()).unwrap(),
        "name=a%20b%26c&x={{nope}}"
    );
    assert_eq!(
        header(&r, "content-type").as_deref(),
        Some("application/x-www-form-urlencoded")
    );
}

#[test]
fn form_data_with_text_and_file_parts() {
    let (dir, mut h, s) = fresh();
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "FILE-CONTENT").unwrap();
    set_url(&mut h, &s.url());
    pick(&mut h, "HTTP method", "POST");
    body_tab(&mut h, "Form Data");
    type_into(&mut h, "Form field 1 key", "title");
    type_into(&mut h, "Form field 1 value", "hello");
    type_into(&mut h, "Form field 2 key", "upload");
    pick(&mut h, "Form field 2 type", "File");
    assert!(has(&h, Role::Button, "Browse file for Form field 2"));
    // what the native dialog would deliver
    let tab_id = h.state().tabs.active_id().unwrap().to_string();
    h.state_mut()
        .apply_picked_file(&tab_id, nexus_gui::runtime::PickTarget::FormRow(1), &file);
    settle(&mut h);
    assert_eq!(value_of(&h, "Form field 2 file"), file.to_string_lossy());
    send(&mut h);
    let r = s.last();
    let ct = header(&r, "content-type").unwrap();
    assert!(ct.starts_with("multipart/form-data; boundary="), "{ct}");
    let body = String::from_utf8(r.body.clone()).unwrap();
    assert!(body.contains("name=\"title\"\r\n\r\nhello\r\n"), "{body}");
    assert!(
        body.contains("name=\"upload\"; filename=\"note.txt\"\r\nContent-Type: text/plain\r\n\r\nFILE-CONTENT\r\n"),
        "{body}"
    );
    let rows = h
        .state()
        .tabs
        .active()
        .unwrap()
        .request
        .body
        .form_data
        .clone()
        .unwrap();
    assert_eq!(rows[1].entry_type, FormDataType::File);
}

#[test]
fn binary_body_sends_the_file_bytes() {
    let (dir, mut h, s) = fresh();
    let file = dir.path().join("blob.bin");
    std::fs::write(&file, [0u8, 159, 146, 150, 255]).unwrap();
    set_url(&mut h, &s.url());
    pick(&mut h, "HTTP method", "PUT");
    body_tab(&mut h, "Binary");
    assert!(has(&h, Role::Button, "Choose binary file"));
    let tab_id = h.state().tabs.active_id().unwrap().to_string();
    h.state_mut()
        .apply_picked_file(&tab_id, nexus_gui::runtime::PickTarget::Binary, &file);
    settle(&mut h);
    assert_eq!(value_of(&h, "Binary file path"), file.to_string_lossy());
    type_into(&mut h, "Binary content type", "application/x-thing");
    send(&mut h);
    let r = s.last();
    assert_eq!(r.method.as_str(), "PUT");
    assert_eq!(r.body, vec![0u8, 159, 146, 150, 255]);
    assert_eq!(
        header(&r, "content-type").as_deref(),
        Some("application/x-thing")
    );
}

#[test]
fn graphql_body_is_a_json_envelope() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    pick(&mut h, "HTTP method", "POST");
    body_tab(&mut h, "GraphQL");
    type_into_editor(
        &mut h,
        "GraphQL query",
        "query Q($id: ID!) { pet(id: $id) { name } }",
    );
    type_into_editor(&mut h, "GraphQL variables", "{\"id\": 7}");
    send(&mut h);
    let r = s.last();
    let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(v["query"], "query Q($id: ID!) { pet(id: $id) { name } }");
    assert_eq!(v["variables"]["id"], 7);
    assert_eq!(
        header(&r, "content-type").as_deref(),
        Some("application/json")
    );
    assert_eq!(
        h.state().tabs.active().unwrap().request.body.mode,
        BodyMode::Graphql
    );
}

// ---- auth ----

#[test]
fn bearer_and_prefix() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    auth_tab(&mut h, "Bearer Token");
    type_into(&mut h, "Token", "tok123");
    send(&mut h);
    assert_eq!(
        header(&s.last(), "authorization").as_deref(),
        Some("Bearer tok123")
    );
    type_into(&mut h, "Prefix", "Token");
    assert_eq!(
        h.state().tabs.active().unwrap().request.auth.extra["prefix"],
        "Token"
    );
}

#[test]
fn basic_auth_and_secret_masking() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    auth_tab(&mut h, "Basic Auth");
    type_into(&mut h, "Username", "me");
    type_into_secret(&mut h, "Password", "pw");
    assert!(has(&h, Role::Button, "Show Password"));
    click(&mut h, Role::Button, "Show Password");
    assert!(has(&h, Role::Button, "Hide Password"));
    send(&mut h);
    // base64("me:pw")
    assert_eq!(
        header(&s.last(), "authorization").as_deref(),
        Some("Basic bWU6cHc=")
    );
}

#[test]
fn api_key_in_header_and_in_query() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    auth_tab(&mut h, "API Key");
    type_into(&mut h, "Key", "X-Api");
    type_into_secret(&mut h, "Value", "secret");
    send(&mut h);
    assert_eq!(header(&s.last(), "x-api").as_deref(), Some("secret"));

    pick(&mut h, "Add to", "Query Params");
    h.state_mut().tabs.active_mut().unwrap().response = None;
    send(&mut h);
    let r = s.last();
    assert!(header(&r, "x-api").is_none());
    assert_eq!(r.url.query(), Some("X-Api=secret"));
}

#[test]
fn oauth2_token_in_header_and_query() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    auth_tab(&mut h, "OAuth 2.0");
    type_into_secret(&mut h, "Access Token", "at-1");
    assert!(has(&h, Role::PasswordInput, "Client Secret"));
    assert!(has(&h, Role::TextInput, "Redirect URI"));
    send(&mut h);
    assert_eq!(
        header(&s.last(), "authorization").as_deref(),
        Some("Bearer at-1")
    );
    pick(&mut h, "Add to", "Query Params");
    h.state_mut().tabs.active_mut().unwrap().response = None;
    send(&mut h);
    assert_eq!(s.last().url.query(), Some("access_token=at-1"));
}

#[test]
fn digest_and_aws_sig_v4_sign_the_request() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    auth_tab(&mut h, "Digest Auth");
    type_into(&mut h, "Username", "u");
    type_into_secret(&mut h, "Password", "p");
    type_into(&mut h, "Realm", "r");
    type_into(&mut h, "Nonce", "n");
    send(&mut h);
    let a = header(&s.last(), "authorization").unwrap();
    assert!(a.starts_with("Digest "), "{a}");
    assert!(
        a.contains("username=\"u\"") && a.contains("realm=\"r\""),
        "{a}"
    );

    pick(&mut h, "Auth type", "AWS Signature v4");
    type_into(&mut h, "Access Key", "AKIDEXAMPLE");
    type_into_secret(&mut h, "Secret Key", "wJalr");
    h.state_mut().tabs.active_mut().unwrap().response = None;
    send(&mut h);
    let r = s.last();
    let a = header(&r, "authorization").unwrap();
    assert!(
        a.starts_with("AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/"),
        "{a}"
    );
    assert!(header(&r, "x-amz-date").is_some());
}

#[test]
fn field_names_match_the_keys_core_reads() {
    let (_d, mut h, _s) = fresh();
    for (ty, fields) in [
        ("Bearer Token", vec![("Token", "token")]),
        (
            "Basic Auth",
            vec![("Username", "username"), ("Password", "password")],
        ),
        ("API Key", vec![("Key", "key"), ("Value", "value")]),
        (
            "OAuth 2.0",
            vec![
                ("Access Token", "accessToken"),
                ("Token Type", "tokenType"),
                ("Auth URL", "authUrl"),
                ("Token URL", "tokenUrl"),
                ("Client ID", "clientId"),
                ("Client Secret", "clientSecret"),
                ("Scope", "scope"),
                ("Redirect URI", "redirectUri"),
            ],
        ),
        (
            "Digest Auth",
            vec![
                ("Realm", "realm"),
                ("Nonce", "nonce"),
                ("Algorithm", "algorithm"),
                ("QOP", "qop"),
                ("Opaque", "opaque"),
            ],
        ),
        (
            "AWS Signature v4",
            vec![
                ("Access Key", "accessKey"),
                ("Secret Key", "secretKey"),
                ("Region", "region"),
                ("Service", "service"),
                ("Session Token", "sessionToken"),
            ],
        ),
    ] {
        auth_tab(&mut h, ty);
        for (label, key) in fields {
            if has(&h, Role::TextInput, label) {
                type_into(&mut h, label, "x");
            } else {
                type_into_secret(&mut h, label, "x");
            }
            let extra = &h.state().tabs.active().unwrap().request.auth.extra;
            assert!(
                extra.contains_key(key),
                "{ty}: {label} should store `{key}`"
            );
        }
    }
    pick(&mut h, "Auth type", "No Auth");
    assert_eq!(
        h.state().tabs.active().unwrap().request.auth.auth_type,
        AuthType::None
    );
    assert!(has(
        &h,
        Role::Label,
        "This request does not use any authentication."
    ));
    pick(&mut h, "Auth type", "Inherit from parent");
    assert!(has(
        &h,
        Role::Label,
        "This request will use the auth configuration from its parent folder or collection."
    ));
}

#[test]
fn inherit_auth_and_scripts_come_from_the_collection_chain() {
    let dir = tempfile::tempdir().unwrap();
    let s = Server::start();
    {
        use nexus_core::types::*;
        let app = nexus_core::service::App::new(dir.path()).unwrap();
        let mut c = create_default_collection("Chain");
        c.auth = Some({
            let mut a = AuthConfig::new(AuthType::Bearer);
            a.extra.insert("token".into(), "from-collection".into());
            a
        });
        c.pre_request_script = Some("nx.request.headers['X-Coll'] = 'c';".into());
        let mut r = create_default_request();
        r.name = "Inner".into();
        r.url = s.url();
        r.pre_request_script = Some("nx.request.headers['X-Req'] = 'r';".into());
        c.items = vec![CollectionItem::Folder(NexusFolder {
            id: "f1".into(),
            name: "F".into(),
            description: None,
            items: vec![CollectionItem::Request(r)],
            auth: None,
            pre_request_script: Some("nx.request.headers['X-Folder'] = 'f';".into()),
            post_response_script: None,
            item_order: None,
        })];
        app.collection_create(&c).unwrap();
    }
    let mut h = harness(dir.path());
    let (d, req) = {
        let st = h.state();
        let m = &st.collections.metas[0];
        let dir_name = nexus_gui::state::collections::dir_name(m);
        let c = st.app.collection_load(&dir_name).unwrap();
        let req = match &c.items[0] {
            nexus_core::types::CollectionItem::Folder(f) => match &f.items[0] {
                nexus_core::types::CollectionItem::Request(r) => r.clone(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        (dir_name, req)
    };
    let app = h.state().app.clone();
    h.state_mut().collections.load(&app, &d);
    h.state_mut().open_collection_request(&d, &req);
    settle(&mut h);
    send(&mut h);
    let r = s.last();
    assert_eq!(
        header(&r, "authorization").as_deref(),
        Some("Bearer from-collection")
    );
    assert_eq!(header(&r, "x-coll").as_deref(), Some("c"));
    assert_eq!(header(&r, "x-folder").as_deref(), Some("f"));
    assert_eq!(header(&r, "x-req").as_deref(), Some("r"));
}

// ---- scripts ----

#[test]
fn pre_request_script_mutation_reaches_the_server() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    click(&mut h, Role::Tab, "Scripts");
    type_into_editor(
        &mut h,
        "Pre-request script",
        "nx.request.headers['X-From-Script'] = 'yes';",
    );
    send(&mut h);
    assert_eq!(header(&s.last(), "x-from-script").as_deref(), Some("yes"));

    // snippets are appended to the script
    click(&mut h, Role::Button, "Insert snippet: Log response");
    let script = h
        .state()
        .tabs
        .active()
        .unwrap()
        .request
        .pre_request_script
        .clone()
        .unwrap();
    assert!(script.ends_with("\nconsole.log(nx.response.json());"));

    // post-response tab edits the other script
    click(&mut h, Role::Tab, "Post-response");
    type_into_editor(&mut h, "Post-response script", "console.log('post');");
    assert_eq!(
        h.state()
            .tabs
            .active()
            .unwrap()
            .request
            .post_response_script
            .as_deref(),
        Some("console.log('post');")
    );
}

// ---- send / cancel / curl ----

#[test]
fn enter_in_the_url_bar_sends() {
    let (_d, mut h, s) = fresh();
    set_url(&mut h, &s.url());
    h.key_press(eframe::egui::Key::Enter);
    wait_response(&mut h);
    assert_eq!(s.count(), 1);
}

#[test]
fn cancel_stops_a_request_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let s = Server::with_delay(std::time::Duration::from_secs(30));
    set_url(&mut h, &s.url());
    click(&mut h, Role::Button, "Send");
    assert!(h.state().tabs.active().unwrap().loading);
    assert!(!has(&h, Role::Button, "Send"));
    click(&mut h, Role::Button, "Cancel");
    assert!(!h.state().tabs.active().unwrap().loading);
    assert!(has(&h, Role::Button, "Send"));
}

#[test]
fn curl_popup_shows_the_resolved_command() {
    let (_d, mut h, _s) = fresh();
    set_url(&mut h, "http://x.test/a?q=1");
    pick(&mut h, "HTTP method", "POST");
    click(&mut h, Role::Tab, "Headers");
    type_into(&mut h, "Header 1 key", "X-A");
    type_into(&mut h, "Header 1 value", "b");
    click(&mut h, Role::Button, "cURL");
    let c = h
        .get_by_role_and_label(Role::MultilineTextInput, "cURL command")
        .value()
        .unwrap_or_default();
    assert!(c.starts_with("curl"), "{c}");
    assert!(
        c.contains("-X POST") && c.contains("'http://x.test/a?q=1'"),
        "{c}"
    );
    assert_eq!(
        c.matches("q=1").count(),
        1,
        "params must not be added twice: {c}"
    );
    assert!(c.contains("'X-A: b'"), "{c}");
    assert!(has(&h, Role::Button, "Copy cURL"));
    click(&mut h, Role::Button, "Close");
    assert!(!has(&h, Role::MultilineTextInput, "cURL command"));
}

#[test]
fn pasting_a_curl_command_fills_the_request() {
    let (_d, mut h, _s) = fresh();
    h.get_by_role_and_label(Role::TextInput, "Request URL")
        .focus();
    settle(&mut h);
    h.event(eframe::egui::Event::Paste(
        "curl -X POST 'http://x.test/p?a=1' -H 'X-K: v' -d 'hello'".into(),
    ));
    settle(&mut h);
    settle(&mut h);
    let r = &h.state().tabs.active().unwrap().request;
    assert_eq!(r.method, nexus_core::types::HttpMethod::Post);
    assert!(r.url.starts_with("http://x.test/p"), "{}", r.url);
    assert!(r.headers.iter().any(|h| h.key == "X-K"));
}

// ---- save + dirty ----

#[test]
fn save_round_trip_and_dirty_dot() {
    let dir = tempfile::tempdir().unwrap();
    nexus_gui::demo::seed(dir.path()).unwrap();
    let mut h = harness(dir.path());
    // open the first demo request through the tree
    click(&mut h, Role::TreeItem, "Petstore API");
    click(&mut h, Role::TreeItem, "Pets");
    click(&mut h, Role::TreeItem, "Create pet");
    let dirty = |h: &H| {
        h.get_by_role_and_label(Role::Tab, "Create pet")
            .accesskit_node()
            .description()
            .as_deref()
            == Some("Unsaved changes")
    };
    assert!(!dirty(&h));
    type_into(&mut h, "Request URL", "/extra");
    assert!(dirty(&h));
    assert!(h.state().tabs.active().unwrap().dirty);

    click(&mut h, Role::Button, "Save");
    assert!(!h.state().tabs.active().unwrap().dirty);
    assert!(!dirty(&h));
    // persisted on disk
    let st = h.state();
    let m = st
        .collections
        .metas
        .iter()
        .find(|m| m.name == "Petstore API")
        .unwrap();
    let c = st
        .app
        .collection_load(&nexus_gui::state::collections::dir_name(m))
        .unwrap();
    fn find<'a>(
        items: &'a [nexus_core::types::CollectionItem],
        name: &str,
    ) -> Option<&'a nexus_core::types::NexusRequest> {
        use nexus_core::types::CollectionItem;
        items.iter().find_map(|i| match i {
            CollectionItem::Request(r) if r.name == name => Some(r),
            CollectionItem::Folder(f) => find(&f.items, name),
            _ => None,
        })
    }
    assert_eq!(
        find(&c.items, "Create pet").unwrap().url,
        "{{baseUrl}}/pets/extra"
    );

    // an unsaved tab is saved into a collection through the Save popup
    click(&mut h, Role::Button, "New request");
    type_into(&mut h, "Request URL", "http://new.test/");
    click(&mut h, Role::Button, "Save");
    click(&mut h, Role::Button, "Save to Petstore API");
    type_into(&mut h, "Request name", "Brand new");
    click(&mut h, Role::Button, "Save request");
    assert!(has(&h, Role::Tab, "Brand new"));
    let t = h.state().tabs.active().unwrap();
    assert!(t.origin.is_some() && !t.dirty);
}

// ---- accessibility inventory ----

#[test]
fn every_builder_control_has_a_role_and_label() {
    let (_d, mut h, _s) = fresh();
    for (role, label) in [
        (Role::ComboBox, "HTTP method"),
        (Role::TextInput, "Request URL"),
        (Role::Button, "Send"),
        (Role::Button, "cURL"),
        (Role::Button, "Save"),
        (Role::Tab, "Params"),
        (Role::Tab, "Headers"),
        (Role::Tab, "Body"),
        (Role::Tab, "Auth"),
        (Role::Tab, "Scripts"),
    ] {
        assert!(has(&h, role, label), "{label}");
    }
    click(&mut h, Role::Tab, "Params");
    for (role, label) in [
        (Role::CheckBox, "Parameter 1 enabled"),
        (Role::TextInput, "Parameter 1 key"),
        (Role::TextInput, "Parameter 1 value"),
        (Role::TextInput, "Parameter 1 description"),
        (Role::Button, "Add Parameter"),
    ] {
        assert!(has(&h, role, label), "{label}");
    }
    click(&mut h, Role::Tab, "Body");
    for m in [
        "None",
        "JSON",
        "XML",
        "Text",
        "HTML",
        "Form Data",
        "URL Encoded",
        "Binary",
        "GraphQL",
    ] {
        assert!(has(&h, Role::RadioButton, m), "{m}");
    }
    click(&mut h, Role::Tab, "Scripts");
    assert!(has(&h, Role::MultilineTextInput, "Pre-request script"));
    assert!(has(&h, Role::Tab, "Pre-request") && has(&h, Role::Tab, "Post-response"));
}
