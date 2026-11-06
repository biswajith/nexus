//! The Runner, WebSocket, GraphQL and MCP panels, driven by role + label against local servers
//! (wiremock, tokio-tungstenite, the MCP fixture over stdio) with a temp data dir.

mod common;

use common::servers;
use common::*;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use nexus_core::service::App;
use nexus_core::types::{
    create_default_collection, create_default_environment, create_default_request, BodyMode,
    CollectionItem, HttpMethod, NexusFolder, Variable, VariableType,
};
use nexus_core::websocket::WsConnectionStatus;
use nexus_gui::state::graphql::ResultTab;
use nexus_gui::state::mcp::McpStatus;
use nexus_gui::state::runner::RunStatus;
use std::time::Duration;

fn var(k: &str, v: &str) -> Variable {
    Variable {
        key: k.into(),
        value: v.into(),
        var_type: VariableType::String,
        enabled: true,
        description: None,
    }
}

/// Collection "Run Me": GET {{base}}/users?name={{user}} and, in a folder, POST {{base}}/echo
/// with a body using the row value plus a post-response script with one passing and one failing
/// test. Environment "Local" defines `base`.
fn seed_runner(home: &std::path::Path, base: &str) {
    let app = App::new(home).unwrap();
    let mut c = create_default_collection("Run Me");
    let mut a = create_default_request();
    a.name = "Get user".into();
    a.url = "{{base}}/users?name={{user}}".into();
    let mut b = create_default_request();
    b.name = "Post echo".into();
    b.method = HttpMethod::Post;
    b.url = "{{base}}/echo".into();
    b.body.mode = BodyMode::Json;
    b.body.raw = Some("{\"who\":\"{{user}}\",\"age\":{{age}}}".into());
    b.post_response_script =
        Some("nx.test('ok', () => {}); nx.test('bad', () => { throw new Error('boom'); });".into());
    c.items = vec![
        CollectionItem::Request(a),
        CollectionItem::Folder(NexusFolder {
            id: "f1".into(),
            name: "Writes".into(),
            description: None,
            items: vec![CollectionItem::Request(b)],
            auth: None,
            pre_request_script: None,
            post_response_script: None,
            item_order: None,
        }),
    ];
    app.collection_create(&c).unwrap();
    let mut env = create_default_environment();
    env.name = "Local".into();
    env.variables = vec![var("base", base)];
    app.environment_save(&env).unwrap();
}

fn goto(h: &mut H, panel: &str) {
    click(h, Role::Tab, panel);
}

