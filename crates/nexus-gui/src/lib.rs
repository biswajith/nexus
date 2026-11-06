//! Nexus native UI (egui). The binary lives in `main.rs`; everything testable is here.

pub mod app;
/// Demo data for screenshots and tests (debug builds only).
#[cfg(debug_assertions)]
pub mod demo;
#[cfg(debug_assertions)]
mod dev;
pub(crate) mod dialog_ops;
pub mod editor;
pub mod highlight;
pub mod panels;
pub mod payload;
pub mod preview;
pub mod runtime;
mod shortcuts;
pub mod state;
pub mod theme;
pub mod url_sync;
pub mod widgets;

pub use app::{NexusApp, Opts};
