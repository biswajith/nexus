//! cURL command -> [`NexusRequest`] (port of `curl-importer.ts`).

use super::util::{parse_method, parse_query_pairs, short_id};
use crate::error::{Error, Result};
use crate::types::{
    AuthConfig, AuthType, BodyMode, FormDataEntry, FormDataType, HttpMethod, KeyValuePair,
    NexusRequest, RequestBody, RequestSettings,
};
use base64::Engine;

/// Parses a curl command (line continuations are normalized to spaces) into a request.
///
/// Errors on an unsupported `-X` method, a URL that cannot be parsed, `-u` credentials that
/// `btoa` could not encode, or oversized input.
pub fn import(curl_command: &str) -> Result<NexusRequest> {
    super::check_size(curl_command)?;
    let normalized = curl_command.replace("\\\n", " ").replace("\\\r\n", " ");
    let tokens = tokenize(normalized.trim());

    let mut method = HttpMethod::Get;
    let mut url = String::new();
    let mut headers: Vec<KeyValuePair> = vec![];
    let mut body_raw: Option<String> = None;
    let mut body_mode = BodyMode::None;
    let mut form_data: Vec<KeyValuePair> = vec![];

    let kv = |key: &str, value: &str| KeyValuePair {
        key: key.to_string(),
        value: value.to_string(),
        enabled: true,
        description: None,
    };

    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].as_str();
        let mut next = || {
            i += 1;
            tokens.get(i).cloned()
        };

        match token {
            "curl" => {}
            "-X" | "--request" => {
                let m = next().unwrap_or_else(|| "GET".to_string());
                method = parse_method(&m)
                    .ok_or_else(|| Error::Other(format!("Unsupported HTTP method \"{m}\"")))?;
            }
            "-H" | "--header" => {
                let header = next().unwrap_or_default();
                if let Some(colon) = header.find(':').filter(|&c| c > 0) {
                    headers.push(kv(header[..colon].trim(), header[colon + 1..].trim()));
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" => {
                let raw = next().unwrap_or_default();
                if method == HttpMethod::Get {
                    method = HttpMethod::Post;
                }
                let content_type = headers
                    .iter()
                    .find(|h| h.key.eq_ignore_ascii_case("content-type"));
                let trimmed = raw.trim();
                body_mode = if content_type.is_some_and(|h| h.value.contains("json"))
                    || trimmed.starts_with('{')
                    || trimmed.starts_with('[')
                {
                    BodyMode::Json
                } else if content_type.is_some_and(|h| h.value.contains("x-www-form-urlencoded"))
                    || raw.contains('=')
                {
                    BodyMode::XWwwFormUrlencoded
                } else {
                    BodyMode::Text
                };
                body_raw = Some(raw);
            }
            "--data-urlencode" => {
                let pair = next().unwrap_or_default();
                if let Some(eq) = pair.find('=').filter(|&e| e > 0) {
                    form_data.push(kv(&pair[..eq], &pair[eq + 1..]));
                }
                body_mode = BodyMode::XWwwFormUrlencoded;
                if method == HttpMethod::Get {
                    method = HttpMethod::Post;
                }
            }
            "-F" | "--form" => {
                let pair = next().unwrap_or_default();
                if let Some(eq) = pair.find('=').filter(|&e| e > 0) {
                    form_data.push(kv(&pair[..eq], &pair[eq + 1..]));
                }
                body_mode = BodyMode::FormData;
                if method == HttpMethod::Get {
                    method = HttpMethod::Post;
                }
            }
            "-u" | "--user" => {
                let creds = next().unwrap_or_default();
                let latin1: Option<Vec<u8>> =
                    creds.chars().map(|c| u8::try_from(c as u32).ok()).collect();
                let latin1 = latin1.ok_or_else(|| {
                    Error::Other("-u credentials contain characters outside Latin-1".into())
                })?;
                let encoded = base64::engine::general_purpose::STANDARD.encode(latin1);
                headers.push(kv("Authorization", &format!("Basic {encoded}")));
            }
            // Insecure / follow-redirects / output and transport flags have no request-level effect.
            "-k" | "--insecure" | "-L" | "--location" | "-v" | "--verbose" | "-s" | "--silent"
            | "--compressed" => {}
            t if !t.starts_with('-') && url.is_empty() => {
                url = strip_edge_quotes(t).to_string();
            }
            _ => {}
        }
        i += 1;
    }

    let body = match body_mode {
        BodyMode::Json => RequestBody {
            mode: BodyMode::Json,
            raw: body_raw,
            ..Default::default()
        },
        BodyMode::XWwwFormUrlencoded => {
            if !form_data.is_empty() {
                RequestBody {
                    mode: BodyMode::XWwwFormUrlencoded,
                    urlencoded: Some(form_data),
                    ..Default::default()
                }
            } else if let Some(raw) = body_raw.filter(|r| !r.is_empty()) {
                RequestBody {
                    mode: BodyMode::XWwwFormUrlencoded,
                    urlencoded: Some(parse_query_pairs(&raw)),
                    ..Default::default()
                }
            } else {
                RequestBody::default()
            }
        }
        BodyMode::FormData => RequestBody {
            mode: BodyMode::FormData,
            form_data: Some(
                form_data
                    .into_iter()
                    .map(|f| FormDataEntry {
                        key: f.key,
                        value: f.value,
                        entry_type: FormDataType::Text,
                        enabled: f.enabled,
                        description: f.description,
                    })
                    .collect(),
            ),
            ..Default::default()
        },
        _ => match body_raw.filter(|r| !r.is_empty()) {
            Some(raw) => RequestBody {
                mode: BodyMode::Text,
                raw: Some(raw),
                ..Default::default()
            },
            None => RequestBody::default(),
        },
    };

    let mut params: Vec<KeyValuePair> = vec![];
    if url.contains('?') {
        let mut parts = url.split('?');
        let base = parts.next().unwrap_or("").to_string();
        let qs = parts.next().unwrap_or("").to_string();
        url = base;
        params = parse_query_pairs(&qs);
    }

    let absolute = if url.starts_with("http://") || url.starts_with("https://") {
        url.clone()
    } else {
        format!("https://{url}")
    };
    let parsed = url::Url::parse(&absolute)
        .map_err(|e| Error::Other(format!("Invalid URL \"{url}\": {e}")))?;

    Ok(NexusRequest {
        id: short_id("req"),
        name: format!("{} {}", method.as_str(), parsed.path()),
        method,
        url,
        headers,
        params,
        body,
        auth: AuthConfig::new(AuthType::None),
        description: None,
        pre_request_script: None,
        post_response_script: None,
        settings: RequestSettings::default(),
    })
}

fn strip_edge_quotes(s: &str) -> &str {
    let s = s.strip_prefix(['\'', '"']).unwrap_or(s);
    s.strip_suffix(['\'', '"']).unwrap_or(s)
}

/// Splits on spaces while honoring `'...'`, `"..."` and Chrome's `$'...'` (with `\n \t \r \\ \'`).
fn tokenize(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = vec![];
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();

        if ch == '$' && next == Some('\'') && !in_single && !in_double {
            in_single = true;
            i += 2;
            continue;
        }
        if ch == '\'' && !in_double {
            in_single = !in_single;
            i += 1;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
            i += 1;
            continue;
        }
        if ch == '\\' && in_single {
            let escaped = match next {
                Some('n') => Some('\n'),
                Some('t') => Some('\t'),
                Some('r') => Some('\r'),
                Some('\\') => Some('\\'),
                Some('\'') => Some('\''),
                _ => None,
            };
            if let Some(e) = escaped {
                current.push(e);
                i += 2;
                continue;
            }
        }
        if ch == ' ' && !in_single && !in_double {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            i += 1;
            continue;
        }
        current.push(ch);
        i += 1;
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests;
