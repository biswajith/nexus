//! Dev flag `--input "do:...|wait:..."`: scripted app commands for screenshots (run one per
//! frame; a `wait:` item blocks the script until its condition holds). Not used by the tests.

use crate::app::NexusApp;
use crate::state::graphql::ResultTab;
use crate::state::mcp::McpStatus;
use crate::state::runner::RunStatus;
use crate::state::ui_state::Panel;
use crate::state::ws::WsRow;

pub(crate) enum Dev {
    /// Not a dev item.
    No,
    Done,
    /// The condition does not hold yet: ask again next frame.
    Wait,
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
}

impl NexusApp {
    pub(crate) fn dev_item(&mut self, item: &str) -> Dev {
        if let Some(c) = item.strip_prefix("wait:") {
            return if self.dev_cond(c) {
                Dev::Done
            } else {
                Dev::Wait
            };
        }
        let Some(cmd) = item.strip_prefix("do:") else {
            return Dev::No;
        };
        let (key, val) = cmd.split_once('=').unwrap_or((cmd, ""));
        let val = unescape(val);
        match key {
            "panel" => {
                self.ui.active_panel = match val.as_str() {
                    "graphql" => Panel::Graphql,
                    "ws" => Panel::Ws,
                    "mcp" => Panel::Mcp,
                    "runner" => Panel::Runner,
                    _ => Panel::Http,
                }
            }
            "ws.url" => self.ws.active_mut().url = val,
            "ws.connect" => self.ws_connect(),
            "ws.draft" => self.ws.active_mut().draft = val,
            "ws.send" => self.ws_send(),
            "ws.select" => {
                let s = self.ws.active_mut();
                let id = s
                    .rows
                    .iter()
                    .filter_map(|r| match r {
                        WsRow::Message(m) => Some(m.id.clone()),
                        _ => None,
                    })
                    .nth(val.parse().unwrap_or(0));
                s.selected = id;
            }
            "gql.url" => self.gql.url = val,
            "gql.query" => self.gql.query = val,
            "gql.vars" => self.gql.variables = val,
            "gql.send" => self.gql_send(),
            "gql.schema" => self.gql_introspect(),
            "gql.sub" => self.gql_subscribe(),
            "gql.search" => self.gql.schema_search = val,
            "gql.type" => {
                self.gql.selected_type = self
                    .gql
                    .schema
                    .as_ref()
                    .and_then(|s| s.types.iter().position(|t| t.name == val));
            }
            "gql.tab" => {
                self.gql.tab = match val.as_str() {
                    "schema" => ResultTab::Schema,
                    "subscription" => ResultTab::Subscription,
                    _ => ResultTab::Response,
                }
            }
            "gql.event" => self.gql.sub_selected = val.parse().ok(),
            "mcp.command" => self.mcp.active_mut().command = val,
            "mcp.connect" => self.mcp_connect(),
            "mcp.tool" => {
                let s = self.mcp.active_mut();
                if let Some(i) = s.tools.iter().position(|t| t.name == val) {
                    s.select_tool(i);
                }
            }
            "mcp.arg" => {
                if let Some((k, v)) = val.split_once('=') {
                    let s = self.mcp.active_mut();
                    if let Some(f) = s.fields.iter_mut().find(|f| f.key == k) {
                        f.value = v.to_string();
                    }
                }
            }
            "mcp.execute" => self.mcp_execute(),
            "mcp.tab" => {
                use crate::state::mcp::ServerTab;
                self.mcp.active_mut().tab = match val.as_str() {
                    "resources" => ServerTab::Resources,
                    "prompts" => ServerTab::Prompts,
                    _ => ServerTab::Tools,
                }
            }
            "run.collection" => {
                let dir = self
                    .collections
                    .metas
                    .iter()
                    .find(|m| m.name == val)
                    .map(crate::state::collections::dir_name);
                if let Some(d) = dir {
                    self.collections.load(&self.app, &d);
                    self.runner.collection = Some(d);
                }
            }
            "run.env" => {
                let id = self
                    .envs
                    .environments
                    .iter()
                    .find(|e| e.name == val)
                    .map(|e| e.id.clone());
                self.set_env(id);
            }
            "run.iterations" => self.runner.iterations = val,
            "run.delay" => self.runner.delay = val,
            "run.data" => self.runner_load_data(std::path::Path::new(&val)),
            "run.start" => self.runner_start(),
            "run.cancel" => self.runner_cancel(),
            "run.select" => self.runner.selected = val.parse().ok(),
            "theme" => {
                self.ui.theme = match val.as_str() {
                    "light" => crate::theme::ThemeMode::Light,
                    "system" => crate::theme::ThemeMode::System,
                    _ => crate::theme::ThemeMode::Dark,
                }
            }
            "dialog" => {
                use crate::state::dialogs::{Dialog, ImportTab};
                match val.as_str() {
                    "import" => self.open_dialog(Dialog::Import),
                    "export" => {
                        self.open_dialog(Dialog::Import);
                        self.dlg.import.tab = ImportTab::Export;
                    }
                    "docs" => self.open_dialog(Dialog::Docs),
                    "envs" => self.open_dialog(Dialog::Envs),
                    "palette" => self.open_dialog(Dialog::Palette),
                    "help" => self.open_dialog(Dialog::Help),
                    _ => self.close_dialog(),
                }
            }
            "palette.query" => {
                self.dlg.palette.query = val;
                self.dlg.palette.refilter();
            }
            "import.file" => self.import_load_file(std::path::Path::new(&val)),
            "import.paste" => {
                self.dlg.import.paste = val;
                self.import_parse_paste();
            }
            "docs.collection" => {
                self.dlg.docs.dir = self
                    .collections
                    .metas
                    .iter()
                    .find(|m| m.name == val)
                    .map(crate::state::collections::dir_name);
            }
            "docs.format" => {
                self.dlg.docs.format = if val == "html" {
                    crate::state::dialogs::DocFormat::Html
                } else {
                    crate::state::dialogs::DocFormat::Markdown
                }
            }
            "docs.generate" => self.docs_generate(),
            "env.edit" => {
                let id = self
                    .envs
                    .environments
                    .iter()
                    .find(|e| e.name == val)
                    .map(|e| e.id.clone());
                if let Some(id) = id {
                    self.env_edit(&id);
                }
            }
            "env.reveal" => {
                self.dlg.envs.revealed.insert(val.parse().unwrap_or(0));
            }
            _ => eprintln!("nexus: unknown dev command {key}"),
        }
        Dev::Done
    }

    fn dev_cond(&self, c: &str) -> bool {
        let (key, val) = c.split_once('=').unwrap_or((c, ""));
        let n: usize = val.parse().unwrap_or(0);
        match key {
            "ws.open" => self.ws.sessions.iter().any(|s| s.is_open()),
            "ws.messages" => self.ws.sessions.iter().any(|s| {
                s.rows
                    .iter()
                    .filter(|r| matches!(r, WsRow::Message(_)))
                    .count()
                    >= n
            }),
            "gql.response" => self.gql.result.is_some(),
            "gql.schema" => self.gql.schema.is_some(),
            "gql.events" => self.gql.sub_events.len() >= n,
            "mcp.connected" => self
                .mcp
                .sessions
                .iter()
                .any(|s| s.status == McpStatus::Connected),
            "mcp.result" => self.mcp.sessions.iter().any(|s| s.result.is_some()),
            "run.results" => self.runner.results.len() >= n,
            "run.done" => matches!(
                self.runner.status,
                RunStatus::Completed | RunStatus::Cancelled | RunStatus::Error
            ),
            "run.data" => self.runner.data.is_some(),
            "import.parsed" => self.dlg.import.parsed.is_some() || self.dlg.import.error.is_some(),
            _ => true,
        }
    }
}
