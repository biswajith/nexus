//! Persistent UI state (`~/.nexus/ui-state.json`): layout, selected environment, open tabs.

use super::forms::PanelForms;
use super::tabs::{Origin, RequestTab, Tabs};
use crate::theme::ThemeMode;
use nexus_core::fs::{write_private_file, WriteMode};
use nexus_core::types::NexusRequest;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarTab {
    #[default]
    Collections,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Panel {
    #[default]
    Http,
    Graphql,
    Ws,
    Mcp,
    Runner,
}

impl Panel {
    pub const ALL: [Panel; 5] = [
        Panel::Http,
        Panel::Graphql,
        Panel::Ws,
        Panel::Mcp,
        Panel::Runner,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Panel::Http => "HTTP",
            Panel::Graphql => "GraphQL",
            Panel::Ws => "WS",
            Panel::Mcp => "MCP",
            Panel::Runner => "Runner",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedTab {
    pub request: NexusRequest,
    pub origin: Option<Origin>,
    #[serde(default)]
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiState {
    pub sidebar_tab: SidebarTab,
    pub sidebar_width: f32,
    pub sidebar_collapsed: bool,
    pub active_panel: Panel,
    pub active_env_id: Option<String>,
    /// Expanded collection ids and folder ids.
    pub expanded: Vec<String>,
    pub tabs: Vec<SavedTab>,
    pub active_tab_id: Option<String>,
    /// Share of the HTTP panel height taken by the request builder.
    pub split: f32,
    pub console_open: bool,
    pub console_height: f32,
    pub theme: ThemeMode,
    /// Runner / WebSocket / GraphQL / MCP form fields (filled from the live panels on save).
    pub forms: PanelForms,
    /// Share of the panel width taken by the first column in each panel's internal split.
    pub splits: PanelSplits,
}

/// Internal splitter positions of the non-HTTP panels (fraction of the panel size).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelSplits {
    pub runner: f32,
    pub ws: f32,
    pub graphql: f32,
    pub mcp: f32,
}

impl Default for PanelSplits {
    fn default() -> Self {
        Self {
            runner: 0.4,
            ws: 0.6,
            graphql: 0.5,
            mcp: 0.35,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            sidebar_tab: SidebarTab::Collections,
            sidebar_width: 300.0,
            sidebar_collapsed: false,
            active_panel: Panel::Http,
            active_env_id: None,
            expanded: Vec::new(),
            tabs: Vec::new(),
            active_tab_id: None,
            split: DEFAULT_SPLIT,
            console_open: false,
            console_height: 220.0,
            theme: ThemeMode::Dark,
            forms: PanelForms::default(),
            splits: PanelSplits::default(),
        }
    }
}

pub const DEFAULT_SPLIT: f32 = 0.45;
pub const MIN_SPLIT: f32 = 0.15;
pub const MAX_SPLIT: f32 = 0.8;
pub const MIN_CONSOLE: f32 = 80.0;
pub const MAX_CONSOLE: f32 = 700.0;
pub const MIN_SIDEBAR: f32 = 200.0;
pub const MAX_SIDEBAR: f32 = 600.0;

impl UiState {
    pub fn path(home: &Path) -> PathBuf {
        home.join("ui-state.json")
    }

    /// Loads the saved state; a missing or unreadable file gives the defaults.
    pub fn load(home: &Path) -> Self {
        let mut s: Self = std::fs::read_to_string(Self::path(home))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        s.sidebar_width = s.sidebar_width.clamp(MIN_SIDEBAR, MAX_SIDEBAR);
        for f in [
            &mut s.splits.runner,
            &mut s.splits.ws,
            &mut s.splits.graphql,
            &mut s.splits.mcp,
        ] {
            *f = if f.is_finite() {
                f.clamp(0.15, 0.85)
            } else {
                0.5
            };
        }
        s.split = if s.split.is_finite() {
            s.split.clamp(MIN_SPLIT, MAX_SPLIT)
        } else {
            DEFAULT_SPLIT
        };
        s.console_height = if s.console_height.is_finite() {
            s.console_height.clamp(MIN_CONSOLE, MAX_CONSOLE)
        } else {
            220.0
        };
        s
    }

    /// Snapshot of the persisted parts of the live state, as pretty JSON.
    pub fn snapshot(
        &self,
        tabs: &Tabs,
        expanded: &std::collections::HashSet<String>,
        forms: PanelForms,
    ) -> String {
        let mut s = self.clone();
        s.forms = forms;
        let mut ex: Vec<String> = expanded.iter().cloned().collect();
        ex.sort();
        s.expanded = ex;
        s.tabs = tabs.tabs.iter().map(RequestTab::to_saved).collect();
        s.active_tab_id = tabs.active_id().map(str::to_string);
        serde_json::to_string_pretty(&s).unwrap_or_default()
    }

    pub fn write(home: &Path, json: &str) -> std::io::Result<()> {
        write_private_file(&Self::path(home), json, WriteMode::Replace)
    }
}
