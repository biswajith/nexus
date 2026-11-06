//! Turns a request tab into the `http_send` payload (and a resolved copy for cURL).
//!
//! Builds the request payload: enabled headers and params, raw / urlencoded / GraphQL /
//! multipart / binary bodies, environment + collection variables (also in auth fields), `inherit`
//! auth from the folder / collection chain, and collection/folder scripts around the request's own.

use crate::url_sync::{effective_url, encode_component};
use indexmap::IndexMap;
use nexus_core::auth::effective_auth;
use nexus_core::environments::{VariableScopes, VariableStore};
use nexus_core::scripts::ScriptRunner;
use nexus_core::service::HttpSendPayload;
use nexus_core::types::{
    AuthConfig, AuthType, BodyMode, FormDataType, NexusCollection, NexusRequest, ScopeLevel,
    Variable,
};
use serde_json::{Map, Value};

/// Variable scopes a request is sent with.
#[derive(Clone, Default)]
pub struct Scopes {
    pub environment: Vec<Variable>,
    pub collection: Vec<Variable>,
    pub global: Vec<Variable>,
}

impl Scopes {
    pub fn store(&self) -> VariableStore {
        VariableStore::new(&VariableScopes {
            global: self.global.clone(),
            collection: self.collection.clone(),
            environment: self.environment.clone(),
        })
    }
}

/// Whether `{{name}}` resolves (a defined enabled variable, or a `$dynamic` generator).
pub fn is_defined(store: &VariableStore, name: &str) -> bool {
    let name = name.trim();
    name.starts_with('$')
        || [
            ScopeLevel::Local,
            ScopeLevel::Environment,
            ScopeLevel::Collection,
            ScopeLevel::Global,
        ]
        .into_iter()
        .any(|l| store.scope(l).get(name).is_some())
}

fn resolve_json(v: &Value, store: &VariableStore) -> Value {
    match v {
        Value::String(s) => Value::String(store.resolve(s)),
        other => other.clone(),
    }
}

/// A copy of `req` with `{{variables}}` replaced everywhere they are sent (secrets included).
pub fn resolve_request(req: &NexusRequest, store: &VariableStore) -> NexusRequest {
    let mut r = req.clone();
    r.url = store.resolve(&r.url);
    for kv in r.headers.iter_mut().chain(r.params.iter_mut()) {
        kv.key = store.resolve(&kv.key);
        kv.value = store.resolve(&kv.value);
    }
    let b = &mut r.body;
    if let Some(raw) = &mut b.raw {
        *raw = store.resolve(raw);
    }
    for kv in b.urlencoded.iter_mut().flatten() {
        kv.key = store.resolve(&kv.key);
        kv.value = store.resolve(&kv.value);
    }
    for fd in b.form_data.iter_mut().flatten() {
        fd.key = store.resolve(&fd.key);
        fd.value = store.resolve(&fd.value);
    }
    if let Some(bin) = &mut b.binary {
        bin.path = store.resolve(&bin.path);
    }
    if let Some(g) = &mut b.graphql {
        g.query = store.resolve(&g.query);
        g.variables = g.variables.as_ref().map(|v| store.resolve(v));
    }
    r.auth.extra = r
        .auth
        .extra
        .iter()
        .map(|(k, v)| (k.clone(), resolve_json(v, store)))
        .collect::<Map<_, _>>();
    r
}

/// The auth that applies to the request: its own unless `inherit`, then the nearest folder or
/// collection setting (none when the request is not in a collection).
pub fn inherited_auth(
    req: &NexusRequest,
    request_id: &str,
    collection: Option<&NexusCollection>,
) -> AuthConfig {
    let Some(c) = collection else {
        return effective_auth(&req.auth, &[]);
    };
    let folders = ScriptRunner::default().find_parent_folders(c, request_id);
    let ancestors: Vec<Option<&AuthConfig>> = folders
        .iter()
        .rev()
        .map(|f| f.auth.as_ref())
        .chain(std::iter::once(c.auth.as_ref()))
        .collect();
    effective_auth(&req.auth, &ancestors)
}

