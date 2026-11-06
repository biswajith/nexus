use super::*;
use crate::auth::{StoredToken, TokenStore};
use crate::collections::CollectionManager;
use crate::history::HistoryManager;
use crate::types::*;
use indexmap::IndexMap;
use std::os::unix::fs::PermissionsExt;

fn mode(p: &Path) -> u32 {
    fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn creates_files_with_mode_0600() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.json");
    write_private_file(&file, "{}", WriteMode::Replace).unwrap();
    assert_eq!(mode(&file), 0o600);
}

#[test]
fn tightens_an_existing_world_readable_file_on_write() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("old.json");
    fs::write(&file, "{}").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    write_private_file(&file, "{\"a\":1}", WriteMode::Replace).unwrap();
    assert_eq!(mode(&file), 0o600);
    assert_eq!(fs::read_to_string(&file).unwrap(), "{\"a\":1}");
}

#[test]
fn appends_with_mode_0600() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("log.jsonl");
    append_private_file(&file, "a\n").unwrap();
    append_private_file(&file, "b\n").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "a\nb\n");
    assert_eq!(mode(&file), 0o600);
}

#[test]
fn creates_and_tightens_directories_to_0700() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("d");
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    ensure_private_dir(&dir).unwrap();
    assert_eq!(mode(&dir), 0o700);

    let nested = tmp.path().join("x/y/z");
    ensure_private_dir(&nested).unwrap();
    assert_eq!(mode(&nested), 0o700);
}

#[test]
fn refuses_to_replace_with_create_new() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.json");
    write_private_file(&file, "1", WriteMode::CreateNew).unwrap();
    let err = write_private_file(&file, "2", WriteMode::CreateNew).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&file).unwrap(), "1");
}

#[test]
fn stores_tokens_history_and_collections_as_0600() {
    let tmp = tempfile::tempdir().unwrap();
    let token_file = tmp.path().join("tokens.json");
    let mut tokens = TokenStore::new(Some(token_file.clone()));
    tokens
        .save_token(
            "k",
            StoredToken {
                access_token: "a".into(),
                token_type: "Bearer".into(),
                expires_in: None,
                expires_at: 0,
                refresh_token: None,
                scope: None,
            },
        )
        .unwrap();
    assert_eq!(mode(&token_file), 0o600);

    let history_dir = tmp.path().join("history");
    let history = HistoryManager::new(&history_dir);
    history
        .log(&HistoryEntry {
            id: "e".into(),
            timestamp: 1_750_000_000_000,
            request: HistoryRequest {
                method: "GET".into(),
                url: "u".into(),
                headers: IndexMap::new(),
                body: None,
            },
            response: HistoryResponse {
                status: 200,
                status_text: "OK".into(),
                response_time: 1.0,
                size: 0,
            },
            collection_id: None,
            request_id: None,
        })
        .unwrap();
    let file = fs::read_dir(&history_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&history_dir), 0o700);

    let collections = CollectionManager::new(tmp.path().join("collections"));
    let dir = collections
        .create_collection(&NexusCollection {
            id: "c".into(),
            name: "C".into(),
            description: None,
            version: None,
            variables: vec![],
            auth: None,
            pre_request_script: None,
            post_response_script: None,
            items: vec![],
            item_order: None,
        })
        .unwrap();
    let base = tmp.path().join("collections").join(&dir);
    assert_eq!(mode(&base.join("collection.json")), 0o600);
    assert_eq!(mode(&base), 0o700);
    assert_eq!(mode(&base.join("requests")), 0o700);
}

#[test]
fn is_safe_file_id_accepts() {
    for id in ["env_1", "abc-DEF_123", "a", &"x".repeat(128)] {
        assert!(is_safe_file_id(id), "{id}");
    }
}

#[test]
fn is_safe_file_id_rejects() {
    for id in [
        "",
        "../x",
        "a/b",
        "a\\b",
        "..",
        "a.json",
        &"x".repeat(129),
        "é",
        "a\0b",
    ] {
        assert!(!is_safe_file_id(id), "{id:?}");
    }
}

/// Port of the `architecture.test.ts` rule "only fs/private-fs.ts writes files" and "directories
/// are created owner-only": production sources must go through the helpers in this module.
#[test]
fn only_the_private_fs_module_writes_files_or_creates_directories() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src.clone()];
    let mut offenders = vec![];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let rel = path
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if !name.ends_with(".rs") || name == "tests.rs" || rel == "fs/mod.rs" {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap();
            let code: String = text
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            let code = code.split("#[cfg(test)]").next().unwrap();
            for needle in [
                "fs::write(",
                "fs::create_dir(",
                "fs::create_dir_all(",
                "File::create(",
                "OpenOptions",
                "DirBuilder",
            ] {
                if code.contains(needle) {
                    offenders.push(format!("{rel}: {needle}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "use crate::fs helpers (0600/0700): {offenders:?}"
    );
}
