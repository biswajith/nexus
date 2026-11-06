//! The eframe app: owns the service layer, the state structs and the runtime, and lays out the shell.

use crate::payload::{build_payload, Scopes};
use crate::preview::{system_opener, Opener};
use crate::runtime::{Msg, PickTarget, Runtime};
use crate::state::ui_state::{MAX_CONSOLE, MAX_SIDEBAR, MIN_CONSOLE, MIN_SIDEBAR};
use crate::state::{
    collections::dir_name, CollectionsState, ConsoleState, EnvState, GqlState, HistoryState,
    McpState, Origin, Panel, ReqTab, Resp, RespTab, RunnerState, SidebarTab, Tabs, UiState,
    ViewState, WsState,
};
use crate::theme::*;
use eframe::egui::{self, vec2, Ui};
use nexus_core::service::App;
use nexus_core::types::{
    HistoryEntry, HistoryFilter, HistoryRequest, HistoryResponse, LogLevel, LogSource, NexusRequest,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default, Clone)]
pub struct Opts {
    pub screenshot: Option<String>,
    pub home: Option<PathBuf>,
    pub size: Option<(f32, f32)>,
    pub autosend: bool,
    pub url: Option<String>,
    pub req_tab: Option<String>,
    pub resp_tab: Option<String>,
    pub sidebar_tab: Option<String>,
    pub panel: Option<String>,
    pub expand_all: bool,
    pub open_first: bool,
    /// Open the collection request with this name (dev flag).
    pub open_request: Option<String>,
    pub seed_demo: bool,
    /// Force the theme: dark | light | system (dev flag).
    pub theme: Option<String>,
    /// Start with the console panel open (dev flag).
    pub console: bool,
    /// Open the body search with this text once a response arrives (dev flag).
    pub search: Option<String>,
    pub view_mode: Option<String>,
    /// Put this file's text into the JSON body of the active tab (dev flag).
    pub body_file: Option<String>,
    /// Put this file's text into the pre-request script of the active tab (dev flag).
    pub script_file: Option<String>,
    /// Focus the first code editor once it is drawn (dev flag).
    pub focus_editor: bool,
    /// Scripted input fed one item per frame: `text:abc`, `key:ctrl+f`, `wait`, `|`-separated.
    pub input: Option<String>,
}

pub struct NexusApp {
    pub app: Arc<App>,
    pub home: PathBuf,
    pub ui: UiState,
    pub tabs: Tabs,
    pub collections: CollectionsState,
    pub history: HistoryState,
    pub envs: EnvState,
    pub view: ViewState,
    pub console: ConsoleState,
    pub runner: RunnerState,
    pub ws: WsState,
    pub gql: GqlState,
    pub mcp: McpState,
    /// Opens temp files (HTML preview, visualizer, binary bodies); replaced in tests.
    pub opener: Opener,
    pub rt: Runtime,
    /// The open modal dialog, if any.
    pub dialog: Option<crate::state::dialogs::Dialog>,
    pub dlg: crate::state::dialogs::Dialogs,
    styled: Option<bool>,
    opts: Opts,
    frame_no: u32,
    shot_requested: bool,
    input_queue: std::collections::VecDeque<String>,
    sent_auto: bool,
    last_save: Instant,
    last_saved_json: String,
}

