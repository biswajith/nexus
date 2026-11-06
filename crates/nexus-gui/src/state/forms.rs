//! The form fields of the Runner / WebSocket / GraphQL / MCP panels that survive a restart
//! (stored in `ui-state.json`). Connections, results and logs are runtime state and are not kept.

use super::graphql::GqlState;
use super::mcp::{McpSession, McpState};
use super::runner::RunnerState;
use super::ws::{WsSession, WsState};
use nexus_core::mcp::McpTransportType;
use nexus_core::types::KeyValuePair;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RunnerForm {
    pub collection: Option<String>,
    pub folder: Option<String>,
    pub iterations: String,
    pub delay: String,
    pub stop_on_error: bool,
    pub data_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WsForm {
    pub name: String,
    pub url: String,
    pub protocols: String,
    pub headers: Vec<KeyValuePair>,
    pub auto_reconnect: bool,
    pub draft: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GqlForm {
    pub url: String,
    pub query: String,
    pub variables: String,
    pub headers: Vec<KeyValuePair>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpForm {
    pub name: String,
    pub http: bool,
    pub command: String,
    pub args: String,
    pub env: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelForms {
    pub runner: Option<RunnerForm>,
    pub ws: Vec<WsForm>,
    pub ws_active: usize,
    pub graphql: Option<GqlForm>,
    pub mcp: Vec<McpForm>,
    pub mcp_active: usize,
}

impl PanelForms {
    pub fn capture(r: &RunnerState, ws: &WsState, g: &GqlState, m: &McpState) -> Self {
        Self {
            runner: Some(RunnerForm {
                collection: r.collection.clone(),
                folder: r.folder.clone(),
                iterations: r.iterations.clone(),
                delay: r.delay.clone(),
                stop_on_error: r.stop_on_error,
                data_path: r
                    .data
                    .as_ref()
                    .map(|d| d.path.to_string_lossy().into_owned()),
            }),
            ws: ws
                .sessions
                .iter()
                .map(|s| WsForm {
                    name: s.name.clone(),
                    url: s.url.clone(),
                    protocols: s.protocols.clone(),
                    headers: s.headers.clone(),
                    auto_reconnect: s.auto_reconnect,
                    draft: s.draft.clone(),
                })
                .collect(),
            ws_active: ws.active,
            graphql: Some(GqlForm {
                url: g.url.clone(),
                query: g.query.clone(),
                variables: g.variables.clone(),
                headers: g.headers.clone(),
            }),
            mcp: m
                .sessions
                .iter()
                .map(|s| McpForm {
                    name: s.name.clone(),
                    http: s.transport == McpTransportType::Http,
                    command: s.command.clone(),
                    args: s.args.clone(),
                    env: s.env.clone(),
                    url: s.url.clone(),
                })
                .collect(),
            mcp_active: m.active,
        }
    }

    /// Fills the panel states from the saved forms (startup). Returns the runner's data file to
    /// reload, if any.
    pub fn restore(
        &self,
        r: &mut RunnerState,
        ws: &mut WsState,
        g: &mut GqlState,
        m: &mut McpState,
    ) -> Option<std::path::PathBuf> {
        let mut data = None;
        if let Some(f) = &self.runner {
            r.collection = f.collection.clone();
            r.folder = f.folder.clone();
            if !f.iterations.is_empty() {
                r.iterations = f.iterations.clone();
            }
            if !f.delay.is_empty() {
                r.delay = f.delay.clone();
            }
            r.stop_on_error = f.stop_on_error;
            data = f.data_path.as_ref().map(std::path::PathBuf::from);
        }
        if !self.ws.is_empty() {
            ws.sessions.clear();
            for (i, f) in self.ws.iter().enumerate() {
                let mut s = WsSession::new(i + 1);
                s.name = f.name.clone();
                s.url = f.url.clone();
                s.protocols = f.protocols.clone();
                s.headers = f.headers.clone();
                s.auto_reconnect = f.auto_reconnect;
                s.draft = f.draft.clone();
                ws.sessions.push(s);
            }
            ws.active = self.ws_active.min(ws.sessions.len() - 1);
            ws.set_counter(ws.sessions.len());
        }
        if let Some(f) = &self.graphql {
            g.url = f.url.clone();
            if !f.query.is_empty() {
                g.query = f.query.clone();
            }
            if !f.variables.is_empty() {
                g.variables = f.variables.clone();
            }
            g.headers = f.headers.clone();
        }
        if !self.mcp.is_empty() {
            m.sessions.clear();
            for (i, f) in self.mcp.iter().enumerate() {
                let mut s = McpSession::new(i + 1);
                s.name = f.name.clone();
                s.transport = if f.http {
                    McpTransportType::Http
                } else {
                    McpTransportType::Stdio
                };
                s.command = f.command.clone();
                s.args = f.args.clone();
                s.env = f.env.clone();
                s.url = f.url.clone();
                m.sessions.push(s);
            }
            m.active = self.mcp_active.min(m.sessions.len() - 1);
            m.set_counter(m.sessions.len());
        }
        data
    }
}
