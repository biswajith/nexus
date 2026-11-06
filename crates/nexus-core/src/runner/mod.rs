//! Runs a collection's requests in order
//! (iterations, data rows, delay, stop-on-error, `nx.execution` flow control) and reports progress
//! through an event sink, so this crate stays independent of any UI.

mod data;
#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Notify;

pub use data::{parse_csv, parse_data_file, parse_json, DataRow};

use crate::auth::effective_auth;
use crate::environments::{VariableScopes, VariableStore};
use crate::http::{parse_method, send_authenticated};
use crate::scripts::{
    LogEntry, ResponseAccess, ScriptResult, ScriptRunner, TestResult, VariableChange,
    VariableChangeAction,
};
use crate::types::{
    BodyMode, CollectionItem, NexusCollection, NexusRequest, Variable, VariableType,
};
use crate::{Error, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunConfig {
    pub collection: NexusCollection,
    #[serde(default)]
    pub environment_vars: Vec<Variable>,
    #[serde(default)]
    pub global_vars: Vec<Variable>,
    /// When non-empty, only these requests run (still in collection order).
    #[serde(default)]
    pub request_ids: Vec<String>,
    pub iterations: u32,
    #[serde(default)]
    pub delay_ms: u64,
    /// One row per iteration, cycled; `None`/empty means a single empty row.
    #[serde(default)]
    pub data_rows: Option<Vec<DataRow>>,
    #[serde(default)]
    pub stop_on_error: bool,
    /// Accepted for compatibility with the UI; the TS runner never used it either.
    #[serde(default)]
    pub persist_variables: bool,
}

/// Progress events, serialized as `{ "type": "...", "data": ... }` like the TS `RunEvent`.
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum RunEvent {
    IterationStart {
        iteration: u32,
        total: u32,
    },
    IterationComplete {
        iteration: u32,
    },
    RequestStart {
        request_id: String,
        name: String,
        iteration: u32,
    },
    RequestComplete(RequestRunResult),
    TestResult(TestResult),
    RunComplete(RunSummary),
    Error {
        message: String,
        request_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRunResult {
    pub request_id: String,
    pub request_name: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub status_text: String,
    pub response_time: f64,
    pub size: u64,
    pub test_results: Vec<TestResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub script_logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub total_requests: usize,
    pub total_iterations: u32,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    /// Milliseconds.
    pub total_time: u64,
    pub results: Vec<RequestRunResult>,
    pub aborted: bool,
}

/// Cooperative cancellation flag shared between the running task and `runner_cancel`.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    fn reset(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }

    /// Resolves once cancelled.
    async fn cancelled(&self) {
        let notified = self.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

/// Variable scopes that persist across the requests (and iterations) of one run.
struct RunState {
    environment: Vec<Variable>,
    collection: Vec<Variable>,
    global: Vec<Variable>,
}

impl RunState {
    /// Replays a script chain's (cumulative) set/unset records onto the run's own scopes.
    fn apply(&mut self, changes: &[VariableChange]) {
        for c in changes {
            let vars = match c.scope.as_str() {
                "environment" => &mut self.environment,
                "collection" => &mut self.collection,
                "global" => &mut self.global,
                _ => continue,
            };
            match c.action {
                VariableChangeAction::Set => {
                    let value = c.value.clone().unwrap_or_default();
                    if let Some(v) = vars.iter_mut().find(|v| v.key == c.key) {
                        v.value = value;
                        v.enabled = true;
                    } else {
                        vars.push(Variable {
                            key: c.key.clone(),
                            value,
                            var_type: VariableType::String,
                            enabled: true,
                            description: None,
                        });
                    }
                }
                VariableChangeAction::Unset => vars.retain(|v| v.key != c.key),
            }
        }
    }
}

/// What a request execution yields besides its result: script flow control.
struct Executed {
    result: RequestRunResult,
    /// `nx.execution.setNextRequest`: `Some(None)` stops the iteration.
    next_request: Option<Option<String>>,
}

/// Executes collections. Cheap to construct; cancellation goes through the [`CancelToken`].
#[derive(Debug, Clone)]
pub struct CollectionRunner {
    scripts: ScriptRunner,
    cancel: CancelToken,
}

impl Default for CollectionRunner {
    fn default() -> Self {
        Self::new(crate::scripts::DEFAULT_TIMEOUT_MS, CancelToken::new())
    }
}

impl CollectionRunner {
    /// `script_timeout_ms`: wall-clock limit per script.
    pub fn new(script_timeout_ms: u64, cancel: CancelToken) -> Self {
        Self {
            scripts: ScriptRunner::new(script_timeout_ms),
            cancel,
        }
    }

    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }

    /// Runs the collection, calling `sink` for each event, and returns the summary (also sent as
    /// the final `run-complete` event). Resets the cancel flag first, like the TS `run()`.
    pub async fn run<F>(&self, config: RunConfig, sink: F) -> RunSummary
    where
        F: Fn(RunEvent) + Send + Sync,
    {
        self.cancel.reset();
        let start = Instant::now();
        let mut all_results: Vec<RequestRunResult> = Vec::new();
        let data_rows = match &config.data_rows {
            Some(rows) if !rows.is_empty() => rows.clone(),
            _ => vec![IndexMap::new()],
        };
        let collection = Arc::new(config.collection.clone());
        let mut state = RunState {
            environment: config.environment_vars.clone(),
            collection: collection.variables.clone(),
            global: config.global_vars.clone(),
        };
        let queue: Vec<NexusRequest> = build_queue(&collection, &config.request_ids);

        for iteration in 0..config.iterations {
            if self.cancel.is_cancelled() {
                break;
            }
            sink(RunEvent::IterationStart {
                iteration,
                total: config.iterations,
            });

            let row = &data_rows[iteration as usize % data_rows.len()];
            let data_vars: Vec<Variable> = row
                .iter()
                .map(|(k, v)| Variable {
                    key: k.clone(),
                    value: v.clone(),
                    var_type: VariableType::String,
                    enabled: true,
                    description: None,
                })
                .collect();

            let mut index = 0;
            while index < queue.len() {
                if self.cancel.is_cancelled() {
                    break;
                }
                let req = &queue[index];
                sink(RunEvent::RequestStart {
                    request_id: req.id.clone(),
                    name: req.name.clone(),
                    iteration,
                });

                let Executed {
                    result,
                    next_request,
                } = self
                    .execute_request(req, &collection, &mut state, &data_vars)
                    .await;
                all_results.push(result.clone());
                sink(RunEvent::RequestComplete(result.clone()));
                for tr in &result.test_results {
                    sink(RunEvent::TestResult(tr.clone()));
                }

                if let (Some(error), true) = (&result.error, config.stop_on_error) {
                    sink(RunEvent::Error {
                        message: error.clone(),
                        request_id: req.id.clone(),
                    });
                    break;
                }
                if config.stop_on_error && result.test_results.iter().any(|t| !t.passed) {
                    break;
                }

                match next_request {
                    Some(None) => break,
                    Some(Some(name)) => {
                        index = queue
                            .iter()
                            .position(|r| r.name == name)
                            .unwrap_or(queue.len())
                    }
                    None => index += 1,
                }

                if config.delay_ms > 0 && index < queue.len() {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(config.delay_ms)) => {}
                        _ = self.cancel.cancelled() => {}
                    }
                }
            }

            sink(RunEvent::IterationComplete { iteration });
        }

        let count = |f: fn(&TestResult) -> bool| {
            all_results
                .iter()
                .flat_map(|r| r.test_results.iter())
                .filter(|t| f(t))
                .count()
        };
        let summary = RunSummary {
            total_requests: all_results.len(),
            total_iterations: config.iterations,
            total_tests: count(|_| true),
            passed_tests: count(|t| t.passed),
            failed_tests: count(|t| !t.passed),
            total_time: start.elapsed().as_millis() as u64,
            results: all_results,
            aborted: self.cancel.is_cancelled(),
        };
        sink(RunEvent::RunComplete(summary.clone()));
        summary
    }

    /// Runs one request; every failure becomes a result with `error` set (never a panic/Err).
    async fn execute_request(
        &self,
        req: &NexusRequest,
        collection: &Arc<NexusCollection>,
        state: &mut RunState,
        data_vars: &[Variable],
    ) -> Executed {
        let mut logs: Vec<LogEntry> = Vec::new();
        let mut next_request = None;
        match self
            .try_execute(
                req,
                collection,
                state,
                data_vars,
                &mut logs,
                &mut next_request,
            )
            .await
        {
            Ok(result) => Executed {
                result,
                next_request,
            },
            Err(e) => Executed {
                result: RequestRunResult {
                    request_id: req.id.clone(),
                    request_name: req.name.clone(),
                    method: req.method.as_str().to_string(),
                    url: req.url.clone(),
                    status: 0,
                    status_text: "Request failed".into(),
                    response_time: 0.0,
                    size: 0,
                    test_results: Vec::new(),
                    error: Some(e.to_string()),
                    script_logs: logs,
                },
                next_request,
            },
        }
    }

    async fn try_execute(
        &self,
        req: &NexusRequest,
        collection: &Arc<NexusCollection>,
        state: &mut RunState,
        data_vars: &[Variable],
        logs: &mut Vec<LogEntry>,
        next_request: &mut Option<Option<String>>,
    ) -> Result<RequestRunResult> {
        // Data row variables win over the environment, only for `{{}}` resolution (as in the TS).
        let store = VariableStore::new(&VariableScopes {
            global: state.global.clone(),
            collection: state.collection.clone(),
            environment: state
                .environment
                .iter()
                .chain(data_vars.iter())
                .cloned()
                .collect(),
        });
        let resolved = resolve_request(req, &store);

        let failure = |method: &str,
                       url: &str,
                       status_text: &str,
                       error: Option<String>,
                       logs: Vec<LogEntry>| {
            RequestRunResult {
                request_id: req.id.clone(),
                request_name: req.name.clone(),
                method: method.to_string(),
                url: url.to_string(),
                status: 0,
                status_text: status_text.to_string(),
                response_time: 0.0,
                size: 0,
                test_results: Vec::new(),
                error,
                script_logs: logs,
            }
        };

        // Pre-request scripts: collection -> folders -> request.
        let pre = {
            let scripts = self.scripts.clone();
            let (collection, resolved, id) = (collection.clone(), resolved.clone(), req.id.clone());
            let (env, global) = (state.environment.clone(), state.global.clone());
            // `state.collection` can differ from the collection's own variables after earlier scripts.
            let mut coll_for_scripts = (*collection).clone();
            coll_for_scripts.variables = state.collection.clone();
            tokio::task::spawn_blocking(move || {
                let folders = scripts.find_parent_folders(&collection, &id);
                scripts.run_pre_request_chain(&coll_for_scripts, &resolved, &folders, &env, &global)
            })
            .await
            .map_err(|e| Error::Other(format!("script task failed: {e}")))??
        };
        for r in &pre.results {
            logs.extend(r.logs.iter().cloned());
            capture_next(next_request, r);
        }
        if let Some(last) = pre.results.last() {
            state.apply(&last.variable_changes);
        }
        let method = pre.request.method.clone();
        let mut url = pre.request.url.clone();
        if let Some(error) = pre.results.iter().find_map(|r| r.error.clone()) {
            return Ok(failure(
                &method,
                &url,
                "Pre-request script error",
                Some(error),
                std::mem::take(logs),
            ));
        }
        if pre.results.iter().any(|r| r.execution_control.skipped) {
            return Ok(failure(
                &method,
                &url,
                "Skipped",
                None,
                std::mem::take(logs),
            ));
        }

        let parent_folders: Vec<_> = self.scripts.find_parent_folders(collection, &req.id);
        let ancestors: Vec<Option<&crate::types::AuthConfig>> = parent_folders
            .iter()
            .rev()
            .map(|f| f.auth.as_ref())
            .chain(std::iter::once(collection.auth.as_ref()))
            .collect();
        let auth = effective_auth(&req.auth, &ancestors);

        let sent = send_authenticated(
            Some(&auth),
            parse_method(&method)?,
            url.clone(),
            pre.request.headers.clone(),
            pre.request.body.clone(),
            req.settings.clone(),
        )
        .await?;
        let response = sent.response;
        url = sent.url;

        // Post-response scripts: request -> folders (innermost first) -> collection.
        let headers: IndexMap<String, Value> = response
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or(Value::Null)))
            .collect();
        let access = ResponseAccess::new(
            response.status,
            &response.status_text,
            headers,
            response.timing.total,
            &response.body_text,
        )
        .json_null_on_error();
        let post: Vec<ScriptResult> = {
            let scripts = self.scripts.clone();
            let (collection, resolved, id) = (collection.clone(), resolved, req.id.clone());
            let (env, global) = (state.environment.clone(), state.global.clone());
            let mut coll_for_scripts = (*collection).clone();
            coll_for_scripts.variables = state.collection.clone();
            tokio::task::spawn_blocking(move || {
                let folders = scripts.find_parent_folders(&collection, &id);
                scripts.run_post_response_chain(
                    &coll_for_scripts,
                    &resolved,
                    &folders,
                    &env,
                    &global,
                    &access,
                )
            })
            .await
            .map_err(|e| Error::Other(format!("script task failed: {e}")))??
        };
        let mut test_results = Vec::new();
        for r in &post {
            logs.extend(r.logs.iter().cloned());
            test_results.extend(r.test_results.iter().cloned());
            capture_next(next_request, r);
        }
        if let Some(last) = post.last() {
            state.apply(&last.variable_changes);
        }

        Ok(RequestRunResult {
            request_id: req.id.clone(),
            request_name: req.name.clone(),
            method,
            url,
            status: response.status,
            status_text: response.status_text,
            response_time: response.timing.total,
            size: response.size,
            test_results,
            error: None,
            script_logs: std::mem::take(logs),
        })
    }
}

