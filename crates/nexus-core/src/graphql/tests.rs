#![allow(clippy::result_large_err)] // tungstenite's handshake callback signature
use super::*;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio_tungstenite::tungstenite::handshake::server::{Request as SrvReq, Response as SrvResp};
use tokio_tungstenite::tungstenite::Message;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req(url: &str, query: &str) -> GraphQLRequestOptions {
    GraphQLRequestOptions {
        url: url.to_string(),
        query: query.to_string(),
        ..Default::default()
    }
}

// ---------- send ----------

#[tokio::test]
async fn send_posts_json_and_parses_the_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(header("content-type", "application/json"))
        .and(header("accept", "application/json"))
        .and(header("x-api-key", "k"))
        .and(body_json(json!({
            "query": "query Q($id: ID!) { user(id: $id) { name } }",
            "variables": {"id": "1"},
            "operationName": "Q"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-trace", "abc")
                .set_body_json(
                    json!({"data": {"user": {"name": "Ada"}}, "extensions": {"cost": 1}}),
                ),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = GraphQLClient::new();
    let r = client
        .send(&GraphQLRequestOptions {
            url: format!("{}/graphql", server.uri()),
            query: "query Q($id: ID!) { user(id: $id) { name } }".into(),
            variables: Some(json!({"id": "1"})),
            operation_name: Some("Q".into()),
            headers: Some(IndexMap::from([("X-Api-Key".to_string(), "k".to_string())])),
        })
        .await
        .unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.data, Some(json!({"user": {"name": "Ada"}})));
    assert_eq!(r.errors, None);
    assert_eq!(r.extensions, Some(json!({"cost": 1})));
    assert_eq!(r.headers.get("x-trace").map(String::as_str), Some("abc"));
    let v = serde_json::to_value(&r).unwrap();
    assert!(v.get("responseTime").is_some() && v.get("errors").is_none());
}

#[tokio::test]
async fn send_omits_absent_variables_and_operation_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_json(json!({"query": "{ a }"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": null})))
        .expect(1)
        .mount(&server)
        .await;
    let r = GraphQLClient::new()
        .send(&req(&server.uri(), "{ a }"))
        .await
        .unwrap();
    // A JSON null `data` stays null.
    assert_eq!(r.data, Some(Value::Null));
}

#[tokio::test]
async fn send_lets_caller_headers_override_the_defaults() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("accept", "application/graphql-response+json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {}})))
        .expect(1)
        .mount(&server)
        .await;
    let mut o = req(&server.uri(), "{ a }");
    o.headers = Some(IndexMap::from([(
        "accept".to_string(),
        "application/graphql-response+json".to_string(),
    )]));
    GraphQLClient::new().send(&o).await.unwrap();
}

#[tokio::test]
async fn send_returns_graphql_errors_and_non_2xx_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(json!({"errors": [{"message": "bad query"}]})),
        )
        .mount(&server)
        .await;
    let r = GraphQLClient::new()
        .send(&req(&server.uri(), "{"))
        .await
        .unwrap();
    assert_eq!(r.status, 400);
    assert_eq!(r.errors, Some(json!([{"message": "bad query"}])));
    assert_eq!(r.data, None);
}

#[tokio::test]
async fn send_reports_invalid_json_bodies() {
    let server = MockServer::start().await;
    let long = "x".repeat(500);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(502).set_body_string(long.clone()))
        .mount(&server)
        .await;
    let r = GraphQLClient::new()
        .send(&req(&server.uri(), "{ a }"))
        .await
        .unwrap();
    assert_eq!(r.status, 502);
    let msg = r.errors.unwrap()[0]["message"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(msg, format!("Invalid JSON response: {}", &long[..200]));
}

#[tokio::test]
async fn send_does_not_follow_redirects() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/a"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/b"))
        .mount(&server)
        .await;
    let r = GraphQLClient::new()
        .send(&req(&format!("{}/a", server.uri()), "{ a }"))
        .await
        .unwrap();
    assert_eq!(r.status, 302);
}

