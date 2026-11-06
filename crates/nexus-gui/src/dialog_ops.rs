//! What the dialogs do: parse / save imports, export to files, generate docs, manage
//! environments. Pure `impl NexusApp`; the dialogs only call these.

use crate::app::NexusApp;
use crate::runtime::Msg;
use crate::state::dialogs::{
    import_format_label, safe_file_stem, Dialog, DocFormat, EnvDraft, ExportFmt, ExportWhat, Parsed,
};
use crate::state::Panel;
use nexus_core::import::ImportFormat;
use nexus_core::service::App;
use nexus_core::types::{CollectionItem, NexusEnvironment, Variable, VariableType};
use std::path::{Path, PathBuf};

/// Detects the format of `text` and imports it.
pub fn parse_import(text: &str) -> Result<Parsed, String> {
    if text.trim().is_empty() {
        return Err("Nothing to import".into());
    }
    let format = App::import_detect_format(text);
    let result = App::import_data(text, Some(format)).map_err(|e| e.to_string())?;
    if result.collections.is_empty() && result.environments.is_empty() {
        return Err("Nothing to import: the file has no collections or environments".into());
    }
    Ok(Parsed { format, result })
}

/// Reads and parses an import file (blocking; call off the UI thread).
pub fn load_import_file(path: &Path) -> Result<Parsed, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() as usize > nexus_core::import::MAX_IMPORT_BYTES {
        return Err("The file is larger than 32 MB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read the file: {e}"))?;
    parse_import(&text)
}

fn first_request(items: &[CollectionItem]) -> Option<&nexus_core::types::NexusRequest> {
    items.iter().find_map(|i| match i {
        CollectionItem::Request(r) => Some(r),
        CollectionItem::Folder(f) => first_request(&f.items),
    })
}

impl NexusApp {
    // ---- import ----

    /// Native file picker for the import dialog.
    pub fn import_pick_file(&mut self) {
        self.rt.spawn(async {
            let path = rfd::AsyncFileDialog::new()
                .add_filter("Collections and specs", &["json", "yaml", "yml"])
                .pick_file()
                .await
                .map(|f| f.path().to_path_buf());
            Msg::ImportPicked(path)
        });
    }

    /// Reads, detects and parses an import file in the background; the preview appears when done.
    pub fn import_load_file(&mut self, path: &Path) {
        let path: PathBuf = path.to_path_buf();
        let source = path
            .file_name()
            .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
        self.import_begin();
        self.rt.spawn_blocking(move || Msg::Imported {
            source,
            result: load_import_file(&path).map(Box::new),
        });
    }

    /// Parses the pasted text in the background.
    pub fn import_parse_paste(&mut self) {
        let text = self.dlg.import.paste.trim().to_string();
        self.import_begin();
        self.rt.spawn_blocking(move || Msg::Imported {
            source: "Pasted text".into(),
            result: parse_import(&text).map(Box::new),
        });
    }

    fn import_begin(&mut self) {
        let st = &mut self.dlg.import;
        st.loading = true;
        st.error = None;
        st.parsed = None;
        st.done = None;
    }

    pub(crate) fn on_imported(&mut self, source: String, result: Result<Box<Parsed>, String>) {
        let st = &mut self.dlg.import;
        st.loading = false;
        match result {
            Ok(p) => {
                st.source = Some(source);
                st.parsed = Some(*p);
            }
            Err(e) => {
                st.source = Some(source);
                st.error = Some(e);
            }
        }
    }

    /// Saves everything in the preview (collections into fresh directories, environments as new).
    pub fn import_commit(&mut self) {
        let Some(p) = self.dlg.import.parsed.take() else {
            return;
        };
        match self
            .app
            .import_save(&p.result.collections, &p.result.environments)
        {
            Ok(lines) => {
                self.dlg.import.done = Some(lines);
                self.dlg.import.paste.clear();
                self.collections.refresh(&self.app);
                self.reload_envs();
            }
            Err(e) => {
                self.dlg.import.error = Some(format!("Failed to save: {e}"));
                self.dlg.import.parsed = Some(p);
            }
        }
    }

    /// Opens an imported cURL request in a new tab (the first request of the preview).
    pub fn import_open_in_tab(&mut self) {
        let Some(p) = &self.dlg.import.parsed else {
            return;
        };
        let req = p
            .result
            .collections
            .iter()
            .find_map(|c| first_request(&c.items))
            .cloned();
        if let Some(r) = req {
            self.tabs.open(r, None);
            self.ui.active_panel = Panel::Http;
            self.dlg.import.paste.clear();
            self.dlg.import.parsed = None;
            self.close_dialog();
        }
    }

