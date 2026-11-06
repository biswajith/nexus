//! OpenAPI 3.x / Swagger 2.0 import, JSON or YAML (port of `openapi-importer.ts`).
//!
//! The spec stays a `serde_json::Value`; only the parts the importer reads are looked at, so
//! partial or slightly invalid specs import as far as they can, like the JS version.

use super::types::ImportResult;
use super::util::{js_string, non_null, short_id, text, truthy};
use crate::error::{Error, Result};
use crate::types::*;
use indexmap::IndexMap;
use serde_json::{json, Value};

const HTTP_METHODS: [&str; 7] = ["get", "post", "put", "patch", "delete", "head", "options"];

/// Parses a JSON or YAML spec and returns one collection, one environment per server (when there
/// is more than one), and warnings. `$ref`s are not resolved (a warning is produced instead).
pub fn import(data: &str) -> Result<ImportResult> {
    super::check_size(data)?;
    let trimmed = data.trim();
    let spec: Value = if trimmed.starts_with('{') {
        serde_json::from_str(trimmed)?
    } else {
        serde_yaml::from_str(trimmed).map_err(|e| Error::Other(format!("Invalid YAML: {e}")))?
    };
    if !spec.is_object() {
        return Err(Error::Other(
            "Invalid OpenAPI document: expected an object".into(),
        ));
    }
    let title = text(spec.get("info").and_then(|i| i.get("title")))
        .ok_or_else(|| Error::Other("Invalid OpenAPI document: missing info.title".into()))?;

    let mut importer = Importer {
        spec,
        warnings: vec![],
    };
    let is_swagger2 = truthy(importer.spec.get("swagger"));
    let base_url = importer.resolve_base_url(is_swagger2);
    let items = importer.build_items(is_swagger2);

    let collection = NexusCollection {
        id: short_id("col"),
        name: title,
        description: text(importer.spec.get("info").and_then(|i| i.get("description"))),
        version: None,
        variables: vec![Variable {
            key: "base_url".into(),
            value: base_url,
            var_type: VariableType::String,
            enabled: true,
            description: Some("API base URL".into()),
        }],
        auth: None,
        pre_request_script: None,
        post_response_script: None,
        items,
        item_order: None,
    };
    let environments = importer.build_environments();
    Ok(ImportResult {
        collections: vec![collection],
        environments,
        warnings: importer.warnings,
    })
}

struct Importer {
    spec: Value,
    warnings: Vec<String>,
}