impl NexusApp {
    /// Opens the app on the data directory `home` (normally `~/.nexus`).
    pub fn new(home: PathBuf, opts: Opts) -> std::io::Result<Self> {
        let app = Arc::new(App::new(&home)?);
        let mut ui = UiState::load(&home);
        match opts.sidebar_tab.as_deref() {
            Some("history") => ui.sidebar_tab = SidebarTab::History,
            Some("collections") => ui.sidebar_tab = SidebarTab::Collections,
            _ => {}
        }
        if let Some(p) = &opts.panel {
            ui.active_panel = match p.as_str() {
                "graphql" => Panel::Graphql,
                "ws" => Panel::Ws,
                "mcp" => Panel::Mcp,
                "runner" => Panel::Runner,
                _ => Panel::Http,
            };
        }
        let mut collections = CollectionsState {
            expanded: ui.expanded.iter().cloned().collect(),
            ..Default::default()
        };
        collections.refresh(&app);
        if opts.expand_all {
            let ids: Vec<String> = collections.metas.iter().map(|m| m.id.clone()).collect();
            collections.expanded.extend(ids);
        }
        collections.load_expanded(&app);
        if opts.expand_all {
            let folders: Vec<String> = collections
                .loaded
                .values()
                .flat_map(|c| folder_ids(&c.items))
                .collect();
            collections.expanded.extend(folders);
        }

        let mut envs = EnvState::default();
        match app.environments_list() {
            Ok(e) => envs.environments = e,
            Err(e) => collections.error = Some(e.to_string()),
        }
        if envs.get(ui.active_env_id.as_deref()).is_none() {
            ui.active_env_id = None;
        }

        let mut tabs = Tabs::from_saved(&ui.tabs, ui.active_tab_id.as_deref());
        if tabs.tabs.is_empty() {
            tabs.open_blank();
        }
        if opts.open_first {
            let first = collections.metas.first().and_then(|m| {
                let dir = dir_name(m);
                let req = collections
                    .loaded
                    .get(&dir)
                    .and_then(|c| first_request(&c.items))?;
                Some((dir, req.clone()))
            });
            if let Some((dir, req)) = first {
                let origin = Origin {
                    dir_name: dir,
                    request_id: req.id.clone(),
                };
                tabs.open(req, Some(origin));
            }
        }
        if let Some(name) = &opts.open_request {
            let metas: Vec<_> = collections.metas.iter().map(dir_name).collect();
            for d in metas {
                collections.load(&app, &d);
                let found = collections
                    .loaded
                    .get(&d)
                    .and_then(|c| find_request(&c.items, name));
                if let Some(req) = found {
                    let origin = Origin {
                        dir_name: d,
                        request_id: req.id.clone(),
                    };
                    tabs.open(req, Some(origin));
                    break;
                }
            }
        }
        if let (Some(u), Some(t)) = (&opts.url, tabs.active_mut()) {
            t.request.url = u.clone();
        }
        if let Some(t) = tabs.active_mut() {
            if let Some(text) = opts
                .body_file
                .as_ref()
                .and_then(|f| std::fs::read_to_string(f).ok())
            {
                t.request.body.mode = nexus_core::types::BodyMode::Json;
                t.request.body.raw = Some(text);
            }
            if let Some(text) = opts
                .script_file
                .as_ref()
                .and_then(|f| std::fs::read_to_string(f).ok())
            {
                t.request.pre_request_script = Some(text);
            }
        }

        let req_tab = match opts.req_tab.as_deref() {
            Some("params") => ReqTab::Params,
            Some("headers") => ReqTab::Headers,
            Some("auth") => ReqTab::Auth,
            Some("scripts") => ReqTab::Scripts,
            _ => ReqTab::Body,
        };
        let resp_tab = match opts.resp_tab.as_deref() {
            Some("headers") => RespTab::Headers,
            Some("tests") => RespTab::Tests,
            Some("timing") => RespTab::Timing,
            Some("visualize") => RespTab::Visualize,
            _ => RespTab::Body,
        };
        let view = ViewState {
            req_tab,
            resp_tab,
            ..Default::default()
        };

        if opts.console {
            ui.console_open = true;
        }
        let opts_input = opts
            .input
            .as_deref()
            .map(|s| s.split('|').map(str::to_string).collect())
            .unwrap_or_default();
        let mut me = Self {
            app,
            home,
            ui,
            tabs,
            collections,
            history: HistoryState::default(),
            envs,
            view,
            console: ConsoleState::default(),
            runner: RunnerState::default(),
            ws: WsState::default(),
            gql: GqlState::default(),
            mcp: McpState::default(),
            opener: system_opener(),
            rt: Runtime::new(),
            dialog: None,
            dlg: Default::default(),
            styled: None,
            opts,
            frame_no: 0,
            shot_requested: false,
            input_queue: opts_input,
            sent_auto: false,
            last_save: Instant::now(),
            last_saved_json: String::new(),
        };
        let forms = me.ui.forms.clone();
        if let Some(p) = forms.restore(&mut me.runner, &mut me.ws, &mut me.gql, &mut me.mcp) {
            if p.is_file() {
                me.runner_load_data(&p);
            }
        }
        me.last_saved_json = me.snapshot();
        me.refresh_history();
        Ok(me)
    }

    // ---- persistence ----

