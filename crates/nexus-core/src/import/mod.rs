//! Port of `packages/core/src/import`: cURL, Postman v2.1, OpenAPI 3 / Swagger 2 importers, the
//! Nexus / Postman exporter and the `pm.*` <-> `nx.*` script transpiler.

pub mod curl;
pub mod exporter;
pub mod openapi;
pub mod postman;
pub mod script_transpiler;
pub mod types;
mod util;

pub use types::{detect_import_format, ImportFormat, ImportResult};

use crate::error::{Error, Result};
use crate::types::{NexusCollection, NexusRequest};
use serde_json::Value;

/// Largest import payload accepted (spec files, Postman exports, cURL text). The TS code had no
/// limit; untrusted files are parsed fully into memory, so cap them.
pub const MAX_IMPORT_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn check_size(data: &str) -> Result<()> {
    if data.len() > MAX_IMPORT_BYTES {
        return Err(Error::Other(format!(
            "Import data is too large ({} bytes, limit {} bytes)",
            data.len(),
            MAX_IMPORT_BYTES
        )));
    }
    Ok(())
}

/// The `import:file` handler: detects the format when none is given, then imports.
pub fn import_data(data: &str, format: Option<ImportFormat>) -> Result<ImportResult> {
    check_size(data)?;
    let format = format.unwrap_or_else(|| detect_import_format(data));
    match format {
        ImportFormat::PostmanCollectionV21 => {
            let parsed: Value = serde_json::from_str(data)?;
            postman::import_collection(&parsed)
        }
        ImportFormat::PostmanEnvironment => {
            let parsed: Value = serde_json::from_str(data)?;
            postman::import_environment(&parsed)
        }
        ImportFormat::Openapi3 | ImportFormat::Swagger2 => openapi::import(data),
        ImportFormat::Curl => {
            let request: NexusRequest = curl::import(data)?;
            let collection = NexusCollection {
                id: util::short_id("col"),
                name: "Imported cURL Request".to_string(),
                description: None,
                version: None,
                variables: vec![],
                auth: None,
                pre_request_script: None,
                post_response_script: None,
                items: vec![crate::types::CollectionItem::Request(request)],
                item_order: None,
            };
            Ok(ImportResult {
                collections: vec![collection],
                environments: vec![],
                warnings: vec![],
            })
        }
        ImportFormat::Nexus => {
            let collection: NexusCollection = serde_json::from_str(data)?;
            Ok(ImportResult {
                collections: vec![collection],
                environments: vec![],
                warnings: vec![],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_by_detected_format() {
        let r = import_data("curl -X POST https://api.example.com/users", None).unwrap();
        assert_eq!(r.collections[0].name, "Imported cURL Request");
        assert!(r.collections[0].id.starts_with("col_"));
        assert_eq!(r.collections[0].items.len(), 1);

        let r = import_data(r#"{"name":"E","values":[{"key":"a","value":"b"}]}"#, None).unwrap();
        assert_eq!(r.environments.len(), 1);

        let r = import_data("openapi: 3.0.0\ninfo:\n  title: T\n  version: '1'\n", None).unwrap();
        assert_eq!(r.collections[0].name, "T");
    }

    #[test]
    fn explicit_format_wins_over_detection() {
        let nexus = r#"{"id":"c1","name":"N","variables":[],"items":[],"version":"1"}"#;
        let r = import_data(nexus, Some(ImportFormat::Nexus)).unwrap();
        assert_eq!(r.collections[0].id, "c1");
        assert!(import_data(nexus, Some(ImportFormat::PostmanCollectionV21)).is_err());
    }

    #[test]
    fn invalid_json_and_oversized_data_are_errors() {
        assert!(import_data("{not json", Some(ImportFormat::Nexus)).is_err());
        let big = " ".repeat(MAX_IMPORT_BYTES + 1);
        assert!(import_data(&big, None).is_err());
    }
}
