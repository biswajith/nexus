//! GraphQL panel state: endpoint, editors, last response, introspected schema, subscription log.

use super::gql_schema::Schema;
use crate::widgets::TextView;
use nexus_core::graphql::{GraphQLResponse, SubscriptionMessage};
use nexus_core::types::KeyValuePair;

pub const DEFAULT_QUERY: &str = "# Enter your GraphQL query\nquery {\n  \n}\n";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ResultTab {
    #[default]
    Response,
    Schema,
    Subscription,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EditorTab {
    #[default]
    Variables,
    Headers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SchemaView {
    #[default]
    Browser,
    Sdl,
}

/// The last query response, ready to show.
pub struct GqlResult {
    pub status: u16,
    pub ms: u64,
    pub errors: Vec<String>,
    pub body: TextView,
    pub has_data: bool,
    /// Set when the request itself failed (network / invalid input).
    pub failure: Option<String>,
}

impl GqlResult {
    pub fn from_response(r: &GraphQLResponse) -> Self {
        let errors = r
            .errors
            .as_ref()
            .and_then(|e| e.as_array())
            .map(|a| {
                a.iter()
                    .map(|e| {
                        e.get("message")
                            .and_then(|m| m.as_str())
                            .map_or_else(|| e.to_string(), str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let data = r.data.as_ref().filter(|d| !d.is_null());
        let mut body = TextView::default();
        if let Some(d) = data {
            body.set(crate::widgets::pretty(d));
        }
        Self {
            status: r.status,
            ms: r.response_time,
            errors,
            body,
            has_data: data.is_some(),
            failure: None,
        }
    }

    pub fn failed(msg: String) -> Self {
        Self {
            status: 0,
            ms: 0,
            errors: vec![msg.clone()],
            body: TextView::default(),
            has_data: false,
            failure: Some(msg),
        }
    }
}

/// One entry of the subscription log.
pub struct SubEntry {
    pub n: usize,
    pub at: i64,
    pub text: String,
    pub is_error: bool,
}

pub struct GqlState {
    pub url: String,
    pub query: String,
    pub variables: String,
    pub headers: Vec<KeyValuePair>,
    pub editor_tab: EditorTab,
    pub tab: ResultTab,

    pub sending: bool,
    pub result: Option<GqlResult>,

    pub schema_loading: bool,
    pub schema_error: Option<String>,
    pub schema: Option<Schema>,
    pub sdl: TextView,
    pub schema_view: SchemaView,
    pub schema_search: String,
    pub selected_type: Option<usize>,
    pub type_view: TextView,
    /// Which type `type_view` shows.
    pub type_view_for: Option<usize>,
    /// URL the schema was introspected from.
    pub schema_url: String,
    /// Type / field names offered by the query editor's autocomplete.
    pub words: std::sync::Arc<Vec<crate::editor::complete::VarItem>>,

    pub sub_active: bool,
    pub sub_events: Vec<SubEntry>,
    pub sub_selected: Option<usize>,
    pub sub_view: TextView,
    pub sub_view_for: Option<usize>,
    pub sub_count: usize,
    pub sub_notice: Option<String>,
}

impl Default for GqlState {
    fn default() -> Self {
        Self {
            url: String::new(),
            query: DEFAULT_QUERY.into(),
            variables: "{}".into(),
            headers: Vec::new(),
            editor_tab: EditorTab::Variables,
            tab: ResultTab::Response,
            sending: false,
            result: None,
            schema_loading: false,
            schema_error: None,
            schema: None,
            sdl: TextView::default(),
            schema_view: SchemaView::Browser,
            schema_search: String::new(),
            selected_type: None,
            type_view: TextView::default(),
            type_view_for: None,
            schema_url: String::new(),
            words: Default::default(),
            sub_active: false,
            sub_events: Vec::new(),
            sub_selected: None,
            sub_view: TextView::default(),
            sub_view_for: None,
            sub_count: 0,
            sub_notice: None,
        }
    }
}

impl GqlState {
    /// Parsed variables: `{}` for blank text; `Err` describes invalid JSON.
    pub fn variables_json(&self) -> Result<serde_json::Value, String> {
        let t = self.variables.trim();
        if t.is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_str(t).map_err(|e| format!("Variables are not valid JSON: {e}"))
    }

    /// Enabled header rows as a map.
    pub fn headers_map(&self) -> Option<indexmap::IndexMap<String, String>> {
        let m: indexmap::IndexMap<String, String> = self
            .headers
            .iter()
            .filter(|h| h.enabled && !h.key.trim().is_empty())
            .map(|h| (h.key.trim().to_string(), h.value.clone()))
            .collect();
        (!m.is_empty()).then_some(m)
    }

    pub fn push_sub(&mut self, m: SubscriptionMessage) {
        let is_error = m.errors.is_some();
        let text = if let Some(e) = &m.errors {
            match e.as_array() {
                Some(a) => a
                    .iter()
                    .map(|x| {
                        x.get("message")
                            .and_then(|m| m.as_str())
                            .map_or_else(|| x.to_string(), str::to_string)
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                None => e.to_string(),
            }
        } else {
            m.data
                .as_ref()
                .map(crate::widgets::pretty)
                .unwrap_or_default()
        };
        self.add_sub(m.timestamp, text, is_error);
    }

    pub fn add_sub(&mut self, at: i64, text: String, is_error: bool) {
        self.sub_count += 1;
        self.sub_events.push(SubEntry {
            n: self.sub_count,
            at,
            text,
            is_error,
        });
        if self.sub_events.len() > 1000 {
            self.sub_events.remove(0);
        }
    }

    pub fn set_schema(&mut self, url: String, sdl: String) {
        let schema = super::gql_schema::parse_sdl(&sdl);
        self.words = std::sync::Arc::new(
            schema
                .words()
                .into_iter()
                .map(|name| crate::editor::complete::VarItem {
                    name,
                    scope: crate::editor::complete::SCHEMA_SCOPE,
                    value: String::new(),
                })
                .collect(),
        );
        self.schema = Some(schema);
        self.sdl.set(sdl);
        self.schema_url = url;
        self.selected_type = None;
        self.type_view_for = None;
    }
}
