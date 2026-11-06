#![allow(clippy::result_large_err)] // tungstenite's handshake callback signature
use super::*;
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request as SrvReq, Response as SrvResp};

/// Echo server; returns its ws:// URL and the number of accepted connections.
async fn echo_server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    tokio::spawn(async move {
        loop {
            let (sock, _) = listener.accept().await.unwrap();
            c.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
                while let Some(Ok(msg)) = ws.next().await {
                    if (msg.is_text() || msg.is_binary()) && ws.send(msg).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    (url, count)
}

fn client() -> (WebSocketClient, UnboundedReceiver<WsEvent>) {
    let (tx, rx) = unbounded_channel();
    (WebSocketClient::new(move |e| drop(tx.send(e))), rx)
}

fn opts(url: &str) -> WsConnectOptions {
    WsConnectOptions {
        url: url.to_string(),
        ..Default::default()
    }
}

/// Collects events until `pred` matches one; returns everything seen so far.
async fn until(
    rx: &mut UnboundedReceiver<WsEvent>,
    pred: impl Fn(&WsEvent) -> bool,
) -> Vec<WsEvent> {
    let mut seen = Vec::new();
    loop {
        let ev = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("event channel closed");
        let done = pred(&ev);
        seen.push(ev);
        if done {
            return seen;
        }
    }
}

fn is_status(s: WsConnectionStatus) -> impl Fn(&WsEvent) -> bool {
    move |e| *e == WsEvent::Status(s)
}

#[tokio::test]
async fn connect_send_receive_close() {
    let (url, _) = echo_server().await;
    let (c, mut rx) = client();
    assert_eq!(c.status(), WsConnectionStatus::Idle);
    c.connect(opts(&url)).unwrap();
    let seen = until(&mut rx, is_status(WsConnectionStatus::Open)).await;
    assert_eq!(
        seen,
        vec![
            WsEvent::Status(WsConnectionStatus::Connecting),
            WsEvent::Status(WsConnectionStatus::Open)
        ]
    );

    let sent = c.send("héllo").unwrap();
    assert_eq!(sent.direction, WsDirection::Sent);
    assert_eq!(sent.kind, WsMessageType::Text);
    assert_eq!(sent.size, 6);
    let seen = until(&mut rx, |e| matches!(e, WsEvent::Message(_))).await;
    let WsEvent::Message(m) = seen.last().unwrap() else {
        unreachable!()
    };
    assert_eq!(
        (m.direction, m.data.as_str(), m.size),
        (WsDirection::Received, "héllo", 6)
    );

    c.disconnect(None, None);
    let seen = until(&mut rx, |e| matches!(e, WsEvent::Close { .. })).await;
    assert_eq!(seen[0], WsEvent::Status(WsConnectionStatus::Closing));
    assert_eq!(seen[1], WsEvent::Status(WsConnectionStatus::Closed));
    assert_eq!(
        seen[2],
        WsEvent::Close {
            code: 1000,
            reason: String::new()
        }
    );
    assert_eq!(c.status(), WsConnectionStatus::Closed);
}

#[tokio::test]
async fn keeps_message_history_for_both_directions() {
    let (url, _) = echo_server().await;
    let (c, mut rx) = client();
    c.connect(opts(&url)).unwrap();
    until(&mut rx, is_status(WsConnectionStatus::Open)).await;
    c.send("a").unwrap();
    until(&mut rx, |e| matches!(e, WsEvent::Message(_))).await;
    let h = c.messages();
    assert_eq!(h.len(), 2);
    assert_eq!(
        (h[0].direction, h[0].data.as_str()),
        (WsDirection::Sent, "a")
    );
    assert_eq!(
        (h[1].direction, h[1].data.as_str()),
        (WsDirection::Received, "a")
    );
    // The returned copy is detached from the log.
    let mut copy = c.messages();
    copy.clear();
    assert_eq!(c.messages().len(), 2);
    c.clear_messages();
    assert!(c.messages().is_empty());
    assert!(matches!(rx.recv().await, Some(WsEvent::MessagesCleared)));
}

#[tokio::test]
async fn binary_frames_are_hex_encoded() {
    let (url, _) = echo_server().await;
    let (c, mut rx) = client();
    c.connect(opts(&url)).unwrap();
    until(&mut rx, is_status(WsConnectionStatus::Open)).await;
    let sent = c.send_binary(vec![0xde, 0xad, 0xbe, 0xef]).unwrap();
    assert_eq!(
        (sent.kind, sent.data.as_str(), sent.size),
        (WsMessageType::Binary, "deadbeef", 4)
    );
    let seen = until(&mut rx, |e| matches!(e, WsEvent::Message(_))).await;
    let WsEvent::Message(m) = seen.last().unwrap() else {
        unreachable!()
    };
    assert_eq!(
        (m.kind, m.data.as_str(), m.size),
        (WsMessageType::Binary, "deadbeef", 4)
    );
}

#[tokio::test]
async fn history_is_capped_at_1000() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
        for i in 0..1005 {
            ws.send(Message::text(i.to_string())).await.unwrap();
        }
        while ws.next().await.is_some() {}
    });
    let (c, mut rx) = client();
    c.connect(opts(&url)).unwrap();
    let mut n = 0;
    while n < 1005 {
        until(&mut rx, |e| matches!(e, WsEvent::Message(_))).await;
        n += 1;
    }
    let h = c.messages();
    assert_eq!(h.len(), 1000);
    assert_eq!(h[0].data, "5");
    assert_eq!(h[999].data, "1004");
}