/// Joins several scripts into one; each runs in its own block so `const`s do not clash.
fn chain(scripts: Vec<&str>) -> Option<String> {
    let scripts: Vec<&str> = scripts
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect();
    match scripts.len() {
        0 => None,
        1 => Some(scripts[0].to_string()),
        _ => Some(
            scripts
                .iter()
                .map(|s| format!("{{\n{s}\n}}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    }
}

fn has_header(headers: &IndexMap<String, String>, name: &str) -> bool {
    headers.keys().any(|h| h.eq_ignore_ascii_case(name))
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "log" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

fn quote_param(s: &str) -> String {
    s.replace('"', "%22")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// The multipart body for the enabled form-data rows. Reads the files (blocking I/O).
pub fn multipart_body(
    entries: &[nexus_core::types::FormDataEntry],
    boundary: &str,
) -> Result<Option<Vec<u8>>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut any = false;
    for e in entries.iter().filter(|e| e.enabled && !e.key.is_empty()) {
        any = true;
        out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match e.entry_type {
            FormDataType::Text => {
                out.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                        quote_param(&e.key)
                    )
                    .as_bytes(),
                );
                out.extend_from_slice(e.value.as_bytes());
            }
            FormDataType::File => {
                let data = std::fs::read(&e.value)
                    .map_err(|err| format!("Cannot read file {}: {err}", e.value))?;
                let name = std::path::Path::new(&e.value)
                    .file_name()
                    .map_or_else(|| e.value.clone(), |n| n.to_string_lossy().into_owned());
                out.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
                        quote_param(&e.key),
                        quote_param(&name),
                        mime_for(&e.value)
                    )
                    .as_bytes(),
                );
                out.extend_from_slice(&data);
            }
        }
        out.extend_from_slice(b"\r\n");
    }
    if !any {
        return Ok(None);
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok(Some(out))
}

/// Builds the payload for `App::http_send`. May read files (multipart / binary bodies), so run
/// it off the UI thread.
pub fn build_payload(
    request: &NexusRequest,
    request_id: &str,
    scopes: &Scopes,
    collection: Option<&NexusCollection>,
) -> Result<HttpSendPayload, String> {
    let store = scopes.store();
    let req = resolve_request(request, &store);

    let mut headers: IndexMap<String, String> = req
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.is_empty())
        .map(|h| (h.key.clone(), h.value.clone()))
        .collect();

    let url = effective_url(&req.url, &req.params);

    let mut body: Option<String> = None;
    let mut body_bytes: Option<Vec<u8>> = None;
    let default_ct = |headers: &mut IndexMap<String, String>, ct: &str| {
        if !has_header(headers, "content-type") {
            headers.insert("content-type".into(), ct.into());
        }
    };
    match req.body.mode {
        BodyMode::None => {}
        BodyMode::Json | BodyMode::Xml | BodyMode::Text | BodyMode::Html => {
            body = req.body.raw.clone().filter(|r| !r.is_empty());
            if body.is_some() {
                default_ct(
                    &mut headers,
                    match req.body.mode {
                        BodyMode::Json => "application/json",
                        BodyMode::Xml => "application/xml",
                        BodyMode::Html => "text/html",
                        _ => "text/plain",
                    },
                );
            }
        }
        BodyMode::XWwwFormUrlencoded => {
            let pairs: Vec<String> = req
                .body
                .urlencoded
                .iter()
                .flatten()
                .filter(|p| p.enabled && !p.key.is_empty())
                .map(|p| {
                    format!(
                        "{}={}",
                        encode_component(&p.key),
                        encode_component(&p.value)
                    )
                })
                .collect();
            if !pairs.is_empty() {
                body = Some(pairs.join("&"));
                default_ct(&mut headers, "application/x-www-form-urlencoded");
            }
        }
        BodyMode::FormData => {
            let boundary = format!("----NexusFormBoundary{}", uuid::Uuid::new_v4().simple());
            let entries = req.body.form_data.clone().unwrap_or_default();
            if let Some(b) = multipart_body(&entries, &boundary)? {
                body_bytes = Some(b);
                // The boundary must match the body, so a hand-written Content-Type is replaced.
                headers.retain(|k, _| !k.eq_ignore_ascii_case("content-type"));
                headers.insert(
                    "content-type".into(),
                    format!("multipart/form-data; boundary={boundary}"),
                );
            }
        }
        BodyMode::Binary => {
            if let Some(b) = req.body.binary.as_ref().filter(|b| !b.path.is_empty()) {
                body_bytes = Some(
                    std::fs::read(&b.path)
                        .map_err(|e| format!("Cannot read file {}: {e}", b.path))?,
                );
                let ct = b
                    .content_type
                    .clone()
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| "application/octet-stream".into());
                default_ct(&mut headers, &ct);
            }
        }
        BodyMode::Graphql => {
            if let Some(g) = req.body.graphql.as_ref().filter(|g| !g.query.is_empty()) {
                let mut m = Map::new();
                m.insert("query".into(), Value::String(g.query.clone()));
                if let Some(v) = g.variables.as_deref().filter(|v| !v.trim().is_empty()) {
                    m.insert(
                        "variables".into(),
                        serde_json::from_str(v)
                            .map_err(|e| format!("GraphQL variables are not valid JSON: {e}"))?,
                    );
                }
                body = Some(Value::Object(m).to_string());
                default_ct(&mut headers, "application/json");
            }
        }
    }

    let auth = inherited_auth(&req, request_id, collection);
    let auth = (auth.auth_type != AuthType::None).then_some(auth);

    let (pre, post) = match collection {
        Some(c) => {
            let folders = ScriptRunner::default().find_parent_folders(c, request_id);
            let mut pre: Vec<&str> = vec![c.pre_request_script.as_deref().unwrap_or("")];
            pre.extend(
                folders
                    .iter()
                    .map(|f| f.pre_request_script.as_deref().unwrap_or("")),
            );
            pre.push(req.pre_request_script.as_deref().unwrap_or(""));
            let mut post: Vec<&str> = vec![req.post_response_script.as_deref().unwrap_or("")];
            post.extend(
                folders
                    .iter()
                    .rev()
                    .map(|f| f.post_response_script.as_deref().unwrap_or("")),
            );
            post.push(c.post_response_script.as_deref().unwrap_or(""));
            (chain(pre), chain(post))
        }
        None => (
            chain(vec![req.pre_request_script.as_deref().unwrap_or("")]),
            chain(vec![req.post_response_script.as_deref().unwrap_or("")]),
        ),
    };

    Ok(HttpSendPayload {
        method: req.method,
        url,
        headers,
        body,
        body_bytes,
        settings: Some(req.settings.clone()),
        auth,
        pre_request_script: pre,
        post_response_script: post,
        environment_vars: scopes.environment.clone(),
        collection_vars: scopes.collection.clone(),
        global_vars: scopes.global.clone(),
        // Set by the caller, which knows whether the collection is trusted.
        untrusted: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::types::{create_default_request, FormDataEntry, KeyValuePair, VariableType};

    fn var(k: &str, v: &str) -> Variable {
        Variable {
            key: k.into(),
            value: v.into(),
            var_type: VariableType::String,
            enabled: true,
            description: None,
        }
    }

    #[test]
    fn resolves_variables_everywhere_including_auth() {
        let mut r = create_default_request();
        r.url = "{{host}}/x".into();
        r.auth = AuthConfig::new(AuthType::Bearer);
        r.auth.extra.insert("token".into(), "{{tok}}".into());
        let scopes = Scopes {
            environment: vec![var("host", "http://h"), var("tok", "abc")],
            ..Default::default()
        };
        let p = build_payload(&r, &r.id, &scopes, None).unwrap();
        assert_eq!(p.url, "http://h/x");
        assert_eq!(p.auth.unwrap().extra["token"], "abc");
        assert!(is_defined(&scopes.store(), "host"));
        assert!(!is_defined(&scopes.store(), "nope"));
        assert!(is_defined(&scopes.store(), "$guid"));
    }

    #[test]
    fn graphql_becomes_a_json_body_and_bad_variables_fail() {
        let mut r = create_default_request();
        r.body.mode = BodyMode::Graphql;
        r.body.graphql = Some(nexus_core::types::GraphqlBody {
            query: "{ a }".into(),
            variables: Some("{\"x\":1}".into()),
        });
        let p = build_payload(&r, &r.id, &Scopes::default(), None).unwrap();
        assert_eq!(
            p.body.as_deref(),
            Some(r#"{"query":"{ a }","variables":{"x":1}}"#)
        );
        assert_eq!(p.headers["content-type"], "application/json");
        r.body.graphql.as_mut().unwrap().variables = Some("{oops".into());
        assert!(build_payload(&r, &r.id, &Scopes::default(), None)
            .unwrap_err()
            .contains("not valid JSON"));
    }

    #[test]
    fn multipart_has_text_and_file_parts() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        std::fs::write(&f, "FILEDATA").unwrap();
        let entries = vec![
            FormDataEntry {
                key: "t".into(),
                value: "v".into(),
                entry_type: FormDataType::Text,
                enabled: true,
                description: None,
            },
            FormDataEntry {
                key: "f".into(),
                value: f.to_string_lossy().into_owned(),
                entry_type: FormDataType::File,
                enabled: true,
                description: None,
            },
            FormDataEntry {
                key: "off".into(),
                value: "x".into(),
                entry_type: FormDataType::Text,
                enabled: false,
                description: None,
            },
        ];
        let b = String::from_utf8(multipart_body(&entries, "B").unwrap().unwrap()).unwrap();
        assert!(b.contains("name=\"t\"\r\n\r\nv\r\n"));
        assert!(b.contains(
            "name=\"f\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nFILEDATA\r\n"
        ));
        assert!(!b.contains("off"));
        assert!(b.ends_with("--B--\r\n"));
        assert!(multipart_body(&[], "B").unwrap().is_none());
    }

    #[test]
    fn params_not_in_url_are_appended() {
        let mut r = create_default_request();
        r.url = "http://h".into();
        r.params.push(KeyValuePair {
            key: "a b".into(),
            value: "1".into(),
            enabled: true,
            description: None,
        });
        let p = build_payload(&r, &r.id, &Scopes::default(), None).unwrap();
        assert_eq!(p.url, "http://h?a%20b=1");
    }

    #[test]
    fn script_chain_wraps_each_script_in_a_block() {
        assert_eq!(chain(vec!["", "  "]), None);
        assert_eq!(chain(vec!["a();"]).unwrap(), "a();");
        assert_eq!(chain(vec!["a", "", "b"]).unwrap(), "{\na\n}\n{\nb\n}");
    }
}
