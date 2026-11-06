//! Port of `packages/core/src/codegen/curl-generator.ts`.
//!
//! Security: every request-derived value goes through [`shell_quote`]; raw bodies use
//! `--data-raw` and form text uses `--form-string` so curl never reads a local file from a value.

use serde_json::{Map, Value};

use crate::types::{AuthType, BodyMode, FormDataType, HttpMethod, NexusRequest};
use crate::Result;

/// Builds a multi-line `curl` command (parts joined by `" \\\n  "`) mirroring the request.
///
/// Fails only when a GraphQL body's `variables` is not valid JSON (the TS code threw there too).
pub fn generate_curl(request: &NexusRequest) -> Result<String> {
    let mut parts: Vec<String> = vec!["curl".to_string()];

    if request.method != HttpMethod::Get {
        // The method is a closed enum of ASCII letters, so it is shell-safe as is.
        parts.push(format!("-X {}", request.method.as_str()));
    }

    let mut url = request.url.clone();
    let query: Vec<String> = request
        .params
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
    if !query.is_empty() {
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&query.join("&"));
    }
    parts.push(shell_quote(&url));

    for h in request
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.is_empty())
    {
        parts.push(format!(
            "-H {}",
            shell_quote(&format!("{}: {}", h.key, h.value))
        ));
    }

    let auth = &request.auth.extra;
    match request.auth.auth_type {
        AuthType::Bearer => {
            let token = auth_str(auth, "token");
            if !token.is_empty() {
                parts.push(format!(
                    "-H {}",
                    shell_quote(&format!("Authorization: Bearer {token}"))
                ));
            }
        }
        AuthType::Basic => {
            let user = auth_str(auth, "username");
            let pass = auth_str(auth, "password");
            parts.push(format!("-u {}", shell_quote(&format!("{user}:{pass}"))));
        }
        AuthType::ApiKey if auth.get("addTo").and_then(Value::as_str) == Some("header") => {
            let key = auth_str(auth, "key");
            let value = auth_str(auth, "value");
            if !key.is_empty() && !value.is_empty() {
                parts.push(format!("-H {}", shell_quote(&format!("{key}: {value}"))));
            }
        }
        _ => {}
    }

    let has_content_type = request
        .headers
        .iter()
        .any(|h| h.enabled && h.key.eq_ignore_ascii_case("content-type"));
    let body = &request.body;
    match body.mode {
        BodyMode::Json | BodyMode::Xml | BodyMode::Text | BodyMode::Html => {
            if let Some(raw) = body.raw.as_deref().filter(|r| !r.is_empty()) {
                let ct = match body.mode {
                    BodyMode::Json => "application/json",
                    BodyMode::Xml => "application/xml",
                    BodyMode::Html => "text/html",
                    _ => "text/plain",
                };
                if !has_content_type {
                    parts.push(format!(
                        "-H {}",
                        shell_quote(&format!("Content-Type: {ct}"))
                    ));
                }
                // --data-raw: with -d, a body starting with "@" makes curl read and send a local file.
                parts.push(format!("--data-raw {}", shell_quote(raw)));
            }
        }
        BodyMode::XWwwFormUrlencoded => {
            for kv in body.urlencoded.iter().flatten().filter(|kv| kv.enabled) {
                // Encoding the name keeps "@" out of it, which curl would read as "name@filename".
                parts.push(format!(
                    "--data-urlencode {}",
                    shell_quote(&format!("{}={}", encode_uri_component(&kv.key), kv.value))
                ));
            }
        }
        BodyMode::FormData => {
            for fd in body.form_data.iter().flatten().filter(|fd| fd.enabled) {
                if fd.entry_type == FormDataType::File {
                    // One line on purpose: the source-scan guard test checks `-F` lines for `=@`.
                    #[rustfmt::skip]
                    parts.push(format!("-F {}", shell_quote(&format!("{}=@{}", fd.key, fd.value))));
                } else {
                    // --form-string: with -F, a value starting with "@" or "<" reads a local file.
                    parts.push(format!(
                        "--form-string {}",
                        shell_quote(&format!("{}={}", fd.key, fd.value))
                    ));
                }
            }
        }
        BodyMode::Binary => {
            if let Some(path) = body
                .binary
                .as_ref()
                .map(|b| b.path.as_str())
                .filter(|p| !p.is_empty())
            {
                parts.push(format!(
                    "--data-binary {}",
                    shell_quote(&format!("@{path}"))
                ));
            }
        }
        BodyMode::Graphql => {
            if let Some(gql) = body.graphql.as_ref().filter(|g| !g.query.is_empty()) {
                let mut payload = Map::new();
                payload.insert("query".to_string(), Value::String(gql.query.clone()));
                if let Some(vars) = gql.variables.as_deref().filter(|v| !v.is_empty()) {
                    payload.insert("variables".to_string(), serde_json::from_str(vars)?);
                }
                if !has_content_type {
                    parts.push(format!(
                        "-H {}",
                        shell_quote("Content-Type: application/json")
                    ));
                }
                parts.push(format!(
                    "--data-raw {}",
                    shell_quote(&serde_json::to_string(&payload)?)
                ));
            }
        }
        BodyMode::None => {}
    }

    if request.settings.follow_redirects != Some(false) {
        parts.push("-L".to_string());
    }
    if request.settings.reject_unauthorized == Some(false) {
        parts.push("-k".to_string());
    }
    // Typed numbers only, so nothing but digits can reach the command line.
    if let Some(timeout) = request.settings.timeout.filter(|t| *t > 0) {
        parts.push(format!("--max-time {}", timeout.div_ceil(1000)));
    }
    if let Some(max) = request.settings.max_redirects.filter(|m| *m > 0) {
        parts.push(format!("--max-redirs {max}"));
    }

    Ok(parts.join(" \\\n  "))
}

/// Quotes one argument for a POSIX shell: wraps it in single quotes and replaces each `'` with
/// `'\''`. Every request-derived value in the command must go through this.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// String form of an auth field (`String(x ?? '')` in the TS code).
fn auth_str(auth: &Map<String, Value>, key: &str) -> String {
    match auth.get(key) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// JS `encodeURIComponent`: everything except `A-Za-z0-9 - _ . ! ~ * ' ( )` is percent-encoded.
pub(crate) fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests;
