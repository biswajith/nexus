//! Port of `packages/core/src/collections/collection-manager.ts`.
//!
//! Layout under the collections root (unchanged from the TS):
//! ```text
//! <slug>/collection.json        meta + itemOrder (no items)
//! <slug>/requests/<slug>.json   one file per root-level request
//! <slug>/folders/<slug>/folder.json  (+ <slug>.json requests and nested <slug>/ folders)
//! ```

use crate::fs::{create_private_dir, ensure_private_dir, write_private_file, WriteMode};
use crate::types::{CollectionItem, NexusCollection, NexusFolder, NexusRequest};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// One entry of `collections:list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionMeta {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub path: String,
}

/// Whether a collection may run scripts and see secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustStatus {
    Trusted,
    /// Never trusted (imported, pulled, copied in).
    Untrusted,
    /// Trusted before, but its scripts or requests changed outside the app since.
    Changed,
}

/// Reads and writes Nexus collection folders under a base path.
pub struct CollectionManager {
    base_path: PathBuf,
}

/// Lowercase, hyphenated slug safe for file and directory names.
fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c);
        } else {
            pending_dash = true;
        }
    }
    out
}

fn slug_or(name: &str, fallback: &str) -> String {
    let s = slugify(name);
    if s.is_empty() {
        fallback.to_string()
    } else {
        s
    }
}

/// A collection directory name must be one plain path component; it never legitimately
/// contains a separator or `..` (names come from `claim_directory`).
fn check_dir_name(dir_name: &str) -> Result<()> {
    if dir_name.is_empty()
        || dir_name == "."
        || dir_name == ".."
        || dir_name.contains(['/', '\\', '\0'])
    {
        return Err(Error::Other(format!(
            "Invalid collection directory: {dir_name:?}"
        )));
    }
    Ok(())
}

/// Serializes `value` without `items` and with `itemOrder` set from the item ids.
fn meta_json<T: Serialize>(value: &T, items: &[CollectionItem]) -> Result<String> {
    let mut v = serde_json::to_value(value)?;
    if let Some(obj) = v.as_object_mut() {
        obj.remove("items");
        obj.insert(
            "itemOrder".into(),
            json!(items.iter().map(item_id).collect::<Vec<_>>()),
        );
    }
    Ok(serde_json::to_string_pretty(&v)?)
}

fn item_id(item: &CollectionItem) -> &str {
    match item {
        CollectionItem::Folder(f) => &f.id,
        CollectionItem::Request(r) => &r.id,
    }
}

fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

/// `*.json` request files directly inside `dir`, sorted by name; unreadable/invalid files are skipped.
fn json_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut dirs: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

fn load_request(path: &Path) -> Option<NexusRequest> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Reorders `items` to follow `order`; ids not in `order` are appended in their original order.
fn sort_by_item_order(
    items: Vec<CollectionItem>,
    order: Option<&Vec<String>>,
) -> Vec<CollectionItem> {
    let Some(order) = order.filter(|o| !o.is_empty()) else {
        return items;
    };
    let mut remaining: Vec<Option<CollectionItem>> = items.into_iter().map(Some).collect();
    let mut sorted = Vec::with_capacity(remaining.len());
    for id in order {
        if let Some(slot) = remaining
            .iter_mut()
            .find(|s| s.as_ref().is_some_and(|i| item_id(i) == id))
        {
            sorted.push(slot.take().unwrap());
        }
    }
    sorted.extend(remaining.into_iter().flatten());
    sorted
}

