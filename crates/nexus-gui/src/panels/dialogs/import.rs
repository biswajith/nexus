//! Import / Export dialog (`ImportExportModal.tsx`).

use super::{card, para};
use crate::app::NexusApp;
use crate::state::dialogs::{import_format_label, ExportFmt, ExportWhat, ImportTab, Parsed};
use crate::theme::*;
use crate::widgets::*;
use eframe::egui::{
    self, accesskit::Role, pos2, vec2, Align, Layout, Rect, ScrollArea, Sense, TextEdit, Ui,
};
use nexus_core::types::CollectionItem;

const MAX_ROWS: usize = 200;

enum Row {
    Folder(usize, String),
    Request(usize, String, String),
    More(usize, usize),
}

fn preview_rows(items: &[CollectionItem], depth: usize, out: &mut Vec<Row>) {
    for (shown, i) in items.iter().enumerate() {
        if out.len() >= MAX_ROWS {
            out.push(Row::More(depth, items.len() - shown));
            return;
        }
        match i {
            CollectionItem::Folder(f) => {
                out.push(Row::Folder(depth, f.name.clone()));
                preview_rows(&f.items, depth + 1, out);
            }
            CollectionItem::Request(r) => out.push(Row::Request(
                depth,
                r.method.as_str().to_string(),
                r.name.clone(),
            )),
        }
    }
}

