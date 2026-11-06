use super::*;
use crate::types::{create_default_collection, create_default_environment, create_default_request};

fn app() -> (tempfile::TempDir, App) {
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(&dir.path().join(".nexus")).unwrap();
    (dir, app)
}

#[test]
fn collection_and_request_crud_roundtrip() {
    let (_d, app) = app();
    let dir = app
        .collection_create(&create_default_collection("Pets"))
        .unwrap();
    let list = app.collections_list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Pets");

    let mut req = create_default_request();
    req.name = "List pets".into();
    app.request_add(&dir, &req).unwrap();
    assert_eq!(app.collection_load(&dir).unwrap().items.len(), 1);

    req.url = "https://x.test".into();
    app.request_update(&dir, &req.id, &req).unwrap();
    app.collection_rename(&dir, "Animals").unwrap();
    let c = app.collection_load(&dir).unwrap();
    assert_eq!(c.name, "Animals");

    app.request_delete(&dir, &req.id).unwrap();
    assert!(app.collection_load(&dir).unwrap().items.is_empty());
    app.collection_delete(&dir).unwrap();
    assert!(app.collections_list().unwrap().is_empty());
}

#[test]
fn environments_save_list_delete() {
    let (_d, app) = app();
    let env = create_default_environment();
    app.environment_save(&env).unwrap();
    assert_eq!(app.environments_list().unwrap().len(), 1);
    app.environment_delete(&env.id).unwrap();
    assert!(app.environments_list().unwrap().is_empty());
}

#[test]
fn import_save_creates_collections_and_never_replaces_environments() {
    let (_d, app) = app();
    let env = create_default_environment();
    let lines = app
        .import_save(
            &[create_default_collection("A")],
            std::slice::from_ref(&env),
        )
        .unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(app.collections_list().unwrap().len(), 1);
    app.import_save(&[], &[env]).unwrap();
    assert_eq!(app.environments_list().unwrap().len(), 2);
}

#[test]
fn history_clear_rejects_bad_dates() {
    let (_d, app) = app();
    assert!(app.history_clear(Some("not a date")).is_err());
    app.history_clear(None).unwrap();
    assert!(app
        .history_query(&HistoryFilter::default())
        .unwrap()
        .is_empty());
}

#[test]
fn export_formats_dispatch() {
    let c = create_default_collection("C");
    assert!(App::export_collection(&c, "postman-v2.1")
        .unwrap()
        .contains("schema"));
    assert!(App::export_collection(&c, "nexus")
        .unwrap()
        .contains("\"name\""));
}

#[test]
fn ws_send_without_connection_is_an_error() {
    let (_d, app) = app();
    assert!(app.ws_send("nope", "x").unwrap_err().contains("nope"));
    assert!(app.ws_messages("nope").is_empty());
}

#[test]
fn created_collections_are_trusted_imported_are_not_and_trust_persists() {
    let (_d, app) = app();
    let own = app
        .collection_create(&create_default_collection("Mine"))
        .unwrap();
    let json = r#"{"id":"c1","name":"Shared","variables":[],"items":[],"version":"1"}"#;
    let imported = App::import_data(json, Some(crate::import::ImportFormat::Nexus)).unwrap();
    app.import_save(&imported.collections, &[]).unwrap();
    let dirs: Vec<String> = app
        .collections_list()
        .unwrap()
        .iter()
        .map(|m| {
            std::path::Path::new(&m.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    let shared = dirs.iter().find(|d| **d != own).unwrap().clone();
    assert!(app.collection_is_trusted(&own));
    assert!(!app.collection_is_trusted(&shared));

    app.collection_trust(&shared).unwrap();
    let again = App::new(&_d.path().join(".nexus")).unwrap();
    assert!(again.collection_is_trusted(&shared));

    // Deleting forgets the trust, so a same-named directory arriving later is untrusted.
    again.collection_delete(&shared).unwrap();
    assert!(!again.collection_is_trusted(&shared));
}

#[test]
fn existing_collections_are_grandfathered_as_trusted_on_first_use() {
    let (d, _app) = app();
    let dir = d.path().join(".nexus/collections/legacy");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("collection.json"),
        r#"{"id":"l","name":"Legacy","variables":[]}"#,
    )
    .unwrap();
    let app = App::new(&d.path().join(".nexus")).unwrap();
    assert!(app.collection_is_trusted("legacy"));
    // Something that shows up later (git pull, copied in) is not.
    let late = d.path().join(".nexus/collections/pulled");
    std::fs::create_dir_all(&late).unwrap();
    std::fs::write(
        late.join("collection.json"),
        r#"{"id":"p","name":"P","variables":[]}"#,
    )
    .unwrap();
    assert!(!app.collection_is_trusted("pulled"));
}

#[test]
fn external_changes_untrust_but_in_app_edits_do_not() {
    use crate::collections::TrustStatus;
    let (d, app) = app();
    let dir = app
        .collection_create(&create_default_collection("Mine"))
        .unwrap();
    let mut req = create_default_request();
    req.name = "r".into();
    app.request_add(&dir, &req).unwrap();
    assert_eq!(app.collection_trust_status(&dir), TrustStatus::Trusted);

    // In-app edit (script added) keeps trust.
    req.pre_request_script = Some("console.log(1)".into());
    app.request_update(&dir, &req.id, &req).unwrap();
    app.collection_rename(&dir, "Renamed").unwrap();
    assert_eq!(app.collection_trust_status(&dir), TrustStatus::Trusted);

    // A "git pull" rewrites a request file behind the app's back.
    let file = std::fs::read_dir(
        d.path()
            .join(".nexus/collections")
            .join(&dir)
            .join("requests"),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap()
    .path();
    let text = std::fs::read_to_string(&file)
        .unwrap()
        .replace("console.log(1)", "evil()");
    std::fs::write(&file, text).unwrap();
    assert_eq!(app.collection_trust_status(&dir), TrustStatus::Changed);
    assert!(!app.collection_is_trusted(&dir));

    // Saving into it in-app does not launder the foreign change.
    app.request_add(&dir, &create_default_request()).unwrap();
    assert_eq!(app.collection_trust_status(&dir), TrustStatus::Changed);

    // Explicit trust re-records the hash.
    app.collection_trust(&dir).unwrap();
    assert_eq!(app.collection_trust_status(&dir), TrustStatus::Trusted);
}