#[tokio::test]
async fn send_fails_when_the_server_is_unreachable() {
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert!(GraphQLClient::new()
        .send(&req(&format!("http://127.0.0.1:{port}"), "{ a }"))
        .await
        .is_err());
}

// ---------- introspection ----------

fn type_ref(kind: &str, name: Option<&str>, of: Option<Value>) -> Value {
    json!({"kind": kind, "name": name, "ofType": of})
}

fn named(kind: &str, name: &str) -> Value {
    type_ref(kind, Some(name), None)
}

fn field(name: &str, ty: Value) -> Value {
    json!({"name": name, "description": null, "args": [], "type": ty, "isDeprecated": false, "deprecationReason": null})
}

fn sample_introspection() -> Value {
    let string = named("SCALAR", "String");
    let non_null_id = type_ref("NON_NULL", None, Some(named("SCALAR", "ID")));
    let list_of_users = type_ref(
        "NON_NULL",
        None,
        Some(type_ref(
            "LIST",
            None,
            Some(type_ref("NON_NULL", None, Some(named("OBJECT", "User")))),
        )),
    );
    let mut user_field = field("user", named("OBJECT", "User"));
    user_field["args"] = json!([{"name": "id", "description": null, "type": non_null_id.clone(), "defaultValue": null}]);
    let mut old = field("legacy", string.clone());
    old["isDeprecated"] = json!(true);
    old["deprecationReason"] = json!("Use name");
    let mut with_desc = field("name", string.clone());
    with_desc["description"] = json!("The display name");
    json!({
        "__schema": {
            "queryType": {"name": "Query"},
            "mutationType": null,
            "subscriptionType": {"name": "Subscription"},
            "types": [
                {"kind": "OBJECT", "name": "Query", "description": null, "fields": [user_field, field("users", list_of_users)], "interfaces": [], "inputFields": null, "enumValues": null, "possibleTypes": null},
                {"kind": "OBJECT", "name": "User", "description": "A person", "fields": [field("id", non_null_id), with_desc, old], "interfaces": [named("INTERFACE", "Node")], "inputFields": null, "enumValues": null, "possibleTypes": null},
                {"kind": "INTERFACE", "name": "Node", "description": null, "fields": [field("id", type_ref("NON_NULL", None, Some(named("SCALAR", "ID"))))], "interfaces": [], "inputFields": null, "enumValues": null, "possibleTypes": [named("OBJECT", "User")]},
                {"kind": "ENUM", "name": "Role", "description": null, "fields": null, "interfaces": null, "inputFields": null, "possibleTypes": null,
                 "enumValues": [{"name": "ADMIN", "description": null, "isDeprecated": false, "deprecationReason": null}, {"name": "GUEST", "description": null, "isDeprecated": true, "deprecationReason": "No longer supported"}]},
                {"kind": "INPUT_OBJECT", "name": "Filter", "description": null, "fields": null, "interfaces": null, "enumValues": null, "possibleTypes": null,
                 "inputFields": [{"name": "limit", "description": null, "type": named("SCALAR", "Int"), "defaultValue": "10"}]},
                {"kind": "UNION", "name": "Result", "description": null, "fields": null, "interfaces": null, "inputFields": null, "enumValues": null, "possibleTypes": [named("OBJECT", "User"), named("OBJECT", "Query")]},
                {"kind": "SCALAR", "name": "DateTime", "description": null, "fields": null, "interfaces": null, "inputFields": null, "enumValues": null, "possibleTypes": null},
                {"kind": "SCALAR", "name": "String", "description": null, "fields": null, "interfaces": null, "inputFields": null, "enumValues": null, "possibleTypes": null},
                {"kind": "OBJECT", "name": "__Schema", "description": null, "fields": [], "interfaces": [], "inputFields": null, "enumValues": null, "possibleTypes": null},
                {"kind": "OBJECT", "name": "Subscription", "description": null, "fields": [field("tick", named("SCALAR", "Int"))], "interfaces": [], "inputFields": null, "enumValues": null, "possibleTypes": null}
            ],
            "directives": [
                {"name": "skip", "description": null, "locations": ["FIELD"], "args": []},
                {"name": "auth", "description": "Requires a role", "locations": ["FIELD_DEFINITION", "OBJECT"], "args": [{"name": "role", "description": null, "type": named("ENUM", "Role"), "defaultValue": "ADMIN"}]}
            ]
        }
    })
}