fn list(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

impl Importer {
    fn resolve_base_url(&self, is_swagger2: bool) -> String {
        if is_swagger2 {
            let scheme =
                text(list(self.spec.get("schemes")).first()).unwrap_or_else(|| "https".into());
            let host = text(self.spec.get("host")).unwrap_or_else(|| "localhost".into());
            let base_path = text(self.spec.get("basePath")).unwrap_or_default();
            return format!("{scheme}://{host}{base_path}");
        }
        text(
            list(self.spec.get("servers"))
                .first()
                .and_then(|s| s.get("url")),
        )
        .unwrap_or_else(|| "http://localhost".into())
    }

    fn build_environments(&self) -> Vec<NexusEnvironment> {
        let servers = list(self.spec.get("servers"));
        if servers.len() <= 1 {
            return vec![];
        }
        servers
            .iter()
            .enumerate()
            .map(|(i, server)| {
                let description = text(server.get("description"));
                NexusEnvironment {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: description
                        .clone()
                        .unwrap_or_else(|| format!("Server {}", i + 1)),
                    variables: vec![Variable {
                        key: "base_url".into(),
                        value: text(server.get("url")).unwrap_or_default(),
                        var_type: VariableType::String,
                        enabled: true,
                        description,
                    }],
                }
            })
            .collect()
    }

    /// Tag folders first (in first-seen order), then untagged requests.
    fn build_items(&mut self, is_swagger2: bool) -> Vec<CollectionItem> {
        let spec = self.spec.clone();
        let mut tag_map: IndexMap<String, Vec<CollectionItem>> = IndexMap::new();
        let mut untagged: Vec<CollectionItem> = vec![];

        if let Some(paths) = spec.get("paths").and_then(Value::as_object) {
            for (path, path_item) in paths {
                let path_params = list(path_item.get("parameters"));
                for method in HTTP_METHODS {
                    let Some(operation) = path_item.get(method).filter(|o| o.is_object()) else {
                        continue;
                    };
                    let request =
                        self.build_request(path, method, operation, path_params, is_swagger2);
                    let tags: Vec<&str> = list(operation.get("tags"))
                        .iter()
                        .filter_map(Value::as_str)
                        .collect();
                    if tags.is_empty() {
                        untagged.push(CollectionItem::Request(request));
                    } else {
                        for tag in tags {
                            tag_map
                                .entry(tag.to_string())
                                .or_default()
                                .push(CollectionItem::Request(request.clone()));
                        }
                    }
                }
            }
        }

        let mut items: Vec<CollectionItem> = tag_map
            .into_iter()
            .map(|(tag, children)| {
                let description = list(spec.get("tags"))
                    .iter()
                    .find(|t| t.get("name").and_then(Value::as_str) == Some(tag.as_str()))
                    .and_then(|t| text(t.get("description")));
                CollectionItem::Folder(NexusFolder {
                    id: short_id("folder"),
                    name: tag,
                    description,
                    items: children,
                    auth: None,
                    pre_request_script: None,
                    post_response_script: None,
                    item_order: None,
                })
            })
            .collect();
        items.extend(untagged);
        items
    }

    fn build_request(
        &mut self,
        path: &str,
        method: &str,
        operation: &Value,
        path_params: &[Value],
        is_swagger2: bool,
    ) -> NexusRequest {
        let all_params: Vec<&Value> = path_params
            .iter()
            .chain(list(operation.get("parameters")))
            .collect();
        let url = format!("{{{{base_url}}}}{}", convert_path_params(path));

        let located = |location: &str| -> Vec<KeyValuePair> {
            all_params
                .iter()
                .filter(|p| p.get("in").and_then(Value::as_str) == Some(location))
                .map(|p| KeyValuePair {
                    key: text(p.get("name")).unwrap_or_default(),
                    value: example_value(p),
                    enabled: p.get("required") == Some(&Value::Bool(true)),
                    description: text(p.get("description")),
                })
                .collect()
        };
        let params = located("query");
        let mut headers = located("header");

        let body = if is_swagger2 {
            self.build_body_swagger2(&all_params, operation)
        } else {
            self.build_body_openapi3(operation)
        };
        if body.mode == BodyMode::Json
            && !headers
                .iter()
                .any(|h| h.key.eq_ignore_ascii_case("content-type"))
        {
            headers.push(KeyValuePair {
                key: "Content-Type".into(),
                value: "application/json".into(),
                enabled: true,
                description: None,
            });
        }

        let name = text(operation.get("summary"))
            .or_else(|| text(operation.get("operationId")))
            .unwrap_or_else(|| format!("{} {}", method.to_uppercase(), path));
        let deprecated = truthy(operation.get("deprecated"));

        NexusRequest {
            id: short_id("req"),
            name: if deprecated {
                format!("[DEPRECATED] {name}")
            } else {
                name
            },
            method: super::util::parse_method(method).unwrap_or(HttpMethod::Get),
            url,
            headers,
            params,
            body,
            auth: AuthConfig::new(AuthType::Inherit),
            description: text(operation.get("description")).filter(|d| !d.is_empty()),
            pre_request_script: None,
            post_response_script: None,
            settings: RequestSettings::default(),
        }
    }

    fn build_body_openapi3(&mut self, operation: &Value) -> RequestBody {
        let Some(content) = operation
            .get("requestBody")
            .and_then(|b| b.get("content"))
            .filter(|c| c.is_object())
        else {
            return RequestBody::default();
        };

        if let Some(json_content) = content.get("application/json").filter(|c| truthy(Some(c))) {
            let example = non_null(json_content, "example").cloned();
            let body = match (
                example,
                json_content.get("schema").filter(|s| truthy(Some(s))),
            ) {
                (Some(example), _) => example,
                (None, Some(schema)) => self.generate_example(schema),
                (None, None) => json!({}),
            };
            return RequestBody {
                mode: BodyMode::Json,
                raw: Some(pretty(&body)),
                ..Default::default()
            };
        }
        if let Some(form) = content
            .get("application/x-www-form-urlencoded")
            .filter(|c| truthy(Some(c)))
        {
            return RequestBody {
                mode: BodyMode::XWwwFormUrlencoded,
                urlencoded: Some(schema_to_pairs(form.get("schema"))),
                ..Default::default()
            };
        }
        if let Some(multipart) = content
            .get("multipart/form-data")
            .filter(|c| truthy(Some(c)))
        {
            let form_data = schema_to_pairs(multipart.get("schema"))
                .into_iter()
                .map(|p| FormDataEntry {
                    key: p.key,
                    value: p.value,
                    entry_type: FormDataType::Text,
                    enabled: p.enabled,
                    description: p.description,
                })
                .collect();
            return RequestBody {
                mode: BodyMode::FormData,
                form_data: Some(form_data),
                ..Default::default()
            };
        }
        if content
            .get("application/xml")
            .is_some_and(|c| truthy(Some(c)))
            || content.get("text/xml").is_some_and(|c| truthy(Some(c)))
        {
            return RequestBody {
                mode: BodyMode::Xml,
                raw: Some("<!-- XML body -->".into()),
                ..Default::default()
            };
        }
        RequestBody {
            mode: BodyMode::Text,
            raw: Some(String::new()),
            ..Default::default()
        }
    }

    fn build_body_swagger2(&mut self, params: &[&Value], operation: &Value) -> RequestBody {
        let body_schema = params
            .iter()
            .find(|p| p.get("in").and_then(Value::as_str) == Some("body"))
            .and_then(|p| p.get("schema"))
            .filter(|s| truthy(Some(s)));
        let Some(schema) = body_schema else {
            let form_params: Vec<&&Value> = params
                .iter()
                .filter(|p| p.get("in").and_then(Value::as_str) == Some("formData"))
                .collect();
            if form_params.is_empty() {
                return RequestBody::default();
            }
            let multipart = list(operation.get("consumes"))
                .iter()
                .any(|c| c.as_str() == Some("multipart/form-data"));
            if multipart {
                return RequestBody {
                    mode: BodyMode::FormData,
                    form_data: Some(
                        form_params
                            .iter()
                            .map(|p| FormDataEntry {
                                key: text(p.get("name")).unwrap_or_default(),
                                value: example_value(p),
                                entry_type: if p.get("type").and_then(Value::as_str) == Some("file")
                                {
                                    FormDataType::File
                                } else {
                                    FormDataType::Text
                                },
                                enabled: true,
                                description: text(p.get("description")),
                            })
                            .collect(),
                    ),
                    ..Default::default()
                };
            }
            return RequestBody {
                mode: BodyMode::XWwwFormUrlencoded,
                urlencoded: Some(
                    form_params
                        .iter()
                        .map(|p| KeyValuePair {
                            key: text(p.get("name")).unwrap_or_default(),
                            value: example_value(p),
                            enabled: true,
                            description: text(p.get("description")),
                        })
                        .collect(),
                ),
                ..Default::default()
            };
        };
        let example = self.generate_example(schema);
        RequestBody {
            mode: BodyMode::Json,
            raw: Some(pretty(&example)),
            ..Default::default()
        }
    }

    /// Placeholder JSON for a schema. Unresolved `$ref`s become `{}` plus a warning.
    fn generate_example(&mut self, schema: &Value) -> Value {
        if let Some(example) = schema.get("example") {
            return example.clone();
        }
        if let Some(reference) = schema.get("$ref").filter(|r| truthy(Some(r))) {
            self.warnings.push(format!(
                "$ref \"{}\" not resolved — using empty object.",
                js_string(reference)
            ));
            return json!({});
        }
        let default = || non_null(schema, "default").cloned();
        match schema.get("type").and_then(Value::as_str) {
            Some("object") => {
                let mut obj = serde_json::Map::new();
                if let Some(props) = schema.get("properties").and_then(Value::as_object) {
                    for (key, prop) in props {
                        obj.insert(key.clone(), self.generate_example(prop));
                    }
                }
                Value::Object(obj)
            }
            Some("array") => match schema.get("items").filter(|i| truthy(Some(i))) {
                Some(items) => Value::Array(vec![self.generate_example(items)]),
                None => Value::Array(vec![]),
            },
            Some("string") => {
                if let Some(values) = schema.get("enum").filter(|e| truthy(Some(e))) {
                    return values.get(0).cloned().unwrap_or(Value::Null);
                }
                match schema.get("format").and_then(Value::as_str) {
                    Some("date") => json!("2026-01-01"),
                    Some("date-time") => json!("2026-01-01T00:00:00Z"),
                    Some("email") => json!("user@example.com"),
                    Some("uri") | Some("url") => json!("https://example.com"),
                    Some("uuid") => json!("550e8400-e29b-41d4-a716-446655440000"),
                    _ => default().unwrap_or_else(|| json!("string")),
                }
            }
            Some("integer") | Some("number") => default().unwrap_or_else(|| json!(0)),
            Some("boolean") => default().unwrap_or_else(|| json!(true)),
            _ => default().unwrap_or(Value::Null),
        }
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| "null".into())
}

/// `/users/{id}` -> `/users/:id`.
fn convert_path_params(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(open) = rest.find('{') {
        match rest[open + 1..].find('}') {
            Some(close) if close > 0 => {
                out.push_str(&rest[..open]);
                out.push(':');
                out.push_str(&rest[open + 1..open + 1 + close]);
                rest = &rest[open + close + 2..];
            }
            _ => {
                out.push_str(&rest[..=open]);
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn schema_to_pairs(schema: Option<&Value>) -> Vec<KeyValuePair> {
    let Some(schema) = schema else { return vec![] };
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return vec![];
    };
    let required: Vec<&str> = list(schema.get("required"))
        .iter()
        .filter_map(Value::as_str)
        .collect();
    props
        .iter()
        .map(|(key, prop)| KeyValuePair {
            key: key.clone(),
            value: non_null(prop, "example")
                .or_else(|| non_null(prop, "default"))
                .map(js_string)
                .unwrap_or_default(),
            enabled: required.contains(&key.as_str()),
            description: text(prop.get("description")),
        })
        .collect()
}

/// Best-effort string for a parameter: example, default, schema example/default, first enum value.
fn example_value(param: &Value) -> String {
    let schema = param.get("schema");
    param
        .get("example")
        .or_else(|| param.get("default"))
        .or_else(|| schema.and_then(|s| s.get("example")))
        .or_else(|| schema.and_then(|s| s.get("default")))
        .or_else(|| schema.and_then(|s| s.get("enum")).and_then(|e| e.get(0)))
        .map(js_string)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
