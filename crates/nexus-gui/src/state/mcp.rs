//! MCP sessions: connection config, discovered tools / resources / prompts, argument forms
//! generated from the tools' JSON schemas, call results and call history. Sessions stay
//! connected while another panel is shown.

use crate::widgets::TextView;
use eframe::egui::TextureHandle;
use nexus_core::mcp::McpTransportType;
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum McpStatus {
    #[default]
    Idle,
    Connecting,
    Connected,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ServerTab {
    #[default]
    Tools,
    Resources,
    Prompts,
}

#[derive(Clone, Debug)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

#[derive(Clone, Debug)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime: String,
}

#[derive(Clone, Debug)]
pub struct McpPrompt {
    pub name: String,
    pub description: String,
    /// `(name, description, required)`
    pub arguments: Vec<(String, String, bool)>,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn parse_tools(v: &[Value]) -> Vec<McpTool> {
    v.iter()
        .map(|t| McpTool {
            name: s(t, "name"),
            description: s(t, "description"),
            schema: t.get("inputSchema").cloned().unwrap_or(Value::Null),
        })
        .collect()
}

pub fn parse_resources(v: &[Value]) -> Vec<McpResource> {
    v.iter()
        .map(|r| McpResource {
            uri: s(r, "uri"),
            name: s(r, "name"),
            description: s(r, "description"),
            mime: s(r, "mimeType"),
        })
        .collect()
}

pub fn parse_prompts(v: &[Value]) -> Vec<McpPrompt> {
    v.iter()
        .map(|p| McpPrompt {
            name: s(p, "name"),
            description: s(p, "description"),
            arguments: p
                .get("arguments")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|x| {
                            (
                                s(x, "name"),
                                s(x, "description"),
                                x.get("required").and_then(Value::as_bool).unwrap_or(false),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

// ---- argument forms ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArgType {
    String,
    Number,
    Integer,
    Boolean,
    /// `object` / `array`: edited as JSON text.
    Json,
}

impl ArgType {
    pub fn label(self) -> &'static str {
        match self {
            ArgType::String => "string",
            ArgType::Number => "number",
            ArgType::Integer => "integer",
            ArgType::Boolean => "boolean",
            ArgType::Json => "json",
        }
    }
}

/// One input of a tool, derived from a property of its `inputSchema`.
#[derive(Clone, Debug)]
pub struct ArgField {
    pub key: String,
    pub ty: ArgType,
    /// The schema's own type name (`object`, `array`, `string`, ...), for display.
    pub schema_type: String,
    pub description: String,
    pub required: bool,
    pub choices: Vec<String>,
    /// Text typed by the user (`true` / `false` for booleans).
    pub value: String,
}

fn type_of(schema: &Value) -> String {
    match schema.get("type") {
        Some(Value::String(t)) => t.clone(),
        // ["string", "null"]
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null")
            .unwrap_or("string")
            .to_string(),
        _ => {
            // anyOf / oneOf: first non-null alternative
            for k in ["anyOf", "oneOf"] {
                if let Some(alts) = schema.get(k).and_then(Value::as_array) {
                    if let Some(t) = alts
                        .iter()
                        .map(type_of)
                        .find(|t| t != "null" && !t.is_empty())
                    {
                        return t;
                    }
                }
            }
            if schema.get("properties").is_some() {
                "object".into()
            } else if schema.get("items").is_some() {
                "array".into()
            } else {
                "string".into()
            }
        }
    }
}

/// Form fields for the properties of a tool's input schema (in schema order).
pub fn fields_for(schema: &Value) -> Vec<ArgField> {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    props
        .iter()
        .map(|(key, p)| {
            let schema_type = type_of(p);
            let ty = match schema_type.as_str() {
                "number" => ArgType::Number,
                "integer" => ArgType::Integer,
                "boolean" => ArgType::Boolean,
                "object" | "array" => ArgType::Json,
                _ => ArgType::String,
            };
            let choices: Vec<String> = p
                .get("enum")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            o => o.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let value = match p.get("default") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(o) => {
                    if ty == ArgType::Json {
                        serde_json::to_string_pretty(o).unwrap_or_default()
                    } else {
                        o.to_string()
                    }
                }
            };
            ArgField {
                key: key.clone(),
                ty,
                schema_type,
                description: s(p, "description"),
                required: required.contains(&key.as_str()),
                choices,
                value,
            }
        })
        .collect()
}

/// The call arguments from the filled-in form. Blank optional fields are left out; invalid
/// numbers / JSON are reported by field name.
pub fn build_args(fields: &[ArgField]) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for f in fields {
        let v = f.value.trim();
        if v.is_empty() && f.ty != ArgType::String {
            if f.required {
                return Err(format!("\"{}\" is required", f.key));
            }
            continue;
        }
        if f.ty == ArgType::String && f.value.is_empty() {
            if f.required {
                out.insert(f.key.clone(), Value::String(String::new()));
            }
            continue;
        }
        let value = match f.ty {
            ArgType::String => Value::String(f.value.clone()),
            ArgType::Number => v
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .ok_or_else(|| format!("\"{}\" must be a number", f.key))?,
            ArgType::Integer => v
                .parse::<i64>()
                .map(|n| Value::Number(n.into()))
                .map_err(|_| format!("\"{}\" must be an integer", f.key))?,
            ArgType::Boolean => Value::Bool(v == "true"),
            ArgType::Json => serde_json::from_str(v)
                .map_err(|e| format!("\"{}\" is not valid JSON: {e}", f.key))?,
        };
        out.insert(f.key.clone(), value);
    }
    Ok(out)
}

// ---- results ----

pub enum ResultItem {
    Text(String),
    Image {
        mime: String,
        data: String,
        texture: Option<TextureHandle>,
        /// Decoding failed (shown as a note instead).
        broken: bool,
    },
    Other(String),
}

/// A `tools/call` result.
pub struct ToolResult {
    pub is_error: bool,
    pub items: Vec<ResultItem>,
    pub structured: Option<String>,
    /// All text of the result (what Copy copies).
    pub text: String,
}

fn format_text(t: &str) -> String {
    match serde_json::from_str::<Value>(t) {
        Ok(v @ (Value::Object(_) | Value::Array(_))) => crate::widgets::pretty(&v),
        _ => t.to_string(),
    }
}

impl ToolResult {
    pub fn from_value(v: &Value) -> Self {
        let mut items = Vec::new();
        let mut text = String::new();
        for c in v
            .get("content")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            match c.get("type").and_then(Value::as_str) {
                Some("text") => {
                    let t = format_text(&s(c, "text"));
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&t);
                    items.push(ResultItem::Text(t));
                }
                Some("image") if c.get("data").is_some() => items.push(ResultItem::Image {
                    mime: c
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("image/png")
                        .to_string(),
                    data: s(c, "data"),
                    texture: None,
                    broken: false,
                }),
                _ => {
                    let t = crate::widgets::pretty(c);
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&t);
                    items.push(ResultItem::Other(t));
                }
            }
        }
        let structured = v
            .get("structuredContent")
            .filter(|x| !x.is_null())
            .map(crate::widgets::pretty);
        Self {
            is_error: v.get("isError").and_then(Value::as_bool).unwrap_or(false),
            items,
            structured,
            text,
        }
    }

    pub fn error(msg: String) -> Self {
        Self {
            is_error: true,
            items: vec![ResultItem::Text(msg.clone())],
            structured: None,
            text: msg,
        }
    }
}

