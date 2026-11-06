//! Tiny MCP server used by the `nexus_core::mcp` tests. Exposes one tool (`echo`), one resource
//! and one prompt. `stdio` (default) serves over stdin/stdout; `http` serves streamable HTTP on
//! an ephemeral port and prints `LISTENING <port>`. If `MCP_FIXTURE_PID_FILE` is set the process
//! writes its pid there (lets tests check that the child was killed). `MCP_FIXTURE_ENV_PROBE`
//! names an environment variable whose value the `env` tool returns.

#![allow(clippy::field_reassign_with_default)] // rmcp result types are non_exhaustive

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListPromptsResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, Prompt, PromptArgument, Resource,
    ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::json;

#[derive(Clone)]
struct Fixture;

fn schema(v: serde_json::Value) -> std::sync::Arc<serde_json::Map<String, serde_json::Value>> {
    std::sync::Arc::new(v.as_object().unwrap().clone())
}

impl ServerHandler for Fixture {
    fn get_info(&self) -> ServerConfig {
        let mut cfg = ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        );
        cfg.server_info.name = "fixture-server".into();
        cfg.server_info.version = "1.2.3".into();
        cfg
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut r = ListToolsResult::default();
        r.tools = vec![
            Tool::new(
                "echo",
                "Echoes the message",
                schema(
                    json!({"type":"object","properties":{"message":{"type":"string"}},"required":["message"]}),
                ),
            ),
            Tool::new(
                "env",
                "Returns an env var",
                schema(json!({"type":"object","properties":{}})),
            ),
        ];
        Ok(r)
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        match req.name.as_ref() {
            "echo" => {
                let msg = req
                    .arguments
                    .as_ref()
                    .and_then(|a| a.get("message"))
                    .and_then(|m| m.as_str());
                match msg {
                    Some("boom") => {
                        Ok(CallToolResult::error(vec![ContentBlock::text("boom")]).into())
                    }
                    Some(m) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                        "echo: {m}"
                    ))])
                    .into()),
                    None => Err(ErrorData::invalid_params("message is required", None)),
                }
            }
            "env" => {
                let name = std::env::var("MCP_FIXTURE_ENV_PROBE").unwrap_or_default();
                let val = std::env::var(&name).unwrap_or_else(|_| "<unset>".into());
                Ok(CallToolResult::success(vec![ContentBlock::text(val)]).into())
            }
            other => Err(ErrorData::invalid_params(
                format!("unknown tool {other}"),
                None,
            )),
        }
    }

    async fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let mut r = ListResourcesResult::default();
        let mut raw = Resource::new("file:///fixture.txt", "fixture");
        raw.description = Some("A fixture resource".into());
        raw.mime_type = Some("text/plain".into());
        r.resources = vec![raw];
        Ok(r)
    }

    async fn list_prompts(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        let mut r = ListPromptsResult::default();
        r.prompts = vec![Prompt::new(
            "greet",
            Some("Greets someone"),
            Some(vec![PromptArgument::new("name").with_required(true)]),
        )];
        Ok(r)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var("MCP_FIXTURE_PID_FILE") {
        std::fs::write(path, std::process::id().to_string())?;
    }
    if std::env::args().nth(1).as_deref() == Some("http") {
        let service: StreamableHttpService<Fixture, LocalSessionManager> =
            StreamableHttpService::new(
                || Ok(Fixture),
                Default::default(),
                StreamableHttpServerConfig::default(),
            );
        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        println!("LISTENING {}", listener.local_addr()?.port());
        axum::serve(listener, router).await?;
    } else {
        let running = Fixture
            .serve((tokio::io::stdin(), tokio::io::stdout()))
            .await?;
        running.waiting().await?;
    }
    Ok(())
}