#[test]
fn runner_runs_iterations_with_csv_rows_and_shows_results() {
    let server = Server::start();
    let dir = tempfile::tempdir().unwrap();
    seed_runner(dir.path(), &server.url());
    let csv = dir.path().join("rows.csv");
    std::fs::write(&csv, "user,age\nalice,30\nbob,41\n").unwrap();
    let mut h = harness(dir.path());
    goto(&mut h, "Runner");

    // nothing to run before a collection is picked
    assert!(has(&h, Role::Button, "Run Collection"));
    assert!(has(
        &h,
        Role::Label,
        "Select a collection and click Run Collection to start"
    ));
    pick(&mut h, "Collection", "Run Me");
    pick(&mut h, "Run environment", "Local");
    assert!(h.state().ui.active_env_id.is_some());

    // data file: parsed off the UI thread; the row count becomes the iteration count
    h.state_mut().runner_load_data(&csv);
    wait_until(&mut h, "data file", |a| a.runner.data.is_some());
    assert!(has(&h, Role::Label, "Data file: rows.csv - 2 rows"));
    assert_eq!(value_of(&h, "Iterations"), "2");

    click(&mut h, Role::Button, "Run Collection");
    wait_until(&mut h, "run", |a| a.runner.status == RunStatus::Completed);
    settle(&mut h);

    // 2 iterations x 2 requests, each with that row's values
    let reqs = server.rt.block_on(server.mock.received_requests()).unwrap();
    assert_eq!(reqs.len(), 4);
    let urls: Vec<String> = reqs.iter().map(|r| r.url.to_string()).collect();
    assert!(urls[0].ends_with("/users?name=alice"), "{urls:?}");
    assert!(urls[2].ends_with("/users?name=bob"), "{urls:?}");
    let body = |i: usize| String::from_utf8(reqs[i].body.clone()).unwrap();
    assert_eq!(body(1), "{\"who\":\"alice\",\"age\":30}");
    assert_eq!(body(3), "{\"who\":\"bob\",\"age\":41}");

    // results, summary and status
    assert_eq!(h.state().runner.results.len(), 4);
    assert_eq!(h.state().runner.results[2].iteration, 2);
    assert!(has(&h, Role::Label, "Run status: Completed"));
    assert!(has(&h, Role::Label, "Iteration 2/2"));
    assert!(has(&h, Role::Label, "Requests: 4"));
    assert!(has(&h, Role::Label, "Passed: 2"));
    assert!(has(&h, Role::Label, "Failed: 2"));
    assert!(has(&h, Role::ProgressIndicator, "Run progress"));
    let rows: Vec<_> = h.get_all_by_role(Role::ListItem).collect();
    assert_eq!(rows.len(), 4);

    // details of a result: tests with the failure
    h.get_all_by_role(Role::ListItem).nth(1).unwrap().click();
    settle(&mut h);
    assert!(has(&h, Role::Button, "Close details"));
    assert!(has(&h, Role::Label, "Passed ok"));
    assert!(has(&h, Role::Label, "Failed bad"));
    click(&mut h, Role::Button, "Close details");
    assert!(!has(&h, Role::Button, "Close details"));

    // the run is kept while another panel is shown
    goto(&mut h, "HTTP");
    goto(&mut h, "Runner");
    assert_eq!(h.state().runner.results.len(), 4);

    // Reset clears the results, keeps the settings
    click(&mut h, Role::Button, "Reset");
    assert!(h.state().runner.results.is_empty());
    assert!(h.state().runner.data.is_some());
}

#[test]
fn runner_fifty_requests_complete_quickly() {
    let server = Server::start();
    let dir = tempfile::tempdir().unwrap();
    seed_runner(dir.path(), &server.url());
    let mut h = harness(dir.path());
    goto(&mut h, "Runner");
    pick(&mut h, "Collection", "Run Me");
    pick(&mut h, "Run environment", "Local");
    h.state_mut().runner.iterations = "25".into();
    let t = std::time::Instant::now();
    click(&mut h, Role::Button, "Run Collection");
    wait_until(&mut h, "run", |a| a.runner.status == RunStatus::Completed);
    eprintln!(
        "runner: 25 iterations x 2 = 50 requests in {:?}",
        t.elapsed()
    );
    assert_eq!(h.state().runner.results.len(), 50);
}

#[test]
fn runner_limits_to_a_folder_and_reports_bad_data_files() {
    let server = Server::start();
    let dir = tempfile::tempdir().unwrap();
    seed_runner(dir.path(), &server.url());
    let mut h = harness(dir.path());
    goto(&mut h, "Runner");
    pick(&mut h, "Collection", "Run Me");
    pick(&mut h, "Run environment", "Local");
    pick(&mut h, "Folder", "Writes");
    // the post script needs {{user}}/{{age}}: unresolved placeholders still produce a request
    click(&mut h, Role::Button, "Run Collection");
    wait_until(&mut h, "run", |a| a.runner.status == RunStatus::Completed);
    let reqs = server.rt.block_on(server.mock.received_requests()).unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].method.to_string(), "POST");

    let bad = dir.path().join("bad.txt");
    std::fs::write(&bad, "x").unwrap();
    h.state_mut().runner_load_data(&bad);
    wait_until(&mut h, "bad data", |a| a.runner.data_error.is_some());
    assert!(h
        .query_by_role_and_label(
            Role::Label,
            "Data file error: Unsupported data file type: bad.txt"
        )
        .is_some());
}

