//! Nexus JSON and Postman Collection v2.1 / environment export (port of `nexus-exporter.ts`).

use super::script_transpiler::transpile_nexus_to_postman;
use super::util::{encode_uri_component, js_string};
use crate::error::Result;
use crate::types::*;
use serde::Serialize;
use serde_json::{json, Value};

const POSTMAN_SCHEMA: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";

fn pretty<T: Serialize>(v: &T) -> Result<String> {
    Ok(serde_json::to_string_pretty(v)?)
}

#[derive(Serialize)]
struct PostmanCollection {
    info: PostmanInfo,
    item: Vec<PostmanItem>,
    variable: Vec<PostmanVariable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<Vec<Value>>,
}

#[derive(Serialize)]
struct PostmanInfo {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    schema: &'static str,
    #[serde(rename = "_postman_id")]
    postman_id: String,
}

#[derive(Serialize)]
struct PostmanVariable {
    key: String,
    value: String,
    disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "type")]
    var_type: &'static str,
}

#[derive(Serialize)]
struct PostmanItem {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<Vec<PostmanItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<PostmanRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<Value>,
}

#[derive(Serialize)]
struct PostmanRequest {
    method: &'static str,
    url: PostmanUrl,
    header: Vec<PostmanKv>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<Value>,
}

#[derive(Serialize)]
struct PostmanUrl {
    raw: String,
    query: Vec<PostmanKv>,
}

