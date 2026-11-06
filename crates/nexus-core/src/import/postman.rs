//! Postman Collection v2.1 / environment import (port of `postman-importer.ts`).
//!
//! Works on `serde_json::Value` so odd-but-real exports (numeric values, `exec` as a string,
//! missing optional parts) import like they did in JS instead of failing a strict schema.

use super::script_transpiler::transpile_postman_to_nexus;
use super::types::ImportResult;
use super::util::{parse_method, parse_query_pairs, short_id, text, truthy};
use crate::error::{Error, Result};
use crate::types::*;
use serde_json::{Map, Value};

/// Imports a Postman collection (v2.1 shape) into one Nexus collection plus warnings.
pub fn import_collection(data: &Value) -> Result<ImportResult> {
    let mut ctx = Ctx::default();
    let info = data.get("info");
    let name = text(info.and_then(|i| i.get("name")))
        .ok_or_else(|| Error::Other("Invalid Postman collection: missing info.name".into()))?;
    let items = data
        .get("item")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Other("Invalid Postman collection: missing item array".into()))?;

    let collection = NexusCollection {
        id: short_id("col"),
        name,
        description: text(info.and_then(|i| i.get("description"))),
        version: None,
        variables: arr(data.get("variable"))
            .iter()
            .map(convert_variable)
            .collect(),
        auth: data.get("auth").map(|a| ctx.convert_auth(a)),
        pre_request_script: Some(ctx.extract_script(data.get("event"), "prerequest")),
        post_response_script: Some(ctx.extract_script(data.get("event"), "test")),
        items: items
            .iter()
            .map(|i| ctx.convert_item(i))
            .collect::<Result<_>>()?,
        item_order: None,
    };
    Ok(ImportResult {
        collections: vec![collection],
        environments: vec![],
        warnings: ctx.warnings,
    })
}

/// Imports a Postman environment export.
pub fn import_environment(data: &Value) -> Result<ImportResult> {
    let name = text(data.get("name"))
        .ok_or_else(|| Error::Other("Invalid Postman environment: missing name".into()))?;
    let values = data
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Other("Invalid Postman environment: missing values array".into()))?;
    let environment = NexusEnvironment {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        variables: values.iter().map(convert_variable).collect(),
    };
    Ok(ImportResult {
        collections: vec![],
        environments: vec![environment],
        warnings: vec![],
    })
}