/// Creates a new directory `slug`, `slug-2`, ... under `parent`; never reuses an existing one.
fn claim_directory(parent: &Path, slug: &str) -> Result<PathBuf> {
    for n in 1.. {
        let dir = parent.join(if n == 1 {
            slug.to_string()
        } else {
            format!("{slug}-{n}")
        });
        match create_private_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!()
}

/// Writes `data` to a new file `<slug>.json` in `dir` (`-2`, `-3`, ... when taken).
fn write_new_file(dir: &Path, slug: &str, data: &str) -> Result<()> {
    for n in 1.. {
        let file = dir.join(format!(
            "{}.json",
            if n == 1 {
                slug.to_string()
            } else {
                format!("{slug}-{n}")
            }
        ));
        match write_private_file(&file, data, WriteMode::CreateNew) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!()
}

fn save_request_file(dir: &Path, request: &NexusRequest) -> Result<()> {
    write_new_file(
        dir,
        &slug_or(&request.name, "request"),
        &serde_json::to_string_pretty(request)?,
    )
}

fn save_folder(folder_path: &Path, folder: &NexusFolder) -> Result<()> {
    write_private_file(
        &folder_path.join("folder.json"),
        &meta_json(folder, &folder.items)?,
        WriteMode::Replace,
    )?;
    for item in &folder.items {
        match item {
            CollectionItem::Folder(f) => {
                save_folder(
                    &claim_directory(folder_path, &slug_or(&f.name, "folder"))?,
                    f,
                )?;
            }
            CollectionItem::Request(r) => save_request_file(folder_path, r)?,
        }
    }
    Ok(())
}

fn load_folder(folder_path: &Path) -> NexusFolder {
    let dir_name = folder_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut meta: NexusFolder = read_json(&folder_path.join("folder.json"))
        .ok()
        .and_then(|mut v| {
            v.as_object_mut()?
                .entry("items")
                .or_insert_with(|| json!([]));
            serde_json::from_value(v).ok()
        })
        .unwrap_or_else(|| NexusFolder {
            id: dir_name.clone(),
            name: dir_name,
            description: None,
            items: vec![],
            auth: None,
            pre_request_script: None,
            post_response_script: None,
            item_order: None,
        });

    let mut items = Vec::new();
    for sub in subdirs(folder_path) {
        items.push(CollectionItem::Folder(load_folder(&sub)));
    }
    for file in json_files(folder_path) {
        if file.file_name().is_some_and(|n| n == "folder.json") {
            continue;
        }
        if let Some(r) = load_request(&file) {
            items.push(CollectionItem::Request(r));
        }
    }
    meta.items = sort_by_item_order(items, meta.item_order.as_ref());
    meta
}

impl CollectionManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Creates the base collections directory (0700) if needed.
    pub fn ensure_directory(&self) -> Result<()> {
        ensure_private_dir(&self.base_path)?;
        Ok(())
    }

    fn collection_dir(&self, dir_name: &str) -> Result<PathBuf> {
        check_dir_name(dir_name)?;
        Ok(self.base_path.join(dir_name))
    }

    /// Subdirectories with a valid `collection.json`; others are skipped.
    pub fn list_collections(&self) -> Result<Vec<CollectionMeta>> {
        self.ensure_directory()?;
        let mut out = Vec::new();
        for dir in subdirs(&self.base_path) {
            let Ok(v) = read_json(&dir.join("collection.json")) else {
                continue;
            };
            #[derive(Deserialize)]
            struct M {
                id: String,
                name: String,
                description: Option<String>,
            }
            let Ok(m) = serde_json::from_value::<M>(v) else {
                continue;
            };
            out.push(CollectionMeta {
                id: m.id,
                name: m.name,
                description: m.description,
                path: dir.to_string_lossy().to_string(),
            });
        }
        Ok(out)
    }

    /// Loads `collection.json`, root-level requests and nested folders.
    pub fn load_collection(&self, dir_name: &str) -> Result<NexusCollection> {
        let dir = self.collection_dir(dir_name)?;
        let mut meta_value = read_json(&dir.join("collection.json"))?;
        if let Some(obj) = meta_value.as_object_mut() {
            obj.entry("items").or_insert_with(|| json!([]));
        }
        let mut meta: NexusCollection = serde_json::from_value(meta_value)?;

        let mut items = Vec::new();
        for file in json_files(&dir.join("requests")) {
            if let Some(r) = load_request(&file) {
                items.push(CollectionItem::Request(r));
            }
        }
        for sub in subdirs(&dir.join("folders")) {
            items.push(CollectionItem::Folder(load_folder(&sub)));
        }
        meta.items = sort_by_item_order(items, meta.item_order.as_ref());
        Ok(meta)
    }

    /// Where trust decisions live: next to (not inside) the collections root, so a shared or
    /// git-synced collections folder cannot carry its own trust.
    fn trust_file(&self) -> PathBuf {
        self.base_path.with_file_name("trusted-collections.json")
    }

    /// Trusted collections: directory name -> hash of their trust-relevant content. Migration:
    /// when the file does not exist yet, every collection already on disk is grandfathered in as
    /// trusted (it is the user's own work) with its current hash recorded; anything that shows
    /// up afterwards (import, git pull, copied in) starts untrusted.
    fn trusted_map(&self) -> Result<BTreeMap<String, String>> {
        let file = self.trust_file();
        if let Ok(text) = std::fs::read_to_string(&file) {
            return Ok(serde_json::from_str(&text).unwrap_or_default());
        }
        self.ensure_directory()?;
        let map: BTreeMap<String, String> = subdirs(&self.base_path)
            .iter()
            .filter(|d| d.join("collection.json").is_file())
            .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().to_string()))
            .map(|n| {
                let h = self.content_hash(&n).unwrap_or_default();
                (n, h)
            })
            .collect();
        self.write_trusted(&map)?;
        Ok(map)
    }

    fn write_trusted(&self, map: &BTreeMap<String, String>) -> Result<()> {
        write_private_file(
            &self.trust_file(),
            &serde_json::to_string_pretty(map)?,
            WriteMode::Replace,
        )?;
        Ok(())
    }

    /// SHA-256 of everything the collection can execute or send: the whole loaded collection
    /// (all scripts at collection, folder and request level, plus auth, URLs, headers, bodies
    /// and variables). Only editing it outside the app changes this without a refresh.
    fn content_hash(&self, dir_name: &str) -> Result<String> {
        use sha2::{Digest, Sha256};
        let c = self.load_collection(dir_name)?;
        Ok(hex::encode(Sha256::digest(serde_json::to_vec(&c)?)))
    }

    /// Trust state of the collection in `dir_name`, checked against the files on disk now.
    pub fn trust_status(&self, dir_name: &str) -> TrustStatus {
        if check_dir_name(dir_name).is_err() {
            return TrustStatus::Untrusted;
        }
        let Ok(mut map) = self.trusted_map() else {
            return TrustStatus::Untrusted;
        };
        let Some(stored) = map.get(dir_name).cloned() else {
            return TrustStatus::Untrusted;
        };
        let Ok(now) = self.content_hash(dir_name) else {
            return TrustStatus::Changed;
        };
        if stored.is_empty() {
            map.insert(dir_name.to_string(), now);
            let _ = self.write_trusted(&map);
            return TrustStatus::Trusted;
        }
        if stored == now {
            TrustStatus::Trusted
        } else {
            TrustStatus::Changed
        }
    }

    /// Whether the user trusts the collection in `dir_name` (scripts and secrets allowed).
    pub fn is_trusted(&self, dir_name: &str) -> bool {
        self.trust_status(dir_name) == TrustStatus::Trusted
    }

    /// Marks the collection in `dir_name` as trusted as it is now (persisted, with its hash).
    pub fn trust_collection(&self, dir_name: &str) -> Result<()> {
        check_dir_name(dir_name)?;
        let mut map = self.trusted_map()?;
        map.insert(dir_name.to_string(), self.content_hash(dir_name)?);
        self.write_trusted(&map)
    }

    /// Runs an in-app edit; if the collection was trusted before it, the stored hash is
    /// refreshed so the user's own edits do not untrust it. An untrusted or externally changed
    /// collection stays so (saving into it must not launder foreign content into trust).
    fn edit<T>(&self, dir_name: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let was_trusted = self.is_trusted(dir_name);
        let r = f()?;
        if was_trusted {
            self.trust_collection(dir_name)?;
        }
        Ok(r)
    }

    /// Like [`Self::create_collection`] for content that did not originate here (imports): the
    /// collection starts untrusted.
    pub fn create_imported_collection(&self, collection: &NexusCollection) -> Result<String> {
        self.trusted_map()?; // migrate existing collections before a new directory appears
        self.write_collection(collection)
    }

    /// Writes a new collection directory (slug from name, never reusing an existing one) and
    /// trusts it: the user created it. Returns the directory name it was written to.
    pub fn create_collection(&self, collection: &NexusCollection) -> Result<String> {
        self.trusted_map()?;
        let dir_name = self.write_collection(collection)?;
        self.trust_collection(&dir_name)?;
        Ok(dir_name)
    }

    fn write_collection(&self, collection: &NexusCollection) -> Result<String> {
        self.ensure_directory()?;
        let dir = claim_directory(&self.base_path, &slug_or(&collection.name, "collection"))?;
        create_private_dir(&dir.join("requests"))?;
        let folders_dir = dir.join("folders");
        create_private_dir(&folders_dir)?;

        write_private_file(
            &dir.join("collection.json"),
            &meta_json(collection, &collection.items)?,
            WriteMode::Replace,
        )?;
        for item in &collection.items {
            match item {
                CollectionItem::Folder(f) => {
                    save_folder(
                        &claim_directory(&folders_dir, &slug_or(&f.name, "folder"))?,
                        f,
                    )?;
                }
                CollectionItem::Request(r) => self.save_request(&dir, r)?,
            }
        }
        Ok(dir.file_name().unwrap().to_string_lossy().to_string())
    }

    /// Writes a request as a new slug-named file under `<collection_dir>/requests`.
    pub fn save_request(&self, collection_dir: &Path, request: &NexusRequest) -> Result<()> {
        let requests = collection_dir.join("requests");
        ensure_private_dir(&requests)?;
        save_request_file(&requests, request)
    }

    pub fn add_request(&self, dir_name: &str, request: &NexusRequest) -> Result<()> {
        self.edit(dir_name, || {
            let dir = self.collection_dir(dir_name)?;
            self.save_request(&dir, request)
        })
    }

    /// Finds the request file whose stored `id` matches: in `requests/`, then in the folders
    /// (nested at any depth).
    fn find_request_file(&self, dir_name: &str, request_id: &str) -> Result<Option<PathBuf>> {
        fn has_id(f: &Path, id: &str) -> bool {
            read_json(f)
                .ok()
                .is_some_and(|v| v.get("id").and_then(Value::as_str) == Some(id))
        }
        fn in_folder(folder: &Path, id: &str) -> Option<PathBuf> {
            json_files(folder)
                .into_iter()
                .filter(|f| f.file_name().is_some_and(|n| n != "folder.json"))
                .find(|f| has_id(f, id))
                .or_else(|| subdirs(folder).iter().find_map(|d| in_folder(d, id)))
        }
        let dir = self.collection_dir(dir_name)?;
        Ok(json_files(&dir.join("requests"))
            .into_iter()
            .find(|f| has_id(f, request_id))
            .or_else(|| {
                subdirs(&dir.join("folders"))
                    .iter()
                    .find_map(|d| in_folder(d, request_id))
            }))
    }

    /// Replaces the request file whose `id` matches (forcing `id = request_id`; requests inside
    /// folders are updated in place), or adds a new one.
    pub fn update_request(
        &self,
        dir_name: &str,
        request_id: &str,
        request: &NexusRequest,
    ) -> Result<()> {
        let mut request = request.clone();
        request.id = request_id.to_string();
        self.edit(dir_name, || {
            match self.find_request_file(dir_name, request_id)? {
                Some(file) => {
                    write_private_file(
                        &file,
                        &serde_json::to_string_pretty(&request)?,
                        WriteMode::Replace,
                    )?;
                    Ok(())
                }
                None => self.add_request(dir_name, &request),
            }
        })
    }

    /// Removes the request file whose `id` matches; a no-op if there is none.
    pub fn delete_request(&self, dir_name: &str, request_id: &str) -> Result<()> {
        self.edit(dir_name, || {
            if let Some(file) = self.find_request_file(dir_name, request_id)? {
                std::fs::remove_file(file)?;
            }
            Ok(())
        })
    }

    /// Deletes the whole collection directory; a missing directory is not an error.
    pub fn delete_collection(&self, dir_name: &str) -> Result<()> {
        let dir = self.collection_dir(dir_name)?;
        match std::fs::remove_dir_all(dir) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        // A new directory later reusing the name must not inherit the trust.
        let mut map = self.trusted_map()?;
        if map.remove(dir_name).is_some() {
            self.write_trusted(&map)?;
        }
        Ok(())
    }

    /// Sets the `name` in `collection.json` (the directory keeps its name).
    pub fn rename_collection(&self, dir_name: &str, new_name: &str) -> Result<()> {
        self.edit(dir_name, || {
            let meta_path = self.collection_dir(dir_name)?.join("collection.json");
            let mut data = read_json(&meta_path)?;
            if let Some(obj) = data.as_object_mut() {
                obj.insert("name".into(), Value::String(new_name.to_string()));
            }
            write_private_file(
                &meta_path,
                &serde_json::to_string_pretty(&data)?,
                WriteMode::Replace,
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
