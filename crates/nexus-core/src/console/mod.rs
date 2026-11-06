//! Port of `packages/core/src/console/nexus-console.ts`: in-memory log buffer with filtering and
//! subscription hooks.

use std::collections::VecDeque;

use serde_json::Value;

use crate::types::{ConsoleEntry, ConsoleFilter, LogLevel, LogSource};

const MAX_ENTRIES: usize = 10_000;

type EntryHandler = Box<dyn Fn(&ConsoleEntry) + Send + Sync>;
type ClearHandler = Box<dyn Fn() + Send + Sync>;

/// Identifies a registered handler so it can be removed again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandlerId(u64);

#[derive(Default)]
pub struct NexusConsole {
    entries: VecDeque<ConsoleEntry>,
    entry_handlers: Vec<(HandlerId, EntryHandler)>,
    clear_handlers: Vec<(HandlerId, ClearHandler)>,
    next_id: u64,
}

impl NexusConsole {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an entry, evicts the oldest beyond 10 000, and notifies entry subscribers.
    pub fn log(
        &mut self,
        level: LogLevel,
        source: LogSource,
        message: impl Into<String>,
        data: Option<Value>,
        request_id: Option<&str>,
    ) {
        let entry = ConsoleEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_ms(),
            level,
            source,
            request_id: request_id.map(str::to_string),
            message: message.into(),
            data,
        };
        self.entries.push_back(entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.pop_front();
        }
        let appended = self.entries.back().expect("just pushed");
        for (_, handler) in &self.entry_handlers {
            handler(appended);
        }
    }

    pub fn info(
        &mut self,
        source: LogSource,
        message: impl Into<String>,
        data: Option<Value>,
        request_id: Option<&str>,
    ) {
        self.log(LogLevel::Info, source, message, data, request_id);
    }

    pub fn warn(
        &mut self,
        source: LogSource,
        message: impl Into<String>,
        data: Option<Value>,
        request_id: Option<&str>,
    ) {
        self.log(LogLevel::Warn, source, message, data, request_id);
    }

    pub fn error(
        &mut self,
        source: LogSource,
        message: impl Into<String>,
        data: Option<Value>,
        request_id: Option<&str>,
    ) {
        self.log(LogLevel::Error, source, message, data, request_id);
    }

    pub fn debug(
        &mut self,
        source: LogSource,
        message: impl Into<String>,
        data: Option<Value>,
        request_id: Option<&str>,
    ) {
        self.log(LogLevel::Debug, source, message, data, request_id);
    }

    /// Entries matching the optional level, source, request id and case-insensitive message filters.
    /// Like the TS code, empty-string `request_id` / `search` filters are ignored.
    pub fn query(&self, filters: &ConsoleFilter) -> Vec<ConsoleEntry> {
        let search = filters
            .search
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase);
        let request_id = filters.request_id.as_deref().filter(|s| !s.is_empty());
        self.entries
            .iter()
            .filter(|e| {
                filters.level.is_none_or(|l| e.level == l)
                    && filters.source.is_none_or(|s| e.source == s)
                    && request_id.is_none_or(|r| e.request_id.as_deref() == Some(r))
                    && search
                        .as_deref()
                        .is_none_or(|s| e.message.to_lowercase().contains(s))
            })
            .cloned()
            .collect()
    }

    /// Snapshot of every stored entry in insertion order.
    pub fn get_all(&self) -> Vec<ConsoleEntry> {
        self.entries.iter().cloned().collect()
    }

    /// Clears the buffer and notifies clear subscribers.
    pub fn clear(&mut self) {
        self.entries.clear();
        for (_, handler) in &self.clear_handlers {
            handler();
        }
    }

    /// Registers a listener for each new entry. Remove it with [`NexusConsole::remove_handler`].
    pub fn on_entry(
        &mut self,
        handler: impl Fn(&ConsoleEntry) + Send + Sync + 'static,
    ) -> HandlerId {
        let id = self.alloc_id();
        self.entry_handlers.push((id, Box::new(handler)));
        id
    }

    /// Registers a listener invoked whenever the buffer is cleared.
    pub fn on_clear(&mut self, handler: impl Fn() + Send + Sync + 'static) -> HandlerId {
        let id = self.alloc_id();
        self.clear_handlers.push((id, Box::new(handler)));
        id
    }

    /// Unsubscribes a handler returned by `on_entry` / `on_clear`.
    pub fn remove_handler(&mut self, id: HandlerId) {
        self.entry_handlers.retain(|(i, _)| *i != id);
        self.clear_handlers.retain(|(i, _)| *i != id);
    }

    fn alloc_id(&mut self) -> HandlerId {
        self.next_id += 1;
        HandlerId(self.next_id)
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests;
