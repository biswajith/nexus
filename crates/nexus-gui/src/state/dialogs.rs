//! State of the modal dialogs (Import/Export, Docs, Environment manager, command palette,
//! shortcut help) and the pure logic behind them (fuzzy matching, command list).

use super::ui_state::Panel;
use crate::theme::ThemeMode;
use crate::widgets::TextView;
use nexus_core::import::{ImportFormat, ImportResult};
use nexus_core::types::{HistoryEntry, NexusRequest, Variable};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialog {
    Import,
    Docs,
    Envs,
    Palette,
    Help,
}

// ---- import / export ----

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ImportTab {
    #[default]
    Import,
    Export,
}

/// A parsed import file, ready for preview.
pub struct Parsed {
    pub format: ImportFormat,
    pub result: ImportResult,
}

#[derive(Default)]
pub struct ImportState {
    pub tab: ImportTab,
    /// Text pasted into the box (cURL, or any supported format).
    pub paste: String,
    /// Name of the file / "Pasted text" the preview comes from.
    pub source: Option<String>,
    pub parsed: Option<Parsed>,
    pub error: Option<String>,
    pub loading: bool,
    /// Status lines after a successful import.
    pub done: Option<Vec<String>>,
    /// Outcome of the last export: `Ok(message)` / `Err(message)`.
    pub export_notice: Option<Result<String, String>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportFmt {
    Nexus,
    Postman,
}

impl ExportFmt {
    pub fn label(self) -> &'static str {
        match self {
            ExportFmt::Nexus => "Nexus JSON",
            ExportFmt::Postman => "Postman v2.1",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExportWhat {
    /// Collection directory name.
    Collection(String),
    /// Environment id.
    Environment(String),
}

/// `Name` -> `Name` with anything but `[A-Za-z0-9-_]` replaced by `_`.
pub fn safe_file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "export".into()
    } else {
        s
    }
}

impl Parsed {
    /// Reads and parses an import file (blocking).
    pub fn from_file(path: &std::path::Path) -> Result<Parsed, String> {
        crate::dialog_ops::load_import_file(path)
    }
}

pub fn import_format_label(f: ImportFormat) -> &'static str {
    match f {
        ImportFormat::PostmanCollectionV21 => "Postman Collection",
        ImportFormat::PostmanEnvironment => "Postman Environment",
        ImportFormat::Openapi3 => "OpenAPI 3.x",
        ImportFormat::Swagger2 => "Swagger 2.0",
        ImportFormat::Curl => "cURL",
        ImportFormat::Nexus => "Nexus",
    }
}

// ---- docs ----

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DocFormat {
    #[default]
    Markdown,
    Html,
}

impl DocFormat {
    pub fn label(self) -> &'static str {
        match self {
            DocFormat::Markdown => "Markdown",
            DocFormat::Html => "HTML",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            DocFormat::Markdown => "md",
            DocFormat::Html => "html",
        }
    }
}

#[derive(Default)]
pub struct DocsState {
    /// Directory name of the chosen collection.
    pub dir: Option<String>,
    pub format: DocFormat,
    /// Generated text.
    pub content: Option<String>,
    pub view: TextView,
    pub error: Option<String>,
    pub notice: Option<String>,
}

// ---- environment manager ----

pub struct EnvDraft {
    pub id: String,
    pub name: String,
    pub vars: Vec<Variable>,
    pub dirty: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct EnvMgrState {
    pub draft: Option<EnvDraft>,
    /// Inline "new environment" input.
    pub creating: Option<String>,
    /// Inline rename: (environment id, buffer).
    pub renaming: Option<(String, String)>,
    pub confirm_delete: Option<String>,
    /// Variable rows shown in clear text although they are secret.
    pub revealed: HashSet<usize>,
    pub error: Option<String>,
}

// ---- command palette ----

#[derive(Clone, Debug)]
pub enum CmdAction {
    NewRequest,
    Send,
    Save,
    CloseTab,
    FocusUrl,
    Panel(Panel),
    ToggleSidebar,
    ToggleConsole,
    Theme(ThemeMode),
    Open(Dialog),
    OpenExport,
    SetEnv(Option<String>),
    OpenRequest {
        dir: String,
        request: Box<NexusRequest>,
    },
    OpenHistory(Box<HistoryEntry>),
}

#[derive(Clone, Debug)]
pub struct Command {
    pub label: String,
    /// Secondary text (collection path, URL...).
    pub detail: String,
    pub category: &'static str,
    pub shortcut: Option<String>,
    pub action: CmdAction,
}

impl Command {
    pub fn accessible(&self) -> String {
        if self.detail.is_empty() {
            format!("{}, {}", self.label, self.category)
        } else {
            format!("{}, {}, {}", self.label, self.detail, self.category)
        }
    }
}

#[derive(Default)]
pub struct PaletteState {
    pub query: String,
    pub selected: usize,
    pub commands: Vec<Command>,
    /// Indices into `commands` of the current results, best first.
    pub results: Vec<usize>,
    pub last_query: Option<String>,
    /// The list must scroll to the selected row.
    pub scroll_to_selected: bool,
}

impl PaletteState {
    pub fn open(&mut self, commands: Vec<Command>) {
        self.query.clear();
        self.selected = 0;
        self.commands = commands;
        self.last_query = None;
        self.scroll_to_selected = true;
        self.refilter();
    }