#[test]
fn runner_cancel_stops_the_run() {
    let server = Server::with_delay(Duration::from_millis(250));
    let dir = tempfile::tempdir().unwrap();
    seed_runner(dir.path(), &server.url());
    let mut h = harness(dir.path());
    goto(&mut h, "Runner");
    pick(&mut h, "Collection", "Run Me");
    pick(&mut h, "Run environment", "Local");
    replace_text(&mut h, Role::TextInput, "Iterations", "20");
    click(&mut h, Role::Button, "Run Collection");
    wait_until(&mut h, "first result", |a| !a.runner.results.is_empty());
    assert!(has(&h, Role::Button, "Cancel"));
    click(&mut h, Role::Button, "Cancel");
    wait_until(&mut h, "cancelled", |a| {
        a.runner.status == RunStatus::Cancelled
    });
    settle(&mut h);
    assert!(has(&h, Role::Label, "Run status: Cancelled"));
    assert!(h.state().runner.summary.as_ref().unwrap().aborted);
    let n = server.count();
    assert!(n < 40, "{n} requests");
    // nothing more arrives afterwards
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(server.count(), n);
    // and a new run can start
    assert!(has(&h, Role::Button, "Run Collection"));
}

fn ws_connect(h: &mut H, url: &str) {
    goto(h, "WS");
    replace_text(h, Role::TextInput, "WebSocket URL", url);
    click(h, Role::Button, "Connect");
    wait_until(h, "open", |a| a.ws.sessions[0].is_open());
}

#[test]
fn websocket_connects_sends_sees_echo_filters_and_disconnects() {
    let rt = server_rt();
    let url = rt.block_on(servers::ws_echo());
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    ws_connect(&mut h, &url);
    assert!(has(&h, Role::Label, "Connection status: Connected"));
    assert!(has(&h, Role::Button, "Disconnect"));

    // composer: type and send with the button
    type_into_editor(&mut h, "Message to send", "hello");
    click(&mut h, Role::Button, "Send");
    wait_until(&mut h, "echo", |a| a.ws.sessions[0].received_count() == 1);
    settle(&mut h);
    assert_eq!(h.state().ws.sessions[0].sent_count(), 1);
    assert!(h.state().ws.sessions[0].draft.is_empty());
    assert_eq!(h.get_all_by_label_contains("Sent ").count(), 1);
    assert_eq!(h.get_all_by_label_contains("echo: hello").count(), 1);

    // keyboard send (Ctrl+Enter) of JSON
    type_into_editor(&mut h, "Message to send", "{\"a\":1}");
    h.key_press_modifiers(eframe::egui::Modifiers::COMMAND, eframe::egui::Key::Enter);
    wait_until(&mut h, "second echo", |a| {
        a.ws.sessions[0].received_count() == 2
    });
    settle(&mut h);

    // selecting a message shows it pretty-printed
    let row = h.get_all_by_label_contains("echo: {").next().unwrap();
    row.click();
    settle(&mut h);
    assert!(has(&h, Role::Document, "Selected message"));

    // filter by text and by direction
    type_into(&mut h, "Filter messages", "hello");
    assert_eq!(h.get_all_by_role(Role::ListItem).count(), 2);
    type_into(&mut h, "Filter messages", "zzz");
    assert!(has(&h, Role::Label, "No messages match the filter"));
    replace_text(&mut h, Role::TextInput, "Filter messages", "");
    click(&mut h, Role::RadioButton, "Received");
    assert!(h.query_all_by_label_contains("Sent ").next().is_none());
    click(&mut h, Role::RadioButton, "All");

    // the connection survives a panel switch
    goto(&mut h, "HTTP");
    goto(&mut h, "WS");
    assert!(h.state().ws.sessions[0].is_open());
    type_into_editor(&mut h, "Message to send", "again");
    click(&mut h, Role::Button, "Send");
    wait_until(&mut h, "third echo", |a| {
        a.ws.sessions[0].received_count() == 3
    });

    click(&mut h, Role::Button, "Clear messages");
    assert!(h.state().ws.sessions[0].rows.is_empty());

    click(&mut h, Role::Button, "Disconnect");
    wait_until(&mut h, "closed", |a| {
        a.ws.sessions[0].status == WsConnectionStatus::Closed
    });
    assert!(has(&h, Role::Label, "Connection status: Disconnected"));
    assert!(has(&h, Role::Button, "Connect"));
    assert!(h
        .get_all_by_label_contains("Connection event: Closed")
        .next()
        .is_some());
}