#[derive(Serialize)]
struct PostmanKv {
    key: String,
    value: String,
    disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

fn to_kv(p: &KeyValuePair) -> PostmanKv {
    PostmanKv {
        key: p.key.clone(),
        value: p.value.clone(),
        disabled: !p.enabled,
        description: p.description.clone(),
    }
}

/// Pretty-printed Nexus collection JSON.
pub fn export_as_nexus_json(collection: &NexusCollection) -> Result<String> {
    pretty(collection)
}

/// Postman Collection v2.1 JSON. Nexus scripts are transpiled back to `pm.*`.
pub fn export_as_postman_v21(collection: &NexusCollection) -> Result<String> {
    let postman = PostmanCollection {
        info: PostmanInfo {
            name: collection.name.clone(),
            description: collection.description.clone(),
            schema: POSTMAN_SCHEMA,
            postman_id: collection.id.clone(),
        },
        item: collection.items.iter().map(convert_item).collect(),
        variable: collection
            .variables
            .iter()
            .map(|v| PostmanVariable {
                key: v.key.clone(),
                value: v.value.clone(),
                disabled: !v.enabled,
                description: v.description.clone(),
                var_type: if v.var_type == VariableType::Secret {
                    "secret"
                } else {
                    "string"
                },
            })
            .collect(),
        auth: collection
            .auth
            .as_ref()
            .filter(|a| !matches!(a.auth_type, AuthType::None | AuthType::Inherit))
            .map(convert_auth),
        event: events(
            collection.pre_request_script.as_deref(),
            collection.post_response_script.as_deref(),
        ),
    };
    pretty(&postman)
}

/// Postman environment JSON; secret values are blanked.
pub fn export_environment_as_postman(env: &NexusEnvironment) -> Result<String> {
    let values: Vec<Value> = env
        .variables
        .iter()
        .map(|v| {
            let secret = v.var_type == VariableType::Secret;
            json!({
                "key": v.key,
                "value": if secret { "" } else { v.value.as_str() },
                "enabled": v.enabled,
                "type": if secret { "secret" } else { "default" },
            })
        })
        .collect();
    pretty(&json!({ "name": env.name, "_postman_variable_scope": "environment", "values": values }))
}

/// Nexus environment JSON (what `export:environment` returns for the `nexus` format).
pub fn export_environment_as_nexus_json(env: &NexusEnvironment) -> Result<String> {
    pretty(env)
}

fn convert_item(item: &CollectionItem) -> PostmanItem {
    match item {
        CollectionItem::Folder(f) => PostmanItem {
            name: f.name.clone(),
            item: Some(f.items.iter().map(convert_item).collect()),
            request: None,
            event: events(
                f.pre_request_script.as_deref(),
                f.post_response_script.as_deref(),
            ),
            auth: f
                .auth
                .as_ref()
                .filter(|a| a.auth_type != AuthType::Inherit)
                .map(convert_auth),
        },
        CollectionItem::Request(r) => convert_request(r),
    }
}

fn convert_request(req: &NexusRequest) -> PostmanItem {
    PostmanItem {
        name: req.name.clone(),
        item: None,
        request: Some(PostmanRequest {
            method: req.method.as_str(),
            url: PostmanUrl {
                raw: format!("{}{}", req.url, query_string(&req.params)),
                query: req.params.iter().map(to_kv).collect(),
            },
            header: req.headers.iter().map(to_kv).collect(),
            body: convert_body(&req.body),
            auth: Some(&req.auth)
                .filter(|a| !matches!(a.auth_type, AuthType::Inherit | AuthType::None))
                .map(convert_auth),
        }),
        event: events(
            req.pre_request_script.as_deref(),
            req.post_response_script.as_deref(),
        ),
        auth: None,
    }
}

fn kv_values(entries: &[KeyValuePair]) -> Vec<Value> {
    entries
        .iter()
        .map(|e| {
            let mut o = json!({ "key": e.key, "value": e.value, "disabled": !e.enabled });
            if let Some(d) = &e.description {
                o["description"] = json!(d);
            }
            o
        })
        .collect()
}

fn convert_body(body: &RequestBody) -> Option<Value> {
    let raw = |language: &str| json!({ "mode": "raw", "raw": body.raw.clone().unwrap_or_default(), "options": { "raw": { "language": language } } });
    match body.mode {
        BodyMode::None | BodyMode::Binary => None,
        BodyMode::Json => Some(raw("json")),
        BodyMode::Xml => Some(raw("xml")),
        BodyMode::Html => Some(raw("html")),
        BodyMode::Text => Some(raw("text")),
        BodyMode::XWwwFormUrlencoded => Some(json!({
            "mode": "urlencoded",
            "urlencoded": kv_values(body.urlencoded.as_deref().unwrap_or(&[])),
        })),
        BodyMode::FormData => Some(json!({
            "mode": "formdata",
            "formdata": body.form_data.as_deref().unwrap_or(&[]).iter().map(|fd| {
                let mut o = json!({
                    "key": fd.key,
                    "value": fd.value,
                    "type": if fd.entry_type == FormDataType::File { "file" } else { "text" },
                    "disabled": !fd.enabled,
                });
                if let Some(d) = &fd.description {
                    o["description"] = json!(d);
                }
                o
            }).collect::<Vec<_>>(),
        })),
        BodyMode::Graphql => {
            let mut g = serde_json::Map::new();
            if let Some(gq) = &body.graphql {
                g.insert("query".into(), json!(gq.query));
                if let Some(v) = &gq.variables {
                    g.insert("variables".into(), json!(v));
                }
            }
            Some(json!({ "mode": "graphql", "graphql": g }))
        }
    }
}

fn convert_auth(auth: &AuthConfig) -> Value {
    let field = |key: &str| {
        auth.extra
            .get(key)
            .filter(|v| !v.is_null())
            .map(js_string)
            .unwrap_or_default()
    };
    let field_or = |key: &str, default: &str| {
        auth.extra
            .get(key)
            .filter(|v| !v.is_null())
            .map(js_string)
            .unwrap_or_else(|| default.to_string())
    };
    let arr = |pairs: Vec<(&str, String)>| -> Vec<Value> {
        pairs
            .into_iter()
            .map(|(k, v)| json!({ "key": k, "value": v }))
            .collect()
    };
    let add_to_query = auth.extra.get("addTo").and_then(Value::as_str) == Some("query");

    match auth.auth_type {
        AuthType::Bearer => {
            json!({ "type": "bearer", "bearer": arr(vec![("token", field("token"))]) })
        }
        AuthType::Basic => {
            json!({ "type": "basic", "basic": arr(vec![("username", field("username")), ("password", field("password"))]) })
        }
        AuthType::ApiKey => json!({ "type": "apikey", "apikey": arr(vec![
            ("key", field("key")),
            ("value", field("value")),
            ("in", if add_to_query { "query" } else { "header" }.to_string()),
        ]) }),
        AuthType::Oauth2 => json!({ "type": "oauth2", "oauth2": arr(vec![
            ("accessToken", field("accessToken")),
            ("tokenType", field_or("tokenType", "Bearer")),
            ("addTokenTo", if add_to_query { "queryParams" } else { "header" }.to_string()),
        ]) }),
        AuthType::Digest => json!({ "type": "digest", "digest": arr(vec![
            ("username", field("username")), ("password", field("password")),
            ("realm", field("realm")), ("nonce", field("nonce")),
        ]) }),
        AuthType::AwsSigV4 => json!({ "type": "awsv4", "awsv4": arr(vec![
            ("accessKey", field("accessKey")), ("secretKey", field("secretKey")),
            ("region", field("region")), ("service", field("service")),
        ]) }),
        _ => json!({ "type": "noauth" }),
    }
}

fn event(listen: &str, script: &str) -> Value {
    let transpiled = transpile_nexus_to_postman(script).script;
    json!({
        "listen": listen,
        "script": { "type": "text/javascript", "exec": transpiled.split('\n').collect::<Vec<_>>() },
    })
}

/// Events for non-empty scripts; `None` when there are none.
fn events(pre: Option<&str>, post: Option<&str>) -> Option<Vec<Value>> {
    let mut out = vec![];
    if let Some(s) = pre.filter(|s| !s.is_empty()) {
        out.push(event("prerequest", s));
    }
    if let Some(s) = post.filter(|s| !s.is_empty()) {
        out.push(event("test", s));
    }
    (!out.is_empty()).then_some(out)
}

fn query_string(params: &[KeyValuePair]) -> String {
    let enabled: Vec<String> = params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .map(|p| {
            format!(
                "{}={}",
                encode_uri_component(&p.key),
                encode_uri_component(&p.value)
            )
        })
        .collect();
    if enabled.is_empty() {
        String::new()
    } else {
        format!("?{}", enabled.join("&"))
    }
}

#[cfg(test)]
mod tests;
