use super::*;
use crate::types::{Variable, VariableType};
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn env(id: &str, name: &str) -> NexusEnvironment {
    NexusEnvironment {
        id: id.into(),
        name: name.into(),
        variables: vec![Variable {
            key: "k".into(),
            value: "v".into(),
            var_type: VariableType::Secret,
            enabled: true,
            description: None,
        }],
    }
}

fn setup() -> (tempfile::TempDir, EnvironmentStore) {
    let tmp = tempfile::tempdir().unwrap();
    let s = EnvironmentStore::new(tmp.path().join("environments"));
    (tmp, s)
}

#[test]
fn lists_empty_and_creates_the_private_directory() {
    let (t, s) = setup();
    assert!(s.list().unwrap().is_empty());
    let mode = fs::metadata(t.path().join("environments"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
}

#[test]
fn create_update_list_delete() {
    let (t, s) = setup();
    s.save_environment(&env("env_1", "One")).unwrap();
    s.save_environment(&env("env_2", "Two")).unwrap();
    s.save_environment(&env("env_1", "One renamed")).unwrap();

    let list = s.list().unwrap();
    assert_eq!(
        list.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["One renamed", "Two"]
    );
    let file = t.path().join("environments/env_1.json");
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(v["variables"][0]["type"], "secret");

    s.delete("env_1").unwrap();
    s.delete("env_1").unwrap();
    assert_eq!(s.list().unwrap().len(), 1);
}

#[test]
fn skips_unparsable_files_and_non_json() {
    let (t, s) = setup();
    s.save_environment(&env("ok", "Ok")).unwrap();
    fs::write(t.path().join("environments/bad.json"), "nope").unwrap();
    fs::write(t.path().join("environments/readme.txt"), "x").unwrap();
    assert_eq!(s.list().unwrap().len(), 1);
}

#[test]
fn rejects_unsafe_ids_on_save_and_delete() {
    let (t, s) = setup();
    for bad in ["../x", "a/b", "..", "", "a.json"] {
        assert!(s.save_environment(&env(bad, "x")).is_err(), "{bad:?}");
        assert!(s.delete(bad).is_err(), "{bad:?}");
    }
    assert!(!t.path().join("x.json").exists());
}

#[test]
fn imported_environment_never_replaces_an_existing_one() {
    let (_t, s) = setup();
    s.save_environment(&env("env_1", "Mine")).unwrap();
    let saved = s.save_imported(&env("env_1", "Imported")).unwrap();
    assert_ne!(saved.id, "env_1");
    assert!(saved.id.starts_with("env_") && is_safe_file_id(&saved.id));
    let list = s.list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list.iter().find(|e| e.id == "env_1").unwrap().name, "Mine");
}

#[test]
fn imported_environment_with_unsafe_id_gets_a_fresh_one() {
    let (t, s) = setup();
    let saved = s.save_imported(&env("../../evil", "Evil")).unwrap();
    assert!(is_safe_file_id(&saved.id));
    assert!(t
        .path()
        .join("environments")
        .join(format!("{}.json", saved.id))
        .is_file());
    assert!(!t.path().join("evil.json").exists());
}

#[test]
fn imported_environment_with_a_free_safe_id_keeps_it() {
    let (_t, s) = setup();
    assert_eq!(
        s.save_imported(&env("env_free", "Free")).unwrap().id,
        "env_free"
    );
}