impl NexusApp {
    pub(super) fn import_dialog(&mut self, ui: &mut Ui) {
        // files dropped on the window while the dialog is open
        let dropped = ui
            .ctx()
            .input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(p) = dropped {
            self.dlg.import.tab = ImportTab::Import;
            self.import_load_file(&p);
        }
        let tab = self.dlg.import.tab;
        let items = [
            (ImportTab::Import, "Import".to_string()),
            (ImportTab::Export, "Export".to_string()),
        ];
        let w = ui.available_width();
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(t) = tab_strip(ui, &items, tab) {
                self.dlg.import.tab = t;
            }
        });
        let _ = w;
        ui.add_space(4.0);
        match tab {
            ImportTab::Import => self.import_pane(ui),
            ImportTab::Export => self.export_pane(ui),
        }
    }

    fn import_pane(&mut self, ui: &mut Ui) {
        let w = ui.available_width();
        // drop zone
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 84.0), Sense::click());
        let hov = resp.hovered();
        let p = ui.painter();
        p.rect_filled(rect, 8, if hov { bg_input_hover() } else { bg_input() });
        p.rect_stroke(
            rect.shrink(0.5),
            8,
            egui::Stroke::new(1.0, if hov { accent() } else { border_strong() }),
            egui::StrokeKind::Inside,
        );
        p.text(
            pos2(rect.center().x, rect.center().y - 12.0),
            egui::Align2::CENTER_CENTER,
            "Drop a file here or click to browse",
            font(13.0, "Medium"),
            fg(),
        );
        p.text(
            pos2(rect.center().x, rect.center().y + 10.0),
            egui::Align2::CENTER_CENTER,
            "Postman collections and environments, OpenAPI 3.x / Swagger 2.0 (JSON or YAML), Nexus exports",
            font(11.5, "Regular"),
            fg_2(),
        );
        if a11y(resp, Role::Button, "Choose a file to import")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.import_pick_file();
        }

        ui.add_space(4.0);
        para(
            ui,
            "Or paste a cURL command, or the contents of a file:",
            12.0,
            fg_2(),
        );
        let mut paste = std::mem::take(&mut self.dlg.import.paste);
        let r = ui.add(
            TextEdit::multiline(&mut paste)
                .font(egui::FontId::new(12.5, egui::FontFamily::Monospace))
                .hint_text(
                    "curl -X GET https://api.example.com/users -H 'Authorization: Bearer token'",
                )
                .desired_width(f32::INFINITY)
                .desired_rows(4),
        );
        a11y_name(&r, "Paste cURL or file contents");
        self.dlg.import.paste = paste;
        let has_paste = !self.dlg.import.paste.trim().is_empty();
        ui.horizontal(|ui| {
            if has_paste
                && button(
                    ui,
                    "Preview import",
                    font(12.0, "Medium"),
                    (12.0, 6.0),
                    28.0,
                    &BtnStyle::outline(),
                )
                .clicked()
            {
                self.import_parse_paste();
            }
            if self.dlg.import.loading {
                ui.add(egui::Spinner::new().size(16.0));
                labeled_text(ui, "Reading...", "Reading", font(12.0, "Regular"), fg_2());
            }
        });

        if let Some(e) = self.dlg.import.error.clone() {
            card(ui, "Error", danger(), &[e]);
        }
        if let Some(lines) = self.dlg.import.done.clone() {
            card(ui, "Imported successfully", success(), &lines);
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(
                    ui,
                    "Close",
                    font(12.0, "Medium"),
                    (14.0, 6.0),
                    30.0,
                    &BtnStyle::primary(accent(), accent_hover()),
                )
                .clicked()
                {
                    self.close_dialog();
                }
            });
            return;
        }
        if self.dlg.import.parsed.is_some() {
            self.import_preview(ui);
        }
    }

    fn import_preview(&mut self, ui: &mut Ui) {
        let Some(p) = &self.dlg.import.parsed else {
            return;
        };
        let ncol = p.result.collections.len();
        let nenv = p.result.environments.len();
        let curl = self.import_is_curl();
        let summary = {
            let mut s = format!("{ncol} collection{}", if ncol == 1 { "" } else { "s" });
            if nenv > 0 {
                s += &format!(", {nenv} environment{}", if nenv == 1 { "" } else { "s" });
            }
            s
        };
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            pill(
                ui,
                import_format_label(p.format),
                accent(),
                accent_subtle(),
                &format!("Detected format: {}", import_format_label(p.format)),
            );
            labeled_text(ui, &summary, &summary, font(12.0, "Regular"), fg_2());
            if let Some(src) = &self.dlg.import.source {
                labeled_text(
                    ui,
                    &format!("from {src}"),
                    &format!("Source: {src}"),
                    font(12.0, "Regular"),
                    fg_3(),
                );
            }
        });
        labeled_text(ui, "PREVIEW", "Preview", font(11.0, "SemiBold"), fg_3());
        preview_tree(ui, p);
        let warnings = p.result.warnings.clone();
        if !warnings.is_empty() {
            let mut lines: Vec<String> = warnings.iter().take(10).cloned().collect();
            if warnings.len() > 10 {
                lines.push(format!("...and {} more", warnings.len() - 10));
            }
            let title = format!(
                "{} warning{} during import",
                warnings.len(),
                if warnings.len() == 1 { "" } else { "s" }
            );
            card(ui, &title, warning(), &lines);
        }
        ui.add_space(6.0);
        let mut commit = false;
        let mut open_tab = false;
        let mut cancel = false;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let primary = BtnStyle::primary(accent(), accent_hover());
            if curl {
                if button(
                    ui,
                    "Open in new tab",
                    font(12.0, "Medium"),
                    (14.0, 6.0),
                    30.0,
                    &primary,
                )
                .clicked()
                {
                    open_tab = true;
                }
                if button(
                    ui,
                    "Import as collection",
                    font(12.0, "Medium"),
                    (14.0, 6.0),
                    30.0,
                    &BtnStyle::outline(),
                )
                .clicked()
                {
                    commit = true;
                }
            } else if button(
                ui,
                "Import",
                font(12.0, "Medium"),
                (14.0, 6.0),
                30.0,
                &primary,
            )
            .clicked()
            {
                commit = true;
            }
            if button(
                ui,
                "Cancel",
                font(12.0, "Medium"),
                (14.0, 6.0),
                30.0,
                &BtnStyle::outline(),
            )
            .clicked()
            {
                cancel = true;
            }
        });
        if commit {
            self.import_commit();
        }
        if open_tab {
            self.import_open_in_tab();
        }
        if cancel {
            self.dlg.import.parsed = None;
            self.close_dialog();
        }
    }

    fn export_pane(&mut self, ui: &mut Ui) {
        let cols: Vec<(String, String)> = self
            .collections
            .metas
            .iter()
            .map(|m| (m.name.clone(), crate::state::collections::dir_name(m)))
            .collect();
        let envs: Vec<(String, String)> = self
            .envs
            .environments
            .iter()
            .map(|e| (e.name.clone(), e.id.clone()))
            .collect();
        let mut export: Option<(ExportWhat, ExportFmt)> = None;
        ScrollArea::vertical()
            .id_salt("export-list")
            .max_height(360.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                labeled_text(
                    ui,
                    "COLLECTIONS",
                    "Collections",
                    font(11.0, "SemiBold"),
                    fg_3(),
                );
                if cols.is_empty() {
                    para(
                        ui,
                        "No collections to export. Import or create a collection first.",
                        12.5,
                        fg_2(),
                    );
                }
                for (name, dir) in &cols {
                    export_row(
                        ui,
                        name,
                        "collection",
                        &mut export,
                        |f| (ExportWhat::Collection(dir.clone()), f),
                        [ExportFmt::Nexus, ExportFmt::Postman],
                    );
                }
                ui.add_space(6.0);
                labeled_text(
                    ui,
                    "ENVIRONMENTS",
                    "Environments",
                    font(11.0, "SemiBold"),
                    fg_3(),
                );
                if envs.is_empty() {
                    para(ui, "No environments.", 12.5, fg_2());
                }
                for (name, id) in &envs {
                    export_row(
                        ui,
                        name,
                        "environment",
                        &mut export,
                        |f| (ExportWhat::Environment(id.clone()), f),
                        [ExportFmt::Nexus, ExportFmt::Postman],
                    );
                }
            });
        if let Some((what, fmt)) = export {
            self.export_dialog(what, fmt);
        }
        match self.dlg.import.export_notice.clone() {
            Some(Ok(m)) => card(ui, "Exported", success(), &[m]),
            Some(Err(e)) => card(ui, "Export failed", danger(), &[e]),
            None => {}
        }
    }
}