#[tokio::test]
async fn sends_custom_headers_and_subprotocols() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let (seen_tx, seen_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let mut seen_tx = Some(seen_tx);
        let ws = accept_hdr_async(sock, |req: &SrvReq, mut resp: SrvResp| {
            let h = req.headers();
            let get = |n: &str| h.get(n).map(|v| v.to_str().unwrap().to_string());
            let _ = seen_tx
                .take()
                .unwrap()
                .send((get("x-token"), get("sec-websocket-protocol")));
            resp.headers_mut()
                .insert("Sec-WebSocket-Protocol", "chat".parse().unwrap());
            Ok(resp)
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        drop(ws);
    });
    let (c, mut rx) = client();
    let mut o = opts(&url);
    o.headers = Some(IndexMap::from([(
        "X-Token".to_string(),
        "secret".to_string(),
    )]));
    o.protocols = Some(vec!["chat".into(), "superchat".into()]);
    c.connect(o).unwrap();
    until(&mut rx, is_status(WsConnectionStatus::Open)).await;
    let (token, protos) = seen_rx.await.unwrap();
    assert_eq!(token.as_deref(), Some("secret"));
    assert_eq!(protos.as_deref(), Some("chat, superchat"));
}

#[tokio::test]
async fn send_requires_an_open_connection() {
    let (c, _rx) = client();
    let err = c.send("x").unwrap_err();
    assert_eq!(err.to_string(), "WebSocket is not connected");
    assert!(c.send_binary(vec![1]).is_err());
    assert!(c.messages().is_empty());
}

#[tokio::test]
async fn rejects_invalid_urls_synchronously() {
    let (c, _rx) = client();
    assert!(c.connect(opts("not a url")).is_err());
    assert!(c.connect(opts("http://localhost:1")).is_err());
    let mut o = opts("ws://localhost:1");
    o.headers = Some(IndexMap::from([(
        "bad header".to_string(),
        "x".to_string(),
    )]));
    assert!(c.connect(o).is_err());
    assert_eq!(c.status(), WsConnectionStatus::Idle);
}

#[tokio::test]
async fn connection_refused_emits_error_then_close() {
    // Grab a free port, then release it so nothing listens there.
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (c, mut rx) = client();
    c.connect(opts(&format!("ws://127.0.0.1:{port}"))).unwrap();
    let seen = until(&mut rx, |e| matches!(e, WsEvent::Close { .. })).await;
    assert_eq!(seen[0], WsEvent::Status(WsConnectionStatus::Connecting));
    assert_eq!(seen[1], WsEvent::Status(WsConnectionStatus::Error));
    assert!(matches!(&seen[2], WsEvent::Error(m) if !m.is_empty()));
    assert_eq!(seen[3], WsEvent::Status(WsConnectionStatus::Closed));
    assert_eq!(
        seen[4],
        WsEvent::Close {
            code: 1006,
            reason: String::new()
        }
    );
    // No auto-reconnect configured: nothing else happens.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(rx.try_recv().is_err());
}

/// Server that completes the handshake and then closes with `code`/`reason`.
async fn closing_server(code: u16, reason: &'static str) -> (String, Arc<AtomicUsize>) {
    closing_server_after(code, reason, 0).await
}

/// Like `closing_server`, but keeps the connection open for `hold_ms` first.
async fn closing_server_after(
    code: u16,
    reason: &'static str,
    hold_ms: u64,
) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    tokio::spawn(async move {
        loop {
            let (sock, _) = listener.accept().await.unwrap();
            c.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
                tokio::time::sleep(Duration::from_millis(hold_ms)).await;
                let frame = CloseFrame {
                    code: CloseCode::from(code),
                    reason: reason.into(),
                };
                let _ = ws.send(Message::Close(Some(frame))).await;
                while ws.next().await.is_some() {}
            });
        }
    });
    (url, count)
}