#[test]
fn websocket_sessions_are_independent() {
    let rt = server_rt();
    let url = rt.block_on(servers::ws_echo());
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    ws_connect(&mut h, &url);
    click(&mut h, Role::Button, "New connection");
    assert_eq!(h.state().ws.sessions.len(), 2);
    assert_eq!(h.state().ws.active, 1);
    // the second session is idle while the first stays connected
    assert!(has(&h, Role::Label, "Connection status: Not connected"));
    assert!(h.state().ws.sessions[0].is_open());
    click(&mut h, Role::Tab, "Connection 1");
    assert!(has(&h, Role::Label, "Connection status: Connected"));
    click(&mut h, Role::Button, "Close Connection 2");
    assert_eq!(h.state().ws.sessions.len(), 1);
}

#[test]
fn websocket_reports_reconnects_and_errors() {
    let rt = server_rt();
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());

    // refused connection -> error row
    goto(&mut h, "WS");
    replace_text(&mut h, Role::TextInput, "WebSocket URL", "ws://127.0.0.1:1");
    click(&mut h, Role::Button, "Connect");
    wait_until(&mut h, "error", |a| a.ws.sessions[0].error.is_some());
    assert!(h
        .get_all_by_label_contains("Connection event: Error")
        .next()
        .is_some());

    // a server that hangs up at once + auto-reconnect -> Reconnecting
    let url = rt.block_on(servers::ws_closer());
    replace_text(&mut h, Role::TextInput, "WebSocket URL", &url);
    click(&mut h, Role::Tab, "Headers");
    click(&mut h, Role::CheckBox, "Auto-reconnect");
    click(&mut h, Role::Tab, "Messages");
    click(&mut h, Role::Button, "Connect");
    wait_until(&mut h, "reconnecting", |a| {
        a.ws.sessions[0].rows.iter().any(|r| {
            matches!(r, nexus_gui::state::ws::WsRow::Note { text, .. } if text.starts_with("Reconnecting"))
        })
    });
    assert!(h
        .get_all_by_label_contains("Connection event: Reconnecting")
        .next()
        .is_some());
    // the connection is up only for an instant each round: stop reconnecting if it is
    if has(&h, Role::Button, "Disconnect") {
        click(&mut h, Role::Button, "Disconnect");
    }
}

fn graphql_setup(server: &wiremock::MockServer) -> (tempfile::TempDir, H) {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    goto(&mut h, "GraphQL");
    replace_text(
        &mut h,
        Role::TextInput,
        "GraphQL endpoint URL",
        &format!("{}/graphql", server.uri()),
    );
    (dir, h)
}

