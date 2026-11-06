//! Host side of `nx.expect`: result type and JSON Schema validation (`ajv` in the TS, the
//! `jsonschema` crate here). The chain API itself is `assertions.js`, which runs in the sandbox.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Outcome of one `nx.test(...)` call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration: f64,
}

/// Validates `value` against `schema` (formats enabled, all errors collected, no remote `$ref`
/// resolution). Returns `Ok(())` or the joined error text (`"<instancePath> <message>"` per error,
/// separated by `; `, like the TS).
pub(crate) fn match_schema(value: &Value, schema: &Value) -> Result<(), String> {
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(|e| e.to_string())?;
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|e| format!("{} {}", e.instance_path(), e))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