fn arr(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn convert_variable(v: &Value) -> Variable {
    Variable {
        key: text(v.get("key")).unwrap_or_default(),
        value: text(v.get("value")).unwrap_or_default(),
        var_type: if v.get("type").and_then(Value::as_str) == Some("secret") {
            VariableType::Secret
        } else {
            VariableType::String
        },
        enabled: !truthy(v.get("disabled")),
        description: text(v.get("description")),
    }
}

/// `{ key, value, disabled?, description? }` -> [`KeyValuePair`] (Postman headers, query, urlencoded).
fn kv(e: &Value) -> KeyValuePair {
    KeyValuePair {
        key: text(e.get("key")).unwrap_or_default(),
        value: text(e.get("value")).unwrap_or_default(),
        enabled: !truthy(e.get("disabled")),
        description: text(e.get("description")),
    }
}

fn pairs(v: Option<&Value>) -> Vec<KeyValuePair> {
    arr(v).iter().map(kv).collect()
}

#[derive(Default)]
struct Ctx {
    warnings: Vec<String>,
}

impl Ctx {
    fn convert_item(&mut self, item: &Value) -> Result<CollectionItem> {
        if truthy(item.get("item")) && !truthy(item.get("request")) {
            Ok(CollectionItem::Folder(self.convert_folder(item)?))
        } else {
            Ok(CollectionItem::Request(self.convert_request(item)?))
        }
    }

    fn convert_folder(&mut self, item: &Value) -> Result<NexusFolder> {
        let children = arr(item.get("item"));
        Ok(NexusFolder {
            id: short_id("folder"),
            name: text(item.get("name")).unwrap_or_default(),
            description: None,
            items: children
                .iter()
                .map(|i| self.convert_item(i))
                .collect::<Result<_>>()?,
            auth: item.get("auth").map(|a| self.convert_auth(a)),
            pre_request_script: Some(self.extract_script(item.get("event"), "prerequest")),
            post_response_script: Some(self.extract_script(item.get("event"), "test")),
            item_order: None,
        })
    }

    fn convert_request(&mut self, item: &Value) -> Result<NexusRequest> {
        let name = text(item.get("name")).unwrap_or_default();
        let req = item
            .get("request")
            .filter(|r| r.is_object())
            .ok_or_else(|| {
                Error::Other(format!(
                    "Postman item \"{name}\" has neither a request nor nested items"
                ))
            })?;

        let method_text = text(req.get("method")).unwrap_or_else(|| "GET".to_string());
        let method = parse_method(&method_text).unwrap_or_else(|| {
            self.warnings.push(format!(
                "Unsupported HTTP method \"{method_text}\" on \"{name}\" — using GET."
            ));
            HttpMethod::Get
        });
        let url = req.get("url");

        Ok(NexusRequest {
            id: short_id("req"),
            name,
            method,
            url: build_url(url),
            headers: pairs(req.get("header")),
            params: extract_params(url),
            body: convert_body(req.get("body")),
            auth: match req.get("auth") {
                Some(a) => self.convert_auth(a),
                None => AuthConfig::new(AuthType::Inherit),
            },
            description: None,
            pre_request_script: Some(self.extract_script(item.get("event"), "prerequest")),
            post_response_script: Some(self.extract_script(item.get("event"), "test")),
            settings: RequestSettings::default(),
        })
    }

    fn convert_auth(&mut self, auth: &Value) -> AuthConfig {
        let find = |list: &str, key: &str| -> String {
            arr(auth.get(list))
                .iter()
                .find(|e| e.get("key").and_then(Value::as_str) == Some(key))
                .and_then(|e| text(e.get("value")))
                .unwrap_or_default()
        };
        let or = |v: String, default: &str| if v.is_empty() { default.to_string() } else { v };
        let make = |t: AuthType, fields: Vec<(&str, String)>| {
            let mut extra = Map::new();
            for (k, v) in fields {
                extra.insert(k.to_string(), Value::String(v));
            }
            AuthConfig {
                auth_type: t,
                extra,
            }
        };

        let ty = text(auth.get("type")).unwrap_or_default();
        match ty.as_str() {
            "bearer" => make(AuthType::Bearer, vec![("token", find("bearer", "token"))]),
            "basic" => make(
                AuthType::Basic,
                vec![
                    ("username", find("basic", "username")),
                    ("password", find("basic", "password")),
                ],
            ),
            "apikey" => make(
                AuthType::ApiKey,
                vec![
                    ("key", find("apikey", "key")),
                    ("value", find("apikey", "value")),
                    (
                        "addTo",
                        if find("apikey", "in") == "query" {
                            "query"
                        } else {
                            "header"
                        }
                        .to_string(),
                    ),
                ],
            ),
            "oauth2" => make(
                AuthType::Oauth2,
                vec![
                    ("accessToken", find("oauth2", "accessToken")),
                    ("tokenType", or(find("oauth2", "tokenType"), "Bearer")),
                    (
                        "addTo",
                        if find("oauth2", "addTokenTo") == "queryParams" {
                            "query"
                        } else {
                            "header"
                        }
                        .to_string(),
                    ),
                    ("authUrl", find("oauth2", "authUrl")),
                    ("tokenUrl", find("oauth2", "accessTokenUrl")),
                    ("clientId", find("oauth2", "clientId")),
                    ("clientSecret", find("oauth2", "clientSecret")),
                    ("scope", find("oauth2", "scope")),
                    ("redirectUri", find("oauth2", "redirect_uri")),
                ],
            ),
            "digest" => make(
                AuthType::Digest,
                vec![
                    ("username", find("digest", "username")),
                    ("password", find("digest", "password")),
                    ("realm", find("digest", "realm")),
                    ("nonce", find("digest", "nonce")),
                    ("algorithm", or(find("digest", "algorithm"), "MD5")),
                    ("qop", find("digest", "qop")),
                    ("opaque", find("digest", "opaque")),
                ],
            ),
            "awsv4" => make(
                AuthType::AwsSigV4,
                vec![
                    ("accessKey", find("awsv4", "accessKey")),
                    ("secretKey", find("awsv4", "secretKey")),
                    ("region", find("awsv4", "region")),
                    ("service", find("awsv4", "service")),
                    ("sessionToken", find("awsv4", "sessionToken")),
                ],
            ),
            "hawk" => {
                self.warnings
                    .push("Hawk auth imported but not fully supported in Nexus.".into());
                AuthConfig::new(AuthType::None)
            }
            "noauth" => AuthConfig::new(AuthType::None),
            other => {
                self.warnings.push(format!(
                    "Unknown auth type \"{other}\" — defaulting to no auth."
                ));
                AuthConfig::new(AuthType::None)
            }
        }
    }

    /// Joins the `exec` lines of the matching event and transpiles them; `""` when there is none.
    fn extract_script(&mut self, events: Option<&Value>, listen: &str) -> String {
        let Some(event) = arr(events)
            .iter()
            .find(|e| e.get("listen").and_then(Value::as_str) == Some(listen))
        else {
            return String::new();
        };
        let raw = match event.get("script").and_then(|s| s.get("exec")) {
            Some(Value::Array(lines)) => lines
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n"),
            Some(Value::String(s)) => s.clone(),
            _ => return String::new(),
        };
        if raw.trim().is_empty() {
            return String::new();
        }
        let result = transpile_postman_to_nexus(&raw);
        self.warnings
            .extend(result.warnings.into_iter().map(|w| format!("[Script] {w}")));
        result.script
    }
}

fn build_url(url: Option<&Value>) -> String {
    match url {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => o
            .get("raw")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

fn extract_params(url: Option<&Value>) -> Vec<KeyValuePair> {
    match url {
        Some(Value::String(s)) => match s.split('?').nth(1) {
            Some(qs) if s.contains('?') => parse_query_pairs(qs),
            _ => vec![],
        },
        Some(Value::Object(o)) => pairs(o.get("query")),
        _ => vec![],
    }
}

fn convert_body(body: Option<&Value>) -> RequestBody {
    let Some(body) = body else {
        return RequestBody::default();
    };
    let Some(mode) = body
        .get("mode")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
    else {
        return RequestBody::default();
    };
    match mode {
        "raw" => {
            let lang = body
                .get("options")
                .and_then(|o| o.get("raw"))
                .and_then(|r| r.get("language"))
                .and_then(Value::as_str)
                .unwrap_or("text");
            let raw = text(body.get("raw")).unwrap_or_default();
            let mode = if lang == "json" || raw.trim().starts_with('{') {
                BodyMode::Json
            } else if lang == "xml" {
                BodyMode::Xml
            } else if lang == "html" {
                BodyMode::Html
            } else {
                BodyMode::Text
            };
            RequestBody {
                mode,
                raw: Some(raw),
                ..Default::default()
            }
        }
        "urlencoded" => RequestBody {
            mode: BodyMode::XWwwFormUrlencoded,
            urlencoded: Some(pairs(body.get("urlencoded"))),
            ..Default::default()
        },
        "formdata" => RequestBody {
            mode: BodyMode::FormData,
            form_data: Some(
                arr(body.get("formdata"))
                    .iter()
                    .map(|fd| FormDataEntry {
                        key: text(fd.get("key")).unwrap_or_default(),
                        value: text(fd.get("value"))
                            .or_else(|| text(fd.get("src")))
                            .unwrap_or_default(),
                        entry_type: if fd.get("type").and_then(Value::as_str) == Some("file") {
                            FormDataType::File
                        } else {
                            FormDataType::Text
                        },
                        enabled: !truthy(fd.get("disabled")),
                        description: text(fd.get("description")),
                    })
                    .collect(),
            ),
            ..Default::default()
        },
        "graphql" => RequestBody {
            mode: BodyMode::Graphql,
            graphql: Some(GraphqlBody {
                query: text(body.get("graphql").and_then(|g| g.get("query"))).unwrap_or_default(),
                variables: text(body.get("graphql").and_then(|g| g.get("variables"))),
            }),
            ..Default::default()
        },
        "file" => RequestBody {
            mode: BodyMode::Binary,
            ..Default::default()
        },
        _ => RequestBody::default(),
    }
}

#[cfg(test)]
mod tests;