#[test]
fn graphql_sends_queries_with_variables_and_headers() {
    let rt = server_rt();
    let server = rt.block_on(servers::graphql_http());
    let (_dir, mut h) = graphql_setup(&server);

    replace_text(
        &mut h,
        Role::MultilineTextInput,
        "GraphQL query",
        "query Q($n: Int) { users(first: $n) { id name } }",
    );
    replace_text(
        &mut h,
        Role::MultilineTextInput,
        "GraphQL variables",
        "{\"n\": 2}",
    );
    click(&mut h, Role::Tab, "Headers");
    type_into(&mut h, "Header 1 key", "X-Token");
    type_into(&mut h, "Header 1 value", "abc");
    click(&mut h, Role::Button, "Send");
    wait_until(&mut h, "response", |a| a.gql.result.is_some());
    settle(&mut h);

    let reqs = rt.block_on(server.received_requests()).unwrap();
    let r = reqs.last().unwrap();
    let body: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    assert_eq!(
        body["query"],
        "query Q($n: Int) { users(first: $n) { id name } }"
    );
    assert_eq!(body["variables"], serde_json::json!({"n": 2}));
    assert_eq!(r.headers.get("x-token").unwrap().to_str().unwrap(), "abc");

    let res = h.state().gql.result.as_ref().unwrap();
    assert!(res.body.text().contains("\"name\": \"Ada\""));
    assert_eq!(res.status, 200);
    assert!(has(&h, Role::Label, "Response status: 200"));
    assert!(has(&h, Role::Document, "GraphQL response data"));

    // invalid variables are reported without a request
    let n = reqs.len();
    click(&mut h, Role::Tab, "Variables");
    replace_text(
        &mut h,
        Role::MultilineTextInput,
        "GraphQL variables",
        "{oops",
    );
    click(&mut h, Role::Button, "Send");
    settle(&mut h);
    assert_eq!(rt.block_on(server.received_requests()).unwrap().len(), n);
    assert!(h
        .get_all_by_label_contains("GraphQL error: Variables are not valid JSON")
        .next()
        .is_some());
}

#[test]
fn graphql_introspection_feeds_the_schema_browser() {
    let rt = server_rt();
    let server = rt.block_on(servers::graphql_http());
    let (_dir, mut h) = graphql_setup(&server);
    click(&mut h, Role::Button, "Schema");
    wait_until(&mut h, "schema", |a| a.gql.schema.is_some());
    settle(&mut h);
    assert_eq!(h.state().gql.tab, ResultTab::Schema);

    // searchable type list
    for t in ["type Query", "type User", "enum Role", "type Subscription"] {
        assert!(has(&h, Role::ListItem, t), "{t}");
    }
    type_into(&mut h, "Search schema", "name");
    assert!(has(&h, Role::ListItem, "type User"));
    assert!(!has(&h, Role::ListItem, "type Subscription"));
    h.get_by_role_and_label(Role::ListItem, "type User").click();
    settle(&mut h);
    // only the matching field is listed
    assert!(has(&h, Role::ListItem, "Field name: String"));
    assert!(!has(&h, Role::ListItem, "Field id: ID!"));
    replace_text(&mut h, Role::TextInput, "Search schema", "");
    h.get_by_role_and_label(Role::ListItem, "type Query")
        .click();
    settle(&mut h);
    assert!(has(&h, Role::ListItem, "Field users: [User!]!"));
    assert!(has(&h, Role::ListItem, "Field user(id: ID!): User"));

    // SDL view
    click(&mut h, Role::RadioButton, "SDL");
    assert!(has(&h, Role::Document, "Schema SDL"));
    assert!(h.state().gql.sdl.text().contains("users: [User!]!"));

    // schema names reach the query editor's autocomplete
    let names: Vec<_> = h
        .state()
        .gql
        .words
        .iter()
        .map(|w| w.name.as_str())
        .collect();
    assert!(names.contains(&"users") && names.contains(&"name") && names.contains(&"User"));
    goto(&mut h, "GraphQL");
    replace_text(
        &mut h,
        Role::MultilineTextInput,
        "GraphQL query",
        "query { us",
    );
    assert!(has(&h, Role::ListBox, "Suggestions"));
    assert!(has(&h, Role::ListBoxOption, "users, schema"));
}

