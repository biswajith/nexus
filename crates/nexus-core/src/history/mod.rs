//! Port of `packages/core/src/history/history-manager.ts`: append-only JSON Lines files,
//! one `YYYY-MM-DD.jsonl` per (UTC) calendar day.

use crate::fs::{append_private_file, ensure_private_dir};
use crate::types::{HistoryEntry, HistoryFilter};
use crate::{Error, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use std::path::{Path, PathBuf};

pub struct HistoryManager {
    base_path: PathBuf,
}

/// Start of the UTC day named by a `YYYY-MM-DD.jsonl` file, like `new Date('YYYY-MM-DD')`.
fn file_date(file_name: &str) -> Option<DateTime<Utc>> {
    let stem = file_name.strip_suffix(".jsonl")?;
    let date = NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()?;
    Some(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0)?))
}

/// Parses what `new Date(string)` accepts for the values the UI sends: RFC 3339 or `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Result<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|d| Utc.from_utc_datetime(&d))
        .ok_or_else(|| Error::Other(format!("Invalid date: {s}")))
}

impl HistoryManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    fn ensure_directory(&self) -> Result<()> {
        ensure_private_dir(&self.base_path)?;
        Ok(())
    }

    /// `.jsonl` file names in the base directory, sorted ascending.
    fn jsonl_files(&self) -> Result<Vec<String>> {
        self.ensure_directory()?;
        let mut files: Vec<String> = std::fs::read_dir(&self.base_path)?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|f| f.ends_with(".jsonl"))
            .collect();
        files.sort();
        Ok(files)
    }

    fn read_lines(path: &Path) -> Result<Vec<String>> {
        let content = std::fs::read_to_string(path)?;
        Ok(content
            .trim()
            .split('\n')
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect())
    }

    /// Appends one entry to the file for the entry's calendar day.
    pub fn log(&self, entry: &HistoryEntry) -> Result<()> {
        self.ensure_directory()?;
        let date = DateTime::from_timestamp_millis(entry.timestamp)
            .ok_or_else(|| Error::Other(format!("Invalid timestamp: {}", entry.timestamp)))?
            .format("%Y-%m-%d");
        let file = self.base_path.join(format!("{date}.jsonl"));
        append_private_file(&file, &(serde_json::to_string(entry)? + "\n"))?;
        Ok(())
    }

    /// Reads newest day files first (newest line first within a file), filters, then paginates.
    /// A zero `startDate`/`endDate`/`statusCode` or empty `urlPattern` counts as unset (JS falsy).
    pub fn query(&self, filters: &HistoryFilter) -> Result<Vec<HistoryEntry>> {
        let limit = filters.limit.unwrap_or(100);
        let offset = filters.offset.unwrap_or(0);
        let start = filters.start_date.filter(|&d| d != 0);
        let end = filters.end_date.filter(|&d| d != 0);
        let status = filters.status_code.filter(|&s| s != 0);
        let pattern = filters.url_pattern.as_deref().filter(|p| !p.is_empty());

        let mut entries = Vec::new();
        for file in self.jsonl_files()?.iter().rev() {
            if let Some(ms) = file_date(file).map(|d| d.timestamp_millis()) {
                if start.is_some_and(|s| ms < s) || end.is_some_and(|e| ms > e) {
                    continue;
                }
            }
            for line in Self::read_lines(&self.base_path.join(file))?.iter().rev() {
                let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) else {
                    continue;
                };
                if filters
                    .method
                    .is_some_and(|m| entry.request.method != m.as_str())
                {
                    continue;
                }
                if status.is_some_and(|s| entry.response.status != s) {
                    continue;
                }
                if pattern.is_some_and(|p| !entry.request.url.contains(p)) {
                    continue;
                }
                entries.push(entry);
            }
        }
        Ok(entries.into_iter().skip(offset).take(limit).collect())
    }

    /// Looks up a single entry by id across all files.
    pub fn get_entry(&self, id: &str) -> Result<Option<HistoryEntry>> {
        for file in self.jsonl_files()? {
            for line in Self::read_lines(&self.base_path.join(file))? {
                if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line) {
                    if entry.id == id {
                        return Ok(Some(entry));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Removes `.jsonl` files; with `before`, only files whose day starts strictly before it.
    /// (Files not named like a date are kept when `before` is set.)
    pub fn clear(&self, before: Option<DateTime<Utc>>) -> Result<()> {
        for file in self.jsonl_files()? {
            if let Some(before) = before {
                match file_date(&file) {
                    Some(d) if d < before => {}
                    _ => continue,
                }
            }
            std::fs::remove_file(self.base_path.join(file))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
