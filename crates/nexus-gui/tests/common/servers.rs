//! Local servers for the panel tests and the dev server example: WebSocket echo, a
//! `graphql-transport-ws` subscription server, a GraphQL-over-HTTP mock and the MCP fixture path.
#![allow(dead_code, clippy::result_large_err)]

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{Request as SrvReq, Response as SrvResp};
use tokio_tungstenite::tungstenite::Message;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Echoes every text frame as `echo: <text>`; returns `ws://127.0.0.1:<port>`.
pub async fn ws_echo() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let Ok(mut ws) = tokio_tungstenite::accept_async(sock).await else {
                    return;
                };
                while let Some(Ok(msg)) = ws.next().await {
                    match msg {
                        Message::Text(t) => {
                            if ws
                                .send(Message::text(format!("echo: {}", t.as_str())))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Message::Close(_) => return,
                        _ => {}
                    }
                }
            });
        }
    });
    url
}

/// Accepts WebSocket connections and closes them at once (for the reconnect test).
pub async fn ws_closer() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                if let Ok(mut ws) = tokio_tungstenite::accept_async(sock).await {
                    let _ = ws.close(None).await;
                }
            });
        }
    });
    url
}

/// A `graphql-transport-ws` server: acks `connection_init`, answers `subscribe` with `events`
/// `next` messages (`{"tick": n}`, `gap_ms` apart) and stays open until the client sends
/// `complete`. Returns `http://127.0.0.1:<port>` (the client turns it into `ws://`).
pub async fn gql_ws(events: u32, gap_ms: u64) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let Ok(mut ws) =
                    tokio_tungstenite::accept_hdr_async(sock, |_: &SrvReq, mut resp: SrvResp| {
                        resp.headers_mut().insert(
                            "Sec-WebSocket-Protocol",
                            "graphql-transport-ws".parse().unwrap(),
                        );
                        Ok(resp)
                    })
                    .await
                else {
                    return;
                };
                while let Some(Ok(msg)) = ws.next().await {
                    let Message::Text(t) = msg else { continue };
                    let v: Value = serde_json::from_str(t.as_str()).unwrap();
                    match v["type"].as_str().unwrap_or("") {
                        "connection_init" => {
                            let _ = ws
                                .send(Message::text(json!({"type":"connection_ack"}).to_string()))
                                .await;
                        }
                        "subscribe" => {
                            let id = v["id"].as_str().unwrap().to_string();
                            for n in 1..=events {
                                let frame = json!({
                                    "id": id, "type": "next",
                                    "payload": {"data": {"tick": n}}
                                });
                                if ws.send(Message::text(frame.to_string())).await.is_err() {
                                    return;
                                }
                                if gap_ms > 0 {
                                    tokio::time::sleep(std::time::Duration::from_millis(gap_ms))
                                        .await;
                                }
                            }
                        }
                        "complete" => return,
                        _ => {}
                    }
                }
            });
        }
    });
    url
}

fn named(kind: &str, name: &str) -> Value {
    json!({"kind": kind, "name": name, "ofType": null})
}

fn wrap(kind: &str, of: Value) -> Value {
    json!({"kind": kind, "name": null, "ofType": of})
}

fn field(name: &str, args: Vec<Value>, ty: Value, desc: Option<&str>) -> Value {
    json!({"name": name, "description": desc, "args": args, "type": ty,
           "isDeprecated": false, "deprecationReason": null})
}

fn object(name: &str, desc: Option<&str>, fields: Vec<Value>) -> Value {
    json!({"kind": "OBJECT", "name": name, "description": desc, "fields": fields,
           "inputFields": null, "interfaces": [], "enumValues": null, "possibleTypes": null})
}

/// The introspection result of a small schema (Query.users / Query.user, User, Role,
/// Subscription.tick).
pub fn introspection() -> Value {
    let non_null = |t: Value| wrap("NON_NULL", t);
    let user = named("OBJECT", "User");
    let id_arg = json!({"name": "id", "description": null,
        "type": non_null(named("SCALAR", "ID")), "defaultValue": null});
    json!({"data": {"__schema": {
        "queryType": {"name": "Query"},
        "mutationType": null,
        "subscriptionType": {"name": "Subscription"},
        "directives": [],
        "types": [
            object("Query", None, vec![
                field("users", vec![], non_null(wrap("LIST", non_null(user.clone()))), Some("All users")),
                field("user", vec![id_arg], user.clone(), None),
            ]),
            object("User", Some("A person"), vec![
                field("id", vec![], non_null(named("SCALAR", "ID")), None),
                field("name", vec![], named("SCALAR", "String"), Some("The display name")),
                field("role", vec![], named("ENUM", "Role"), None),
            ]),
            json!({"kind": "ENUM", "name": "Role", "description": null, "fields": null,
                   "inputFields": null, "interfaces": null, "possibleTypes": null,
                   "enumValues": [
                       {"name": "ADMIN", "description": null, "isDeprecated": false, "deprecationReason": null},
                       {"name": "GUEST", "description": null, "isDeprecated": false, "deprecationReason": null}]}),
            object("Subscription", None, vec![field("tick", vec![], named("SCALAR", "Int"), None)]),
        ]
    }}})
}

/// GraphQL over HTTP at `/graphql`: introspection gets [`introspection`], every other query
/// `{"data":{"users":[{"id":"1","name":"Ada"}]}}`.
pub async fn graphql_http() -> MockServer {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("IntrospectionQuery"))
        .respond_with(ResponseTemplate::new(200).set_body_json(introspection()))
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"data": {"users": [{"id": "1", "name": "Ada"}, {"id": "2", "name": "Grace"}]}}),
        ))
        .mount(&s)
        .await;
    s
}

/// Path of the MCP fixture server (built by `cargo test --workspace` as an example of
/// nexus-core; built on demand otherwise).
pub fn mcp_fixture() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    let exe = p.join("examples").join("mcp_fixture_server");
    if !exe.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let ok = std::process::Command::new(cargo)
            .args([
                "build",
                "-p",
                "nexus-core",
                "--example",
                "mcp_fixture_server",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok && exe.exists(), "cannot build {}", exe.display());
    }
    exe
}
