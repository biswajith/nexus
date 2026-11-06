//! Keyboard shortcuts, the
//! help list, and the command list of the palette.

use crate::app::NexusApp;
use crate::state::dialogs::{CmdAction, Command, Dialog, ImportTab};
use crate::state::Panel;
use crate::theme::ThemeMode;
use eframe::egui::{self, Key, Modifiers};
use nexus_core::types::CollectionItem;

/// Name of the primary modifier in the UI.
pub const MOD: &str = if cfg!(target_os = "macos") {
    "Cmd+"
} else {
    "Ctrl+"
};

fn k(rest: &str) -> String {
    format!("{MOD}{rest}")
}

/// Shortcut groups for the help overlay: `(group, [(keys, description)])`.
pub fn help_groups() -> Vec<(&'static str, Vec<(String, &'static str)>)> {
    vec![
        (
            "Request",
            vec![
                (k("Enter"), "Send the request"),
                (k("S"), "Save the request"),
                (k("N"), "New request tab"),
                (k("T"), "New request tab"),
                (k("W"), "Close the tab"),
                (k("L"), "Focus the URL bar"),
            ],
        ),
        (
            "Navigation",
            vec![
                (k("K"), "Command palette"),
                (k("1"), "HTTP"),
                (k("2"), "GraphQL"),
                (k("3"), "WebSocket"),
                (k("4"), "MCP"),
                (k("5"), "Runner"),
                (k("Shift+E"), "Focus environments"),
            ],
        ),
        (
            "View",
            vec![
                (k("B"), "Toggle the sidebar"),
                (k("J"), "Toggle the console"),
                (k("F"), "Find in the response / editor"),
                ("F1".into(), "This help (or ?)"),
                ("Esc".into(), "Close a dialog"),
            ],
        ),
        (
            "Editor",
            vec![
                (k("Space"), "Autocomplete"),
                (k("Z"), "Undo"),
                (k("Shift+Z"), "Redo"),
                (k("/"), "Toggle comment"),
                ("Alt+Shift+F".into(), "Format JSON"),
                ("Esc, then Tab".into(), "Leave the editor"),
            ],
        ),
    ]
}

impl NexusApp {
    /// Opens a dialog (closing any other) and prepares its state.
    pub fn open_dialog(&mut self, d: Dialog) {
        match d {
            Dialog::Palette => {
                // every collection is searched, not just the expanded ones
                let dirs: Vec<String> = self
                    .collections
                    .metas
                    .iter()
                    .map(crate::state::collections::dir_name)
                    .filter(|d| !self.collections.loaded.contains_key(d))
                    .collect();
                for d in dirs {
                    self.collections.load(&self.app, &d);
                }
                if self.history.entries.is_empty() {
                    self.refresh_history();
                }
                let cmds = self.build_commands();
                self.dlg.palette.open(cmds);
            }
            Dialog::Import => {
                let st = &mut self.dlg.import;
                st.error = None;
                st.done = None;
                st.export_notice = None;
            }
            Dialog::Docs => {
                self.dlg.docs.notice = None;
                self.dlg.docs.error = None;
                if self.dlg.docs.dir.is_none() {
                    self.dlg.docs.dir = self
                        .collections
                        .metas
                        .first()
                        .map(crate::state::collections::dir_name);
                }
            }
            Dialog::Envs => {
                self.dlg.envs = Default::default();
            }
            Dialog::Help => {}
        }
        self.dialog = Some(d);
    }

    pub fn close_dialog(&mut self) {
        self.dialog = None;
    }

    pub fn cycle_theme(&mut self) {
        self.ui.theme = self.ui.theme.next();
    }

