//! GraphQL subscriptions over the `graphql-transport-ws` protocol (the `graphql-ws` library's
//! wire format): `connection_init`/`connection_ack`, `subscribe`, `next`, `error`, `complete`
//! and `ping`/`pong`.

use futures_util::{SinkExt, StreamExt};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

const SUBPROTOCOL: &str = "graphql-transport-ws";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionOptions {
    /// `http(s)://` URLs are converted to `ws(s)://`.
    pub url: String,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
    /// Sent as the `connection_init` payload (graphql-ws `connectionParams`), not as HTTP headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
}

/// One `next` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionMessage {
    pub id: String,
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errors: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionEvent {
    Data(SubscriptionMessage),
    /// A human-readable error (connection failure, protocol error, GraphQL `error` message).
    Error(String),
    Complete {
        message_count: u64,
    },
}

/// `url.replace(/^http/, 'ws')`.
pub fn ws_url_for(url: &str) -> String {
    match url.strip_prefix("http") {
        Some(rest) => format!("ws{rest}"),
        None => url.to_string(),
    }
}

fn text(v: Value) -> Message {
    Message::text(v.to_string())
}

pub(super) async fn run(
    opts: SubscriptionOptions,
    sink: impl Fn(SubscriptionEvent) + Send + Sync + 'static,
    mut stop: oneshot::Receiver<()>,
) {
    let sub_id = uuid::Uuid::new_v4().to_string();
    let mut count = 0u64;
    let mut subscribed = false;

    let mut req = match ws_url_for(&opts.url).as_str().into_client_request() {
        Ok(r) => r,
        Err(e) => return sink(SubscriptionEvent::Error(e.to_string())),
    };
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        SUBPROTOCOL.parse().expect("static header value"),
    );

    let mut ws = tokio::select! {
        _ = &mut stop => return,
        res = connect_async(req) => match res {
            Ok((ws, _)) => ws,
            Err(e) => return sink(SubscriptionEvent::Error(e.to_string())),
        },
    };

    let mut init = json!({ "type": "connection_init" });
    if let Some(h) = &opts.headers {
        init["payload"] = json!(h);
    }
    if let Err(e) = ws.send(text(init)).await {
        return sink(SubscriptionEvent::Error(e.to_string()));
    }

    loop {
        tokio::select! {
            _ = &mut stop => {
                // Unsubscribed by the caller: tell the server, then close; no more events.
                if subscribed {
                    let _ = ws.send(text(json!({ "id": sub_id, "type": "complete" }))).await;
                }
                let frame = CloseFrame { code: CloseCode::Normal, reason: "Normal Closure".into() };
                let _ = tokio::time::timeout(Duration::from_secs(1), ws.send(Message::Close(Some(frame)))).await;
                return;
            }
            frame = ws.next() => {
                let msg = match frame {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => return sink(SubscriptionEvent::Error(e.to_string())),
                    None => return sink(SubscriptionEvent::Error("Connection closed".into())),
                };
                match msg {
                    Message::Text(t) => {
                        let Ok(v) = serde_json::from_str::<Value>(t.as_str()) else {
                            return sink(SubscriptionEvent::Error("Invalid message: not JSON".into()));
                        };
                        match v.get("type").and_then(Value::as_str) {
                            Some("connection_ack") if !subscribed => {
                                subscribed = true;
                                let mut payload = serde_json::Map::new();
                                payload.insert("query".into(), Value::String(opts.query.clone()));
                                if let Some(vars) = &opts.variables {
                                    payload.insert("variables".into(), vars.clone());
                                }
                                if let Some(n) = &opts.operation_name {
                                    payload.insert("operationName".into(), Value::String(n.clone()));
                                }
                                let sub = json!({ "id": sub_id, "type": "subscribe", "payload": payload });
                                if let Err(e) = ws.send(text(sub)).await {
                                    return sink(SubscriptionEvent::Error(e.to_string()));
                                }
                            }
                            Some("ping") => {
                                if let Err(e) = ws.send(text(json!({ "type": "pong" }))).await {
                                    return sink(SubscriptionEvent::Error(e.to_string()));
                                }
                            }
                            Some("next") if v.get("id").and_then(Value::as_str) == Some(&sub_id) => {
                                count += 1;
                                let payload = v.get("payload").cloned().unwrap_or(Value::Null);
                                sink(SubscriptionEvent::Data(SubscriptionMessage {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                    data: payload.get("data").cloned(),
                                    errors: payload.get("errors").cloned(),
                                }));
                            }
                            Some("error") if v.get("id").and_then(Value::as_str) == Some(&sub_id) => {
                                return sink(SubscriptionEvent::Error(describe_errors(v.get("payload"))));
                            }
                            Some("complete") if v.get("id").and_then(Value::as_str) == Some(&sub_id) => {
                                return sink(SubscriptionEvent::Complete { message_count: count });
                            }
                            _ => {}
                        }
                    }
                    Message::Close(frame) => {
                        let (code, reason) = frame.map(|f| (u16::from(f.code), f.reason.to_string())).unwrap_or((1005, String::new()));
                        return sink(if code == 1000 {
                            SubscriptionEvent::Complete { message_count: count }
                        } else {
                            SubscriptionEvent::Error(format!("Connection closed: {code} {reason}").trim_end().to_string())
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}

/// The `error` message payload is a list of GraphQL errors.
fn describe_errors(payload: Option<&Value>) -> String {
    match payload {
        Some(Value::Array(errs)) => {
            let msgs: Vec<&str> = errs
                .iter()
                .filter_map(|e| e.get("message").and_then(Value::as_str))
                .collect();
            if msgs.is_empty() {
                Value::Array(errs.clone()).to_string()
            } else {
                msgs.join(", ")
            }
        }
        Some(v) => v.to_string(),
        None => "Subscription error".to_string(),
    }
}