    pub fn import_is_curl(&self) -> bool {
        self.dlg
            .import
            .parsed
            .as_ref()
            .is_some_and(|p| p.format == ImportFormat::Curl)
    }

    // ---- export ----

    fn export_content(
        &self,
        what: &ExportWhat,
        fmt: ExportFmt,
    ) -> Result<(String, String), String> {
        match what {
            ExportWhat::Collection(dir) => {
                let c = self.app.collection_load(dir).map_err(|e| e.to_string())?;
                let (suffix, name) = match fmt {
                    ExportFmt::Nexus => ("nexus", "nexus"),
                    ExportFmt::Postman => ("postman_collection", "postman-v2.1"),
                };
                let text = App::export_collection(&c, name).map_err(|e| e.to_string())?;
                Ok((format!("{}.{suffix}.json", safe_file_stem(&c.name)), text))
            }
            ExportWhat::Environment(id) => {
                let e = self
                    .envs
                    .environments
                    .iter()
                    .find(|e| e.id == *id)
                    .ok_or("Environment not found")?;
                let (suffix, name) = match fmt {
                    ExportFmt::Nexus => ("nexus_environment", "nexus"),
                    ExportFmt::Postman => ("postman_environment", "postman"),
                };
                let text = App::export_environment(e, name).map_err(|e| e.to_string())?;
                Ok((format!("{}.{suffix}.json", safe_file_stem(&e.name)), text))
            }
        }
    }

    /// Asks where to save the export (native dialog); the file is written when it closes.
    pub fn export_dialog(&mut self, what: ExportWhat, fmt: ExportFmt) {
        let (name, _) = match self.export_content(&what, fmt) {
            Ok(v) => v,
            Err(e) => {
                self.dlg.import.export_notice = Some(Err(e));
                return;
            }
        };
        self.rt.spawn(async move {
            let path = rfd::AsyncFileDialog::new()
                .set_file_name(name)
                .save_file()
                .await
                .map(|f| f.path().to_path_buf());
            Msg::ExportPicked { what, fmt, path }
        });
    }

    /// Writes the export to `path`.
    pub fn export_to(&mut self, what: &ExportWhat, fmt: ExportFmt, path: &Path) {
        let r = self.export_content(what, fmt).and_then(|(_, text)| {
            std::fs::write(path, text).map_err(|e| format!("Could not save: {e}"))?;
            Ok(format!("Exported to {}", path.display()))
        });
        self.dlg.import.export_notice = Some(r);
    }

    // ---- docs ----

    /// Generates the docs of the chosen collection in the chosen format.
    pub fn docs_generate(&mut self) {
        let st = &mut self.dlg.docs;
        st.notice = None;
        let Some(dir) = st.dir.clone() else {
            return;
        };
        let r = self
            .app
            .collection_load(&dir)
            .and_then(|c| match st.format {
                DocFormat::Markdown => nexus_core::docs::generate_markdown(&c),
                DocFormat::Html => nexus_core::docs::generate_html(&c),
            });
        match r {
            Ok(text) => {
                st.view.set(text.clone());
                st.content = Some(text);
                st.error = None;
            }
            Err(e) => {
                st.content = None;
                st.error = Some(e.to_string());
            }
        }
    }

    pub fn docs_file_name(&self) -> String {
        let name = self
            .collections
            .metas
            .iter()
            .find(|m| Some(&crate::state::collections::dir_name(m)) == self.dlg.docs.dir.as_ref())
            .map_or("docs", |m| m.name.as_str());
        format!(
            "{}-docs.{}",
            safe_file_stem(name),
            self.dlg.docs.format.ext()
        )
    }

    pub fn docs_save_dialog(&mut self) {
        let name = self.docs_file_name();
        self.rt.spawn(async move {
            let path = rfd::AsyncFileDialog::new()
                .set_file_name(name)
                .save_file()
                .await
                .map(|f| f.path().to_path_buf());
            Msg::DocsPicked(path)
        });
    }

    pub fn docs_save_to(&mut self, path: &Path) {
        let st = &mut self.dlg.docs;
        let Some(text) = &st.content else { return };
        st.notice = Some(match std::fs::write(path, text) {
            Ok(()) => format!("Saved to {}", path.display()),
            Err(e) => format!("Could not save: {e}"),
        });
    }