pub struct CallEntry {
    pub tool: String,
    pub args: Value,
    pub at: i64,
    pub ms: f64,
    pub result: ToolResult,
}

// ---- session ----

pub struct McpSession {
    pub id: String,
    pub name: String,
    pub transport: McpTransportType,
    pub command: String,
    pub args: String,
    pub env: String,
    pub url: String,
    pub status: McpStatus,
    pub error: Option<String>,
    pub server_name: String,
    pub server_version: String,
    pub caps: [bool; 3],
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub prompts: Vec<McpPrompt>,
    pub tab: ServerTab,
    pub filter: String,
    pub selected_tool: Option<usize>,
    pub fields: Vec<ArgField>,
    pub form_error: Option<String>,
    pub executing: bool,
    pub result: Option<ToolResult>,
    pub result_view: TextView,
    pub history: Vec<CallEntry>,
    pub selected_resource: Option<usize>,
    pub selected_prompt: Option<usize>,
    pub detail: TextView,
    /// Per-tool form values kept when switching tools.
    pub saved_forms: HashMap<String, Vec<ArgField>>,
}

impl McpSession {
    pub fn new(n: usize) -> Self {
        Self {
            id: format!("mcp_{}", uuid::Uuid::new_v4()),
            name: format!("Server {n}"),
            transport: McpTransportType::Stdio,
            command: String::new(),
            args: String::new(),
            env: String::new(),
            url: String::new(),
            status: McpStatus::Idle,
            error: None,
            server_name: String::new(),
            server_version: String::new(),
            caps: [false; 3],
            tools: Vec::new(),
            resources: Vec::new(),
            prompts: Vec::new(),
            tab: ServerTab::Tools,
            filter: String::new(),
            selected_tool: None,
            fields: Vec::new(),
            form_error: None,
            executing: false,
            result: None,
            result_view: TextView::default(),
            history: Vec::new(),
            selected_resource: None,
            selected_prompt: None,
            detail: TextView::default(),
            saved_forms: HashMap::new(),
        }
    }