fn capture_next(slot: &mut Option<Option<String>>, r: &ScriptResult) {
    if let Some(n) = &r.execution_control.next_request {
        *slot = Some(n.clone());
    }
}

/// Requests in depth-first order (the collection manager already applied `itemOrder`),
/// optionally restricted to `ids`.
fn build_queue(collection: &NexusCollection, ids: &[String]) -> Vec<NexusRequest> {
    fn walk(items: &[CollectionItem], out: &mut Vec<NexusRequest>) {
        for item in items {
            match item {
                CollectionItem::Request(r) => out.push(r.clone()),
                CollectionItem::Folder(f) => walk(&f.items, out),
            }
        }
    }
    let mut all = Vec::new();
    walk(&collection.items, &mut all);
    if ids.is_empty() {
        all
    } else {
        all.into_iter().filter(|r| ids.contains(&r.id)).collect()
    }
}

/// Copy of `req` with `{{vars}}` resolved in the url, header values and body.
fn resolve_request(req: &NexusRequest, store: &VariableStore) -> NexusRequest {
    let mut r = req.clone();
    r.url = store.resolve(&r.url);
    for h in r.headers.iter_mut().filter(|h| h.enabled) {
        h.value = store.resolve(&h.value);
    }
    match r.body.mode {
        BodyMode::None => r.body.raw = None,
        _ => {
            if let Some(raw) = r.body.raw.take() {
                r.body.raw = (!raw.is_empty()).then(|| store.resolve(&raw));
            }
            for p in r.body.urlencoded.iter_mut().flatten().filter(|p| p.enabled) {
                p.key = store.resolve(&p.key);
                p.value = store.resolve(&p.value);
            }
            if let Some(g) = r.body.graphql.as_mut() {
                g.query = store.resolve(&g.query);
                g.variables = g.variables.take().map(|v| store.resolve(&v));
            }
        }
    }
    r
}
