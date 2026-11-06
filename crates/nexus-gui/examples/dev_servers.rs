//! Local servers for screenshots and manual poking at the Runner / WebSocket / GraphQL panels:
//! `cargo run -p nexus-gui --example dev_servers`. Prints one `name=url` line per server and
//! runs until killed. With `--seed <home>` it also writes a "Smoke tests" collection, a "Local"
//! environment pointing at the HTTP server and `rows.csv` into that data directory.

#[path = "../tests/common/servers.rs"]
mod servers;

use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::main]
async fn main() {
    let http = MockServer::start().await;
    Mock::given(any())
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{\"ok\":true}")
                .set_delay(std::time::Duration::from_millis(120)),
        )
        .mount(&http)
        .await;
    let gql = servers::graphql_http().await;
    let args: Vec<String> = std::env::args().collect();
    if let Some(home) = args
        .iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1))
    {
        seed(std::path::Path::new(home), &http.uri());
    }
    println!("http={}", http.uri());
    println!("ws={}", servers::ws_echo().await);
    println!("gql={}/graphql", gql.uri());
    println!("gqlws={}", servers::gql_ws(6, 400).await);
    use std::io::Write;
    let _ = std::io::stdout().flush();
    std::future::pending::<()>().await;
}

fn seed(home: &std::path::Path, base: &str) {
    use nexus_core::types::*;
    let app = nexus_core::service::App::new(home).unwrap();
    let req = |name: &str, method: HttpMethod, url: &str| {
        let mut r = create_default_request();
        r.name = name.into();
        r.method = method;
        r.url = url.into();
        r
    };
    let mut c = create_default_collection("Smoke tests");
    let mut users = req(
        "List users",
        HttpMethod::Get,
        "{{base}}/users?name={{user}}",
    );
    users.post_response_script =
        Some("nx.test('status is 200', () => nx.expect(nx.response.code).to.equal(200));".into());
    let mut create = req("Create user", HttpMethod::Post, "{{base}}/users");
    create.body.mode = BodyMode::Json;
    create.body.raw = Some("{\"name\": \"{{user}}\", \"age\": {{age}}}".into());
    create.post_response_script = Some(
        "nx.test('created', () => {}); nx.test('has an id', () => { throw new Error('expected body.id'); });"
            .into(),
    );
    let health = req("Health", HttpMethod::Get, "{{base}}/health");
    c.items = vec![
        CollectionItem::Request(health),
        CollectionItem::Folder(NexusFolder {
            id: "users".into(),
            name: "Users".into(),
            description: None,
            items: vec![
                CollectionItem::Request(users),
                CollectionItem::Request(create),
            ],
            auth: None,
            pre_request_script: None,
            post_response_script: None,
            item_order: None,
        }),
    ];
    app.collection_create(&c).unwrap();
    let mut env = create_default_environment();
    env.name = "Local".into();
    env.variables = vec![Variable {
        key: "base".into(),
        value: base.into(),
        var_type: VariableType::String,
        enabled: true,
        description: None,
    }];
    app.environment_save(&env).unwrap();
    std::fs::write(
        home.join("rows.csv"),
        "user,age\nalice,30\nbob,41\ncarol,27\n",
    )
    .unwrap();
}
