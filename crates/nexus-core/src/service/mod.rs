//! Application service layer: everything the UI needs, as plain methods (no UI-framework types).
//!
//! `App` owns the managers rooted at `~/.nexus` plus the long-lived sessions (runner cancel flag,
//! WebSocket connections, GraphQL subscription, MCP connections). Long-running operations take an
//! event callback instead of a framework-specific emitter.

pub mod http;

pub use http::{HttpSendPayload, HttpSendResponse};

use crate::auth::TokenStore;
use crate::collections::{CollectionManager, CollectionMeta};
use crate::environments::EnvironmentStore;
use crate::fs::ensure_private_dir;
use crate::graphql::{
    GraphQLClient, GraphQLRequestOptions, GraphQLResponse, SubscriptionEvent, SubscriptionOptions,
};
use crate::history::{parse_date, HistoryManager};
use crate::import::{self, exporter, ImportFormat, ImportResult};
use crate::mcp::{McpClients, McpConnectConfig};
use crate::runner::{CancelToken, CollectionRunner, RunConfig, RunEvent, RunSummary};
use crate::scripts::DEFAULT_TIMEOUT_MS;
use crate::types::{HistoryEntry, HistoryFilter, NexusCollection, NexusEnvironment, NexusRequest};
use crate::websocket::{WebSocketClient, WsConnectOptions, WsEvent, WsMessage};
use indexmap::IndexMap;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Shared application state and operations.
pub struct App {
    /// `~/.nexus`, the app data directory.
    pub nexus_home: PathBuf,
    pub collections: CollectionManager,
    pub environments: EnvironmentStore,
    pub history: HistoryManager,
    /// OAuth2 tokens (`~/.nexus/tokens.json`); not exposed to the UI.
    pub tokens: Mutex<TokenStore>,
    /// Cancellation flag of the (single) collection run.
    pub runner_cancel: CancelToken,
    ws: Mutex<HashMap<String, WebSocketClient>>,
    /// GraphQL client: schema cache + the single active subscription.
    pub graphql: GraphQLClient,
    /// MCP tester connections by caller-chosen id.
    pub mcp: McpClients,
}

impl App {
    /// Builds the managers rooted at `nexus_home` and makes the directory owner-only
    /// (it holds tokens, secrets and history), also for installs made by older versions.
    pub fn new(nexus_home: &Path) -> std::io::Result<Self> {
        ensure_private_dir(nexus_home)?;
        let mut tokens = TokenStore::new(Some(nexus_home.join("tokens.json")));
        tokens.load();
        Ok(Self {
            nexus_home: nexus_home.to_path_buf(),
            collections: CollectionManager::new(nexus_home.join("collections")),
            environments: EnvironmentStore::new(nexus_home.join("environments")),
            history: HistoryManager::new(nexus_home.join("history")),
            tokens: Mutex::new(tokens),
            runner_cancel: CancelToken::new(),
            ws: Mutex::new(HashMap::new()),
            graphql: GraphQLClient::new(),
            mcp: McpClients::new(),
        })
    }

