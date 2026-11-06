//! Collection runner state (replaces `runner-store.ts` and the panel's local state).

use nexus_core::runner::{DataRow, RequestRunResult, RunSummary};
use nexus_core::types::{CollectionItem, NexusCollection};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RunStatus {
    #[default]
    Idle,
    Running,
    Completed,
    Cancelled,
    Error,
}

/// A CSV / JSON data file whose rows drive the iterations.
pub struct DataFile {
    pub name: String,
    pub path: PathBuf,
    pub rows: Vec<DataRow>,
}

/// One finished request of the run, tagged with its iteration (0-based).
pub struct RunRow {
    pub iteration: u32,
    pub result: RequestRunResult,
}

pub struct RunnerState {
    /// Directory name of the picked collection.
    pub collection: Option<String>,
    /// Folder id; `None` runs the whole collection.
    pub folder: Option<String>,
    pub iterations: String,
    pub delay: String,
    pub stop_on_error: bool,
    pub data: Option<DataFile>,
    pub data_error: Option<String>,

    pub status: RunStatus,
    /// Ignores events of an earlier run.
    pub run_id: u64,
    pub iteration: u32,
    pub total_iterations: u32,
    /// Requests per iteration (progress denominator).
    pub per_iteration: usize,
    pub current: Option<String>,
    pub results: Vec<RunRow>,
    pub summary: Option<RunSummary>,
    pub error: Option<String>,
    pub selected: Option<usize>,
    pub started: Option<Instant>,
    /// Cancel was requested; the run is still winding down.
    pub cancelling: bool,
}

impl Default for RunnerState {
    fn default() -> Self {
        Self {
            collection: None,
            folder: None,
            iterations: "1".into(),
            delay: "0".into(),
            stop_on_error: false,
            data: None,
            data_error: None,
            status: RunStatus::Idle,
            run_id: 0,
            iteration: 0,
            total_iterations: 0,
            per_iteration: 0,
            current: None,
            results: Vec::new(),
            summary: None,
            error: None,
            selected: None,
            started: None,
            cancelling: false,
        }
    }
}

impl RunnerState {
    pub fn running(&self) -> bool {
        self.status == RunStatus::Running
    }

    pub fn reset(&mut self) {
        let keep = self.run_id;
        let cfg = (
            self.collection.take(),
            self.folder.take(),
            std::mem::take(&mut self.iterations),
            std::mem::take(&mut self.delay),
            self.stop_on_error,
            self.data.take(),
        );
        *self = Self::default();
        self.run_id = keep;
        (
            self.collection,
            self.folder,
            self.iterations,
            self.delay,
            self.stop_on_error,
            self.data,
        ) = cfg;
    }

    /// Fraction of the expected requests that are done.
    pub fn progress(&self) -> f32 {
        let total = self.per_iteration * self.total_iterations as usize;
        if total == 0 {
            return 0.0;
        }
        (self.results.len() as f32 / total as f32).clamp(0.0, 1.0)
    }

    pub fn iterations_value(&self) -> u32 {
        self.iterations
            .trim()
            .parse::<u32>()
            .unwrap_or(1)
            .clamp(1, 1000)
    }

    pub fn delay_value(&self) -> u64 {
        self.delay.trim().parse::<u64>().unwrap_or(0).min(60_000)
    }
}

/// A folder the run can be limited to: label ("Users / Admin"), id and the requests inside it.
pub struct FolderChoice {
    pub label: String,
    pub id: String,
    pub request_ids: Vec<String>,
}

/// Every folder of a collection (depth first), with the ids of the requests below it.
pub fn folders_of(c: &NexusCollection) -> Vec<FolderChoice> {
    fn walk(items: &[CollectionItem], prefix: &str, out: &mut Vec<FolderChoice>) {
        for i in items {
            if let CollectionItem::Folder(f) = i {
                let label = if prefix.is_empty() {
                    f.name.clone()
                } else {
                    format!("{prefix} / {}", f.name)
                };
                out.push(FolderChoice {
                    label: label.clone(),
                    id: f.id.clone(),
                    request_ids: request_ids(&f.items),
                });
                walk(&f.items, &label, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&c.items, "", &mut out);
    out
}

pub fn request_ids(items: &[CollectionItem]) -> Vec<String> {
    let mut out = Vec::new();
    for i in items {
        match i {
            CollectionItem::Request(r) => out.push(r.id.clone()),
            CollectionItem::Folder(f) => out.extend(request_ids(&f.items)),
        }
    }
    out
}

/// Column names of a data file (keys of its first row).
pub fn columns(rows: &[DataRow]) -> Vec<String> {
    rows.first()
        .map(|r| r.keys().cloned().collect())
        .unwrap_or_default()
}
