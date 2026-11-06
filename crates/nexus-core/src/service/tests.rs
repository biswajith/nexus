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