#[test]
fn prints_sdl_like_graphql_js() {
    let sdl = print_introspection_schema(&sample_introspection()).unwrap();
    let expected = r#""""Requires a role"""
directive @auth(role: Role = ADMIN) on FIELD_DEFINITION | OBJECT

type Query {
  user(id: ID!): User
  users: [User!]!
}

"""A person"""
type User implements Node {
  id: ID!

  """The display name"""
  name: String
  legacy: String @deprecated(reason: "Use name")
}

interface Node {
  id: ID!
}

enum Role {
  ADMIN
  GUEST @deprecated
}

input Filter {
  limit: Int = 10
}

union Result = User | Query

scalar DateTime

type Subscription {
  tick: Int
}"#;
    assert_eq!(sdl, expected);
}

#[test]
fn prints_a_schema_block_for_custom_root_names() {
    let data = json!({"__schema": {"queryType": {"name": "RootQuery"}, "mutationType": {"name": "RootMutation"}, "subscriptionType": null, "types": [], "directives": []}});
    assert_eq!(
        print_introspection_schema(&data).unwrap(),
        "schema {\n  query: RootQuery\n  mutation: RootMutation\n}"
    );
}

#[test]
fn prints_arg_descriptions_and_multiline_blocks() {
    let mut f = field("f", named("SCALAR", "Int"));
    f["args"] = json!([
        {"name": "a", "description": "first", "type": named("SCALAR", "Int"), "defaultValue": null},
        {"name": "b", "description": "line1\nline2", "type": named("SCALAR", "Int"), "defaultValue": null}
    ]);
    let data = json!({"__schema": {"queryType": {"name": "Query"}, "types": [
        {"kind": "OBJECT", "name": "Query", "description": "x".repeat(80), "fields": [f], "interfaces": []}
    ], "directives": []}});
    let sdl = print_introspection_schema(&data).unwrap();
    let long = "x".repeat(80);
    assert_eq!(
        sdl,
        format!("\"\"\"\n{long}\n\"\"\"\ntype Query {{\n  f(\n    \"\"\"first\"\"\"\n    a: Int\n\n    \"\"\"\n    line1\n    line2\n    \"\"\"\n    b: Int\n  ): Int\n}}")
    );
}

#[test]
fn rejects_results_without_a_schema() {
    assert!(print_introspection_schema(&json!({})).is_err());
}

#[tokio::test]
async fn introspect_returns_sdl_and_caches_per_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("authorization", "Bearer t"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"data": sample_introspection()})),
        )
        .expect(2) // first call, and again after the cache is cleared
        .mount(&server)
        .await;
    let c = GraphQLClient::new();
    let headers = Some(IndexMap::from([(
        "Authorization".to_string(),
        "Bearer t".to_string(),
    )]));
    let sdl = c.introspect(&server.uri(), headers.clone()).await.unwrap();
    assert!(sdl.contains("type Query {"));
    assert!(!sdl.contains("__Schema") && !sdl.contains("directive @skip"));
    // Served from the cache (no second request, even without headers).
    assert_eq!(c.introspect(&server.uri(), None).await.unwrap(), sdl);
    c.clear_schema_cache(Some(&server.uri()));
    assert_eq!(c.introspect(&server.uri(), headers).await.unwrap(), sdl);
}

