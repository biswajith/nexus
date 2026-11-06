//! Data files for data-driven iterations: CSV (header row + records) and JSON (array of objects)
//! parsed into the `dataRows` a run cycles through.

use indexmap::IndexMap;
use serde_json::Value;

use crate::{Error, Result};

pub type DataRow = IndexMap<String, String>;

/// Parses CSV with a header row. A UTF-8 BOM is stripped, quoted fields may contain commas,
/// quotes (`""`) and newlines, short records are padded with empty strings and extra fields are
/// ignored. Blank lines are skipped.
pub fn parse_csv(text: &str) -> Result<Vec<DataRow>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| Error::Other(format!("Invalid CSV: {e}")))?
        .iter()
        .map(str::to_string)
        .collect();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| Error::Other(format!("Invalid CSV: {e}")))?;
        rows.push(
            headers
                .iter()
                .enumerate()
                .map(|(i, h)| (h.clone(), record.get(i).unwrap_or("").to_string()))
                .collect(),
        );
    }
    Ok(rows)
}

/// Parses a JSON array of objects (a single object counts as one row). Scalars are stringified
/// (`null` becomes `""`), nested values are kept as compact JSON text.
pub fn parse_json(text: &str) -> Result<Vec<DataRow>> {
    let value: Value = serde_json::from_str(text)
        .map_err(|e| Error::Other(format!("Invalid JSON data file: {e}")))?;
    let items = match value {
        Value::Array(items) => items,
        obj @ Value::Object(_) => vec![obj],
        _ => {
            return Err(Error::Other(
                "JSON data file must be an array of objects".into(),
            ))
        }
    };
    items
        .into_iter()
        .map(|item| match item {
            Value::Object(map) => Ok(map
                .into_iter()
                .map(|(k, v)| {
                    let s = match v {
                        Value::String(s) => s,
                        Value::Null => String::new(),
                        other => other.to_string(),
                    };
                    (k, s)
                })
                .collect()),
            _ => Err(Error::Other(
                "JSON data file must be an array of objects".into(),
            )),
        })
        .collect()
}

/// Picks the parser from the file name's extension (`.csv` / `.json`).
pub fn parse_data_file(file_name: &str, text: &str) -> Result<Vec<DataRow>> {
    match file_name
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => parse_csv(text),
        Some("json") => parse_json(text),
        _ => Err(Error::Other(format!(
            "Unsupported data file type: {file_name}"
        ))),
    }
}
