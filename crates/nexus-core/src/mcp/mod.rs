//! MCP tester client (port of the `mcp:*` IPC handlers): connects to a Model Context Protocol
//! server over stdio (spawned child process, direct exec, no shell) or streamable HTTP and
//! exposes list/call operations as JSON values shaped like the TypeScript SDK results.

use std::collections::HashMap;
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, Implementation, InitializeRequestParams,
};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{Peer, ServiceExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::sync::Mutex;

use crate::{Error, Result};

/// Same default as the TS SDK request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Environment variables a stdio server inherits (the TS SDK's `getDefaultEnvironment`,
/// POSIX list). Everything else (tokens, API keys, ...) must be passed via `env` explicitly.
const INHERITED_ENV: [&str; 6] = ["HOME", "LOGNAME", "PATH", "SHELL", "TERM", "USER"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransportType {
    Stdio,
    Http,
}

/// Argument of `mcp_connect`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConnectConfig {
    #[serde(rename = "type")]
    pub transport: McpTransportType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

type Connection = RunningService<RoleClient, InitializeRequestParams>;

/// All live MCP connections by renderer-chosen id.
#[derive(Default)]
pub struct McpClients {
    conns: Mutex<HashMap<String, Connection>>,
}

fn err(msg: impl Into<String>) -> Error {
    Error::Other(msg.into())
}

fn to_value<T: Serialize>(v: &T) -> Result<Value> {
    Ok(serde_json::to_value(v)?)
}

/// Environment for a spawned server: the safe inherited subset overlaid with `extra`.
fn build_env(extra: Option<&HashMap<String, String>>) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = Vec::new();
    for key in INHERITED_ENV {
        if let Ok(value) = std::env::var(key) {
            // Skip exported shell functions, like the SDK.
            if !value.starts_with("()") {
                env.push((key.to_string(), value));
            }
        }
    }
    if let Some(extra) = extra {
        for (k, v) in extra {
            env.retain(|(ek, _)| ek != k);
            env.push((k.clone(), v.clone()));
        }
    }
    env
}

fn client_info() -> InitializeRequestParams {
    InitializeRequestParams::new(
        ClientCapabilities::default(),
        Implementation::new("nexus-mcp-tester", "0.1.0"),
    )
}

async fn with_timeout<T, E: std::fmt::Display>(
    what: &str,
    fut: impl std::future::Future<Output = std::result::Result<T, E>>,
) -> Result<T> {
    match tokio::time::timeout(REQUEST_TIMEOUT, fut).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(err(e.to_string())),
        Err(_) => Err(err(format!("{what} timed out"))),
    }
}

impl McpClients {
    pub fn new() -> Self {
        Self::default()
    }

    /// Connects (closing any previous connection with the same id first) and returns
    /// `{ serverInfo, capabilities }`.
    pub async fn connect(&self, id: &str, config: &McpConnectConfig) -> Result<Value> {
        self.disconnect(id).await;

        let handler = client_info();
        let conn: Connection = match config.transport {
            McpTransportType::Stdio => {
                let command = config.command.as_deref().filter(|c| !c.is_empty());
                let command =
                    command.ok_or_else(|| err("Command is required for stdio transport"))?;
                let mut cmd = tokio::process::Command::new(command);
                cmd.args(config.args.as_deref().unwrap_or_default())
                    .env_clear()
                    .envs(build_env(config.env.as_ref()))
                    .kill_on_drop(true);
                let transport = TokioChildProcess::new(cmd)
                    .map_err(|e| err(format!("Failed to start MCP server `{command}`: {e}")))?;
                with_timeout("MCP connect", handler.serve(transport)).await?
            }
            McpTransportType::Http => {
                let url = config.url.as_deref().filter(|u| !u.is_empty());
                let url = url.ok_or_else(|| err("URL is required for HTTP transport"))?;
                let parsed = url::Url::parse(url).map_err(|e| err(format!("Invalid URL: {e}")))?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err(err("URL must use http or https"));
                }
                let transport = StreamableHttpClientTransport::from_uri(url.to_string());
                with_timeout("MCP connect", handler.serve(transport)).await?
            }
        };

        let info = conn.peer_info();
        let result = json!({
            "serverInfo": info.as_ref().and_then(|i| i.server_info.as_ref()).map(to_value).transpose()?,
            "capabilities": info.as_ref().map(|i| to_value(&i.capabilities)).transpose()?,
        });
        self.conns.lock().await.insert(id.to_string(), conn);
        Ok(result)
    }

    async fn peer(&self, id: &str) -> Result<Peer<RoleClient>> {
        self.conns
            .lock()
            .await
            .get(id)
            .map(|c| c.peer().clone())
            .ok_or_else(|| err("Not connected to MCP server"))
    }

    /// `tools/list`, first page, returns the `tools` array.
    pub async fn list_tools(&self, id: &str) -> Result<Value> {
        let peer = self.peer(id).await?;
        let r = with_timeout("tools/list", peer.list_tools(None)).await?;
        to_value(&r.tools)
    }

    /// `tools/call`; returns the SDK result object (`content`, `isError`, ...).
    pub async fn call_tool(
        &self,
        id: &str,
        name: &str,
        args: Option<Map<String, Value>>,
    ) -> Result<Value> {
        let peer = self.peer(id).await?;
        let mut params = CallToolRequestParams::new(name.to_string());
        params.arguments = args;
        let r = with_timeout("tools/call", peer.call_tool(params)).await?;
        to_value(&r)
    }

    /// `resources/list`, first page, returns the `resources` array.
    pub async fn list_resources(&self, id: &str) -> Result<Value> {
        let peer = self.peer(id).await?;
        let r = with_timeout("resources/list", peer.list_resources(None)).await?;
        to_value(&r.resources)
    }

    /// `prompts/list`, first page, returns the `prompts` array.
    pub async fn list_prompts(&self, id: &str) -> Result<Value> {
        let peer = self.peer(id).await?;
        let r = with_timeout("prompts/list", peer.list_prompts(None)).await?;
        to_value(&r.prompts)
    }

    /// Closes the connection (killing a stdio child). Unknown ids are ignored.
    pub async fn disconnect(&self, id: &str) {
        let conn = self.conns.lock().await.remove(id);
        if let Some(mut conn) = conn {
            let _ = conn.close_with_timeout(CLOSE_TIMEOUT).await;
        }
    }

    /// Closes every connection (app exit).
    pub async fn disconnect_all(&self) {
        let all: Vec<Connection> = self.conns.lock().await.drain().map(|(_, c)| c).collect();
        for mut conn in all {
            let _ = conn.close_with_timeout(CLOSE_TIMEOUT).await;
        }
    }
}

#[cfg(test)]
mod tests;
