//! The console buffer on top of nexus-core's
//! `NexusConsole`, with a cached filtered view for the panel.

use nexus_core::console::NexusConsole;
use nexus_core::scripts::LogEntry;
use nexus_core::types::{ConsoleEntry, ConsoleFilter, LogLevel, LogSource};
use serde_json::Value;

pub struct ConsoleState {
    console: NexusConsole,
    /// Bumped by every change of the buffer.
    version: u64,
    count: usize,
    pub level: Option<LogLevel>,
    pub search: String,
    pub auto_scroll: bool,
    filtered: Vec<ConsoleEntry>,
    key: Option<(u64, Option<LogLevel>, String)>,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self {
            console: NexusConsole::new(),
            version: 0,
            count: 0,
            level: None,
            search: String::new(),
            auto_scroll: true,
            filtered: Vec::new(),
            key: None,
        }
    }
}

impl ConsoleState {
    pub fn log(
        &mut self,
        level: LogLevel,
        source: LogSource,
        message: impl Into<String>,
        request_id: Option<&str>,
    ) {
        self.console.log(level, source, message, None, request_id);
        self.version += 1;
        self.count = (self.count + 1).min(10_000);
    }

    /// Adds the `nx.log` / `console.*` output of a script run.
    pub fn add_script_logs(&mut self, logs: &[LogEntry], request_id: Option<&str>) {
        for l in logs {
            let level = match l.level.as_str() {
                "warn" => LogLevel::Warn,
                "error" => LogLevel::Error,
                "debug" => LogLevel::Debug,
                _ => LogLevel::Info,
            };
            let msg = l
                .args
                .iter()
                .map(|a| match a {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            self.log(level, LogSource::Script, msg, request_id);
        }
    }

    pub fn clear(&mut self) {
        self.console.clear();
        self.version += 1;
        self.count = 0;
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Entries matching the level chip and the text filter.
    pub fn visible(&mut self) -> &[ConsoleEntry] {
        let key = (self.version, self.level, self.search.clone());
        if self.key.as_ref() != Some(&key) {
            self.filtered = self.console.query(&ConsoleFilter {
                level: self.level,
                search: Some(self.search.clone()),
                ..Default::default()
            });
            self.key = Some(key);
        }
        &self.filtered
    }
}

pub fn level_label(l: LogLevel) -> &'static str {
    match l {
        LogLevel::Info => "INFO",
        LogLevel::Warn => "WARN",
        LogLevel::Error => "ERR",
        LogLevel::Debug => "DBG",
    }
}

pub fn source_label(s: LogSource) -> &'static str {
    match s {
        LogSource::Script => "Script",
        LogSource::Http => "HTTP",
        LogSource::Variable => "Var",
        LogSource::System => "Sys",
    }
}