    /// Opens the generated HTML in the browser (scripts blocked, as for response previews).
    pub fn docs_open_browser(&mut self) {
        let Some(text) = self.dlg.docs.content.clone() else {
            return;
        };
        let html = crate::preview::preview_html(&text);
        self.dlg.docs.notice = Some(match self.open_temp("docs", "html", html.as_bytes()) {
            Ok(()) => "Opened in the browser".into(),
            Err(e) => format!("Could not open the browser: {e}"),
        });
    }

    // ---- environments ----

    pub fn reload_envs(&mut self) {
        match self.app.environments_list() {
            Ok(e) => self.envs.environments = e,
            Err(e) => self.dlg.envs.error = Some(e.to_string()),
        }
        if self.envs.get(self.ui.active_env_id.as_deref()).is_none() {
            self.ui.active_env_id = None;
        }
    }

    fn env_persist(&mut self, env: &NexusEnvironment) -> bool {
        match self.app.environment_save(env) {
            Ok(()) => {
                self.dlg.envs.error = None;
                self.reload_envs();
                true
            }
            Err(e) => {
                self.dlg.envs.error = Some(e.to_string());
                false
            }
        }
    }

    /// Creates an empty environment; returns its id.
    pub fn env_create(&mut self, name: &str) -> Option<String> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let env = NexusEnvironment {
            id: format!("env_{}", uuid::Uuid::new_v4()),
            name: name.to_string(),
            variables: Vec::new(),
        };
        self.env_persist(&env).then_some(env.id)
    }

    pub fn env_rename(&mut self, id: &str, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(mut e) = self.envs.environments.iter().find(|e| e.id == id).cloned() {
            e.name = name.to_string();
            self.env_persist(&e);
        }
    }

    pub fn env_duplicate(&mut self, id: &str) -> Option<String> {
        let mut e = self.envs.environments.iter().find(|e| e.id == id)?.clone();
        e.id = format!("env_{}", uuid::Uuid::new_v4());
        e.name = format!("{} Copy", e.name);
        self.env_persist(&e).then_some(e.id)
    }

    pub fn env_delete(&mut self, id: &str) {
        match self.app.environment_delete(id) {
            Ok(()) => {
                if self.ui.active_env_id.as_deref() == Some(id) {
                    self.ui.active_env_id = None;
                }
                if self.dlg.envs.draft.as_ref().is_some_and(|d| d.id == id) {
                    self.dlg.envs.draft = None;
                }
                self.dlg.envs.error = None;
                self.reload_envs();
            }
            Err(e) => self.dlg.envs.error = Some(e.to_string()),
        }
    }

    pub fn env_edit(&mut self, id: &str) {
        if let Some(e) = self.envs.environments.iter().find(|e| e.id == id) {
            self.dlg.envs.draft = Some(EnvDraft {
                id: e.id.clone(),
                name: e.name.clone(),
                vars: e.variables.clone(),
                dirty: false,
                error: None,
            });
            self.dlg.envs.revealed.clear();
        }
    }

    /// Saves the open draft (rows without a key are dropped).
    pub fn env_save_draft(&mut self) {
        let Some(d) = self.dlg.envs.draft.as_mut() else {
            return;
        };
        if d.name.trim().is_empty() {
            d.error = Some("The name must not be empty".into());
            return;
        }
        let env = NexusEnvironment {
            id: d.id.clone(),
            name: d.name.trim().to_string(),
            variables: d
                .vars
                .iter()
                .filter(|v| !v.key.trim().is_empty())
                .cloned()
                .map(|mut v| {
                    v.key = v.key.trim().to_string();
                    v
                })
                .collect(),
        };
        if self.env_persist(&env) {
            if let Some(d) = self.dlg.envs.draft.as_mut() {
                d.vars = env.variables;
                d.name = env.name;
                d.dirty = false;
                d.error = None;
            }
        }
    }

    pub fn new_variable() -> Variable {
        Variable {
            key: String::new(),
            value: String::new(),
            var_type: VariableType::String,
            enabled: true,
            description: None,
        }
    }

    pub fn import_label(&self) -> Option<&'static str> {
        self.dlg
            .import
            .parsed
            .as_ref()
            .map(|p| import_format_label(p.format))
    }

    #[allow(dead_code)]
    pub(crate) fn dialog_is(&self, d: Dialog) -> bool {
        self.dialog == Some(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_import_rejects_blank_and_unknown() {
        assert!(parse_import("   ").is_err());
        assert!(parse_import("hello world").is_err());
    }

    #[test]
    fn parse_import_reads_curl_as_a_request() {
        let p = parse_import("curl -X POST https://x.test/a -H 'A: b'").unwrap();
        assert_eq!(p.format, ImportFormat::Curl);
        assert_eq!(p.result.collections.len(), 1);
    }
}
