//! Tokio runtime owned by the app. Work is spawned with [`Runtime::spawn`]; its result comes
//! back as a [`Msg`] over a channel and wakes the UI with `request_repaint`.

use crate::state::Resp;
use nexus_core::graphql::SubscriptionEvent;
use nexus_core::runner::{DataRow, RunEvent, RunSummary};
use nexus_core::scripts::LogEntry;
use nexus_core::types::HistoryEntry;
use nexus_core::websocket::WsEvent;
use serde_json::Value;
use std::future::Future;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Where a picked file goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickTarget {
    /// Value of the n-th form-data row.
    FormRow(usize),
    /// The binary body.
    Binary,
}

/// Results of background work, drained once per frame by the app.
#[allow(clippy::large_enum_variant)]
pub enum Msg {
    /// A request finished (tab id = request id).
    Http {
        tab_id: String,
        result: Result<Box<(Resp, Vec<LogEntry>)>, String>,
    },
    /// The save dialog closed (`None` = cancelled).
    SavePicked {
        tab_id: String,
        path: Option<std::path::PathBuf>,
    },
    /// A file dialog closed (`None` = cancelled).
    FilePicked {
        tab_id: String,
        target: PickTarget,
        path: Option<std::path::PathBuf>,
    },
    /// Fresh history entries (newest first).
    History(Result<Vec<HistoryEntry>, String>),
    /// A runner progress event of run `run`.
    Runner {
        run: u64,
        event: RunEvent,
    },
    /// Run `run` ended.
    RunnerDone {
        run: u64,
        summary: Box<RunSummary>,
    },
    /// A runner data file was read and parsed.
    DataFile {
        path: std::path::PathBuf,
        result: Result<Vec<DataRow>, String>,
    },
    /// The data-file dialog closed (`None` = cancelled).
    DataPicked(Option<std::path::PathBuf>),
    /// An event of WebSocket connection `conn`.
    Ws {
        conn: String,
        event: WsEvent,
    },
    Gql(GqlMsg),
    Mcp(McpMsg),
    /// The import file dialog closed (`None` = cancelled).
    ImportPicked(Option<std::path::PathBuf>),
    /// An import text was parsed (`source` = file name or "Pasted text").
    Imported {
        source: String,
        result: Result<Box<crate::state::dialogs::Parsed>, String>,
    },
    /// The export save dialog closed.
    ExportPicked {
        what: crate::state::dialogs::ExportWhat,
        fmt: crate::state::dialogs::ExportFmt,
        path: Option<std::path::PathBuf>,
    },
    /// The docs save dialog closed.
    DocsPicked(Option<std::path::PathBuf>),
}

/// GraphQL panel results.
#[allow(clippy::large_enum_variant)]
pub enum GqlMsg {
    Response(Result<nexus_core::graphql::GraphQLResponse, String>),
    Schema {
        url: String,
        result: Result<String, String>,
    },
    Sub(SubscriptionEvent),
}

/// What an MCP server offered right after connecting.
pub struct McpSnapshot {
    pub info: Value,
    pub tools: Vec<Value>,
    pub resources: Vec<Value>,
    pub prompts: Vec<Value>,
}

/// MCP panel results; `sid` is the session the work belongs to.
pub enum McpMsg {
    Connected {
        sid: String,
        result: Result<Box<McpSnapshot>, String>,
    },
    Called {
        sid: String,
        tool: String,
        args: Value,
        ms: f64,
        result: Result<Value, String>,
    },
}

/// A cloneable handle for threads that are not async tasks (event sinks): sends a message and
/// wakes the UI.
#[derive(Clone)]
pub struct Notifier {
    tx: Sender<Msg>,
    ctx: Option<eframe::egui::Context>,
}

impl Notifier {
    pub fn send(&self, msg: Msg) {
        let _ = self.tx.send(msg);
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
    }
}

pub struct Runtime {
    rt: tokio::runtime::Runtime,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: Option<eframe::egui::Context>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            rt: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio runtime"),
            tx,
            rx,
            ctx: None,
        }
    }

    /// Repaints are requested on this context when a task finishes.
    pub fn set_ctx(&mut self, ctx: &eframe::egui::Context) {
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }
    }

    pub fn notifier(&self) -> Notifier {
        Notifier {
            tx: self.tx.clone(),
            ctx: self.ctx.clone(),
        }
    }

    pub fn handle(&self) -> tokio::runtime::Handle {
        self.rt.handle().clone()
    }

    /// Runs `fut` on the runtime; its output is delivered through [`Runtime::drain`].
    pub fn spawn(
        &self,
        fut: impl Future<Output = Msg> + Send + 'static,
    ) -> tokio::task::AbortHandle {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        self.rt
            .spawn(async move {
                let msg = fut.await;
                let _ = tx.send(msg);
                if let Some(ctx) = ctx {
                    ctx.request_repaint();
                }
            })
            .abort_handle()
    }

    /// Runs a blocking closure (file I/O) off the UI thread.
    pub fn spawn_blocking(&self, f: impl FnOnce() -> Msg + Send + 'static) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        self.rt.spawn_blocking(move || {
            let _ = tx.send(f());
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    pub fn drain(&self) -> Vec<Msg> {
        self.rx.try_iter().collect()
    }
}
