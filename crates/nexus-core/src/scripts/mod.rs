//! Port of `packages/core/src/scripts`: user-script execution.
//!
//! Scripts run in an embedded QuickJS (`rquickjs`), one fresh runtime + context per execution.
//! `sandbox_runtime.js` (the `nx` API, console capture, `nx.test`) and `assertions.js` (`nx.expect`)
//! run *inside* the context; Rust is the host side (tracked variable scopes, request mutation,
//! visualizer, execution control, JSON Schema validation). Only JSON strings cross the boundary.

mod assertions;
mod sandbox;
mod script_runner;

#[cfg(test)]
mod assertions_tests;
#[cfg(test)]
mod sandbox_tests;
#[cfg(test)]
mod script_runner_tests;

pub use assertions::TestResult;
pub use sandbox::{
    ExecutionControl, LogEntry, MutableRequest, RequestMutations, ResponseAccess, ScriptContext,
    ScriptResult, ScriptSandbox, TrackedScope, VariableChange, VariableChangeAction,
    DEFAULT_TIMEOUT_MS,
};
pub use script_runner::{ScriptChainResult, ScriptRunner};

/// Runs a script on a blocking thread (QuickJS is synchronous and may use its whole timeout) and
/// hands the context back so scopes persist across a chain.
pub async fn execute_blocking(
    sandbox: &ScriptSandbox,
    script: &str,
    mut ctx: ScriptContext,
) -> crate::Result<(ScriptResult, ScriptContext)> {
    let sandbox = sandbox.clone();
    let script = script.to_string();
    tokio::task::spawn_blocking(move || {
        let result = sandbox.execute(&script, &mut ctx);
        (result, ctx)
    })
    .await
    .map_err(|e| crate::Error::Other(format!("script task failed: {e}")))
}
