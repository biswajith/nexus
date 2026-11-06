//! History list state (replaces `history-store.ts`).

use nexus_core::types::HistoryEntry;

#[derive(Default)]
pub struct HistoryState {
    pub entries: Vec<HistoryEntry>,
    pub loading: bool,
    pub error: Option<String>,
}
