//! Port of `packages/core/src/websocket/websocket-client.ts`.
//!
//! An event-driven WebSocket client: it tracks the connection status, keeps a bounded
//! history of sent/received messages and optionally reconnects after a close. Events are
//! delivered through the sink closure passed to [`WebSocketClient::new`].

#[cfg(test)]
mod tests;

use crate::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue, Request};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{connect_async, connect_async_tls_with_config, Connector};

const MAX_MESSAGES: usize = 1000;
/// The kept log holds at most this many bytes of message data (oldest evicted first).
pub const MAX_LOG_BYTES: usize = 16 * 1024 * 1024;
/// Message data kept per message for display; `size` still reports the real length.
pub const MAX_DISPLAY_BYTES: usize = 256 * 1024;

/// Cuts `s` to at most `max` bytes on a char boundary and marks the cut.
pub fn truncate_display(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut cut = max;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push_str("\n[message truncated]");
    }
    s
}

/// Drops the oldest messages until both the count and byte caps hold (the newest always stays).
pub fn evict_messages(messages: &mut Vec<WsMessage>, max_count: usize, max_bytes: usize) {
    let mut total: usize = messages.iter().map(|m| m.data.len()).sum();
    let mut drop = messages.len().saturating_sub(max_count);
    for m in &messages[..drop] {
        total -= m.data.len();
    }
    while total > max_bytes && drop + 1 < messages.len() {
        total -= messages[drop].data.len();
        drop += 1;
    }
    messages.drain(..drop);
}
const DEFAULT_RECONNECT_DELAY_MS: u64 = 3000;
const DEFAULT_MAX_RECONNECT_ATTEMPTS: u32 = 5;
/// How long to wait for the peer's close frame after we sent ours (same as `ws`).
const CLOSE_TIMEOUT: Duration = Duration::from_secs(30);
/// A connection must stay open this long before the reconnect counter is reset, so a server that
/// accepts and immediately closes cannot cause endless reconnects (the TS reset on every open).
#[cfg(not(test))]
const STABLE_OPEN: Duration = Duration::from_secs(5);
#[cfg(test)]
const STABLE_OPEN: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsConnectionStatus {
    Idle,
    Connecting,
    Open,
    Closing,
    Closed,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsDirection {
    Sent,
    Received,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsMessageType {
    Text,
    Binary,
}

/// One logged frame. Binary payloads are stored as lower-case hex.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsMessage {
    pub id: String,
    pub direction: WsDirection,
    /// Milliseconds since the Unix epoch.
    pub timestamp: i64,
    #[serde(rename = "type")]
    pub kind: WsMessageType,
    pub data: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsConnectOptions {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocols: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
    /// `false` accepts any TLS certificate (default `true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reject_unauthorized: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_reconnect: Option<bool>,
    /// Milliseconds (default 3000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconnect_delay: Option<u64>,
    /// Default 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_reconnect_attempts: Option<u32>,
}

/// What the TS client emitted as `status` / `message` / `close` / `error` / `messages-cleared`.
#[derive(Debug, Clone, PartialEq)]
pub enum WsEvent {
    Status(WsConnectionStatus),
    Message(WsMessage),
    Close { code: u16, reason: String },
    Error(String),
    MessagesCleared,
}

enum Cmd {
    Send(Message),
    Close { code: u16, reason: String },
}

struct State {
    status: WsConnectionStatus,
    messages: Vec<WsMessage>,
    options: Option<WsConnectOptions>,
    cmd: Option<mpsc::UnboundedSender<Cmd>>,
    task: Option<JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    sink: Box<dyn Fn(WsEvent) + Send + Sync>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn emit(&self, event: WsEvent) {
        (self.sink)(event);
    }

    fn set_status(&self, status: WsConnectionStatus) {
        self.lock().status = status;
        self.emit(WsEvent::Status(status));
    }

    fn add_message(&self, msg: WsMessage) {
        let mut st = self.lock();
        st.messages.push(msg);
        evict_messages(&mut st.messages, MAX_MESSAGES, MAX_LOG_BYTES);
    }
}

/// A single WebSocket connection (with optional auto-reconnect) and its message log.
pub struct WebSocketClient {
    shared: Arc<Shared>,
}

impl WebSocketClient {
    pub fn new(sink: impl Fn(WsEvent) + Send + Sync + 'static) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    status: WsConnectionStatus::Idle,
                    messages: Vec::new(),
                    options: None,
                    cmd: None,
                    task: None,
                }),
                sink: Box::new(sink),
            }),
        }
    }

    pub fn status(&self) -> WsConnectionStatus {
        self.shared.lock().status
    }

    /// A copy of the sent/received message log (newest last, at most 1000 entries).
    pub fn messages(&self) -> Vec<WsMessage> {
        self.shared.lock().messages.clone()
    }

    pub fn clear_messages(&self) {
        self.shared.lock().messages.clear();
        self.shared.emit(WsEvent::MessagesCleared);
    }

    /// Validates the URL/headers, then connects in the background (must be called inside a
    /// tokio runtime). A previous connection of this client is dropped. Connection
    /// failures are reported as events (`error`, then `close` with 1006), as `ws` did.
    pub fn connect(&self, opts: WsConnectOptions) -> Result<()> {
        build_request(&opts)?;
        let (tx, rx) = mpsc::unbounded_channel();
        let mut st = self.shared.lock();
        if let Some(old) = st.task.take() {
            old.abort();
        }
        st.options = Some(opts);
        st.cmd = Some(tx);
        let shared = self.shared.clone();
        st.task = Some(tokio::spawn(supervise(shared, rx)));
        Ok(())
    }

    /// Sends a text frame and logs it.
    pub fn send(&self, data: &str) -> Result<WsMessage> {
        let msg = WsMessage {
            id: uuid::Uuid::new_v4().to_string(),
            direction: WsDirection::Sent,
            timestamp: now_ms(),
            kind: WsMessageType::Text,
            data: data.to_string(),
            size: data.len() as u64,
        };
        self.send_frame(Message::text(data), msg)
    }

    /// Sends a binary frame and logs it (payload stored as hex).
    pub fn send_binary(&self, data: Vec<u8>) -> Result<WsMessage> {
        let msg = WsMessage {
            id: uuid::Uuid::new_v4().to_string(),
            direction: WsDirection::Sent,
            timestamp: now_ms(),
            kind: WsMessageType::Binary,
            data: hex::encode(&data),
            size: data.len() as u64,
        };
        self.send_frame(Message::binary(data), msg)
    }

    fn send_frame(&self, frame: Message, msg: WsMessage) -> Result<WsMessage> {
        let not_connected = || Error::Other("WebSocket is not connected".into());
        {
            let st = self.shared.lock();
            if st.status != WsConnectionStatus::Open {
                return Err(not_connected());
            }
            st.cmd
                .as_ref()
                .ok_or_else(not_connected)?
                .send(Cmd::Send(frame))
                .map_err(|_| not_connected())?;
        }
        self.shared.add_message(msg.clone());
        Ok(msg)
    }

    /// Disables auto-reconnect, cancels a pending reconnect and closes the socket
    /// (code defaults to 1000).
    pub fn disconnect(&self, code: Option<u16>, reason: Option<String>) {
        let (live, tx) = {
            let mut st = self.shared.lock();
            if let Some(o) = st.options.as_mut() {
                o.auto_reconnect = Some(false);
            }
            let live = matches!(
                st.status,
                WsConnectionStatus::Open | WsConnectionStatus::Connecting
            );
            (live, st.cmd.clone())
        };
        if live {
            self.shared.set_status(WsConnectionStatus::Closing);
        }
        if let Some(tx) = tx {
            let _ = tx.send(Cmd::Close {
                code: code.unwrap_or(1000),
                reason: reason.unwrap_or_default(),
            });
        }
    }
}