    pub fn connected(&self) -> bool {
        self.status == McpStatus::Connected
    }

    pub fn can_connect(&self) -> bool {
        match self.transport {
            McpTransportType::Stdio => !self.command.trim().is_empty(),
            McpTransportType::Http => !self.url.trim().is_empty(),
        }
    }

    pub fn clear_server(&mut self) {
        self.server_name.clear();
        self.server_version.clear();
        self.caps = [false; 3];
        self.tools.clear();
        self.resources.clear();
        self.prompts.clear();
        self.selected_tool = None;
        self.selected_resource = None;
        self.selected_prompt = None;
        self.fields.clear();
        self.saved_forms.clear();
        self.result = None;
        self.executing = false;
    }

    pub fn select_tool(&mut self, i: usize) {
        if let Some(cur) = self.selected_tool {
            if let Some(t) = self.tools.get(cur) {
                self.saved_forms.insert(t.name.clone(), self.fields.clone());
            }
        }
        self.selected_tool = Some(i);
        self.result = None;
        self.form_error = None;
        if let Some(t) = self.tools.get(i) {
            self.fields = self
                .saved_forms
                .get(&t.name)
                .cloned()
                .unwrap_or_else(|| fields_for(&t.schema));
        }
    }

    /// Tool indices matching the filter.
    pub fn visible_tools(&self) -> Vec<usize> {
        let f = self.filter.trim().to_lowercase();
        self.tools
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                f.is_empty()
                    || t.name.to_lowercase().contains(&f)
                    || t.description.to_lowercase().contains(&f)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn set_result(&mut self, r: ToolResult) {
        self.result_view.set(r.text.clone());
        self.result = Some(r);
    }
}

pub struct McpState {
    pub sessions: Vec<McpSession>,
    pub active: usize,
    counter: usize,
}

impl Default for McpState {
    fn default() -> Self {
        Self {
            sessions: vec![McpSession::new(1)],
            active: 0,
            counter: 1,
        }
    }
}

impl McpState {
    pub fn add(&mut self) {
        self.counter += 1;
        self.sessions.push(McpSession::new(self.counter));
        self.active = self.sessions.len() - 1;
    }