#[tokio::test]
async fn introspect_sends_the_standard_introspection_query() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|r: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&r.body).unwrap();
            let q = body["query"].as_str().unwrap();
            assert!(
                q.contains("query IntrospectionQuery") && q.contains("fragment TypeRef on __Type")
            );
            ResponseTemplate::new(200).set_body_json(json!({"data": sample_introspection()}))
        })
        .expect(1)
        .mount(&server)
        .await;
    GraphQLClient::new()
        .introspect(&server.uri(), None)
        .await
        .unwrap();
}

#[tokio::test]
async fn introspect_error_cases() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/errors"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"errors": [{"message": "no"}, {"message": "nope"}]})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/empty"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": null})))
        .mount(&server)
        .await;
    let c = GraphQLClient::new();
    let e = c
        .introspect(&format!("{}/errors", server.uri()), None)
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Introspection failed: no, nope");
    let e = c
        .introspect(&format!("{}/empty", server.uri()), None)
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), "Introspection returned no data");
}

// ---------- subscriptions (graphql-transport-ws) ----------

#[test]
fn http_urls_become_ws_urls() {
    assert_eq!(ws_url_for("http://h/graphql"), "ws://h/graphql");
    assert_eq!(ws_url_for("https://h/graphql"), "wss://h/graphql");
    assert_eq!(ws_url_for("ws://h/graphql"), "ws://h/graphql");
}

#[derive(Debug)]
enum Seen {
    Protocol(Option<String>),
    Frame(Value),
}

/// A graphql-transport-ws server. After `subscribe` it sends `script` (JSON frames with the
/// literal `$ID` replaced by the subscription id), then forwards every client frame to `seen`.
async fn gql_server(
    script: Vec<Value>,
    close_after: Option<(u16, &'static str)>,
) -> (String, UnboundedReceiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = unbounded_channel();
    tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let tx2 = tx.clone();
        let mut ws =
            tokio_tungstenite::accept_hdr_async(sock, move |r: &SrvReq, mut resp: SrvResp| {
                let p = r
                    .headers()
                    .get("sec-websocket-protocol")
                    .map(|v| v.to_str().unwrap().to_string());
                let _ = tx2.send(Seen::Protocol(p));
                resp.headers_mut().insert(
                    "Sec-WebSocket-Protocol",
                    "graphql-transport-ws".parse().unwrap(),
                );
                Ok(resp)
            })
            .await
            .unwrap();
        while let Some(Ok(msg)) = ws.next().await {
            let Message::Text(t) = msg else { continue };
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            let _ = tx.send(Seen::Frame(v.clone()));
            match v["type"].as_str().unwrap() {
                "connection_init" => ws
                    .send(Message::text(json!({"type": "connection_ack"}).to_string()))
                    .await
                    .unwrap(),
                "subscribe" => {
                    let sub_id = v["id"].as_str().unwrap().to_string();
                    for frame in &script {
                        let s = frame.to_string().replace("$ID", &sub_id);
                        ws.send(Message::text(s)).await.unwrap();
                    }
                    if let Some((code, reason)) = close_after {
                        let f = tokio_tungstenite::tungstenite::protocol::CloseFrame {
                            code: code.into(),
                            reason: reason.into(),
                        };
                        ws.send(Message::Close(Some(f))).await.unwrap();
                    }
                }
                _ => {}
            }
        }
    });
    (url, rx)
}

fn collector() -> (
    impl Fn(SubscriptionEvent) + Send + Sync + 'static,
    UnboundedReceiver<SubscriptionEvent>,
) {
    let (tx, rx) = unbounded_channel();
    (move |e| drop(tx.send(e)), rx)
}

async fn next_event(rx: &mut UnboundedReceiver<SubscriptionEvent>) -> SubscriptionEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out")
        .expect("closed")
}

async fn next_seen(rx: &mut UnboundedReceiver<Seen>) -> Seen {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out")
        .expect("closed")
}

fn sub(url: &str) -> SubscriptionOptions {
    SubscriptionOptions {
        url: url.to_string(),
        query: "subscription { tick }".into(),
        ..Default::default()
    }
}

