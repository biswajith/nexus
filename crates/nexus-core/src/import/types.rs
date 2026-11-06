use super::util::truthy;
use crate::types::{NexusCollection, NexusEnvironment};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub collections: Vec<NexusCollection>,
    pub environments: Vec<NexusEnvironment>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportFormat {
    #[serde(rename = "postman-collection-v2.1")]
    PostmanCollectionV21,
    #[serde(rename = "postman-environment")]
    PostmanEnvironment,
    #[serde(rename = "openapi-3")]
    Openapi3,
    #[serde(rename = "swagger-2")]
    Swagger2,
    #[serde(rename = "curl")]
    Curl,
    #[serde(rename = "nexus")]
    Nexus,
}

/// Guesses the format of raw import text (`import:detect-format`). Unknown input is `Nexus`.
pub fn detect_import_format(data: &str) -> ImportFormat {
    let trimmed = data.trim();

    if trimmed.starts_with('{') {
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            let info = parsed.get("info");
            let schema_is_collection = info
                .and_then(|i| i.get("schema"))
                .and_then(Value::as_str)
                .is_some_and(|s| s.contains("collection.json"));
            if schema_is_collection || truthy(info.and_then(|i| i.get("_postman_id"))) {
                return ImportFormat::PostmanCollectionV21;
            }

            let scope_env = parsed
                .get("_postman_variable_scope")
                .and_then(Value::as_str)
                == Some("environment");
            let named_values =
                truthy(parsed.get("name")) && parsed.get("values").is_some_and(Value::is_array);
            if scope_env || named_values {
                return ImportFormat::PostmanEnvironment;
            }

            if parsed
                .get("openapi")
                .and_then(Value::as_str)
                .is_some_and(|s| s.starts_with('3'))
            {
                return ImportFormat::Openapi3;
            }
            if parsed.get("swagger").and_then(Value::as_str) == Some("2.0") {
                return ImportFormat::Swagger2;
            }
            if truthy(parsed.get("id"))
                && truthy(parsed.get("items"))
                && truthy(parsed.get("version"))
            {
                return ImportFormat::Nexus;
            }
        }
    }

    if trimmed.starts_with("openapi:") || trimmed.contains("openapi: ") {
        return ImportFormat::Openapi3;
    }
    if trimmed.starts_with("swagger:") || trimmed.contains("swagger: ") {
        return ImportFormat::Swagger2;
    }
    if trimmed.starts_with("curl ") || trimmed.starts_with("curl\t") {
        return ImportFormat::Curl;
    }
    ImportFormat::Nexus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_each_format() {
        assert_eq!(
            detect_import_format(
                r#"{"info":{"name":"x","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[]}"#
            ),
            ImportFormat::PostmanCollectionV21
        );
        assert_eq!(
            detect_import_format(r#"{"info":{"_postman_id":"abc"}}"#),
            ImportFormat::PostmanCollectionV21
        );
        assert_eq!(
            detect_import_format(r#"{"name":"Env","values":[]}"#),
            ImportFormat::PostmanEnvironment
        );
        assert_eq!(
            detect_import_format(r#"{"_postman_variable_scope":"environment"}"#),
            ImportFormat::PostmanEnvironment
        );
        assert_eq!(
            detect_import_format(r#"{"openapi":"3.0.1"}"#),
            ImportFormat::Openapi3
        );
        assert_eq!(
            detect_import_format(r#"{"swagger":"2.0"}"#),
            ImportFormat::Swagger2
        );
        assert_eq!(
            detect_import_format(r#"{"id":"c","items":[],"version":"1"}"#),
            ImportFormat::Nexus
        );
        assert_eq!(
            detect_import_format("openapi: 3.0.0\ninfo: {}"),
            ImportFormat::Openapi3
        );
        assert_eq!(
            detect_import_format("swagger: '2.0'"),
            ImportFormat::Swagger2
        );
        assert_eq!(
            detect_import_format("  curl https://x.com"),
            ImportFormat::Curl
        );
        assert_eq!(detect_import_format("something else"), ImportFormat::Nexus);
    }

    #[test]
    fn format_names_match_ts() {
        assert_eq!(
            serde_json::to_string(&ImportFormat::PostmanCollectionV21).unwrap(),
            "\"postman-collection-v2.1\""
        );
        assert_eq!(
            serde_json::from_str::<ImportFormat>("\"swagger-2\"").unwrap(),
            ImportFormat::Swagger2
        );
    }
}
