//! WebSocket sessions . Each session is one connection
//! with its own URL, headers, composer and message log; they stay alive while another panel is
//! shown.

use crate::widgets::TextView;
use nexus_core::types::KeyValuePair;
use nexus_core::websocket::{WsConnectionStatus, WsDirection, WsEvent, WsMessage};

/// The message log keeps this many rows (core keeps 1000 messages).
pub const MAX_ROWS: usize = 2000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DirFilter {
    #[default]
    All,
    Sent,
    Received,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WsTab {
    #[default]
    Messages,
    Headers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoteKind {
    Info,
    Warn,
    Error,
}

/// A log row: a frame, or a connection event (opened, closed, error, reconnecting).
#[derive(Clone, Debug)]
pub enum WsRow {
    Message(WsMessage),
    Note {
        at: i64,
        kind: NoteKind,
        text: String,
    },
}

pub struct WsSession {
    /// Session id (stable, names the tab).
    pub id: String,
    /// Id of the live connection; every `connect` creates a new one so late events of an older
    /// connection are ignored.
    pub conn: Option<String>,
    pub name: String,
    pub url: String,
    pub protocols: String,
    pub headers: Vec<KeyValuePair>,
    pub auto_reconnect: bool,
    pub tab: WsTab,
    pub status: WsConnectionStatus,
    /// Set after the first close of an auto-reconnecting connection: `Connecting` then means
    /// "reconnecting".
    pub reconnecting: bool,
    pub error: Option<String>,
    pub rows: Vec<WsRow>,
    pub draft: String,
    pub filter: String,
    pub dir_filter: DirFilter,
    pub auto_scroll: bool,
    /// Id of the message shown in the detail pane.
    pub selected: Option<String>,
    pub detail: TextView,
    pub detail_for: Option<String>,
}

impl WsSession {
    pub fn new(n: usize) -> Self {
        Self {
            id: format!("ws_{}", uuid::Uuid::new_v4()),
            conn: None,
            name: format!("Connection {n}"),
            url: "ws://localhost:8080".into(),
            protocols: String::new(),
            headers: Vec::new(),
            auto_reconnect: false,
            tab: WsTab::Messages,
            status: WsConnectionStatus::Idle,
            reconnecting: false,
            error: None,
            rows: Vec::new(),
            draft: String::new(),
            filter: String::new(),
            dir_filter: DirFilter::All,
            auto_scroll: true,
            selected: None,
            detail: TextView::default(),
            detail_for: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.status == WsConnectionStatus::Open
    }

    /// Connecting, open or closing: the URL cannot be edited and Connect is not offered.
    pub fn is_live(&self) -> bool {
        matches!(
            self.status,
            WsConnectionStatus::Connecting | WsConnectionStatus::Open | WsConnectionStatus::Closing
        )
    }

    pub fn push(&mut self, row: WsRow) {
        self.rows.push(row);
        if self.rows.len() > MAX_ROWS {
            let excess = self.rows.len() - MAX_ROWS;
            self.rows.drain(..excess);
        }
    }

    pub fn note(&mut self, kind: NoteKind, text: impl Into<String>) {
        self.push(WsRow::Note {
            at: chrono::Utc::now().timestamp_millis(),
            kind,
            text: text.into(),
        });
    }

    /// Applies an event of this session's current connection.
    pub fn apply(&mut self, event: WsEvent) {
        match event {
            WsEvent::Status(s) => {
                let prev = self.status;
                self.status = s;
                match s {
                    WsConnectionStatus::Open => {
                        self.error = None;
                        self.reconnecting = false;
                        self.note(NoteKind::Info, "Connected");
                    }
                    WsConnectionStatus::Connecting if prev == WsConnectionStatus::Closed => {
                        self.reconnecting = true;
                        self.note(NoteKind::Warn, "Reconnecting...");
                    }
                    _ => {}
                }
            }
            WsEvent::Message(m) => self.push(WsRow::Message(m)),
            WsEvent::Close { code, reason } => {
                let why = if reason.is_empty() {
                    String::new()
                } else {
                    format!(" ({reason})")
                };
                self.note(NoteKind::Warn, format!("Closed with code {code}{why}"));
            }
            WsEvent::Error(e) => {
                self.error = Some(e.clone());
                self.note(NoteKind::Error, format!("Error: {e}"));
            }
            WsEvent::MessagesCleared => self.rows.clear(),
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.selected = None;
        self.detail_for = None;
    }

    pub fn sent_count(&self) -> usize {
        self.count(WsDirection::Sent)
    }

    pub fn received_count(&self) -> usize {
        self.count(WsDirection::Received)
    }

    fn count(&self, d: WsDirection) -> usize {
        self.rows
            .iter()
            .filter(|r| matches!(r, WsRow::Message(m) if m.direction == d))
            .count()
    }

    /// Rows that pass the direction and text filters (connection notes are kept unless a text
    /// filter is active).
    pub fn visible(&self) -> Vec<usize> {
        let f = self.filter.trim().to_lowercase();
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| match r {
                WsRow::Note { .. } => f.is_empty() && self.dir_filter == DirFilter::All,
                WsRow::Message(m) => {
                    let dir_ok = match self.dir_filter {
                        DirFilter::All => true,
                        DirFilter::Sent => m.direction == WsDirection::Sent,
                        DirFilter::Received => m.direction == WsDirection::Received,
                    };
                    dir_ok && (f.is_empty() || m.data.to_lowercase().contains(&f))
                }
            })
            .map(|(i, _)| i)
            .collect()
    }
}

pub struct WsState {
    pub sessions: Vec<WsSession>,
    pub active: usize,
    counter: usize,
}

impl Default for WsState {
    fn default() -> Self {
        Self {
            sessions: vec![WsSession::new(1)],
            active: 0,
            counter: 1,
        }
    }
}

impl WsState {
    pub fn add(&mut self) {
        self.counter += 1;
        self.sessions.push(WsSession::new(self.counter));
        self.active = self.sessions.len() - 1;
    }

    pub fn set_counter(&mut self, n: usize) {
        self.counter = n;
    }

    pub fn by_conn(&mut self, conn: &str) -> Option<&mut WsSession> {
        self.sessions
            .iter_mut()
            .find(|s| s.conn.as_deref() == Some(conn))
    }

    pub fn active_mut(&mut self) -> &mut WsSession {
        let i = self.active.min(self.sessions.len() - 1);
        &mut self.sessions[i]
    }
}
