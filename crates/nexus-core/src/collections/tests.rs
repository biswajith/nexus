use super::*;
use crate::types::*;
use std::fs;

fn make_request(id: &str, name: &str) -> NexusRequest {
    let mut r = create_default_request();
    r.id = id.into();
    r.name = name.into();
    r.url = format!("https://api.example.com/{name}");
    r
}

fn req(id: &str, name: &str) -> CollectionItem {
    CollectionItem::Request(make_request(id, name))
}

fn folder(id: &str, name: &str, items: Vec<CollectionItem>) -> CollectionItem {
    CollectionItem::Folder(NexusFolder {
        id: id.into(),
        name: name.into(),
        description: None,
        items,
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        item_order: None,
    })
}

fn collection(id: &str, name: &str, items: Vec<CollectionItem>) -> NexusCollection {
    NexusCollection {
        id: id.into(),
        name: name.into(),
        description: None,
        version: None,
        variables: vec![],
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        items,
        item_order: None,
    }
}

fn ids(items: &[CollectionItem]) -> Vec<&str> {
    items.iter().map(item_id).collect()
}

fn setup() -> (tempfile::TempDir, CollectionManager) {
    let tmp = tempfile::tempdir().unwrap();
    let m = CollectionManager::new(tmp.path());
    (tmp, m)
}

fn only_dir(tmp: &Path) -> String {
    fs::read_dir(tmp)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name()
        .into_string()
        .unwrap()
}

fn as_folder(item: &CollectionItem) -> &NexusFolder {
    match item {
        CollectionItem::Folder(f) => f,
        _ => panic!("not a folder"),
    }
}

// --- item ordering ---

#[test]
fn preserves_top_level_request_order() {
    let (t, m) = setup();
    m.create_collection(&collection(
        "col_order",
        "Order Test",
        vec![
            req("req_aaa", "third-request"),
            req("req_bbb", "first-request"),
            req("req_ccc", "second-request"),
        ],
    ))
    .unwrap();
    let loaded = m.load_collection(&only_dir(t.path())).unwrap();
    assert_eq!(ids(&loaded.items), ["req_aaa", "req_bbb", "req_ccc"]);
}

#[test]
fn preserves_folder_item_order() {
    let (t, m) = setup();
    let f = folder(
        "folder_abc",
        "My Folder",
        vec![
            req("req_f1", "zebra"),
            req("req_f2", "alpha"),
            req("req_f3", "middle"),
        ],
    );
    m.create_collection(&collection("c", "Folder Order Test", vec![f]))
        .unwrap();
    let loaded = m.load_collection(&only_dir(t.path())).unwrap();
    assert_eq!(
        ids(&as_folder(&loaded.items[0]).items),
        ["req_f1", "req_f2", "req_f3"]
    );
}

#[test]
fn preserves_mixed_folders_and_requests_in_order() {
    let (t, m) = setup();
    let items = vec![
        req("req_1", "first"),
        folder("folder_1", "middle-folder", vec![req("req_inner", "inner")]),
        req("req_2", "last"),
    ];
    m.create_collection(&collection("c", "Mixed Order Test", items))
        .unwrap();
    let loaded = m.load_collection(&only_dir(t.path())).unwrap();
    assert_eq!(ids(&loaded.items), ["req_1", "folder_1", "req_2"]);
}

#[test]
fn preserves_deeply_nested_folder_order() {
    let (t, m) = setup();
    let child = folder(
        "folder_child",
        "Child",
        vec![
            req("req_deep1", "deep-first"),
            req("req_deep2", "deep-second"),
        ],
    );
    let parent = folder(
        "folder_parent",
        "Parent",
        vec![req("req_top", "top-of-parent"), child],
    );
    m.create_collection(&collection("c", "Nested Order Test", vec![parent]))
        .unwrap();
    let loaded = m.load_collection(&only_dir(t.path())).unwrap();
    let p = as_folder(&loaded.items[0]);
    assert_eq!(ids(&p.items), ["req_top", "folder_child"]);
    assert_eq!(
        ids(&as_folder(&p.items[1]).items),
        ["req_deep1", "req_deep2"]
    );
}

