//! Small helpers shared by the importers/exporter, mimicking the JS semantics the TS code relied on.

use crate::types::HttpMethod;
use serde_json::Value;

/// `` `${prefix}_${crypto.randomUUID().slice(0, 8)}` ``.
pub(crate) fn short_id(prefix: &str) -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &id[..8])
}

/// JS `String(value)` for the JSON value kinds an import can produce.
pub(crate) fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e21 => {
                format!("{}", f as i128)
            }
            _ => n.to_string(),
        },
        Value::Array(a) => a
            .iter()
            .map(|x| {
                if x.is_null() {
                    String::new()
                } else {
                    js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// Text of a scalar JSON value (strings as-is, numbers/bools stringified); `None` otherwise.
pub(crate) fn text(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) => Some(s.clone()),
        Some(v @ (Value::Number(_) | Value::Bool(_))) => Some(js_string(v)),
        _ => None,
    }
}

/// JS truthiness.
pub(crate) fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// A present, non-null member (JS `??` semantics).
pub(crate) fn non_null<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get(key).filter(|x| !x.is_null())
}

/// Upper-cases and maps to [`HttpMethod`]; `None` for methods Nexus does not model.
pub(crate) fn parse_method(s: &str) -> Option<HttpMethod> {
    Some(match s.to_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        _ => return None,
    })
}

/// JS `decodeURIComponent`, except that malformed input is returned unchanged instead of throwing,
/// so one odd query string cannot abort a whole import.
pub(crate) fn decode_uri_component(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok());
            match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                None => return s.to_string(),
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// JS `encodeURIComponent`.
pub(crate) fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// `qs.split('&').map(p => { const [k = '', v = ''] = p.split('='); ... decodeURIComponent })`.
pub(crate) fn parse_query_pairs(qs: &str) -> Vec<crate::types::KeyValuePair> {
    qs.split('&')
        .map(|pair| {
            let mut parts = pair.split('=');
            let key = parts.next().unwrap_or("");
            let value = parts.next().unwrap_or("");
            crate::types::KeyValuePair {
                key: decode_uri_component(key),
                value: decode_uri_component(value),
                enabled: true,
                description: None,
            }
        })
        .collect()
}
