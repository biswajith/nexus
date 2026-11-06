//! Collection list, loaded collections and tree state (replaces `collection-store.ts`).

use nexus_core::collections::CollectionMeta;
use nexus_core::service::App;
use nexus_core::types::{create_default_collection, NexusCollection, NexusRequest};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Default)]
pub struct CollectionsState {
    pub metas: Vec<CollectionMeta>,
    /// Loaded collections by directory name.
    pub loaded: HashMap<String, NexusCollection>,
    /// Expanded collection ids and folder ids.
    pub expanded: HashSet<String>,
    /// Inline "new collection" input.
    pub creating: Option<String>,
    /// Inline rename: (collection id, buffer).
    pub renaming: Option<(String, String)>,
    /// Inline "new request" input: (directory name, buffer).
    pub adding_request: Option<(String, String)>,
    pub error: Option<String>,
}

/// Directory name of a collection (last path segment).
pub fn dir_name(meta: &CollectionMeta) -> String {
    Path::new(&meta.path)
        .file_name()
        .map_or_else(|| meta.path.clone(), |n| n.to_string_lossy().into_owned())
}

impl CollectionsState {
    fn fail<T>(&mut self, r: nexus_core::Result<T>) -> Option<T> {
        match r {
            Ok(v) => {
                self.error = None;
                Some(v)
            }
            Err(e) => {
                self.error = Some(e.to_string());
                None
            }
        }
    }

    pub fn refresh(&mut self, app: &App) {
        if let Some(m) = self.fail(app.collections_list()) {
            self.metas = m;
        }
    }

    pub fn load(&mut self, app: &App, dir: &str) {
        if let Some(c) = self.fail(app.collection_load(dir)) {
            self.loaded.insert(dir.to_string(), c);
        }
    }

    /// Reloads every expanded collection (startup).
    pub fn load_expanded(&mut self, app: &App) {
        let dirs: Vec<String> = self
            .metas
            .iter()
            .filter(|m| self.expanded.contains(&m.id))
            .map(dir_name)
            .collect();
        for d in dirs {
            self.load(app, &d);
        }
    }

    pub fn toggle_collection(&mut self, app: &App, meta: &CollectionMeta) {
        if !self.expanded.remove(&meta.id) {
            self.expanded.insert(meta.id.clone());
            self.load(app, &dir_name(meta));
        }
    }

    pub fn toggle_folder(&mut self, id: &str) {
        if !self.expanded.remove(id) {
            self.expanded.insert(id.to_string());
        }
    }

    pub fn create(&mut self, app: &App, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if self
            .fail(app.collection_create(&create_default_collection(name)))
            .is_some()
        {
            self.refresh(app);
        }
    }

    pub fn rename(&mut self, app: &App, dir: &str, new_name: &str) {
        let name = new_name.trim();
        if name.is_empty() {
            return;
        }
        if self.fail(app.collection_rename(dir, name)).is_some() {
            self.refresh(app);
            self.load(app, dir);
        }
    }

    pub fn delete(&mut self, app: &App, dir: &str) {
        if self.fail(app.collection_delete(dir)).is_some() {
            self.loaded.remove(dir);
            self.refresh(app);
        }
    }

    pub fn add_request(&mut self, app: &App, dir: &str, request: &NexusRequest) -> bool {
        let ok = self.fail(app.request_add(dir, request)).is_some();
        if ok {
            self.load(app, dir);
        }
        ok
    }

    pub fn update_request(&mut self, app: &App, dir: &str, request: &NexusRequest) -> bool {
        let ok = self
            .fail(app.request_update(dir, &request.id, request))
            .is_some();
        if ok {
            self.load(app, dir);
        }
        ok
    }

    pub fn delete_request(&mut self, app: &App, dir: &str, request_id: &str) {
        if self.fail(app.request_delete(dir, request_id)).is_some() {
            self.load(app, dir);
        }
    }
}