    /// The palette's command list: actions, panels, themes, environments, requests, history.
    pub fn build_commands(&self) -> Vec<Command> {
        let mut v = Vec::new();
        let mut add = |label: &str, cat: &'static str, key: Option<String>, action: CmdAction| {
            v.push(Command {
                label: label.to_string(),
                detail: String::new(),
                category: cat,
                shortcut: key,
                action,
            });
        };
        add(
            "New Request",
            "Request",
            Some(k("N")),
            CmdAction::NewRequest,
        );
        add("Send Request", "Request", Some(k("Enter")), CmdAction::Send);
        add("Save Request", "Request", Some(k("S")), CmdAction::Save);
        add("Close Tab", "Request", Some(k("W")), CmdAction::CloseTab);
        add(
            "Focus URL Bar",
            "Request",
            Some(k("L")),
            CmdAction::FocusUrl,
        );
        for (i, p) in Panel::ALL.into_iter().enumerate() {
            let name = match p {
                Panel::Http => "HTTP",
                Panel::Graphql => "GraphQL",
                Panel::Ws => "WebSocket",
                Panel::Mcp => "MCP Tester",
                Panel::Runner => "Runner",
            };
            add(
                &format!("Switch to {name}"),
                "Navigation",
                Some(k(&(i + 1).to_string())),
                CmdAction::Panel(p),
            );
        }
        add(
            "Toggle Sidebar",
            "View",
            Some(k("B")),
            CmdAction::ToggleSidebar,
        );
        add(
            "Toggle Console",
            "View",
            Some(k("J")),
            CmdAction::ToggleConsole,
        );
        add(
            "Keyboard Shortcuts",
            "View",
            Some("F1".into()),
            CmdAction::Open(Dialog::Help),
        );
        add(
            "Import...",
            "Actions",
            None,
            CmdAction::Open(Dialog::Import),
        );
        add("Export...", "Actions", None, CmdAction::OpenExport);
        add(
            "Generate API Docs",
            "Actions",
            None,
            CmdAction::Open(Dialog::Docs),
        );
        add(
            "Manage Environments",
            "Actions",
            None,
            CmdAction::Open(Dialog::Envs),
        );
        for m in [ThemeMode::Dark, ThemeMode::Light, ThemeMode::System] {
            add(
                &format!("Theme: {}", m.label()),
                "Settings",
                None,
                CmdAction::Theme(m),
            );
        }
        add(
            "Environment: None",
            "Environment",
            None,
            CmdAction::SetEnv(None),
        );
        for e in &self.envs.environments {
            add(
                &format!("Environment: {}", e.name),
                "Environment",
                None,
                CmdAction::SetEnv(Some(e.id.clone())),
            );
        }
        // requests of every collection
        for meta in &self.collections.metas {
            let dir = crate::state::collections::dir_name(meta);
            if let Some(c) = self.collections.loaded.get(&dir) {
                collect_requests(&c.items, &meta.name, &dir, &mut v);
            }
        }
        for h in self.history.entries.iter().take(50) {
            v.push(Command {
                label: format!("{} {}", h.request.method, h.request.url),
                detail: format!("{}", h.response.status),
                category: "History",
                shortcut: None,
                action: CmdAction::OpenHistory(Box::new(h.clone())),
            });
        }
        v
    }

    pub fn run_command(&mut self, action: CmdAction) {
        match action {
            CmdAction::NewRequest => {
                self.ui.active_panel = Panel::Http;
                self.tabs.open_blank();
            }
            CmdAction::Send => {
                self.ui.active_panel = Panel::Http;
                self.send_active();
            }
            CmdAction::Save => self.save_active_tab(),
            CmdAction::CloseTab => {
                if let Some(id) = self.tabs.active_id().map(str::to_string) {
                    self.tabs.close(&id);
                }
            }
            CmdAction::FocusUrl => {
                self.ui.active_panel = Panel::Http;
                self.view.focus_url = true;
            }
            CmdAction::Panel(p) => self.ui.active_panel = p,
            CmdAction::ToggleSidebar => self.ui.sidebar_collapsed = !self.ui.sidebar_collapsed,
            CmdAction::ToggleConsole => self.toggle_console(),
            CmdAction::Theme(m) => self.ui.theme = m,
            CmdAction::Open(d) => self.open_dialog(d),
            CmdAction::OpenExport => {
                self.open_dialog(Dialog::Import);
                self.dlg.import.tab = ImportTab::Export;
            }
            CmdAction::SetEnv(id) => self.set_env(id),
            CmdAction::OpenRequest { dir, request } => {
                self.open_collection_request(&dir, &request);
            }
            CmdAction::OpenHistory(e) => self.open_history_entry(&e),
        }
    }

    /// Global shortcuts. Runs before the panels so a shortcut wins over a focused widget.
    pub(crate) fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let pressed = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        if pressed(cmd, Key::K) {
            if self.dialog == Some(Dialog::Palette) {
                self.close_dialog();
            } else {
                self.open_dialog(Dialog::Palette);
            }
            return;
        }
        // Ctrl+/ toggles comments inside an editor, so the text shortcuts only apply outside text
        // fields; F1 works everywhere.
        let typing = ctx.egui_wants_keyboard_input();
        let help = pressed(Modifiers::NONE, Key::F1)
            || (!typing
                && (pressed(cmd, Key::Slash)
                    || ctx.input(|i| {
                        i.events
                            .iter()
                            .any(|e| matches!(e, egui::Event::Text(t) if t == "?"))
                    })));
        if help && self.dialog.is_none() {
            self.open_dialog(Dialog::Help);
            return;
        }
        if self.dialog.is_some() {
            return;
        }
        if pressed(cmd, Key::S) {
            self.save_active_tab();
        }
        if pressed(cmd, Key::W) {
            self.run_command(CmdAction::CloseTab);
        }
        if pressed(cmd, Key::T) || pressed(cmd, Key::N) {
            self.run_command(CmdAction::NewRequest);
        }
        if pressed(cmd, Key::J) {
            self.toggle_console();
        }
        if pressed(cmd, Key::B) {
            self.ui.sidebar_collapsed = !self.ui.sidebar_collapsed;
        }
        if pressed(cmd, Key::L) {
            self.run_command(CmdAction::FocusUrl);
        }
        if pressed(cmd | Modifiers::SHIFT, Key::E) {
            self.view.focus_env = true;
        }
        if self.ui.active_panel == Panel::Http && pressed(cmd, Key::Enter) {
            self.send_active();
        }
        for (i, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5]
            .into_iter()
            .enumerate()
        {
            if pressed(cmd, key) {
                self.ui.active_panel = Panel::ALL[i];
            }
        }
    }
}

fn collect_requests(items: &[CollectionItem], path: &str, dir: &str, out: &mut Vec<Command>) {
    for i in items {
        match i {
            CollectionItem::Request(r) => out.push(Command {
                label: format!("{} {}", r.method.as_str(), r.name),
                detail: path.to_string(),
                category: "Request",
                shortcut: None,
                action: CmdAction::OpenRequest {
                    dir: dir.to_string(),
                    request: Box::new(r.clone()),
                },
            }),
            CollectionItem::Folder(f) => {
                collect_requests(&f.items, &format!("{path} / {}", f.name), dir, out);
            }
        }
    }
}