    /// `~/.nexus` for the current user.
    pub fn default_home() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(|h| PathBuf::from(h).join(".nexus"))
    }

    // ---- collections ----
    pub fn collections_list(&self) -> crate::Result<Vec<CollectionMeta>> {
        self.collections.list_collections()
    }
    pub fn collection_load(&self, dir_name: &str) -> crate::Result<NexusCollection> {
        self.collections.load_collection(dir_name)
    }
    /// Returns the directory name the collection was written to.
    pub fn collection_create(&self, c: &NexusCollection) -> crate::Result<String> {
        self.collections.create_collection(c)
    }
    pub fn collection_is_trusted(&self, dir_name: &str) -> bool {
        self.collections.is_trusted(dir_name)
    }
    pub fn collection_trust_status(&self, dir_name: &str) -> crate::collections::TrustStatus {
        self.collections.trust_status(dir_name)
    }
    pub fn collection_trust(&self, dir_name: &str) -> crate::Result<()> {
        self.collections.trust_collection(dir_name)
    }
    pub fn collection_delete(&self, dir_name: &str) -> crate::Result<()> {
        self.collections.delete_collection(dir_name)
    }
    /// Changes the name in `collection.json`, not the directory.
    pub fn collection_rename(&self, dir_name: &str, new_name: &str) -> crate::Result<()> {
        self.collections.rename_collection(dir_name, new_name)
    }
    pub fn request_add(&self, dir_name: &str, r: &NexusRequest) -> crate::Result<()> {
        self.collections.add_request(dir_name, r)
    }
    pub fn request_update(
        &self,
        dir_name: &str,
        request_id: &str,
        r: &NexusRequest,
    ) -> crate::Result<()> {
        self.collections.update_request(dir_name, request_id, r)
    }
    pub fn request_delete(&self, dir_name: &str, request_id: &str) -> crate::Result<()> {
        self.collections.delete_request(dir_name, request_id)
    }

    // ---- environments ----
    pub fn environments_list(&self) -> crate::Result<Vec<NexusEnvironment>> {
        self.environments.list()
    }
    /// Creates or replaces `<id>.json`; the id must be a safe file name.
    pub fn environment_save(&self, env: &NexusEnvironment) -> crate::Result<()> {
        self.environments.save_environment(env)
    }
    pub fn environment_delete(&self, env_id: &str) -> crate::Result<()> {
        self.environments.delete(env_id)
    }

    // ---- history ----
    pub fn history_log(&self, entry: &HistoryEntry) -> crate::Result<()> {
        self.history.log(entry)
    }
    pub fn history_query(&self, filters: &HistoryFilter) -> crate::Result<Vec<HistoryEntry>> {
        self.history.query(filters)
    }
    /// `before` is an ISO date string; `None`/empty clears everything.
    pub fn history_clear(&self, before: Option<&str>) -> crate::Result<()> {
        let before = before
            .filter(|b| !b.is_empty())
            .map(parse_date)
            .transpose()?;
        self.history.clear(before)
    }

    // ---- import / export ----
    pub fn import_detect_format(data: &str) -> ImportFormat {
        import::detect_import_format(data)
    }
    pub fn import_data(data: &str, format: Option<ImportFormat>) -> crate::Result<ImportResult> {
        import::import_data(data, format)
    }
    pub fn import_curl(curl_command: &str) -> crate::Result<NexusRequest> {
        import::curl::import(curl_command)
    }
    /// `postman-v2.1` gives Postman JSON, anything else Nexus JSON.
    pub fn export_collection(c: &NexusCollection, format: &str) -> crate::Result<String> {
        if format == "postman-v2.1" {
            exporter::export_as_postman_v21(c)
        } else {
            exporter::export_as_nexus_json(c)
        }
    }
    /// `postman` gives Postman JSON, anything else Nexus JSON.
    pub fn export_environment(e: &NexusEnvironment, format: &str) -> crate::Result<String> {
        if format == "postman" {
            exporter::export_environment_as_postman(e)
        } else {
            exporter::export_environment_as_nexus_json(e)
        }
    }
    /// Persists imported collections (each into a fresh directory, untrusted) and environments (never
    /// replacing an existing one). Returns one status line per item.
    pub fn import_save(
        &self,
        collections: &[NexusCollection],
        environments: &[NexusEnvironment],
    ) -> crate::Result<Vec<String>> {
        let mut results = Vec::new();
        for c in collections {
            self.collections.create_imported_collection(c)?;
            results.push(format!("Collection \"{}\" imported", c.name));
        }
        for e in environments {
            self.environments.save_imported(e)?;
            results.push(format!("Environment \"{}\" imported", e.name));
        }
        Ok(results)
    }

    // ---- http ----
    /// variables -> pre-script -> auth -> send -> post-script.
    pub async fn http_send(&self, payload: HttpSendPayload) -> Result<HttpSendResponse, String> {
        http::execute(payload, crate::scripts::ScriptSandbox::default()).await
    }

    // ---- runner ----
    /// Runs the collection; every progress event goes to `sink`.
    pub async fn runner_start(
        &self,
        config: RunConfig,
        sink: impl Fn(RunEvent) + Send + Sync + 'static,
    ) -> RunSummary {
        CollectionRunner::new(DEFAULT_TIMEOUT_MS, self.runner_cancel.clone())
            .run(config, sink)
            .await
    }
    pub fn runner_cancel(&self) {
        self.runner_cancel.cancel();
    }

    // ---- websocket ----
    fn ws_map(&self) -> std::sync::MutexGuard<'_, HashMap<String, WebSocketClient>> {
        self.ws.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Replaces any existing connection with the same id; `sink` receives `(id, event)`.
    pub fn ws_connect(
        &self,
        id: &str,
        opts: WsConnectOptions,
        sink: impl Fn(&str, WsEvent) + Send + Sync + 'static,
    ) -> crate::Result<()> {
        let mut clients = self.ws_map();
        if let Some(existing) = clients.get(id) {
            existing.disconnect(None, None);
        }
        let event_id = id.to_string();
        let client = WebSocketClient::new(move |event| sink(&event_id, event));
        client.connect(opts)?;
        clients.insert(id.to_string(), client);
        Ok(())
    }
    pub fn ws_send(&self, id: &str, data: &str) -> Result<WsMessage, String> {
        let clients = self.ws_map();
        let client = clients
            .get(id)
            .ok_or_else(|| format!("No WebSocket connection with id: {id}"))?;
        client.send(data).map_err(|e| e.to_string())
    }
    pub fn ws_disconnect(&self, id: &str) {
        // The connection task finishes the close handshake (and emits its events) on its own.
        if let Some(client) = self.ws_map().remove(id) {
            client.disconnect(None, None);
        }
    }
    pub fn ws_messages(&self, id: &str) -> Vec<WsMessage> {
        self.ws_map()
            .get(id)
            .map(WebSocketClient::messages)
            .unwrap_or_default()
    }

    // ---- graphql ----
    pub async fn graphql_send(
        &self,
        opts: &GraphQLRequestOptions,
    ) -> crate::Result<GraphQLResponse> {
        self.graphql.send(opts).await
    }
    /// Returns the schema as SDL.
    pub async fn graphql_introspect(
        &self,
        url: &str,
        headers: Option<IndexMap<String, String>>,
    ) -> crate::Result<String> {
        self.graphql.introspect(url, headers).await
    }
    pub fn graphql_subscribe(
        &self,
        opts: SubscriptionOptions,
        sink: impl Fn(SubscriptionEvent) + Send + Sync + 'static,
    ) {
        self.graphql.subscribe(opts, sink);
    }
    pub fn graphql_unsubscribe(&self) {
        self.graphql.unsubscribe();
    }

    // ---- mcp ----
    pub async fn mcp_connect(&self, id: &str, config: &McpConnectConfig) -> crate::Result<Value> {
        self.mcp.connect(id, config).await
    }
    pub async fn mcp_list_tools(&self, id: &str) -> crate::Result<Value> {
        self.mcp.list_tools(id).await
    }
    pub async fn mcp_call_tool(
        &self,
        id: &str,
        tool_name: &str,
        args: Option<Map<String, Value>>,
    ) -> crate::Result<Value> {
        self.mcp.call_tool(id, tool_name, args).await
    }
    pub async fn mcp_list_resources(&self, id: &str) -> crate::Result<Value> {
        self.mcp.list_resources(id).await
    }
    pub async fn mcp_list_prompts(&self, id: &str) -> crate::Result<Value> {
        self.mcp.list_prompts(id).await
    }
    pub async fn mcp_disconnect(&self, id: &str) {
        self.mcp.disconnect(id).await;
    }
    pub async fn mcp_disconnect_all(&self) {
        self.mcp.disconnect_all().await;
    }
}

#[cfg(test)]
mod tests;
