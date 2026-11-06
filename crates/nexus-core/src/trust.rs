//! Trust boundary for collections that did not originate in this app (imports, git pulls).
//!
//! An untrusted collection never runs its scripts and never sees Secret environment variables
//! (neither through scripts nor `{{var}}` substitution) until the user trusts it. Separately,
//! configured auth is bound to the origin the request was configured with (all collections).

use crate::scripts::LogEntry;
use crate::types::{
    AuthConfig, BodyMode, CollectionItem, FormDataType, NexusCollection, NexusFolder, NexusRequest,
    RequestSettings, Variable, VariableType,
};
use serde_json::Value;
use std::collections::HashSet;

/// `variables` without the Secret-typed ones.
pub fn without_secrets(variables: &[Variable]) -> Vec<Variable> {
    variables
        .iter()
        .filter(|v| v.var_type != VariableType::Secret)
        .cloned()
        .collect()
}

/// Number of Secret variables that [`without_secrets`] drops.
pub fn secret_count(variables: &[Variable]) -> usize {
    variables
        .iter()
        .filter(|v| v.enabled && v.var_type == VariableType::Secret)
        .count()
}

/// A `warn` console entry.
pub fn warning(message: impl Into<String>) -> LogEntry {
    LogEntry {
        level: "warn".into(),
        args: vec![Value::String(message.into())],
        timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

/// Scripts (collection, folder and request level, pre-request and test) in the collection.
pub fn script_count(c: &NexusCollection) -> usize {
    fn has(s: &Option<String>) -> usize {
        usize::from(s.as_deref().is_some_and(|s| !s.trim().is_empty()))
    }
    fn walk(items: &[CollectionItem]) -> usize {
        items
            .iter()
            .map(|i| match i {
                CollectionItem::Request(r) => {
                    has(&r.pre_request_script) + has(&r.post_response_script)
                }
                CollectionItem::Folder(f) => folder(f),
            })
            .sum()
    }
    fn folder(f: &NexusFolder) -> usize {
        has(&f.pre_request_script) + has(&f.post_response_script) + walk(&f.items)
    }
    has(&c.pre_request_script) + has(&c.post_response_script) + walk(&c.items)
}

/// Copy of `c` with every script removed.
pub fn strip_scripts(c: &NexusCollection) -> NexusCollection {
    fn walk(items: &mut [CollectionItem]) {
        for i in items {
            match i {
                CollectionItem::Request(r) => {
                    r.pre_request_script = None;
                    r.post_response_script = None;
                }
                CollectionItem::Folder(f) => {
                    f.pre_request_script = None;
                    f.post_response_script = None;
                    walk(&mut f.items);
                }
            }
        }
    }
    let mut c = c.clone();
    c.pre_request_script = None;
    c.post_response_script = None;
    walk(&mut c.items);
    c
}

/// `scheme://host:port` of an http(s) URL (default ports normalised, host lowercased).
pub fn origin_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url.trim()).ok()?;
    matches!(u.scheme(), "http" | "https").then(|| u.origin().ascii_serialization())
}

/// The auth to attach to a request whose URL was `configured_url` before scripts ran and is
/// `final_url` now. Auth is dropped, with a warning, when the origin changed.
pub fn bind_auth_to_origin<'a>(
    auth: Option<&'a AuthConfig>,
    configured_url: &str,
    final_url: &str,
) -> (Option<&'a AuthConfig>, Option<LogEntry>) {
    let (before, after) = (origin_of(configured_url), origin_of(final_url));
    if auth.is_none() || before == after {
        return (auth, None);
    }
    let show = |o: &Option<String>| o.clone().unwrap_or_else(|| "(invalid URL)".into());
    let msg = format!(
        "Auth not sent: pre-request script changed the request origin from {} to {}",
        show(&before),
        show(&after)
    );
    (None, Some(warning(msg)))
}

/// Local files the request's form-data / binary body would read on send.
pub fn body_file_paths(req: &NexusRequest) -> Vec<String> {
    let mut out = Vec::new();
    match req.body.mode {
        BodyMode::FormData => out.extend(
            req.body
                .form_data
                .iter()
                .flatten()
                .filter(|e| e.enabled && !e.key.is_empty() && e.entry_type == FormDataType::File)
                .map(|e| e.value.clone()),
        ),
        BodyMode::Binary => out.extend(req.body.binary.iter().map(|b| b.path.clone())),
        _ => {}
    }
    out.retain(|p| !p.is_empty());
    out
}

/// For a request of an untrusted collection: an error naming the first body file that the user
/// did not pick in this app (`picked`), which must not be read and uploaded.
pub fn check_body_files(req: &NexusRequest, picked: &HashSet<String>) -> Result<(), String> {
    match body_file_paths(req).into_iter().find(|p| !picked.contains(p)) {
        Some(p) => Err(format!(
            "This imported request would upload local file {p}. Trust the collection or re-pick the file."
        )),
        None => Ok(()),
    }
}

/// Turns off the "skip TLS verification" opt-out in `settings`; true when it was set.
pub fn restore_tls_verification(settings: &mut RequestSettings) -> bool {
    let was = settings.reject_unauthorized == Some(false);
    if was {
        settings.reject_unauthorized = None;
    }
    was
}

/// Drops `rejectUnauthorized: false` from every request in `c`; returns how many were changed.
pub fn strip_insecure_tls(c: &mut NexusCollection) -> usize {
    fn walk(items: &mut [CollectionItem]) -> usize {
        items
            .iter_mut()
            .map(|i| match i {
                CollectionItem::Request(r) => {
                    usize::from(restore_tls_verification(&mut r.settings))
                }
                CollectionItem::Folder(f) => walk(&mut f.items),
            })
            .sum()
    }
    walk(&mut c.items)
}

#[cfg(test)]
mod tests;