    pub fn set_counter(&mut self, n: usize) {
        self.counter = n;
    }

    pub fn by_id(&mut self, id: &str) -> Option<&mut McpSession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn active_mut(&mut self) -> &mut McpSession {
        let i = self.active.min(self.sessions.len() - 1);
        &mut self.sessions[i]
    }
}

/// Splits a command line into words; single / double quotes group words.
pub fn split_args(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for c in input.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                has = true;
            }
            (None, c) if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `KEY=value KEY2="a b"` -> map.
pub fn parse_env(input: &str) -> HashMap<String, String> {
    split_args(input)
        .into_iter()
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            (!k.is_empty()).then(|| (k.to_string(), v.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn form_from_schema_and_build_args() {
        let schema = json!({
            "type": "object",
            "required": ["name", "count"],
            "properties": {
                "name": {"type": "string", "description": "who"},
                "count": {"type": "integer"},
                "ratio": {"type": ["number", "null"], "default": 0.5},
                "on": {"type": "boolean"},
                "mode": {"type": "string", "enum": ["a", "b"]},
                "opts": {"type": "object", "default": {"x": 1}},
                "tags": {"anyOf": [{"type": "null"}, {"type": "array", "items": {"type": "string"}}]}
            }
        });
        let mut f = fields_for(&schema);
        assert_eq!(f.len(), 7);
        assert_eq!(f[0].ty, ArgType::String);
        assert!(f[0].required);
        assert_eq!(f[2].ty, ArgType::Number);
        assert_eq!(f[2].value, "0.5");
        assert_eq!(f[4].choices, ["a", "b"]);
        assert_eq!(f[5].ty, ArgType::Json);
        assert!(f[5].value.contains("\"x\""));
        assert_eq!(f[6].schema_type, "array");

        // required count is empty
        f[0].value = "bob".into();
        assert_eq!(build_args(&f).unwrap_err(), "\"count\" is required");
        f[1].value = "3".into();
        f[3].value = "true".into();
        f[6].value = "[\"a\",\"b\"]".into();
        let a = build_args(&f).unwrap();
        assert_eq!(
            Value::Object(a),
            json!({"name":"bob","count":3,"ratio":0.5,"on":true,"opts":{"x":1},"tags":["a","b"]})
        );
        f[1].value = "x".into();
        assert!(build_args(&f).unwrap_err().contains("integer"));
        f[1].value = "3".into();
        f[5].value = "{oops".into();
        assert!(build_args(&f).unwrap_err().contains("not valid JSON"));
    }

    #[test]
    fn no_properties_means_no_form() {
        assert!(fields_for(&json!({"type":"object","properties":{}})).is_empty());
        assert!(fields_for(&Value::Null).is_empty());
        assert!(build_args(&[]).unwrap().is_empty());
    }

    #[test]
    fn results_parse_text_json_and_errors() {
        let r = ToolResult::from_value(&json!({
            "content": [{"type":"text","text":"{\"a\":1}"},{"type":"text","text":"plain"},{"type":"resource","uri":"x"}],
            "isError": true,
            "structuredContent": {"k": 1}
        }));
        assert!(r.is_error);
        assert_eq!(r.items.len(), 3);
        assert!(r.text.starts_with("{\n  \"a\": 1\n}\nplain\n{"));
        assert!(r.text.contains("\"uri\": \"x\""));
        assert!(r.structured.unwrap().contains("\"k\""));
    }

    #[test]
    fn splits_args_and_env() {
        assert_eq!(
            split_args(r#"-y "my pkg" 'a b' c"#),
            ["-y", "my pkg", "a b", "c"]
        );
        assert!(split_args("   ").is_empty());
        let e = parse_env(r#"A=1 B="x y" bad =no"#);
        assert_eq!(e["A"], "1");
        assert_eq!(e["B"], "x y");
        assert_eq!(e.len(), 2);
    }
}
