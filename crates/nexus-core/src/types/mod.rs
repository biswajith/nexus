//! Serde port of `packages/core/src/types/index.ts`. JSON produced/consumed here must match the
//! TypeScript shapes exactly (camelCase keys, omitted optionals, same enum strings).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BodyMode {
    #[default]
    #[serde(rename = "none")]
    None,
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "xml")]
    Xml,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "html")]
    Html,
    #[serde(rename = "form-data")]
    FormData,
    #[serde(rename = "x-www-form-urlencoded")]
    XWwwFormUrlencoded,
    #[serde(rename = "binary")]
    Binary,
    #[serde(rename = "graphql")]
    Graphql,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryBody {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphqlBody {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestBody {
    pub mode: BodyMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form_data: Option<Vec<FormDataEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urlencoded: Option<Vec<KeyValuePair>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<BinaryBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graphql: Option<GraphqlBody>,
}

impl Default for RequestBody {
    fn default() -> Self {
        Self {
            mode: BodyMode::None,
            raw: None,
            form_data: None,
            urlencoded: None,
            binary: None,
            graphql: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormDataType {
    Text,
    File,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormDataEntry {
    pub key: String,
    pub value: String,
    #[serde(rename = "type")]
    pub entry_type: FormDataType,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthType {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "inherit")]
    Inherit,
    #[serde(rename = "api-key")]
    ApiKey,
    #[serde(rename = "bearer")]
    Bearer,
    #[serde(rename = "basic")]
    Basic,
    #[serde(rename = "oauth2")]
    Oauth2,
    #[serde(rename = "digest")]
    Digest,
    #[serde(rename = "aws-sig-v4")]
    AwsSigV4,
    #[serde(rename = "hawk")]
    Hawk,
}

/// `{ type, ...arbitrary keys }`. The extra keys (token, username, ...) are kept verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(rename = "type")]
    pub auth_type: AuthType,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl AuthConfig {
    pub fn new(auth_type: AuthType) -> Self {
        Self {
            auth_type,
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_redirects: Option<u32>,
    /// Milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reject_unauthorized: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusRequest {
    pub id: String,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<KeyValuePair>,
    pub params: Vec<KeyValuePair>,
    pub body: RequestBody,
    pub auth: AuthConfig,
    /// Not in the TS `NexusRequest` type, but the OpenAPI importer writes it and it must survive a round trip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_request_script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_response_script: Option<String>,
    pub settings: RequestSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFolder {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub items: Vec<CollectionItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_request_script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_response_script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_order: Option<Vec<String>>,
}

/// `NexusRequest | NexusFolder`. Discriminated structurally like the TS code (`'items' in item`).
/// `Folder` is tried first because a request never has `items` but a folder has no `method`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum CollectionItem {
    Folder(NexusFolder),
    Request(NexusRequest),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusCollection {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub variables: Vec<Variable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_request_script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_response_script: Option<String>,
    pub items: Vec<CollectionItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_order: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    String,
    Secret,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub key: String,
    pub value: String,
    #[serde(rename = "type")]
    pub var_type: VariableType,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusEnvironment {
    pub id: String,
    pub name: String,
    pub variables: Vec<Variable>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTiming {
    pub dns: f64,
    pub tcp: f64,
    pub tls: f64,
    pub ttfb: f64,
    pub download: f64,
    pub total: f64,
}

/// `Record<string, string | string[]>` value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HeaderValue {
    One(String),
    Many(Vec<String>),
}

/// Engine-side response. The raw `body` bytes are never sent to the UI (the UI only needs
/// `bodyText`/`bodyJson`), so it is skipped during (de)serialization.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: IndexMap<String, HeaderValue>,
    #[serde(skip)]
    pub body: Vec<u8>,
    pub body_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_json: Option<Value>,
    pub timing: ResponseTiming,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseFormat {
    Json,
    Xml,
    Html,
    Text,
    Image,
    Pdf,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRequest {
    pub method: String,
    pub url: String,
    pub headers: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResponse {
    pub status: u16,
    pub status_text: String,
    pub response_time: f64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    /// Epoch milliseconds.
    pub timestamp: i64,
    pub request: HistoryRequest,
    pub response: HistoryResponse,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<HttpMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogSource {
    Script,
    Http,
    Variable,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleEntry {
    pub id: String,
    pub timestamp: i64,
    pub level: LogLevel,
    pub source: LogSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<LogLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<LogSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeLevel {
    Local,
    Environment,
    Collection,
    Global,
}

impl CollectionItem {
    pub fn is_folder(&self) -> bool {
        matches!(self, CollectionItem::Folder(_))
    }

    pub fn is_request(&self) -> bool {
        matches!(self, CollectionItem::Request(_))
    }
}

/// Free-function form of [`CollectionItem::is_folder`] (mirrors TS `isFolder`).
pub fn is_folder(item: &CollectionItem) -> bool {
    item.is_folder()
}

/// Mirrors TS `isRequest`.
pub fn is_request(item: &CollectionItem) -> bool {
    item.is_request()
}

/// Mirrors TS `createDefaultRequest()` with no overrides; mutate fields for `partial`.
pub fn create_default_request() -> NexusRequest {
    NexusRequest {
        id: uuid::Uuid::new_v4().to_string(),
        name: "New Request".to_string(),
        method: HttpMethod::Get,
        url: String::new(),
        headers: vec![],
        params: vec![],
        body: RequestBody::default(),
        auth: AuthConfig::new(AuthType::Inherit),
        description: None,
        pre_request_script: None,
        post_response_script: None,
        settings: RequestSettings::default(),
    }
}

/// Mirrors TS `createDefaultEnvironment()` with no overrides.
pub fn create_default_environment() -> NexusEnvironment {
    NexusEnvironment {
        id: uuid::Uuid::new_v4().to_string(),
        name: "New Environment".to_string(),
        variables: vec![],
    }
}

/// An empty collection with a fresh id.
pub fn create_default_collection(name: &str) -> NexusCollection {
    NexusCollection {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        description: None,
        version: None,
        variables: vec![],
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        items: vec![],
        item_order: None,
    }
}

#[cfg(test)]
mod tests;