#[test]
fn appends_items_not_in_item_order_at_the_end() {
    let (t, m) = setup();
    m.create_collection(&collection(
        "c",
        "Extra Items Test",
        vec![req("req_a", "a"), req("req_b", "b")],
    ))
    .unwrap();
    let slug = only_dir(t.path());
    m.add_request(&slug, &make_request("req_extra", "extra"))
        .unwrap();
    let loaded = m.load_collection(&slug).unwrap();
    let got = ids(&loaded.items);
    let pos = |id| got.iter().position(|x| *x == id).unwrap();
    assert!(pos("req_a") < pos("req_b"));
    assert_eq!(pos("req_extra"), got.len() - 1);
}

// --- name collisions never overwrite ---

#[test]
fn colliding_slug_goes_to_a_new_directory() {
    let (_t, m) = setup();
    let mut mine = collection("col_mine", "Payments API", vec![req("req_mine", "charge")]);
    mine.variables = vec![Variable {
        key: "baseUrl".into(),
        value: "https://pay.internal".into(),
        var_type: VariableType::String,
        enabled: true,
        description: None,
    }];
    mine.pre_request_script = Some(String::new());
    let mut imported = collection(
        "col_evil",
        "payments  API!!",
        vec![req("req_evil", "charge")],
    );
    imported.pre_request_script = Some("console.log(\"attacker\")".into());

    let first = m.create_collection(&mine).unwrap();
    let second = m.create_collection(&imported).unwrap();
    assert_eq!(first, "payments-api");
    assert_eq!(second, "payments-api-2");

    let original = m.load_collection(&first).unwrap();
    assert_eq!(original.id, "col_mine");
    assert_eq!(original.variables[0].value, "https://pay.internal");
    assert_eq!(original.pre_request_script.as_deref(), Some(""));
    assert_eq!(ids(&original.items), ["req_mine"]);

    let copy = m.load_collection(&second).unwrap();
    assert_eq!(copy.id, "col_evil");
    assert_eq!(ids(&copy.items), ["req_evil"]);
}

#[test]
fn keeps_both_requests_with_the_same_name() {
    let (_t, m) = setup();
    let dir = m
        .create_collection(&collection(
            "c",
            "Dup",
            vec![req("req_1", "Get User"), req("req_2", "get-user")],
        ))
        .unwrap();
    let mut got: Vec<String> = ids(&m.load_collection(&dir).unwrap().items)
        .iter()
        .map(|s| s.to_string())
        .collect();
    got.sort();
    assert_eq!(got, ["req_1", "req_2"]);
}

#[test]
fn keeps_both_folders_with_the_same_name() {
    let (_t, m) = setup();
    let items = vec![
        folder("fld_1", "Users", vec![req("req_a", "a")]),
        folder("fld_2", "users", vec![req("req_b", "b")]),
    ];
    let dir = m
        .create_collection(&collection("c", "Dup folders", items))
        .unwrap();
    let mut got: Vec<String> = ids(&m.load_collection(&dir).unwrap().items)
        .iter()
        .map(|s| s.to_string())
        .collect();
    got.sort();
    assert_eq!(got, ["fld_1", "fld_2"]);
}

#[test]
fn does_not_write_into_the_base_directory_for_a_name_without_slug_characters() {
    let (t, m) = setup();
    let dir = m
        .create_collection(&collection("c", "!!!", vec![]))
        .unwrap();
    assert_eq!(dir, "collection");
    assert!(!t.path().join("collection.json").exists());
}

// --- CRUD (covered only through the IPC handlers in the TS) ---

#[test]
fn slugify_matches_the_ts_regexes() {
    assert_eq!(slugify("Payments API"), "payments-api");
    assert_eq!(slugify("  --Hello__World!! "), "hello-world");
    assert_eq!(slugify("a1 b2"), "a1-b2");
    assert_eq!(slugify("!!!"), "");
    assert_eq!(slugify("Ünï"), "n");
}

#[test]
fn lists_collections_and_skips_invalid_directories() {
    let (t, m) = setup();
    let mut c = collection("c1", "First", vec![]);
    c.description = Some("desc".into());
    let dir = m.create_collection(&c).unwrap();
    fs::create_dir(t.path().join("junk")).unwrap();
    fs::create_dir(t.path().join("broken")).unwrap();
    fs::write(t.path().join("broken/collection.json"), "nope").unwrap();
    fs::write(t.path().join("stray.txt"), "x").unwrap();

    let list = m.list_collections().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "c1");
    assert_eq!(list[0].description.as_deref(), Some("desc"));
    assert_eq!(list[0].path, t.path().join(&dir).to_string_lossy());
    let json = serde_json::to_value(&list[0]).unwrap();
    assert_eq!(json["name"], "First");
}

