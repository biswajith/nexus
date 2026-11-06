//! Collection runner: pick a collection (and
//! folder), environment, iterations, delay, stop-on-error and an optional CSV / JSON data file,
//! then watch the run live.

use crate::app::NexusApp;
use crate::runtime::Msg;
use crate::state::runner::{columns, folders_of, request_ids, DataFile, RunRow, RunStatus};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align, Color32, FontId, Layout, Margin, Rect, RichText,
    ScrollArea, Ui,
};
use nexus_core::runner::{RunConfig, RunEvent};
use nexus_core::types::NexusCollection;
use std::path::Path;

/// Data files larger than this are refused.
const MAX_DATA_BYTES: u64 = 20 * 1024 * 1024;
const ROW_H: f32 = 32.0;

fn mono(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Monospace)
}

enum Action {
    Start,
    Cancel,
    Reset,
    PickData,
    ClearData,
    Collection(usize),
    Folder(Option<usize>),
    Environment(Option<String>),
}

impl NexusApp {
    // ---- actions ----

    /// Starts a run of the picked collection with the current settings.
    pub fn runner_start(&mut self) {
        if self.runner.running() {
            return;
        }
        let Some(dir) = self.runner.collection.clone() else {
            return;
        };
        if !self.collections.loaded.contains_key(&dir) {
            self.collections.load(&self.app, &dir);
        }
        let Some(collection) = self.collections.loaded.get(&dir).cloned() else {
            self.runner.status = RunStatus::Error;
            self.runner.error = Some("Could not load the collection".into());
            return;
        };
        let ids = match &self.runner.folder {
            Some(f) => folders_of(&collection)
                .into_iter()
                .find(|x| &x.id == f)
                .map(|x| x.request_ids)
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let per_iteration = if ids.is_empty() {
            request_ids(&collection.items).len()
        } else {
            ids.len()
        };
        let iterations = self.runner.iterations_value();
        let config = RunConfig {
            environment_vars: self.envs.variables(self.ui.active_env_id.as_deref()),
            global_vars: Vec::new(),
            request_ids: ids,
            iterations,
            delay_ms: self.runner.delay_value(),
            data_rows: self.runner.data.as_ref().map(|d| d.rows.clone()),
            stop_on_error: self.runner.stop_on_error,
            persist_variables: false,
            collection,
        };
        let r = &mut self.runner;
        r.run_id += 1;
        r.status = RunStatus::Running;
        r.cancelling = false;
        r.iteration = 0;
        r.total_iterations = iterations;
        r.per_iteration = per_iteration;
        r.current = None;
        r.results.clear();
        r.summary = None;
        r.error = None;
        r.selected = None;
        r.started = Some(std::time::Instant::now());
        let run = r.run_id;
        let app = self.app.clone();
        let notifier = self.rt.notifier();
        self.rt.spawn(async move {
            let n2 = notifier.clone();
            let summary = app
                .runner_start(config, move |event| n2.send(Msg::Runner { run, event }))
                .await;
            Msg::RunnerDone {
                run,
                summary: Box::new(summary),
            }
        });
    }

    pub fn runner_cancel(&mut self) {
        if self.runner.running() && !self.runner.cancelling {
            self.runner.cancelling = true;
            self.app.runner_cancel();
        }
    }

    /// Asks for a CSV / JSON data file (native dialog).
    pub fn runner_pick_data(&mut self) {
        self.rt.spawn(async move {
            let path = rfd::AsyncFileDialog::new()
                .add_filter("Data files", &["csv", "json"])
                .pick_file()
                .await
                .map(|f| f.path().to_path_buf());
            Msg::DataPicked(path)
        });
    }

    /// Reads and parses a data file off the UI thread; the rows arrive as `Msg::DataFile`.
    pub fn runner_load_data(&mut self, path: &Path) {
        let path = path.to_path_buf();
        self.rt.spawn_blocking(move || {
            let result = (|| {
                let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                if meta.len() > MAX_DATA_BYTES {
                    return Err("Data file is larger than 20 MB".to_string());
                }
                let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let rows =
                    nexus_core::runner::parse_data_file(&name, &text).map_err(|e| e.to_string())?;
                if rows.is_empty() {
                    return Err("The data file has no rows".to_string());
                }
                Ok(rows)
            })();
            Msg::DataFile { path, result }
        });
    }

    pub(crate) fn on_runner_event(&mut self, run: u64, event: RunEvent) {
        let r = &mut self.runner;
        if run != r.run_id {
            return;
        }
        match event {
            RunEvent::IterationStart { iteration, total } => {
                r.iteration = iteration + 1;
                r.total_iterations = total;
            }
            RunEvent::RequestStart { name, .. } => r.current = Some(name),
            RunEvent::RequestComplete(result) => r.results.push(RunRow {
                iteration: r.iteration,
                result,
            }),
            RunEvent::Error { message, .. } => r.error = Some(message),
            _ => {}
        }
    }

    pub(crate) fn on_runner_done(&mut self, run: u64, summary: nexus_core::runner::RunSummary) {
        let r = &mut self.runner;
        if run != r.run_id {
            return;
        }
        r.status = if summary.aborted {
            RunStatus::Cancelled
        } else {
            RunStatus::Completed
        };
        r.current = None;
        r.cancelling = false;
        r.summary = Some(summary);
    }

    pub(crate) fn on_data_file(
        &mut self,
        path: std::path::PathBuf,
        result: Result<Vec<nexus_core::runner::DataRow>, String>,
    ) {
        match result {
            Ok(rows) => {
                self.runner.iterations = rows.len().clamp(1, 1000).to_string();
                self.runner.data_error = None;
                self.runner.data = Some(DataFile {
                    name: path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    path,
                    rows,
                });
            }
            Err(e) => self.runner.data_error = Some(e),
        }
    }

    // ---- drawing ----

    pub(crate) fn runner_panel(&mut self, ui: &mut Ui) {
        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0, bg_app());
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut action: Option<Action> = None;
        self.runner_config(ui, &mut action);
        self.runner_progress(ui);
        self.runner_results(ui);
        match action {
            Some(Action::Start) => self.runner_start(),
            Some(Action::Cancel) => self.runner_cancel(),
            Some(Action::Reset) => self.runner.reset(),
            Some(Action::PickData) => self.runner_pick_data(),
            Some(Action::ClearData) => {
                self.runner.data = None;
                self.runner.data_error = None;
            }
            Some(Action::Collection(i)) => {
                if let Some(m) = self.collections.metas.get(i).cloned() {
                    let dir = crate::state::collections::dir_name(&m);
                    self.collections.load(&self.app, &dir);
                    self.runner.collection = Some(dir);
                    self.runner.folder = None;
                }
            }
            Some(Action::Folder(i)) => {
                self.runner.folder = i.and_then(|i| {
                    let c = self.runner_collection()?;
                    folders_of(c).into_iter().nth(i).map(|f| f.id)
                });
            }
            Some(Action::Environment(id)) => self.set_env(id),
            None => {}
        }
    }