fn export_row(
    ui: &mut Ui,
    name: &str,
    kind: &str,
    out: &mut Option<(ExportWhat, ExportFmt)>,
    make: impl Fn(ExportFmt) -> (ExportWhat, ExportFmt),
    fmts: [ExportFmt; 2],
) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 38.0), Sense::hover());
    ui.painter().rect_filled(rect, 6, bg_input());
    ui.painter().rect_stroke(
        rect.shrink(0.5),
        6,
        egui::Stroke::new(1.0, border_subtle()),
        egui::StrokeKind::Inside,
    );
    let inner = Rect::from_min_max(
        pos2(rect.left() + 12.0, rect.top()),
        pos2(rect.right() - 8.0, rect.bottom()),
    );
    sub(ui, inner, Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for f in fmts.into_iter().rev() {
            let r = button(
                ui,
                f.label(),
                font(12.0, "Medium"),
                (10.0, 4.0),
                26.0,
                &BtnStyle::outline(),
            );
            let r = a11y(
                r,
                Role::Button,
                &format!("Export {kind} {name} as {}", f.label()),
            );
            if r.clicked() {
                *out = Some(make(f));
            }
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            labeled_text(
                ui,
                name,
                &format!("{kind} {name}"),
                font(13.0, "Medium"),
                fg(),
            );
        });
    });
}

fn preview_tree(ui: &mut Ui, p: &Parsed) {
    let mut rows: Vec<Row> = Vec::new();
    for c in &p.result.collections {
        rows.push(Row::Folder(0, c.name.clone()));
        preview_rows(&c.items, 1, &mut rows);
    }
    for e in &p.result.environments {
        rows.push(Row::Folder(
            0,
            format!("Environment: {} ({} variables)", e.name, e.variables.len()),
        ));
    }
    let w = ui.available_width();
    let (frame, _) = ui.allocate_exact_size(vec2(w, 200.0), Sense::hover());
    ui.painter().rect_filled(frame, 6, bg_sunken());
    let mut cui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(frame.shrink(4.0))
            .layout(Layout::top_down(Align::Min)),
    );
    cui.set_clip_rect(frame.shrink(2.0).intersect(ui.clip_rect()));
    ScrollArea::vertical()
        .id_salt("import-preview")
        .auto_shrink([false, false])
        .show_rows(&mut cui, 22.0, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in range {
                let w = ui.available_width();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
                let (depth, label) = match &rows[i] {
                    Row::Folder(d, n) => (*d, format!("Folder {n}")),
                    Row::Request(d, m, n) => (*d, format!("{m} {n}")),
                    Row::More(d, n) => (*d, format!("and {n} more items")),
                };
                let x = r.left() + 6.0 + depth as f32 * 16.0;
                match &rows[i] {
                    Row::Folder(_, n) => {
                        ui.painter().text(
                            pos2(x, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            n,
                            font(12.5, "SemiBold"),
                            fg(),
                        );
                    }
                    Row::Request(_, m, n) => {
                        let mw = ui
                            .painter()
                            .text(
                                pos2(x, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                m,
                                egui::FontId::new(11.0, mono_bold()),
                                method_color(m),
                            )
                            .width();
                        ui.painter().text(
                            pos2(x + mw.max(40.0) + 8.0, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            n,
                            font(12.5, "Regular"),
                            fg_2(),
                        );
                    }
                    Row::More(..) => {
                        ui.painter().text(
                            pos2(x, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("...{label}"),
                            font(12.0, "Regular"),
                            fg_3(),
                        );
                    }
                }
                a11y(resp, Role::ListItem, &label);
            }
        });
}