    fn snapshot(&self) -> String {
        self.ui.snapshot(
            &self.tabs,
            &self.collections.expanded,
            crate::state::forms::PanelForms::capture(&self.runner, &self.ws, &self.gql, &self.mcp),
        )
    }

    /// Writes `ui-state.json` if anything persisted changed.
    pub fn save_ui_state(&mut self) {
        let json = self.snapshot();
        if json != self.last_saved_json && UiState::write(&self.home, &json).is_ok() {
            self.last_saved_json = json;
        }
        self.last_save = Instant::now();
    }

    // ---- history ----

    pub fn refresh_history(&mut self) {
        self.history.loading = true;
        let app = self.app.clone();
        self.rt.spawn_blocking(move || {
            Msg::History(
                app.history_query(&HistoryFilter {
                    limit: Some(100),
                    ..Default::default()
                })
                .map_err(|e| e.to_string()),
            )
        });
    }

    pub fn clear_history(&mut self) {
        match self.app.history_clear(None) {
            Ok(()) => self.history.entries.clear(),
            Err(e) => self.history.error = Some(e.to_string()),
        }
    }

    // ---- environments ----

    pub fn set_env(&mut self, id: Option<String>) {
        self.ui.active_env_id = id;
    }

    // ---- tabs ----

    /// Opens a history entry as a new tab.
    pub fn open_history_entry(&mut self, e: &HistoryEntry) {
        let mut r = nexus_core::types::create_default_request();
        r.name = e.request.url.clone();
        r.url = e.request.url.clone();
        if let Ok(m) = nexus_core::http::parse_method(&e.request.method) {
            r.method = m;
        }
        self.tabs.open(r, None);
        self.ui.active_panel = Panel::Http;
    }

    pub fn open_collection_request(&mut self, dir: &str, req: &NexusRequest) {
        let origin = Origin {
            dir_name: dir.to_string(),
            request_id: req.id.clone(),
        };
        self.tabs.open(req.clone(), Some(origin));
        self.ui.active_panel = Panel::Http;
    }

    /// Saves the active tab back to its collection request.
    pub fn save_active_tab(&mut self) {
        let Some(t) = self.tabs.active() else { return };
        let Some(origin) = t.origin.clone() else {
            return;
        };
        let req = t.request.clone();
        if self
            .collections
            .update_request(&self.app, &origin.dir_name, &req)
        {
            if let Some(t) = self.tabs.active_mut() {
                t.dirty = false;
            }
        }
    }

    /// Adds the active tab's request to a collection and links the tab to it.
    pub fn save_active_tab_to(&mut self, dir: &str) {
        let Some(t) = self.tabs.active() else { return };
        let mut req = t.request.clone();
        if t.origin.is_some() {
            req.id = uuid::Uuid::new_v4().to_string();
        }
        if req.name == "New Request" && !req.url.is_empty() {
            req.name = req.url.clone();
        }
        if self.collections.add_request(&self.app, dir, &req) {
            if let Some(t) = self.tabs.active_mut() {
                t.request = req.clone();
                t.origin = Some(Origin {
                    dir_name: dir.to_string(),
                    request_id: req.id,
                });
                t.dirty = false;
            }
        }
    }

    // ---- send ----

    /// Variable scopes for a tab: active environment, the collection it belongs to, globals.
    pub fn scopes_for(&self, tab: &crate::state::RequestTab) -> Scopes {
        Scopes {
            environment: {
                let vars = self.envs.variables(self.ui.active_env_id.as_deref());
                if self.tab_untrusted(tab) {
                    nexus_core::trust::without_secrets(&vars)
                } else {
                    vars
                }
            },
            collection: self
                .collection_of(tab)
                .map(|c| c.variables.clone())
                .unwrap_or_default(),
            global: Vec::new(),
        }
    }

    /// Whether the tab belongs to a collection the user has not trusted.
    pub fn tab_untrusted(&self, tab: &crate::state::RequestTab) -> bool {
        tab.origin
            .as_ref()
            .is_some_and(|o| self.collections.is_untrusted(&o.dir_name))
    }

    /// The loaded collection a tab was opened from or saved to.
    pub fn collection_of(
        &self,
        tab: &crate::state::RequestTab,
    ) -> Option<&nexus_core::types::NexusCollection> {
        tab.origin
            .as_ref()
            .and_then(|o| self.collections.loaded.get(&o.dir_name))
    }