impl Drop for WebSocketClient {
    /// Dropping the client closes the connection gracefully (and ends reconnecting).
    fn drop(&mut self) {
        self.disconnect(None, None);
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn build_request(opts: &WsConnectOptions) -> Result<Request<()>> {
    let mut req = opts
        .url
        .as_str()
        .into_client_request()
        .map_err(|e| Error::Other(format!("Invalid WebSocket URL: {e}")))?;
    match req.uri().scheme_str() {
        Some("ws") | Some("wss") => {}
        _ => return Err(Error::Other(format!("Invalid WebSocket URL: {}", opts.url))),
    }
    for (name, value) in opts.headers.iter().flatten() {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| Error::Other(format!("Invalid header name: {name}")))?;
        let value = HeaderValue::from_str(value)
            .map_err(|_| Error::Other(format!("Invalid value for header {name}")))?;
        req.headers_mut().insert(name, value);
    }
    if let Some(protocols) = opts.protocols.as_ref().filter(|p| !p.is_empty()) {
        let value = HeaderValue::from_str(&protocols.join(", "))
            .map_err(|_| Error::Other("Invalid subprotocol list".to_string()))?;
        req.headers_mut().insert("Sec-WebSocket-Protocol", value);
    }
    Ok(req)
}

/// Accept-anything certificate verifier, used only for `rejectUnauthorized: false`.
#[derive(Debug)]
struct NoVerify(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn insecure_connector() -> Connector {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("aws-lc-rs supports the default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerify(provider)))
        .with_no_client_auth();
    Connector::Rustls(Arc::new(config))
}

type Stream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Close code + reason that ended one connection.
struct Closed {
    code: u16,
    reason: String,
}

impl Closed {
    fn abnormal() -> Self {
        Closed {
            code: 1006,
            reason: String::new(),
        }
    }
}

/// Connect, serve, and (if configured) reconnect until closed for good.
async fn supervise(shared: Arc<Shared>, mut rx: mpsc::UnboundedReceiver<Cmd>) {
    let mut attempts = 0u32;
    loop {
        let Some(opts) = shared.lock().options.clone() else {
            return;
        };
        shared.set_status(WsConnectionStatus::Connecting);
        let mut opened_at = None;
        let closed = connect_and_serve(&shared, &opts, &mut rx, &mut opened_at).await;
        if opened_at.is_some_and(|t: tokio::time::Instant| t.elapsed() >= STABLE_OPEN) {
            attempts = 0;
        }
        shared.set_status(WsConnectionStatus::Closed);
        shared.emit(WsEvent::Close {
            code: closed.code,
            reason: closed.reason,
        });

        // `disconnect` flips `autoReconnect` off in the stored options.
        let Some(cur) = shared.lock().options.clone() else {
            return;
        };
        let max = cur
            .max_reconnect_attempts
            .unwrap_or(DEFAULT_MAX_RECONNECT_ATTEMPTS);
        if !(cur.auto_reconnect == Some(true) && attempts < max) {
            return;
        }
        attempts += 1;
        let sleep = tokio::time::sleep(Duration::from_millis(
            cur.reconnect_delay.unwrap_or(DEFAULT_RECONNECT_DELAY_MS),
        ));
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                _ = &mut sleep => break,
                cmd = rx.recv() => match cmd {
                    // A disconnect while waiting cancels the reconnect.
                    Some(Cmd::Close { .. }) | None => return,
                    Some(Cmd::Send(_)) => {}
                },
            }
        }
    }
}

async fn connect_and_serve(
    shared: &Shared,
    opts: &WsConnectOptions,
    rx: &mut mpsc::UnboundedReceiver<Cmd>,
    opened_at: &mut Option<tokio::time::Instant>,
) -> Closed {
    let fail = |msg: String| {
        shared.set_status(WsConnectionStatus::Error);
        shared.emit(WsEvent::Error(msg));
        Closed::abnormal()
    };
    let req = match build_request(opts) {
        Ok(r) => r,
        Err(e) => return fail(e.to_string()),
    };
    let connecting = async {
        if opts.reject_unauthorized == Some(false) {
            connect_async_tls_with_config(req, None, false, Some(insecure_connector())).await
        } else {
            connect_async(req).await
        }
    };
    tokio::pin!(connecting);
    let mut ws: Stream = loop {
        tokio::select! {
            res = &mut connecting => match res {
                Ok((ws, _)) => break ws,
                Err(e) => return fail(e.to_string()),
            },
            cmd = rx.recv() => match cmd {
                Some(Cmd::Close { .. }) | None => {
                    return fail("WebSocket was closed before the connection was established".into());
                }
                Some(Cmd::Send(_)) => {}
            },
        }
    };

    *opened_at = Some(tokio::time::Instant::now());
    shared.set_status(WsConnectionStatus::Open);

    let mut closing = false;
    let deadline = tokio::time::sleep(Duration::from_secs(60 * 60 * 24));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            cmd = rx.recv(), if !closing => match cmd {
                Some(Cmd::Send(frame)) => {
                    if let Err(e) = ws.send(frame).await {
                        return fail(e.to_string());
                    }
                }
                Some(Cmd::Close { code, reason }) => {
                    let frame = CloseFrame { code: CloseCode::from(code), reason: reason.into() };
                    if ws.send(Message::Close(Some(frame))).await.is_err() {
                        return Closed::abnormal();
                    }
                    closing = true;
                    deadline.as_mut().reset(tokio::time::Instant::now() + CLOSE_TIMEOUT);
                }
                // The client was dropped.
                None => return Closed::abnormal(),
            },
            _ = &mut deadline, if closing => return Closed::abnormal(),
            frame = ws.next() => match frame {
                Some(Ok(Message::Text(text))) => received(shared, truncate_display(text.as_str().to_string(), MAX_DISPLAY_BYTES), text.len(), WsMessageType::Text),
                Some(Ok(Message::Binary(bytes))) => received(shared, truncate_display(hex::encode(&bytes[..bytes.len().min(MAX_DISPLAY_BYTES / 2)]), MAX_DISPLAY_BYTES), bytes.len(), WsMessageType::Binary),
                Some(Ok(Message::Close(frame))) => {
                    // Let tungstenite flush its close reply before the socket is dropped.
                    let _ = tokio::time::timeout(Duration::from_secs(1), ws.close(None)).await;
                    return match frame {
                        Some(f) => Closed { code: u16::from(f.code), reason: f.reason.to_string() },
                        None => Closed { code: 1005, reason: String::new() },
                    };
                }
                // Ping/pong are answered by tungstenite itself.
                Some(Ok(_)) => {}
                Some(Err(WsError::ConnectionClosed | WsError::AlreadyClosed)) | None => return Closed::abnormal(),
                Some(Err(WsError::Protocol(tokio_tungstenite::tungstenite::error::ProtocolError::ResetWithoutClosingHandshake))) => {
                    return Closed::abnormal();
                }
                Some(Err(e)) => return fail(e.to_string()),
            },
        }
    }
}

fn received(shared: &Shared, data: String, size: usize, kind: WsMessageType) {
    let msg = WsMessage {
        id: uuid::Uuid::new_v4().to_string(),
        direction: WsDirection::Received,
        timestamp: now_ms(),
        kind,
        data,
        size: size as u64,
    };
    shared.add_message(msg.clone());
    shared.emit(WsEvent::Message(msg));
}