#[tokio::test]
async fn server_close_reports_code_and_reason() {
    let (url, _) = closing_server(4001, "bye").await;
    let (c, mut rx) = client();
    c.connect(opts(&url)).unwrap();
    let seen = until(&mut rx, |e| matches!(e, WsEvent::Close { .. })).await;
    assert_eq!(
        seen.last().unwrap(),
        &WsEvent::Close {
            code: 4001,
            reason: "bye".into()
        }
    );
    assert_eq!(c.status(), WsConnectionStatus::Closed);
}

#[tokio::test]
async fn auto_reconnect_retries_up_to_the_max_then_stops() {
    // Nothing listens: every attempt fails before opening, so the counter is never reset.
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (c, mut rx) = client();
    let mut o = opts(&format!("ws://127.0.0.1:{port}"));
    o.auto_reconnect = Some(true);
    o.reconnect_delay = Some(20);
    o.max_reconnect_attempts = Some(2);
    c.connect(o).unwrap();
    let mut closes = 0;
    while closes < 3 {
        until(&mut rx, |e| matches!(e, WsEvent::Close { .. })).await;
        closes += 1;
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    // The first attempt plus two reconnects, then it gives up.
    assert!(rx.try_recv().is_err());
    assert_eq!(c.status(), WsConnectionStatus::Closed);
}

#[tokio::test]
async fn immediate_close_after_open_does_not_reset_the_reconnect_counter() {
    // The server closes every connection right after the handshake. A bare open must not reset
    // the counter (the TS did, reconnecting forever): with a budget of 1 we get one reconnect.
    let (url, count) = closing_server(1000, "").await;
    let (c, mut rx) = client();
    let mut o = opts(&url);
    o.auto_reconnect = Some(true);
    o.reconnect_delay = Some(10);
    o.max_reconnect_attempts = Some(1);
    c.connect(o).unwrap();
    until(&mut rx, |e| matches!(e, WsEvent::Close { .. })).await;
    until(&mut rx, |e| matches!(e, WsEvent::Close { .. })).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(c.status(), WsConnectionStatus::Closed);
    while let Ok(e) = rx.try_recv() {
        assert!(
            !is_status(WsConnectionStatus::Open)(&e),
            "reconnected beyond the budget"
        );
    }
}

#[tokio::test]
async fn stable_connection_resets_the_reconnect_counter() {
    // Each connection stays open longer than STABLE_OPEN (300 ms in tests), so the budget of 1
    // is renewed every time and reconnecting continues.
    let (url, count) = closing_server_after(1000, "", 450).await;
    let (c, mut rx) = client();
    let mut o = opts(&url);
    o.auto_reconnect = Some(true);
    o.reconnect_delay = Some(10);
    o.max_reconnect_attempts = Some(1);
    c.connect(o).unwrap();
    for _ in 0..3 {
        until(&mut rx, is_status(WsConnectionStatus::Open)).await;
    }
    assert!(count.load(Ordering::SeqCst) >= 3);
}

#[tokio::test]
async fn disconnect_cancels_auto_reconnect() {
    let (url, count) = echo_server().await;
    let (c, mut rx) = client();
    let mut o = opts(&url);
    o.auto_reconnect = Some(true);
    o.reconnect_delay = Some(20);
    c.connect(o).unwrap();
    until(&mut rx, is_status(WsConnectionStatus::Open)).await;
    c.disconnect(Some(1000), Some("done".into()));
    until(&mut rx, |e| matches!(e, WsEvent::Close { .. })).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(c.status(), WsConnectionStatus::Closed);
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn disconnect_during_reconnect_wait_stops_the_loop() {
    let (url, count) = closing_server(1001, "").await;
    let (c, mut rx) = client();
    let mut o = opts(&url);
    o.auto_reconnect = Some(true);
    o.reconnect_delay = Some(300);
    c.connect(o).unwrap();
    until(&mut rx, |e| matches!(e, WsEvent::Close { .. })).await;
    c.disconnect(None, None);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(c.status(), WsConnectionStatus::Closed);
}

#[test]
fn options_and_messages_use_the_ts_json_shapes() {
    let o: WsConnectOptions = serde_json::from_str(
        r#"{"url":"ws://x","autoReconnect":true,"reconnectDelay":5,"maxReconnectAttempts":1,"rejectUnauthorized":false}"#,
    )
    .unwrap();
    assert_eq!(
        (
            o.auto_reconnect,
            o.reconnect_delay,
            o.max_reconnect_attempts
        ),
        (Some(true), Some(5), Some(1))
    );
    let m = WsMessage {
        id: "1".into(),
        direction: WsDirection::Sent,
        timestamp: 7,
        kind: WsMessageType::Text,
        data: "d".into(),
        size: 1,
    };
    assert_eq!(
        serde_json::to_value(&m).unwrap(),
        serde_json::json!({"id":"1","direction":"sent","timestamp":7,"type":"text","data":"d","size":1})
    );
    assert_eq!(
        serde_json::to_value(WsConnectionStatus::Closing).unwrap(),
        "closing"
    );
}