#[test]
fn list_creates_the_base_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let m = CollectionManager::new(tmp.path().join("collections"));
    assert!(m.list_collections().unwrap().is_empty());
    assert!(tmp.path().join("collections").is_dir());
}

#[test]
fn collection_json_has_item_order_and_no_items() {
    let (t, m) = setup();
    let dir = m
        .create_collection(&collection("c", "C", vec![req("r1", "a"), req("r2", "b")]))
        .unwrap();
    let v: Value = serde_json::from_str(
        &fs::read_to_string(t.path().join(&dir).join("collection.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(v["itemOrder"], json!(["r1", "r2"]));
    assert!(v.get("items").is_none());
    assert!(t.path().join(&dir).join("requests/a.json").is_file());
    assert!(t.path().join(&dir).join("folders").is_dir());
}

#[test]
fn load_of_a_missing_collection_fails() {
    let (_t, m) = setup();
    assert!(m.load_collection("nope").is_err());
}

#[test]
fn loads_a_folder_without_folder_json_with_defaults() {
    let (t, m) = setup();
    let dir = m.create_collection(&collection("c", "C", vec![])).unwrap();
    let f = t.path().join(&dir).join("folders/loose");
    fs::create_dir(&f).unwrap();
    fs::write(
        f.join("x.json"),
        serde_json::to_string(&make_request("rx", "x")).unwrap(),
    )
    .unwrap();
    let loaded = m.load_collection(&dir).unwrap();
    let lf = as_folder(&loaded.items[0]);
    assert_eq!((lf.id.as_str(), lf.name.as_str()), ("loose", "loose"));
    assert_eq!(ids(&lf.items), ["rx"]);
}

#[test]
fn add_update_and_delete_request() {
    let (_t, m) = setup();
    let dir = m
        .create_collection(&collection("c", "C", vec![req("r1", "one")]))
        .unwrap();

    m.add_request(&dir, &make_request("r2", "two")).unwrap();
    assert_eq!(m.load_collection(&dir).unwrap().items.len(), 2);

    let mut updated = make_request("ignored", "one renamed");
    updated.url = "https://changed.example".into();
    m.update_request(&dir, "r1", &updated).unwrap();
    let loaded = m.load_collection(&dir).unwrap();
    assert_eq!(loaded.items.len(), 2);
    match loaded.items.iter().find(|i| item_id(i) == "r1").unwrap() {
        CollectionItem::Request(r) => {
            assert_eq!(r.url, "https://changed.example");
            assert_eq!(r.name, "one renamed");
        }
        _ => unreachable!(),
    }

    m.delete_request(&dir, "r1").unwrap();
    assert_eq!(ids(&m.load_collection(&dir).unwrap().items), ["r2"]);
    // deleting an unknown id is a no-op
    m.delete_request(&dir, "missing").unwrap();
}

#[test]
fn update_and_delete_find_requests_inside_nested_folders() {
    let (_t, m) = setup();
    let inner = NexusFolder {
        id: "inner".into(),
        name: "Inner".into(),
        description: None,
        items: vec![req("deep", "deep")],
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        item_order: None,
    };
    let outer = NexusFolder {
        id: "outer".into(),
        name: "Outer".into(),
        description: None,
        items: vec![req("shallow", "shallow"), CollectionItem::Folder(inner)],
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        item_order: None,
    };
    let dir = m
        .create_collection(&collection("c", "C", vec![CollectionItem::Folder(outer)]))
        .unwrap();
    for id in ["shallow", "deep"] {
        let mut u = make_request(id, "edited");
        u.url = "https://changed.example".into();
        m.update_request(&dir, id, &u).unwrap();
    }
    let loaded = m.load_collection(&dir).unwrap();
    // no top-level duplicates were created, the nested requests changed in place
    assert_eq!(loaded.items.len(), 1);
    let json = serde_json::to_string(&loaded).unwrap();
    assert_eq!(json.matches("https://changed.example").count(), 2);
    m.delete_request(&dir, "deep").unwrap();
    assert!(!serde_json::to_string(&m.load_collection(&dir).unwrap())
        .unwrap()
        .contains("\"id\":\"deep\""));
}

#[test]
fn update_request_adds_when_the_id_is_not_found() {
    let (_t, m) = setup();
    let dir = m.create_collection(&collection("c", "C", vec![])).unwrap();
    m.update_request(&dir, "new_id", &make_request("other", "fresh"))
        .unwrap();
    assert_eq!(ids(&m.load_collection(&dir).unwrap().items), ["new_id"]);
}

#[test]
fn update_request_keeps_unrelated_request_files_untouched() {
    let (t, m) = setup();
    let dir = m
        .create_collection(&collection("c", "C", vec![req("r1", "a"), req("r2", "b")]))
        .unwrap();
    let before = fs::read_to_string(t.path().join(&dir).join("requests/b.json")).unwrap();
    m.update_request(&dir, "r1", &make_request("r1", "a"))
        .unwrap();
    assert_eq!(
        fs::read_to_string(t.path().join(&dir).join("requests/b.json")).unwrap(),
        before
    );
}

#[test]
fn renames_a_collection_without_moving_its_directory() {
    let (t, m) = setup();
    let dir = m
        .create_collection(&collection("c", "Old", vec![req("r1", "a")]))
        .unwrap();
    m.rename_collection(&dir, "New Name").unwrap();
    assert!(t.path().join(&dir).is_dir());
    let loaded = m.load_collection(&dir).unwrap();
    assert_eq!(loaded.name, "New Name");
    assert_eq!(ids(&loaded.items), ["r1"]);
}

#[test]
fn deletes_a_collection_and_tolerates_a_missing_one() {
    let (t, m) = setup();
    let dir = m
        .create_collection(&collection("c", "C", vec![req("r1", "a")]))
        .unwrap();
    m.delete_collection(&dir).unwrap();
    assert!(!t.path().join(&dir).exists());
    m.delete_collection(&dir).unwrap();
}

#[test]
fn unknown_fields_in_request_files_are_ignored_on_load() {
    let (t, m) = setup();
    let dir = m
        .create_collection(&collection("c", "C", vec![req("r1", "a")]))
        .unwrap();
    let p = t.path().join(&dir).join("requests/a.json");
    let mut v: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
    v["somethingNew"] = json!(1);
    fs::write(&p, v.to_string()).unwrap();
    assert_eq!(ids(&m.load_collection(&dir).unwrap().items), ["r1"]);
}

// --- guards from architecture.test.ts / path traversal ---

#[test]
fn directory_names_with_path_components_are_rejected() {
    let (t, m) = setup();
    let victim = tempfile::tempdir_in(t.path()).unwrap();
    let victim_name = victim
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    for bad in [
        "",
        ".",
        "..",
        "../x",
        "a/b",
        "a\\b",
        &format!("../{victim_name}"),
    ] {
        assert!(m.load_collection(bad).is_err(), "{bad:?}");
        assert!(m.delete_collection(bad).is_err(), "{bad:?}");
        assert!(m.rename_collection(bad, "x").is_err(), "{bad:?}");
        assert!(
            m.add_request(bad, &make_request("r", "r")).is_err(),
            "{bad:?}"
        );
        assert!(
            m.update_request(bad, "r", &make_request("r", "r")).is_err(),
            "{bad:?}"
        );
        assert!(m.delete_request(bad, "r").is_err(), "{bad:?}");
    }
    assert!(victim.path().exists());
}

#[test]
fn request_ids_never_become_file_names() {
    let (t, m) = setup();
    let dir = m
        .create_collection(&collection("c", "C", vec![req("../../evil", "x")]))
        .unwrap();
    // The file is named from the slugified name, not from the id.
    assert!(t.path().join(&dir).join("requests/x.json").is_file());
    assert!(!t.path().parent().unwrap().join("evil.json").exists());
}

/// Port of the architecture rule: `createCollection` must go through `claimDirectory`.
#[test]
fn create_collection_claims_a_fresh_directory() {
    let src = include_str!("mod.rs");
    let body = src
        .split("fn write_collection")
        .nth(1)
        .unwrap()
        .split("pub fn save_request")
        .next()
        .unwrap();
    assert!(body.contains("claim_directory("));
    assert!(!body.contains("create_dir_all") && !body.contains("ensure_private_dir(&dir"));
}

#[test]
fn collection_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let (t, m) = setup();
    let dir = m
        .create_collection(&collection(
            "c",
            "C",
            vec![req("r", "a"), folder("f", "F", vec![req("r2", "b")])],
        ))
        .unwrap();
    let mode = |p: PathBuf| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    let base = t.path().join(dir);
    assert_eq!(mode(base.join("requests/a.json")), 0o600);
    assert_eq!(mode(base.join("folders/f")), 0o700);
    assert_eq!(mode(base.join("folders/f/folder.json")), 0o600);
    assert_eq!(mode(base.join("folders/f/b.json")), 0o600);
}