#[test]
fn graphql_subscription_streams_events_and_stops() {
    let rt = server_rt();
    let url = rt.block_on(servers::gql_ws(3, 20));
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    goto(&mut h, "GraphQL");
    replace_text(&mut h, Role::TextInput, "GraphQL endpoint URL", &url);
    replace_text(
        &mut h,
        Role::MultilineTextInput,
        "GraphQL query",
        "subscription { tick }",
    );
    click(&mut h, Role::Button, "Subscribe");
    assert!(has(&h, Role::Button, "Stop"));
    wait_until(&mut h, "3 events", |a| a.gql.sub_events.len() == 3);
    settle(&mut h);
    assert_eq!(h.state().gql.tab, ResultTab::Subscription);
    assert!(has(&h, Role::Label, "Subscription status: Listening"));
    assert_eq!(h.get_all_by_label_contains("Event ").count(), 3);
    h.get_all_by_label_contains("Event 2")
        .next()
        .unwrap()
        .click();
    settle(&mut h);
    assert!(h.state().gql.sub_view.text().contains("\"tick\": 2"));

    // the stream survives a panel switch
    goto(&mut h, "HTTP");
    goto(&mut h, "GraphQL");
    assert!(h.state().gql.sub_active);

    click(&mut h, Role::Button, "Stop");
    assert!(!h.state().gql.sub_active);
    assert!(has(&h, Role::Label, "Subscription status: Stopped"));
    click(&mut h, Role::Button, "Clear events");
    assert!(h.state().gql.sub_events.is_empty());
    assert!(has(&h, Role::Label, "Subscribe to see real-time data"));
}

#[test]
fn mcp_connects_to_the_fixture_lists_calls_and_disconnects() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    goto(&mut h, "MCP");
    assert!(has(
        &h,
        Role::Label,
        "Connect to an MCP server to discover and test tools"
    ));
    let exe = servers::mcp_fixture();
    type_into(&mut h, "Command", exe.to_str().unwrap());
    click(&mut h, Role::Button, "Connect");
    wait_until(&mut h, "connected", |a| {
        a.mcp.sessions[0].status == McpStatus::Connected
    });
    settle(&mut h);
    assert!(has(&h, Role::Label, "Server name: fixture-server"));
    assert!(has(&h, Role::Label, "Server version: v1.2.3"));
    for c in ["Tools", "Resources", "Prompts"] {
        assert!(has(&h, Role::Label, &format!("Capability: {c}")), "{c}");
    }
    assert!(has(&h, Role::Button, "Disconnect"));

    // tools + generated form
    assert!(has(&h, Role::ListItem, "Tool echo"));
    assert!(has(&h, Role::ListItem, "Tool env"));
    h.get_by_role_and_label(Role::ListItem, "Tool echo").click();
    settle(&mut h);
    assert!(has(&h, Role::Label, "Selected tool: echo"));
    assert!(has(&h, Role::Label, "Argument message"));
    type_into(&mut h, "Argument message value", "hi there");
    click(&mut h, Role::Button, "Execute");
    wait_until(&mut h, "result", |a| a.mcp.sessions[0].result.is_some());
    settle(&mut h);
    let s = &h.state().mcp.sessions[0];
    assert_eq!(s.result.as_ref().unwrap().text, "echo: hi there");
    assert!(has(&h, Role::Label, "Tool result: Success"));
    assert!(has(&h, Role::ListItem, "Call echo"));

    // a tool-level error and a missing required argument
    replace_text(&mut h, Role::TextInput, "Argument message value", "boom");
    click(&mut h, Role::Button, "Execute");
    wait_until(&mut h, "error result", |a| {
        a.mcp.sessions[0]
            .result
            .as_ref()
            .is_some_and(|r| r.is_error)
    });
    settle(&mut h);
    assert!(has(&h, Role::Label, "Tool result: Error"));
    assert_eq!(h.state().mcp.sessions[0].history.len(), 2);

    // a tool without arguments
    h.get_by_role_and_label(Role::ListItem, "Tool env").click();
    settle(&mut h);
    assert!(has(&h, Role::Label, "This tool takes no arguments"));

    // resources and prompts
    click(&mut h, Role::Tab, "Resources (1)");
    h.get_by_role_and_label(Role::ListItem, "Resource fixture")
        .click();
    settle(&mut h);
    assert!(has(&h, Role::Document, "Resource details"));
    assert!(h.state().mcp.sessions[0]
        .detail
        .text()
        .contains("file:///fixture.txt"));
    click(&mut h, Role::Tab, "Prompts (1)");
    h.get_by_role_and_label(Role::ListItem, "Prompt greet")
        .click();
    settle(&mut h);
    assert!(has(&h, Role::Document, "Prompt details"));
    assert!(h.state().mcp.sessions[0]
        .detail
        .text()
        .contains("Greets someone"));

    // stays connected while another panel is shown
    goto(&mut h, "HTTP");
    goto(&mut h, "MCP");
    assert!(h.state().mcp.sessions[0].connected());

    click(&mut h, Role::Button, "Disconnect");
    assert_eq!(h.state().mcp.sessions[0].status, McpStatus::Idle);
    assert!(has(&h, Role::Button, "Connect"));
}

