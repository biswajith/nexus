use super::*;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

/// `cargo test` builds examples; the fixture lives next to the test binary's `deps` dir.
fn fixture_path() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop(); // test binary
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("examples").join(if cfg!(windows) {
        "mcp_fixture_server.exe"
    } else {
        "mcp_fixture_server"
    })
}

fn stdio_config(env: &[(&str, &str)]) -> McpConnectConfig {
    McpConnectConfig {
        transport: McpTransportType::Stdio,
        command: Some(fixture_path().to_string_lossy().into_owned()),
        args: None,
        env: Some(
            env.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        ),
        url: None,
    }
}

fn text_of(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap().to_string()
}

fn pid_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
        && std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map(|s| !s.contains(") Z"))
            .unwrap_or(false)
}

async fn wait_dead(pid: u32) -> bool {
    for _ in 0..100 {
        if !pid_alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

async fn read_pid(file: &std::path::Path) -> u32 {
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(file) {
            if let Ok(pid) = s.trim().parse() {
                return pid;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("pid file never written");
}

#[tokio::test]
async fn stdio_connect_list_and_call() {
    let clients = McpClients::new();
    let info = clients.connect("a", &stdio_config(&[])).await.unwrap();
    assert_eq!(info["serverInfo"]["name"], "fixture-server");
    assert_eq!(info["serverInfo"]["version"], "1.2.3");
    assert!(info["capabilities"]["tools"].is_object());
    assert!(info["capabilities"]["resources"].is_object());
    assert!(info["capabilities"]["prompts"].is_object());

    let tools = clients.list_tools("a").await.unwrap();
    assert_eq!(tools[0]["name"], "echo");
    assert_eq!(tools[0]["description"], "Echoes the message");
    assert_eq!(tools[0]["inputSchema"]["required"][0], "message");

    let args = json!({"message": "hi"}).as_object().cloned();
    let result = clients.call_tool("a", "echo", args).await.unwrap();
    assert_eq!(result["content"][0]["type"], "text");
    assert_eq!(text_of(&result), "echo: hi");

    let args = json!({"message": "boom"}).as_object().cloned();
    let result = clients.call_tool("a", "echo", args).await.unwrap();
    assert_eq!(result["isError"], true);

    let resources = clients.list_resources("a").await.unwrap();
    assert_eq!(resources[0]["uri"], "file:///fixture.txt");
    assert_eq!(resources[0]["mimeType"], "text/plain");

    let prompts = clients.list_prompts("a").await.unwrap();
    assert_eq!(prompts[0]["name"], "greet");
    assert_eq!(prompts[0]["arguments"][0]["name"], "name");

    clients.disconnect("a").await;
}

#[tokio::test]
async fn protocol_error_is_reported() {
    let clients = McpClients::new();
    clients.connect("a", &stdio_config(&[])).await.unwrap();
    let e = clients.call_tool("a", "echo", None).await.unwrap_err();
    assert!(e.to_string().contains("message is required"), "{e}");
    let e = clients.call_tool("a", "nope", None).await.unwrap_err();
    assert!(e.to_string().contains("unknown tool"), "{e}");
    clients.disconnect("a").await;
}

#[tokio::test]
async fn operations_on_unknown_id_fail() {
    let clients = McpClients::new();
    for e in [
        clients.list_tools("x").await.unwrap_err(),
        clients.list_resources("x").await.unwrap_err(),
        clients.list_prompts("x").await.unwrap_err(),
        clients.call_tool("x", "echo", None).await.unwrap_err(),
    ] {
        assert_eq!(e.to_string(), "Not connected to MCP server");
    }
    // Disconnecting an unknown id is a no-op.
    clients.disconnect("x").await;
}

#[tokio::test]
async fn disconnect_closes_and_kills_child() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let clients = McpClients::new();
    clients
        .connect(
            "a",
            &stdio_config(&[("MCP_FIXTURE_PID_FILE", pid_file.to_str().unwrap())]),
        )
        .await
        .unwrap();
    let pid = read_pid(&pid_file).await;
    assert!(pid_alive(pid));
    clients.disconnect("a").await;
    assert!(wait_dead(pid).await, "child still running after disconnect");
    let e = clients.list_tools("a").await.unwrap_err();
    assert_eq!(e.to_string(), "Not connected to MCP server");
}

#[tokio::test]
async fn reconnecting_same_id_closes_old_connection() {
    let dir = tempfile::tempdir().unwrap();
    let (pid1, pid2) = (dir.path().join("pid1"), dir.path().join("pid2"));
    let clients = McpClients::new();
    clients
        .connect(
            "a",
            &stdio_config(&[("MCP_FIXTURE_PID_FILE", pid1.to_str().unwrap())]),
        )
        .await
        .unwrap();
    let old = read_pid(&pid1).await;
    clients
        .connect(
            "a",
            &stdio_config(&[("MCP_FIXTURE_PID_FILE", pid2.to_str().unwrap())]),
        )
        .await
        .unwrap();
    let new = read_pid(&pid2).await;
    assert_ne!(old, new);
    assert!(wait_dead(old).await, "old child survived reconnect");
    assert!(pid_alive(new));
    assert_eq!(clients.list_tools("a").await.unwrap()[0]["name"], "echo");
    clients.disconnect_all().await;
    assert!(wait_dead(new).await, "child survived disconnect_all");
}

#[tokio::test]
async fn stdio_env_is_not_inherited_beyond_defaults() {
    let clients = McpClients::new();
    // CARGO_MANIFEST_DIR is set in this test process but is not in the inherited allowlist.
    let cfg = stdio_config(&[("MCP_FIXTURE_ENV_PROBE", "CARGO_MANIFEST_DIR")]);
    clients.connect("a", &cfg).await.unwrap();
    let r = clients.call_tool("a", "env", None).await.unwrap();
    assert_eq!(text_of(&r), "<unset>");
    clients.disconnect("a").await;

    // Explicitly passed variables do arrive.
    let cfg = stdio_config(&[
        ("MCP_FIXTURE_ENV_PROBE", "NEXUS_PROBE"),
        ("NEXUS_PROBE", "explicit"),
    ]);
    clients.connect("a", &cfg).await.unwrap();
    let r = clients.call_tool("a", "env", None).await.unwrap();
    assert_eq!(text_of(&r), "explicit");
    clients.disconnect("a").await;
}

#[test]
fn build_env_allowlist_and_override() {
    let env = build_env(None);
    assert!(env.iter().all(|(k, _)| INHERITED_ENV.contains(&k.as_str())));
    assert!(!env.iter().any(|(k, _)| k == "CARGO_MANIFEST_DIR"));
    let extra: HashMap<String, String> = [("PATH".to_string(), "/x".to_string())].into();
    let env = build_env(Some(&extra));
    assert_eq!(env.iter().filter(|(k, _)| k == "PATH").count(), 1);
    assert!(env.contains(&("PATH".to_string(), "/x".to_string())));
}

#[tokio::test]
async fn connect_validation() {
    let clients = McpClients::new();
    let mut cfg = stdio_config(&[]);
    cfg.command = None;
    assert_eq!(
        clients.connect("a", &cfg).await.unwrap_err().to_string(),
        "Command is required for stdio transport"
    );
    cfg.command = Some(String::new());
    assert_eq!(
        clients.connect("a", &cfg).await.unwrap_err().to_string(),
        "Command is required for stdio transport"
    );

    let http = |url: Option<&str>| McpConnectConfig {
        transport: McpTransportType::Http,
        command: None,
        args: None,
        env: None,
        url: url.map(str::to_string),
    };
    assert_eq!(
        clients
            .connect("a", &http(None))
            .await
            .unwrap_err()
            .to_string(),
        "URL is required for HTTP transport"
    );
    assert!(clients
        .connect("a", &http(Some("not a url")))
        .await
        .unwrap_err()
        .to_string()
        .starts_with("Invalid URL"));
    assert_eq!(
        clients
            .connect("a", &http(Some("file:///etc/passwd")))
            .await
            .unwrap_err()
            .to_string(),
        "URL must use http or https"
    );

    cfg.command = Some("/definitely/not/a/real/command".into());
    let e = clients.connect("a", &cfg).await.unwrap_err();
    assert!(
        e.to_string().starts_with("Failed to start MCP server"),
        "{e}"
    );
}

#[test]
fn config_deserializes_from_ui_shape() {
    let c: McpConnectConfig = serde_json::from_value(
        json!({"type":"stdio","command":"node","args":["a.js"],"env":{"K":"V"}}),
    )
    .unwrap();
    assert_eq!(c.transport, McpTransportType::Stdio);
    assert_eq!(c.args.unwrap(), ["a.js"]);
    let c: McpConnectConfig =
        serde_json::from_value(json!({"type":"http","url":"http://x/mcp"})).unwrap();
    assert_eq!(c.transport, McpTransportType::Http);
    assert!(serde_json::from_value::<McpConnectConfig>(json!({"type":"ws"})).is_err());
}

#[tokio::test]
async fn streamable_http_connect_list_and_call() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let mut child = tokio::process::Command::new(fixture_path())
        .arg("http")
        .env("MCP_FIXTURE_PID_FILE", &pid_file)
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let line = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let port: u16 = line.strip_prefix("LISTENING ").unwrap().parse().unwrap();

    let clients = McpClients::new();
    let cfg = McpConnectConfig {
        transport: McpTransportType::Http,
        command: None,
        args: None,
        env: None,
        url: Some(format!("http://127.0.0.1:{port}/mcp")),
    };
    let info = clients.connect("h", &cfg).await.unwrap();
    assert_eq!(info["serverInfo"]["name"], "fixture-server");
    assert!(info["capabilities"]["tools"].is_object());
    assert_eq!(clients.list_tools("h").await.unwrap()[0]["name"], "echo");
    let r = clients
        .call_tool(
            "h",
            "echo",
            json!({"message": "over http"}).as_object().cloned(),
        )
        .await
        .unwrap();
    assert_eq!(text_of(&r), "echo: over http");
    assert_eq!(
        clients.list_resources("h").await.unwrap()[0]["name"],
        "fixture"
    );
    assert_eq!(clients.list_prompts("h").await.unwrap()[0]["name"], "greet");
    clients.disconnect("h").await;
    assert_eq!(
        clients.list_tools("h").await.unwrap_err().to_string(),
        "Not connected to MCP server"
    );

    let pid = read_pid(&pid_file).await;
    child.kill().await.unwrap();
    assert!(wait_dead(pid).await);
}