    pub fn send_active(&mut self) {
        if let Some(dir) = self
            .tabs
            .active()
            .and_then(|t| t.origin.as_ref().map(|o| o.dir_name.clone()))
        {
            self.collections.recheck_trust(&self.app, &dir);
        }
        let Some(t) = self.tabs.active() else { return };
        if t.loading {
            return;
        }
        let scopes = self.scopes_for(t);
        let collection = self.collection_of(t).cloned();
        let untrusted = self.tab_untrusted(t);
        let request = t.request.clone();
        let file_check = if untrusted {
            nexus_core::trust::check_body_files(&request, &t.picked_files)
        } else {
            Ok(())
        };
        let request_id = t
            .origin
            .as_ref()
            .map_or_else(|| t.request.id.clone(), |o| o.request_id.clone());
        let tab_id = t.id.clone();
        let app = self.app.clone();
        let line = format!("{} {}", t.request.method.as_str(), t.request.url);
        self.console.log(
            LogLevel::Info,
            LogSource::Http,
            format!("-> {line}"),
            Some(&tab_id),
        );
        let handle = self.rt.spawn(async move {
            // File reads (multipart / binary bodies) happen off the UI thread.
            let built = tokio::task::spawn_blocking(move || {
                file_check?;
                build_payload(&request, &request_id, &scopes, collection.as_ref())
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
            let payload = match built {
                Ok(mut p) => {
                    p.untrusted = untrusted;
                    p
                }
                Err(e) => {
                    return Msg::Http {
                        tab_id,
                        result: Err(e),
                    }
                }
            };
            let entry_req = HistoryRequest {
                method: payload.method.as_str().to_string(),
                url: payload.url.clone(),
                headers: payload.headers.clone(),
                body: payload.body.clone(),
            };
            let result = app.http_send(payload).await;
            let response = match &result {
                Ok(r) => HistoryResponse {
                    status: r.status,
                    status_text: r.status_text.clone(),
                    response_time: r.timing.total,
                    size: r.size,
                },
                Err(e) => HistoryResponse {
                    status: 0,
                    status_text: e.clone(),
                    response_time: 0.0,
                    size: 0,
                },
            };
            let entry = HistoryEntry {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                request: entry_req,
                response,
                collection_id: None,
                request_id: None,
            };
            let _ = app.history_log(&entry);
            // pretty-printing, line indexing and image decoding stay off the UI thread
            let result = match result {
                Ok(r) => tokio::task::spawn_blocking(move || Resp::from_http(r))
                    .await
                    .map(Box::new)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            Msg::Http { tab_id, result }
        });
        if let Some(t) = self.tabs.active_mut() {
            t.loading = true;
            t.response = None;
            t.abort = Some(handle);
        }
    }

    /// Opens the native file dialog for a form-data row or the binary body of the active tab.
    pub fn pick_file(&mut self, target: PickTarget) {
        let Some(tab_id) = self.tabs.active_id().map(str::to_string) else {
            return;
        };
        self.rt.spawn(async move {
            let path = rfd::AsyncFileDialog::new()
                .pick_file()
                .await
                .map(|f| f.path().to_path_buf());
            Msg::FilePicked {
                tab_id,
                target,
                path,
            }
        });
    }

    /// Stores a chosen file path into the tab's body.
    pub fn apply_picked_file(&mut self, tab_id: &str, target: PickTarget, path: &std::path::Path) {
        let Some(t) = self.tabs.by_id_mut(tab_id) else {
            return;
        };
        let p = path.to_string_lossy().into_owned();
        t.picked_files.insert(p.clone());
        let body = &mut t.request.body;
        match target {
            PickTarget::FormRow(i) => {
                if let Some(e) = body.form_data.as_mut().and_then(|f| f.get_mut(i)) {
                    e.value = p;
                }
            }
            PickTarget::Binary => {
                let ct = body.binary.as_ref().and_then(|b| b.content_type.clone());
                body.binary = Some(nexus_core::types::BinaryBody {
                    path: p,
                    content_type: ct,
                });
            }
        }
        t.dirty = true;
    }

    /// Asks where to save the active response's body (native dialog).
    pub fn save_response_dialog(&mut self) {
        let Some(t) = self.tabs.active() else { return };
        let Some(r) = &t.response else { return };
        let name = match &r.binary {
            Some(b) => format!("response.{}", crate::preview::ext_for(&b.content_type)),
            None => match r.format {
                nexus_core::types::ResponseFormat::Json => "response.json",
                nexus_core::types::ResponseFormat::Html => "response.html",
                nexus_core::types::ResponseFormat::Xml => "response.xml",
                _ => "response.txt",
            }
            .to_string(),
        };
        let tab_id = t.id.clone();
        self.rt.spawn(async move {
            let path = rfd::AsyncFileDialog::new()
                .set_file_name(name)
                .save_file()
                .await
                .map(|f| f.path().to_path_buf());
            Msg::SavePicked { tab_id, path }
        });
    }

    /// Writes the shown body of a tab's response (bytes for binary bodies) to `path`.
    pub fn save_response_to(&mut self, tab_id: &str, path: &std::path::Path) {
        let Some(r) = self
            .tabs
            .by_id_mut(tab_id)
            .and_then(|t| t.response.as_mut())
        else {
            return;
        };
        let data: &[u8] = match &r.binary {
            Some(b) => &b.bytes,
            None if r.view.mode == crate::state::response::ViewMode::Raw => r.raw.text.as_bytes(),
            None => r.pretty.text.as_bytes(),
        };
        r.view.notice = Some(match std::fs::write(path, data) {
            Ok(()) => format!("Saved to {}", path.display()),
            Err(e) => format!("Could not save: {e}"),
        });
    }

    /// Writes `bytes` to a private temp file and opens it with the system handler.
    pub fn open_temp(&mut self, prefix: &str, ext: &str, bytes: &[u8]) -> Result<(), String> {
        let path = crate::preview::write_temp(&self.home, prefix, ext, bytes)
            .map_err(|e| e.to_string())?;
        (self.opener)(&path)
    }

    pub fn toggle_console(&mut self) {
        self.ui.console_open = !self.ui.console_open;
    }

    pub fn cancel_active(&mut self) {
        let mut cancelled = None;
        if let Some(t) = self.tabs.active_mut() {
            if let Some(a) = t.abort.take() {
                a.abort();
            }
            if t.loading {
                t.loading = false;
                t.response = Some(Resp::cancelled());
                cancelled = Some(t.id.clone());
            }
        }
        if let Some(id) = cancelled {
            self.console.log(
                LogLevel::Warn,
                LogSource::Http,
                "Request cancelled",
                Some(&id),
            );
        }
    }

    fn poll(&mut self) {
        for msg in self.rt.drain() {
            match msg {
                Msg::Http { tab_id, result } => {
                    let resp = match result {
                        Ok(done) => {
                            let (resp, logs) = *done;
                            self.console.add_script_logs(&logs, Some(&tab_id));
                            resp
                        }
                        Err(e) => Resp::from_error(e),
                    };
                    let level = match (&resp.error, resp.status) {
                        (Some(_), _) | (_, 500..) => LogLevel::Error,
                        (_, 400..=499) => LogLevel::Warn,
                        _ => LogLevel::Info,
                    };
                    self.console.log(
                        level,
                        LogSource::Http,
                        format!("<- {}", resp.summary()),
                        Some(&tab_id),
                    );
                    let mut resp = resp;
                    if let Some(q) = &self.opts.search {
                        resp.view.search_open = true;
                        resp.view.search = q.clone();
                    }
                    match self.opts.view_mode.as_deref() {
                        Some("raw") => resp.view.mode = crate::state::response::ViewMode::Raw,
                        Some("preview") => {
                            resp.view.mode = crate::state::response::ViewMode::Preview
                        }
                        _ => {}
                    }
                    if let Some(t) = self.tabs.by_id_mut(&tab_id) {
                        t.loading = false;
                        t.abort = None;
                        t.response = Some(resp);
                    }
                    self.refresh_history();
                }
                Msg::SavePicked { tab_id, path } => {
                    if let Some(p) = path {
                        self.save_response_to(&tab_id, &p);
                    }
                }
                Msg::FilePicked {
                    tab_id,
                    target,
                    path,
                } => {
                    if let Some(p) = path {
                        self.apply_picked_file(&tab_id, target, &p);
                    }
                }
                Msg::Runner { run, event } => self.on_runner_event(run, event),
                Msg::RunnerDone { run, summary } => self.on_runner_done(run, *summary),
                Msg::DataFile { path, result } => self.on_data_file(path, result),
                Msg::DataPicked(path) => {
                    if let Some(p) = path {
                        self.runner_load_data(&p);
                    }
                }
                Msg::Ws { conn, event } => {
                    if let Some(s) = self.ws.by_conn(&conn) {
                        s.apply(event);
                    }
                }
                Msg::Gql(m) => self.on_gql(m),
                Msg::Mcp(m) => self.on_mcp(m),
                Msg::ImportPicked(p) => {
                    if let Some(p) = p {
                        self.import_load_file(&p);
                    }
                }
                Msg::Imported { source, result } => self.on_imported(source, result),
                Msg::ExportPicked { what, fmt, path } => {
                    if let Some(p) = path {
                        self.export_to(&what, fmt, &p);
                    }
                }
                Msg::DocsPicked(p) => {
                    if let Some(p) = p {
                        self.docs_save_to(&p);
                    }
                }
                Msg::History(r) => {
                    self.history.loading = false;
                    match r {
                        Ok(e) => {
                            self.history.entries = e;
                            self.history.error = None;
                        }
                        Err(e) => self.history.error = Some(e),
                    }
                }
            }
        }
    }

    // ---- frame ----

    /// One frame of the whole UI. Called by the eframe `ui` hook and by the kittest harness.
    pub fn show(&mut self, ui: &mut Ui) {
        if self.frame_no == 0 {
            match self.opts.theme.as_deref() {
                Some("light") => self.ui.theme = crate::theme::ThemeMode::Light,
                Some("dark") => self.ui.theme = crate::theme::ThemeMode::Dark,
                Some("system") => self.ui.theme = crate::theme::ThemeMode::System,
                _ => {}
            }
        }
        self.frame_no += 1;
        let ctx = ui.ctx().clone();
        self.feed_scripted_input(&ctx);
        self.rt.set_ctx(&ctx);
        crate::theme::sync_theme(&ctx, self.ui.theme, &mut self.styled);
        self.poll();
        self.shortcuts(&ctx);
        if self.opts.autosend && !self.sent_auto && self.frame_no > 2 {
            self.sent_auto = true;
            self.send_active();
        }
        self.screenshot_hook(&ctx);

        // title bar
        egui::Panel::top("titlebar")
            .exact_size(38.0)
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show(ui, |ui| self.title_bar(ui));

        // sidebar
        if !self.ui.sidebar_collapsed {
            let r = egui::Panel::left("sidebar")
                .default_size(self.ui.sidebar_width)
                .size_range(MIN_SIDEBAR..=MAX_SIDEBAR)
                .resizable(true)
                .frame(egui::Frame::NONE.fill(bg_surface()))
                .show_separator_line(false)
                .show(ui, |ui| {
                    self.sidebar(ui);
                    let r = ui.max_rect();
                    ui.painter().vline(
                        r.right() - 0.5,
                        r.y_range(),
                        egui::Stroke::new(1.0, border()),
                    );
                });
            let w = r.response.rect.width().clamp(MIN_SIDEBAR, MAX_SIDEBAR);
            if (w - self.ui.sidebar_width).abs() > 0.5 {
                self.ui.sidebar_width = w;
            }
        }

        // main
        egui::CentralPanel::no_frame().show(ui, |ui| {
            ui.painter().rect_filled(ui.max_rect(), 0, bg_app());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            if self.ui.console_open {
                let r = egui::Panel::bottom("console")
                    .default_size(self.ui.console_height)
                    .size_range(MIN_CONSOLE..=MAX_CONSOLE)
                    .resizable(true)
                    .frame(egui::Frame::NONE.fill(bg_surface()))
                    .show_separator_line(false)
                    .show(ui, |ui| self.console_panel(ui));
                let h = r.response.rect.height().clamp(MIN_CONSOLE, MAX_CONSOLE);
                if (h - self.ui.console_height).abs() > 0.5 {
                    self.ui.console_height = h;
                }
            }
            self.toolbar(ui);
            match self.ui.active_panel {
                Panel::Http => self.http_panel(ui),
                Panel::Graphql => self.graphql_panel(ui),
                Panel::Ws => self.ws_panel(ui),
                Panel::Mcp => self.mcp_panel(ui),
                Panel::Runner => self.runner_panel(ui),
            }
        });

        self.dialogs(&ctx);

        if self.last_save.elapsed() > Duration::from_millis(500) {
            self.save_ui_state();
        }
    }

    /// Dev flag `--input`: one scripted item per frame, after the first layout.
    fn feed_scripted_input(&mut self, ctx: &egui::Context) {
        if self.opts.focus_editor && self.frame_no == 6 {
            crate::widgets::focus_next_editor(ctx);
        }
        if self.frame_no < 10 {
            return;
        }
        let Some(item) = self.input_queue.pop_front() else {
            return;
        };
        #[cfg(debug_assertions)]
        match self.dev_item(&item) {
            crate::dev::Dev::Done => {
                ctx.request_repaint();
                return;
            }
            crate::dev::Dev::Wait => {
                self.input_queue.push_front(item);
                ctx.request_repaint_after(Duration::from_millis(30));
                return;
            }
            crate::dev::Dev::No => {}
        }
        let events = parse_input(&item);
        ctx.input_mut(|i| i.events.extend(events));
        ctx.request_repaint();
    }

    fn screenshot_hook(&mut self, ctx: &egui::Context) {
        let Some(path) = self.opts.screenshot.clone() else {
            return;
        };
        ctx.request_repaint();
        let pending = self.tabs.active().is_some_and(|t| t.loading);
        let waiting_response =
            self.opts.autosend && self.tabs.active().is_none_or(|t| t.response.is_none());
        let ready =
            self.frame_no >= 20 && !pending && !waiting_response && self.input_queue.is_empty();
        if ready && self.frame_no.is_multiple_of(5) && !self.shot_requested {
            let ppp = ctx.pixels_per_point();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                egui::ScreenshotCallback::new(move |img| {
                    let [w, h] = img.size;
                    if let Some(buf) =
                        image::RgbaImage::from_raw(w as u32, h as u32, img.as_raw().to_vec())
                    {
                        buf.save(&path).expect("save png");
                        eprintln!("screenshot {w}x{h} ppp={ppp} -> {path}");
                    }
                    std::process::exit(0);
                }),
            ));
            self.shot_requested = true;
        }
    }
}

impl eframe::App for NexusApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_ui_state();
        let app = self.app.clone();
        self.rt.handle().block_on(app.mcp_disconnect_all());
    }
}