#[test]
fn mcp_reports_a_failed_connection() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    goto(&mut h, "MCP");
    type_into(&mut h, "Command", "/nonexistent/mcp-server");
    click(&mut h, Role::Button, "Connect");
    wait_until(&mut h, "error", |a| {
        a.mcp.sessions[0].status == McpStatus::Error
    });
    settle(&mut h);
    assert!(has(&h, Role::Label, "Connection status: Error"));
    assert!(h
        .get_all_by_label_contains("Error: Failed to start MCP server")
        .next()
        .is_some());
    click(&mut h, Role::Button, "Dismiss error");
    assert!(h.state().mcp.sessions[0].error.is_none());
    // HTTP transport needs a URL
    pick(&mut h, "Transport", "HTTP");
    assert!(has(&h, Role::TextInput, "Server URL"));
}

#[test]
fn panel_widgets_all_have_roles_and_labels() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    for (panel, items) in [
        (
            "Runner",
            vec![
                (Role::ComboBox, "Collection"),
                (Role::ComboBox, "Folder"),
                (Role::ComboBox, "Run environment"),
                (Role::TextInput, "Iterations"),
                (Role::TextInput, "Delay (ms)"),
                (Role::CheckBox, "Stop on error"),
                (Role::Button, "Choose data file..."),
                (Role::Button, "Run Collection"),
            ],
        ),
        (
            "WS",
            vec![
                (Role::TextInput, "WebSocket URL"),
                (Role::Button, "Connect"),
                (Role::TextInput, "Filter messages"),
                (Role::CheckBox, "Auto-scroll"),
                (Role::Button, "Clear messages"),
                (Role::MultilineTextInput, "Message to send"),
                (Role::Button, "New connection"),
            ],
        ),
        (
            "GraphQL",
            vec![
                (Role::TextInput, "GraphQL endpoint URL"),
                (Role::Button, "Send"),
                (Role::Button, "Schema"),
                (Role::Button, "Subscribe"),
                (Role::MultilineTextInput, "GraphQL query"),
                (Role::MultilineTextInput, "GraphQL variables"),
                (Role::Tab, "Response"),
                (Role::Tab, "Schema"),
                (Role::Tab, "Subscription"),
            ],
        ),
        (
            "MCP",
            vec![
                (Role::ComboBox, "Transport"),
                (Role::TextInput, "Command"),
                (Role::TextInput, "Arguments"),
                (Role::TextInput, "Environment"),
                (Role::Button, "Connect"),
            ],
        ),
    ] {
        goto(&mut h, panel);
        for (role, label) in items {
            assert!(has(&h, role, label), "{panel}: {role:?} {label:?}");
        }
    }
}
