//! `ScriptRunner`: pre-request / post-response script chains (port of `script-runner.ts`).

use serde_json::{json, Map, Value};

use super::sandbox::{
    MutableRequest, ResponseAccess, ScriptContext, ScriptResult, ScriptSandbox, TrackedScope,
};
use crate::types::{
    BodyMode, CollectionItem, NexusCollection, NexusFolder, NexusRequest, Variable,
};
use crate::{Error, Result};

/// Per-script results plus the request after the chain (or early exit).
#[derive(Debug, Clone)]
pub struct ScriptChainResult {
    pub results: Vec<ScriptResult>,
    pub request: MutableRequest,
}

/// Executes script chains in a [`ScriptSandbox`], sharing variable scopes between scripts.
#[derive(Debug, Clone, Default)]
pub struct ScriptRunner {
    sandbox: ScriptSandbox,
}

impl ScriptRunner {
    /// `timeout_ms`: per-script wall-clock limit.
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            sandbox: ScriptSandbox::new(timeout_ms),
        }
    }

    /// Runs pre-request scripts collection -> each parent folder (root first) -> request, applying
    /// each script's request mutations to the shared request and stopping after a script that
    /// calls `skipRequest`. Errors only if a GraphQL body's `variables` is not valid JSON.
    pub fn run_pre_request_chain(
        &self,
        collection: &NexusCollection,
        request: &NexusRequest,
        parent_folders: &[&NexusFolder],
        env_vars: &[Variable],
        global_vars: &[Variable],
    ) -> Result<ScriptChainResult> {
        let mut ctx = self.new_context(collection, request, env_vars, global_vars)?;

        let mut scripts: Vec<&str> = Vec::new();
        scripts.extend(non_empty(&collection.pre_request_script));
        for folder in parent_folders {
            scripts.extend(non_empty(&folder.pre_request_script));
        }
        scripts.extend(non_empty(&request.pre_request_script));

        let mut results = Vec::new();
        for script in scripts {
            let result = self.sandbox.execute(script, &mut ctx);
            if let Some(m) = &result.request_mutations {
                if let Some(url) = &m.url {
                    ctx.request.url = url.clone();
                }
                if let Some(method) = &m.method {
                    ctx.request.method = method.clone();
                }
                if let Some(headers) = &m.headers {
                    ctx.request.headers = headers.clone();
                }
                if let Some(body) = &m.body {
                    ctx.request.body = body.clone();
                }
            }
            let skipped = result.execution_control.skipped;
            results.push(result);
            if skipped {
                break;
            }
        }
        Ok(ScriptChainResult {
            results,
            request: ctx.request,
        })
    }

    /// Runs post-response scripts request -> parent folders (innermost first) -> collection.
    pub fn run_post_response_chain(
        &self,
        collection: &NexusCollection,
        request: &NexusRequest,
        parent_folders: &[&NexusFolder],
        env_vars: &[Variable],
        global_vars: &[Variable],
        response: &ResponseAccess,
    ) -> Result<Vec<ScriptResult>> {
        let mut ctx = self.new_context(collection, request, env_vars, global_vars)?;
        ctx.response = Some(response.clone());

        let mut scripts: Vec<&str> = Vec::new();
        scripts.extend(non_empty(&request.post_response_script));
        for folder in parent_folders.iter().rev() {
            scripts.extend(non_empty(&folder.post_response_script));
        }
        scripts.extend(non_empty(&collection.post_response_script));

        Ok(scripts
            .into_iter()
            .map(|script| self.sandbox.execute(script, &mut ctx))
            .collect())
    }

    /// Folders from the root to the request's immediate parent; empty if the id isn't found.
    pub fn find_parent_folders<'a>(
        &self,
        collection: &'a NexusCollection,
        request_id: &str,
    ) -> Vec<&'a NexusFolder> {
        fn search<'a>(
            items: &'a [CollectionItem],
            trail: &mut Vec<&'a NexusFolder>,
            id: &str,
        ) -> bool {
            for item in items {
                match item {
                    CollectionItem::Folder(f) => {
                        trail.push(f);
                        if search(&f.items, trail, id) {
                            return true;
                        }
                        trail.pop();
                    }
                    CollectionItem::Request(r) if r.id == id => return true,
                    CollectionItem::Request(_) => {}
                }
            }
            false
        }
        let mut trail = Vec::new();
        if search(&collection.items, &mut trail, request_id) {
            trail
        } else {
            Vec::new()
        }
    }

    fn new_context(
        &self,
        collection: &NexusCollection,
        request: &NexusRequest,
        env_vars: &[Variable],
        global_vars: &[Variable],
    ) -> Result<ScriptContext> {
        Ok(ScriptContext::new(
            TrackedScope::new("environment", env_vars),
            TrackedScope::new("collection", &collection.variables),
            TrackedScope::new("global", global_vars),
            MutableRequest {
                url: request.url.clone(),
                method: request.method.as_str().to_string(),
                headers: request
                    .headers
                    .iter()
                    .filter(|h| h.enabled)
                    .map(|h| (h.key.clone(), h.value.clone()))
                    .collect(),
                body: extract_body(request)?,
            },
        ))
    }
}

fn non_empty(script: &Option<String>) -> Option<&str> {
    script.as_deref().filter(|s| !s.is_empty())
}

/// JS `encodeURIComponent`: everything but `A-Za-z0-9 - _ . ! ~ * ' ( )` is percent-encoded.
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The request body as a script sees it (`nx.request.body`).
fn extract_body(request: &NexusRequest) -> Result<Option<String>> {
    let body = &request.body;
    Ok(match body.mode {
        BodyMode::Json | BodyMode::Xml | BodyMode::Text | BodyMode::Html => body.raw.clone(),
        BodyMode::XWwwFormUrlencoded => body.urlencoded.as_ref().map(|pairs| {
            pairs
                .iter()
                .filter(|p| p.enabled)
                .map(|p| {
                    format!(
                        "{}={}",
                        encode_uri_component(&p.key),
                        encode_uri_component(&p.value)
                    )
                })
                .collect::<Vec<_>>()
                .join("&")
        }),
        BodyMode::Graphql => match &body.graphql {
            None => None,
            Some(g) => {
                let mut obj = Map::new();
                obj.insert("query".into(), json!(g.query));
                if let Some(vars) = g.variables.as_deref().filter(|v| !v.is_empty()) {
                    let parsed: Value =
                        serde_json::from_str(vars).map_err(|e| Error::Other(e.to_string()))?;
                    obj.insert("variables".into(), parsed);
                }
                Some(Value::Object(obj).to_string())
            }
        },
        _ => None,
    })
}