use std::time::Duration;

#[tokio::test]
async fn subscription_full_flow_next_ping_complete() {
    let script = vec![
        json!({"type": "ping"}),
        json!({"id": "$ID", "type": "next", "payload": {"data": {"tick": 1}}}),
        json!({"id": "other", "type": "next", "payload": {"data": {"tick": 99}}}),
        json!({"id": "$ID", "type": "next", "payload": {"errors": [{"message": "partial"}]}}),
        json!({"type": "pong"}),
        json!({"id": "$ID", "type": "complete"}),
    ];
    let (url, mut seen) = gql_server(script, None).await;
    let (sink, mut events) = collector();
    let client = GraphQLClient::new();
    let mut o = sub(&url);
    o.variables = Some(json!({"n": 2}));
    o.operation_name = Some("Tick".into());
    o.headers = Some(IndexMap::from([(
        "authorization".to_string(),
        "Bearer t".to_string(),
    )]));
    client.subscribe(o, sink);

    let SubscriptionEvent::Data(m1) = next_event(&mut events).await else {
        panic!("expected data")
    };
    assert_eq!(m1.data, Some(json!({"tick": 1})));
    assert_eq!(m1.errors, None);
    assert!(m1.timestamp > 0 && !m1.id.is_empty());
    let SubscriptionEvent::Data(m2) = next_event(&mut events).await else {
        panic!("expected data")
    };
    assert_eq!(m2.data, None);
    assert_eq!(m2.errors, Some(json!([{"message": "partial"}])));
    assert_eq!(
        next_event(&mut events).await,
        SubscriptionEvent::Complete { message_count: 2 }
    );

    // What the server saw: subprotocol, connection_init with the headers as payload,
    // subscribe with the query/variables/operationName, and a pong for the ping.
    assert!(
        matches!(next_seen(&mut seen).await, Seen::Protocol(Some(p)) if p == "graphql-transport-ws")
    );
    let Seen::Frame(init) = next_seen(&mut seen).await else {
        panic!()
    };
    assert_eq!(
        init,
        json!({"type": "connection_init", "payload": {"authorization": "Bearer t"}})
    );
    let Seen::Frame(s) = next_seen(&mut seen).await else {
        panic!()
    };
    assert_eq!(s["type"], "subscribe");
    assert_eq!(
        s["payload"],
        json!({"query": "subscription { tick }", "variables": {"n": 2}, "operationName": "Tick"})
    );
    let Seen::Frame(pong) = next_seen(&mut seen).await else {
        panic!()
    };
    assert_eq!(pong, json!({"type": "pong"}));
}

#[tokio::test]
async fn connection_init_has_no_payload_without_headers() {
    let (url, mut seen) = gql_server(vec![json!({"id": "$ID", "type": "complete"})], None).await;
    let (sink, mut events) = collector();
    let client = GraphQLClient::new();
    client.subscribe(sub(&url), sink);
    assert_eq!(
        next_event(&mut events).await,
        SubscriptionEvent::Complete { message_count: 0 }
    );
    next_seen(&mut seen).await;
    let Seen::Frame(init) = next_seen(&mut seen).await else {
        panic!()
    };
    assert_eq!(init, json!({"type": "connection_init"}));
    let Seen::Frame(s) = next_seen(&mut seen).await else {
        panic!()
    };
    assert_eq!(s["payload"], json!({"query": "subscription { tick }"}));
}

#[tokio::test]
async fn subscription_error_message_is_reported() {
    let script = vec![
        json!({"id": "$ID", "type": "error", "payload": [{"message": "boom"}, {"message": "bang"}]}),
    ];
    let (url, _seen) = gql_server(script, None).await;
    let (sink, mut events) = collector();
    let client = GraphQLClient::new();
    client.subscribe(sub(&url), sink);
    assert_eq!(
        next_event(&mut events).await,
        SubscriptionEvent::Error("boom, bang".into())
    );
}