    fn runner_collection(&self) -> Option<&NexusCollection> {
        self.collections
            .loaded
            .get(self.runner.collection.as_deref()?)
    }

    fn runner_config(&mut self, ui: &mut Ui, action: &mut Option<Action>) {
        let collection_names: Vec<String> = self
            .collections
            .metas
            .iter()
            .map(|m| m.name.clone())
            .collect();
        let current_collection = self
            .runner
            .collection
            .as_ref()
            .and_then(|d| {
                self.collections
                    .metas
                    .iter()
                    .find(|m| &crate::state::collections::dir_name(m) == d)
            })
            .map_or("Select collection...".to_string(), |m| m.name.clone());
        let mut folder_names = vec!["All requests".to_string()];
        let mut current_folder = "All requests".to_string();
        if let Some(c) = self.runner_collection() {
            for f in folders_of(c) {
                if self.runner.folder.as_deref() == Some(f.id.as_str()) {
                    current_folder = f.label.clone();
                }
                folder_names.push(f.label);
            }
        }
        let env_names: Vec<String> = std::iter::once("No Environment".to_string())
            .chain(self.envs.environments.iter().map(|e| e.name.clone()))
            .collect();
        let env_ids: Vec<Option<String>> = std::iter::once(None)
            .chain(self.envs.environments.iter().map(|e| Some(e.id.clone())))
            .collect();
        let current_env = self.envs.name(self.ui.active_env_id.as_deref()).to_string();
        let running = self.runner.running();
        let status = self.runner.status;
        let can_run = self.runner.collection.is_some() && !running;
        let cancelling = self.runner.cancelling;
        let data_info = self.runner.data.as_ref().map(|d| {
            (
                d.name.clone(),
                d.rows.len(),
                columns(&d.rows).join(", "),
                d.path.display().to_string(),
            )
        });

        egui::Frame::NONE
            .fill(bg_surface())
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().interact_size.y = 30.0;
                ui.add_enabled_ui(!running, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        field_label(ui, "Collection");
                        if let Some(i) = combo_pick(
                            ui,
                            "runner-collection",
                            "Collection",
                            230.0,
                            &current_collection,
                            &collection_names,
                        ) {
                            *action = Some(Action::Collection(i));
                        }
                        ui.add_space(8.0);
                        field_label(ui, "Folder");
                        if let Some(i) = combo_pick(
                            ui,
                            "runner-folder",
                            "Folder",
                            220.0,
                            &current_folder,
                            &folder_names,
                        ) {
                            *action = Some(Action::Folder(i.checked_sub(1)));
                        }
                        ui.add_space(8.0);
                        field_label(ui, "Environment");
                        if let Some(i) = combo_pick(
                            ui,
                            "runner-env",
                            "Run environment",
                            200.0,
                            &current_env,
                            &env_names,
                        ) {
                            *action = Some(Action::Environment(env_ids[i].clone()));
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        field_label(ui, "Iterations");
                        field(
                            ui,
                            "Iterations",
                            &mut self.runner.iterations,
                            64.0,
                            30.0,
                            mono(12.5),
                            "1",
                            bg_input(),
                            true,
                        );
                        ui.add_space(8.0);
                        field_label(ui, "Delay (ms)");
                        field(
                            ui,
                            "Delay (ms)",
                            &mut self.runner.delay,
                            76.0,
                            30.0,
                            mono(12.5),
                            "0",
                            bg_input(),
                            true,
                        );
                        ui.add_space(8.0);
                        checkbox(ui, "Stop on error", &mut self.runner.stop_on_error);
                        field_label(ui, "Stop on error");
                        ui.add_space(16.0);
                        field_label(ui, "Data file");
                        match &data_info {
                            None => {
                                if button(
                                    ui,
                                    "Choose data file...",
                                    font(12.0, "Medium"),
                                    (12.0, 5.0),
                                    30.0,
                                    &BtnStyle::outline(),
                                )
                                .on_hover_text("A CSV (header row) or JSON array of objects; one row per iteration")
                                .clicked()
                                {
                                    *action = Some(Action::PickData);
                                }
                            }
                            Some((name, rows, cols, path)) => {
                                let plural = if *rows == 1 { "row" } else { "rows" };
                                let text = format!("{name} - {rows} {plural}");
                                labeled_text(
                                    ui,
                                    &text,
                                    &format!("Data file: {text}"),
                                    font(12.0, "Medium"),
                                    fg(),
                                )
                                .on_hover_text(format!("{path}\nColumns: {cols}"));
                                if button(
                                    ui,
                                    "Remove data file",
                                    font(12.0, "Medium"),
                                    (10.0, 5.0),
                                    30.0,
                                    &BtnStyle::outline(),
                                )
                                .clicked()
                                {
                                    *action = Some(Action::ClearData);
                                }
                            }
                        }
                    });
                });
                if let Some(e) = &self.runner.data_error {
                    ui.add_space(6.0);
                    labeled_text(
                        ui,
                        e,
                        &format!("Data file error: {e}"),
                        font(12.0, "Regular"),
                        danger(),
                    );
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if running {
                        let label = if cancelling { "Cancelling..." } else { "Cancel" };
                        let b = button(
                            ui,
                            label,
                            font(13.0, "SemiBold"),
                            (18.0, 6.0),
                            32.0,
                            &BtnStyle::primary(danger(), danger().gamma_multiply(0.85)),
                        );
                        if b.clicked() && !cancelling {
                            *action = Some(Action::Cancel);
                        }
                    } else {
                        ui.add_enabled_ui(can_run, |ui| {
                            let st = if can_run {
                                BtnStyle::primary(accent(), accent_hover())
                            } else {
                                BtnStyle::primary(border(), border())
                            };
                            if button(
                                ui,
                                "Run Collection",
                                font(13.0, "SemiBold"),
                                (18.0, 6.0),
                                32.0,
                                &st,
                            )
                            .clicked()
                                && can_run
                            {
                                *action = Some(Action::Start);
                            }
                        });
                        if status != RunStatus::Idle
                            && button(
                                ui,
                                "Reset",
                                font(12.0, "Medium"),
                                (14.0, 6.0),
                                32.0,
                                &BtnStyle::outline(),
                            )
                            .clicked()
                        {
                            *action = Some(Action::Reset);
                        }
                    }
                    if !running && self.runner.collection.is_none() {
                        field_label(ui, "Pick a collection to run");
                    }
                });
            });
        let bottom = ui.cursor().top();
        hline(
            ui,
            bottom - 1.0,
            ui.max_rect().left(),
            ui.max_rect().right(),
            border(),
        );
    }

    fn runner_progress(&mut self, ui: &mut Ui) {
        let r = &self.runner;
        let status_text = match r.status {
            RunStatus::Running => None,
            RunStatus::Completed => Some(("Completed", success(), success_subtle())),
            RunStatus::Cancelled => Some(("Cancelled", warning(), warning_subtle())),
            RunStatus::Error => Some(("Error", danger(), danger_subtle())),
            RunStatus::Idle => return,
        };
        let h = 40.0;
        let rect = bar(ui, h, bg_app(), Some(border_subtle()));
        let frac = r.progress();
        let iteration = format!("Iteration {}/{}", r.iteration, r.total_iterations);
        let count = format!(
            "{} {}",
            r.results.len(),
            if r.results.len() == 1 {
                "request"
            } else {
                "requests"
            }
        );
        let current = r.current.clone();
        let error = r.error.clone();
        sub(ui, rect.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if let Some((t, fg, bg)) = status_text {
                pill(ui, t, fg, bg, &format!("Run status: {t}"));
            }
            labeled_text(ui, &iteration, &iteration, font(12.0, "Medium"), fg());
            progress_bar(ui, "Run progress", frac, 220.0);
            labeled_text(ui, &count, &count, font(12.0, "Regular"), fg_2());
            if let Some(c) = current {
                let t = format!("Running: {c}");
                labeled_text(ui, &t, &t, font(12.0, "Regular"), fg_3());
            }
            if let Some(e) = error {
                let t = format!("Error: {e}");
                labeled_text(ui, &t, &t, font(12.0, "Regular"), danger());
            }
        });
    }

    fn runner_results(&mut self, ui: &mut Ui) {
        let area = ui.available_rect_before_wrap();
        if self.runner.results.is_empty() {
            if self.runner.running() {
                note(ui, "Waiting for the first result", "Running...");
            } else {
                note(
                    ui,
                    "Select a collection and click Run Collection to start",
                    "Select a collection and click \"Run Collection\" to start",
                );
            }
            return;
        }
        let summary_h = if self.runner.summary.is_some() {
            64.0
        } else {
            0.0
        };
        let list_area = Rect::from_min_max(area.min, pos2(area.max.x, area.max.y - summary_h));
        let (list_rect, detail_rect) = if self.runner.selected.is_some() {
            split(
                ui,
                list_area,
                true,
                &mut self.ui.splits.runner,
                280.0,
                300.0,
                "Resize results and detail",
            )
        } else {
            (list_area, Rect::from_min_max(list_area.max, list_area.max))
        };
        let _ = ui.allocate_rect(area, egui::Sense::hover());

        let multi = self.runner.total_iterations > 1;
        let n = self.runner.results.len();
        let running = self.runner.running();
        let mut clicked: Option<usize> = None;
        let selected = self.runner.selected;
        sub(ui, list_rect, Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let mut sa = ScrollArea::vertical()
                .id_salt("runner-results")
                .auto_shrink([false, false]);
            if running {
                sa = sa.stick_to_bottom(true);
            }
            sa.show_rows(ui, ROW_H, n, |ui, range| {
                for i in range {
                    let row = &self.runner.results[i];
                    let rr = &row.result;
                    let passed = rr.test_results.iter().filter(|t| t.passed).count();
                    let failed = rr.test_results.len() - passed;
                    let status = if rr.error.is_some() {
                        "Error".to_string()
                    } else {
                        rr.status.to_string()
                    };
                    let label = format!(
                        "Result {}: {} {} {} {} ms, {} passed, {} failed",
                        i + 1,
                        rr.method,
                        rr.request_name,
                        status,
                        rr.response_time.round(),
                        passed,
                        failed
                    );
                    let resp = list_row(ui, ROW_H, selected == Some(i), &label, |ui, rect| {
                        let rect = rect.shrink2(vec2(12.0, 0.0));
                        let mut x = rect.left();
                        if multi {
                            x = row_text(
                                ui,
                                rect,
                                x,
                                &format!("#{}", row.iteration),
                                mono(11.0),
                                fg_3(),
                            ) + 10.0;
                        }
                        let mx = row_text(
                            ui,
                            rect,
                            x,
                            &rr.method,
                            FontId::new(11.5, mono_bold()),
                            method_color(&rr.method),
                        );
                        x = x.max(mx) + 10.0;
                        // right side, laid out from the edge inwards
                        let mut rx = rect.right();
                        if failed > 0 {
                            rx = row_text_right(
                                ui,
                                rect,
                                rx,
                                &format!("{failed} \u{2717}"),
                                font(12.0, "SemiBold"),
                                danger(),
                            ) - 10.0;
                        }
                        if passed > 0 {
                            rx = row_text_right(
                                ui,
                                rect,
                                rx,
                                &format!("{passed} \u{2713}"),
                                font(12.0, "SemiBold"),
                                success(),
                            ) - 10.0;
                        }
                        rx = row_text_right(
                            ui,
                            rect,
                            rx,
                            &fmt_dur(rr.response_time),
                            mono(12.0),
                            fg_2(),
                        ) - 14.0;
                        let sc = if rr.error.is_some() {
                            danger()
                        } else {
                            status_colors(rr.status).1
                        };
                        rx = row_text_right(ui, rect, rx, &status, font(12.0, "SemiBold"), sc)
                            - 14.0;
                        let clip = Rect::from_min_max(rect.min, pos2(rx, rect.max.y));
                        row_text(ui, clip, x, &rr.request_name, font(12.5, "Regular"), fg());
                    });
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
        if let Some(i) = clicked {
            self.runner.selected = if selected == Some(i) { None } else { Some(i) };
        }

        if let Some(i) = self.runner.selected {
            if let Some(row) = self.runner.results.get(i) {
                let mut close = false;
                sub(ui, detail_rect, Layout::top_down(Align::Min), |ui| {
                    ui.painter().rect_filled(detail_rect, 0, bg_surface());
                    ui.painter().vline(
                        detail_rect.left() + 0.5,
                        detail_rect.y_range(),
                        egui::Stroke::new(1.0, border()),
                    );
                    close = result_detail(ui, row, i);
                });
                if close {
                    self.runner.selected = None;
                }
            }
        }

        if let Some(s) = &self.runner.summary {
            let rect = Rect::from_min_max(pos2(area.min.x, area.max.y - summary_h), area.max);
            ui.painter().rect_filled(rect, 0, bg_surface());
            hline(ui, rect.top(), rect.left(), rect.right(), border());
            let items: [(&str, String, Option<Color32>); 4] = [
                ("Requests", s.total_requests.to_string(), None),
                ("Passed", s.passed_tests.to_string(), Some(success())),
                (
                    "Failed",
                    s.failed_tests.to_string(),
                    (s.failed_tests > 0).then_some(danger()),
                ),
                (
                    "Total",
                    format!("{:.1}s", s.total_time as f64 / 1000.0),
                    None,
                ),
            ];
            sub(ui, rect.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
                ui.spacing_mut().item_spacing.x = 40.0;
                for (caption, value, color) in items {
                    stat(ui, caption, &value, color.unwrap_or(fg()));
                }
            });
        }
    }
}

/// Value over caption.
fn stat(ui: &mut Ui, caption: &str, value: &str, color: Color32) {
    let g = ui
        .painter()
        .layout_no_wrap(value.to_string(), font(20.0, "SemiBold"), color);
    let c = ui
        .painter()
        .layout_no_wrap(caption.to_string(), font(11.0, "Medium"), fg_3());
    let w = g.size().x.max(c.size().x).max(48.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 48.0), egui::Sense::hover());
    ui.painter()
        .galley(pos2(rect.left(), rect.top() + 2.0), g, color);
    ui.painter()
        .galley(pos2(rect.left(), rect.top() + 30.0), c, fg_3());
    a11y(resp, Role::Label, &format!("{caption}: {value}"));
}

/// Details of one result; returns true when the pane's close button was clicked.
fn result_detail(ui: &mut Ui, row: &RunRow, index: usize) -> bool {
    let r = &row.result;
    let mut close = false;
    let hdr = bar(ui, 38.0, bg_surface(), Some(border()));
    sub(ui, hdr.shrink2(vec2(12.0, 0.0)), ltr(), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        labeled_text(
            ui,
            &format!("Result {}", index + 1),
            &format!("Result {} details", index + 1),
            font(13.0, "SemiBold"),
            fg(),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, "Close details", 24.0, |ui, c, col| {
                x_icon(ui, c, 3.5, col)
            })
            .clicked()
            {
                close = true;
            }
        });
    });
    ScrollArea::vertical()
        .id_salt(("runner-detail", index))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.label(
                        RichText::new(&r.request_name)
                            .font(font(14.0, "SemiBold"))
                            .color(fg()),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("{} {}", r.method, r.url))
                                .font(mono(12.0))
                                .color(fg_2()),
                        )
                        .wrap()
                        .selectable(true),
                    );
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        if let Some(e) = &r.error {
                            pill(
                                ui,
                                "Error",
                                danger(),
                                danger_subtle(),
                                "Result status: Error",
                            );
                            let _ = e;
                        } else {
                            let (bg, fg) = status_colors(r.status);
                            let t = format!("{} {}", r.status, r.status_text);
                            pill(ui, &t, fg, bg, &format!("Result status: {t}"));
                        }
                        let t = fmt_dur(r.response_time);
                        labeled_text(ui, &t, &format!("Result time: {t}"), mono(12.0), fg_2());
                        let t = fmt_size(r.size);
                        labeled_text(ui, &t, &format!("Result size: {t}"), mono(12.0), fg_2());
                        if row.iteration > 0 {
                            let t = format!("Iteration {}", row.iteration);
                            labeled_text(ui, &t, &t, font(12.0, "Regular"), fg_3());
                        }
                    });
                    if let Some(e) = &r.error {
                        egui::Frame::NONE
                            .fill(danger_subtle())
                            .corner_radius(6)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(e).font(mono(12.0)).color(danger()),
                                    )
                                    .wrap()
                                    .selectable(true),
                                );
                            });
                    }
                    ui.add_space(4.0);
                    detail_caption(ui, &format!("Tests ({})", r.test_results.len()));
                    if r.test_results.is_empty() {
                        ui.label(
                            RichText::new("No tests")
                                .font(font(12.0, "Regular"))
                                .color(fg_3()),
                        );
                    }
                    for t in &r.test_results {
                        let (mark, col) = if t.passed {
                            ("\u{2713}", success())
                        } else {
                            ("\u{2717}", danger())
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(mark).font(font(13.0, "Bold")).color(col));
                            let l = format!(
                                "{} {}",
                                if t.passed { "Passed" } else { "Failed" },
                                t.name
                            );
                            let resp = ui.label(
                                RichText::new(&t.name)
                                    .font(font(12.5, "Regular"))
                                    .color(fg()),
                            );
                            a11y(resp, Role::Label, &l);
                        });
                        if let Some(e) = &t.error {
                            ui.label(RichText::new(e).font(mono(11.5)).color(danger()));
                        }
                    }
                    if !r.script_logs.is_empty() {
                        ui.add_space(4.0);
                        detail_caption(ui, &format!("Script logs ({})", r.script_logs.len()));
                        for l in &r.script_logs {
                            let text = l
                                .args
                                .iter()
                                .map(|a| match a {
                                    serde_json::Value::String(s) => s.clone(),
                                    o => o.to_string(),
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            let col = match l.level.as_str() {
                                "error" => danger(),
                                "warn" => warning(),
                                _ => fg_2(),
                            };
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("{}  {text}", l.level))
                                        .font(mono(11.5))
                                        .color(col),
                                )
                                .wrap()
                                .selectable(true),
                            );
                        }
                    }
                });
        });
    close
}

fn detail_caption(ui: &mut Ui, text: &str) {
    labeled_text(
        ui,
        &text.to_uppercase(),
        text,
        font(11.0, "SemiBold"),
        fg_3(),
    );
}