    /// Recomputes the results when the query changed.
    pub fn refilter(&mut self) {
        if self.last_query.as_deref() == Some(self.query.as_str()) {
            return;
        }
        self.last_query = Some(self.query.clone());
        self.results = filter(&self.commands, &self.query);
        self.selected = 0;
        self.scroll_to_selected = true;
    }

    pub fn current(&self) -> Option<&Command> {
        self.results
            .get(self.selected)
            .and_then(|&i| self.commands.get(i))
    }

    pub fn step(&mut self, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let n = self.results.len() as isize;
        self.selected = (self.selected as isize + delta).clamp(0, n - 1) as usize;
        self.scroll_to_selected = true;
    }
}

pub const MAX_RESULTS: usize = 60;

/// Commands matching `query`, best first (all of them in their given order for a blank query).
pub fn filter(commands: &[Command], query: &str) -> Vec<usize> {
    let q = query.trim();
    if q.is_empty() {
        return (0..commands.len().min(MAX_RESULTS)).collect();
    }
    let mut scored: Vec<(i32, usize)> = commands
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let hay = format!("{} {}", c.label, c.category);
            let s = fuzzy(q, &c.label)
                .map(|s| s + 20)
                .or_else(|| fuzzy(q, &hay))
                .or_else(|| fuzzy(q, &c.detail).map(|s| s - 10))?;
            Some((s, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(MAX_RESULTS)
        .map(|(_, i)| i)
        .collect()
}

/// Fuzzy subsequence match of every whitespace-separated token of `query` in `text`
/// (case-insensitive). `None` when some token does not match; higher is better. Consecutive
/// characters and word starts score extra, gaps cost a little.
pub fn fuzzy(query: &str, text: &str) -> Option<i32> {
    let hay: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let mut total = 0;
    let mut any = false;
    for tok in query.split_whitespace() {
        any = true;
        total += fuzzy_token(
            &tok.chars().flat_map(char::to_lowercase).collect::<Vec<_>>(),
            &hay,
        )?;
    }
    any.then_some(total)
}

fn fuzzy_token(q: &[char], hay: &[char]) -> Option<i32> {
    // Every character after the first must continue the previous match or start a word, so
    // "pet" finds "List pets" but not "Development".
    let word_start = |i: usize| i == 0 || !hay[i - 1].is_alphanumeric();
    'start: for first in 0..hay.len() {
        if hay[first] != q[0] {
            continue;
        }
        let mut score =
            10 + if word_start(first) { 12 } else { 0 } + if first == 0 { 8 } else { 0 };
        let mut prev = first;
        for &c in &q[1..] {
            let next = if hay.get(prev + 1) == Some(&c) {
                prev + 1
            } else if let Some(i) = (prev + 2..hay.len()).find(|&i| hay[i] == c && word_start(i)) {
                score -= ((i - prev - 1) as i32).min(10);
                score += 12;
                i
            } else {
                continue 'start;
            };
            if next == prev + 1 {
                score += 15;
            }
            score += 10;
            prev = next;
        }
        return Some(score);
    }
    None
}

#[derive(Default)]
pub struct Dialogs {
    pub import: ImportState,
    pub docs: DocsState,
    pub envs: EnvMgrState,
    pub palette: PaletteState,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(label: &str, category: &'static str) -> Command {
        Command {
            label: label.into(),
            detail: String::new(),
            category,
            shortcut: None,
            action: CmdAction::NewRequest,
        }
    }

    #[test]
    fn fuzzy_needs_all_chars_in_order() {
        assert!(fuzzy("gus", "Get users").is_some());
        assert!(fuzzy("xyz", "Get users").is_none());
        assert!(fuzzy("sug", "Get users").is_none());
        assert!(fuzzy("pet", "Environment: Development").is_none());
        assert!(fuzzy("pet", "List pets").is_some());
        assert!(fuzzy("", "x").is_none());
    }

    #[test]
    fn fuzzy_prefers_prefix_and_consecutive() {
        let prefix = fuzzy("get", "Get users").unwrap();
        let scattered = fuzzy("get", "Forget it").unwrap();
        assert!(prefix > scattered);
        // multi-word queries match each token separately
        assert!(fuzzy("us get", "Get users").is_some());
    }

    #[test]
    fn filter_orders_by_score_and_keeps_all_for_blank() {
        let cmds = vec![
            cmd("Toggle sidebar", "View"),
            cmd("Switch to GraphQL", "Navigation"),
            cmd("New request", "Request"),
        ];
        assert_eq!(filter(&cmds, "").len(), 3);
        let r = filter(&cmds, "graph");
        assert_eq!(r, vec![1]);
        let r = filter(&cmds, "nr");
        assert_eq!(r[0], 2);
    }

    #[test]
    fn export_names_are_sanitised() {
        assert_eq!(safe_file_stem("My API / v2"), "My_API___v2");
        assert_eq!(safe_file_stem(""), "export");
    }
}