#[tokio::test]
async fn subscription_reports_abnormal_close_and_connection_failure() {
    let (url, _seen) = gql_server(vec![], Some((4401, "Unauthorized"))).await;
    let (sink, mut events) = collector();
    let client = GraphQLClient::new();
    client.subscribe(sub(&url), sink);
    assert_eq!(
        next_event(&mut events).await,
        SubscriptionEvent::Error("Connection closed: 4401 Unauthorized".into())
    );

    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (sink, mut events) = collector();
    client.subscribe(sub(&format!("http://127.0.0.1:{port}")), sink);
    assert!(matches!(next_event(&mut events).await, SubscriptionEvent::Error(m) if !m.is_empty()));
}

#[tokio::test]
async fn normal_server_close_counts_as_complete() {
    let script = vec![json!({"id": "$ID", "type": "next", "payload": {"data": {"tick": 1}}})];
    let (url, _seen) = gql_server(script, Some((1000, ""))).await;
    let (sink, mut events) = collector();
    let client = GraphQLClient::new();
    client.subscribe(sub(&url), sink);
    assert!(matches!(
        next_event(&mut events).await,
        SubscriptionEvent::Data(_)
    ));
    assert_eq!(
        next_event(&mut events).await,
        SubscriptionEvent::Complete { message_count: 1 }
    );
}

#[tokio::test]
async fn unsubscribe_sends_complete_and_stops_events() {
    let script = vec![json!({"id": "$ID", "type": "next", "payload": {"data": {"tick": 1}}})];
    let (url, mut seen) = gql_server(script, None).await;
    let (sink, mut events) = collector();
    let client = GraphQLClient::new();
    client.subscribe(sub(&url), sink);
    assert!(matches!(
        next_event(&mut events).await,
        SubscriptionEvent::Data(_)
    ));
    client.unsubscribe();
    // Frames seen: protocol, init, subscribe, then the client's `complete`.
    let mut complete_seen = false;
    for _ in 0..4 {
        if let Seen::Frame(f) = next_seen(&mut seen).await {
            if f["type"] == "complete" {
                complete_seen = true;
                break;
            }
        }
    }
    assert!(
        complete_seen,
        "server never saw the client's complete message"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(events.try_recv().is_err(), "no events after unsubscribe");
}

#[tokio::test]
async fn subscribe_replaces_the_previous_subscription() {
    let (url1, mut seen1) = gql_server(vec![], None).await;
    let (url2, _seen2) = gql_server(
        vec![json!({"id": "$ID", "type": "next", "payload": {"data": {"n": 2}}})],
        None,
    )
    .await;
    let client = GraphQLClient::new();
    let (sink1, mut events1) = collector();
    client.subscribe(sub(&url1), sink1);
    // Wait until the first subscription is live on the server (protocol, init, subscribe).
    for _ in 0..3 {
        next_seen(&mut seen1).await;
    }
    let (sink2, mut events2) = collector();
    client.subscribe(sub(&url2), sink2);
    assert!(matches!(
        next_event(&mut events2).await,
        SubscriptionEvent::Data(_)
    ));
    let Seen::Frame(f) = next_seen(&mut seen1).await else {
        panic!()
    };
    assert_eq!(f["type"], "complete");
    assert!(events1.try_recv().is_err());
}

#[test]
fn types_use_the_ts_json_shapes() {
    let o: GraphQLRequestOptions = serde_json::from_value(
        json!({"url": "u", "query": "q", "operationName": "O", "variables": {"a": 1}}),
    )
    .unwrap();
    assert_eq!(o.operation_name.as_deref(), Some("O"));
    let m = SubscriptionMessage {
        id: "i".into(),
        timestamp: 5,
        data: Some(json!(1)),
        errors: None,
    };
    assert_eq!(
        serde_json::to_value(&m).unwrap(),
        json!({"id": "i", "timestamp": 5, "data": 1})
    );
}