fn folder_ids(items: &[nexus_core::types::CollectionItem]) -> Vec<String> {
    use nexus_core::types::CollectionItem;
    let mut out = Vec::new();
    for i in items {
        if let CollectionItem::Folder(f) = i {
            out.push(f.id.clone());
            out.extend(folder_ids(&f.items));
        }
    }
    out
}

fn find_request(items: &[nexus_core::types::CollectionItem], name: &str) -> Option<NexusRequest> {
    use nexus_core::types::CollectionItem;
    items.iter().find_map(|i| match i {
        CollectionItem::Request(r) if r.name == name => Some(r.clone()),
        CollectionItem::Folder(f) => find_request(&f.items, name),
        _ => None,
    })
}

fn first_request(items: &[nexus_core::types::CollectionItem]) -> Option<&NexusRequest> {
    use nexus_core::types::CollectionItem;
    for i in items {
        match i {
            CollectionItem::Request(r) => return Some(r),
            CollectionItem::Folder(f) => {
                if let Some(r) = first_request(&f.items) {
                    return Some(r);
                }
            }
        }
    }
    None
}

/// `text:abc\n` / `key:ctrl+shift+f` -> egui events (dev flag `--input`).
fn parse_input(item: &str) -> Vec<egui::Event> {
    if let Some(t) = item.strip_prefix("text:") {
        return vec![egui::Event::Text(t.replace("\\n", "\n"))];
    }
    let Some(combo) = item.strip_prefix("key:") else {
        return Vec::new();
    };
    let mut mods = egui::Modifiers::NONE;
    let mut key = None;
    for part in combo.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" => {
                mods.ctrl = true;
                mods.command = true;
            }
            "shift" => mods.shift = true,
            "alt" => mods.alt = true,
            _ => key = egui::Key::from_name(part),
        }
    }
    key.map(|key| egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: mods,
    })
    .into_iter()
    .collect()
}
